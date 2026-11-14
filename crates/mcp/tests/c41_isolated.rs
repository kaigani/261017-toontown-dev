//! M4 C4.1 builder test of `tt-mcp` that measures a process-wide quantity, the number of OS threads, in a binary of its
//! own so no other test's threads disturb it (orchestrator ruling D-2). Builder-owned.
//!
//! Invariant I-1 of reviews/M4-design-http-admission.md as corrected by ruling O-20: the server holds at most
//! `3 + C + K·S` threads (accept, closer, sweeper; C connection workers; K threads per session, S sessions), and
//! everything it held goes when it stops.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use tt_mcp::http::{HttpOptions, Limits, Stats};
use tt_mcp::server::{PERSISTENT_THREADS, SESSION_THREADS};

/// The OS threads of this process.
fn threads() -> usize {
    #[cfg(windows)]
    let text = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).Threads.Count", std::process::id())])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap();
    #[cfg(target_os = "macos")]
    let text = std::process::Command::new("ps")
        .args(["-M", "-p", &std::process::id().to_string()])
        .output()
        .map(|o| (String::from_utf8_lossy(&o.stdout).lines().count().saturating_sub(1)).to_string())
        .unwrap();
    #[cfg(target_os = "linux")]
    let text = std::fs::read_dir("/proc/self/task").unwrap().count().to_string();
    text.parse().unwrap()
}

fn until(s: &tt_mcp::http::HttpServer, what: &str, pred: impl Fn(Stats) -> bool) {
    let start = Instant::now();
    while !pred(s.stats()) {
        assert!(start.elapsed() < Duration::from_secs(10), "{what}: {:?}", s.stats());
        std::thread::yield_now();
    }
}

fn initialize(addr: std::net::SocketAddr) -> String {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"c41","version":"1"}}}"#;
    let mut c = TcpStream::connect(addr).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    write!(c, "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer t\r\nContent-Length: {}\r\n\r\n{body}", body.len())
        .unwrap();
    let mut text = String::new();
    c.read_to_string(&mut text).unwrap();
    text.lines().find_map(|l| l.strip_prefix("Mcp-Session-Id: ")).expect("a session").to_owned()
}

#[test]
fn saturated_server_threads_obey_the_formula_and_all_go_at_stop() {
    let (c, u, l) = (8usize, 2usize, 8usize);
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    let baseline = threads();
    let s = tt_mcp::http::start_with(
        h.clone(),
        HttpOptions { bind: "127.0.0.1:0".parse().unwrap(), token: "t".into(), server: Default::default() },
        Limits { max_connections: c, max_unauthenticated: u, max_lingering: l, ..Default::default() },
    )
    .unwrap();
    // 32 sessions, six of them streaming (workers), two pending heads, then a burst of refusals held open.
    let sids: Vec<String> = (0..32).map(|_| initialize(s.addr)).collect();
    let mut held = Vec::new();
    for sid in sids.iter().take(c - u) {
        let mut st = TcpStream::connect(s.addr).unwrap();
        st.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        write!(st, "GET /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer t\r\nMcp-Session-Id: {sid}\r\n\r\n").unwrap();
        let mut r = BufReader::new(st);
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        assert!(line.contains("200"), "{line}");
        held.push(r);
    }
    let pending: Vec<TcpStream> = (0..u).map(|_| TcpStream::connect(s.addr).unwrap()).collect();
    until(&s, "saturated", |st| st.workers == c && st.pending == u && st.sessions == 32);
    let refused: Vec<TcpStream> = (0..64).map(|_| TcpStream::connect(s.addr).unwrap()).collect();
    let st = s.stats();
    let measured = threads();
    let formula = baseline + 3 + c + SESSION_THREADS * 32;
    // Idle sessions own exactly their persistent threads: the measured count is far below the formula's ceiling.
    let idle = baseline + 3 + c + PERSISTENT_THREADS * 32;
    eprintln!("baseline={baseline} measured={measured} idle-sessions={idle} formula={formula} stats={st:?}");
    assert!(st.workers <= c && st.pending <= u && st.lingering <= l, "{st:?}");
    assert!(measured <= formula, "{measured} > {formula}");
    assert!(measured <= idle + 4, "idle sessions own only their persistent threads: {measured} > {idle} + 4");
    s.stop();
    drop((held, pending, refused));
    until(&s, "stopped", |st| st.workers == 0 && st.sessions == 0 && st.lingering == 0 && st.pending == 0);
    let start = Instant::now();
    loop {
        let after = threads();
        if after <= baseline {
            break;
        }
        eprintln!("t={:?} after={after}", start.elapsed()); assert!(start.elapsed() < Duration::from_secs(40), "threads remain after stop: {after} > {baseline}");
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(h);
}
