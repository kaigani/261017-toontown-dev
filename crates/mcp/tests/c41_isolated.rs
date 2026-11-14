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

mod support;

use tt_mcp::http::{HttpOptions, Limits, Stats};
use tt_mcp::server::{MAX_JOB_WAITS, MAX_SERVER_WORK, PERSISTENT_THREADS, SESSION_THREADS};

/// The tests of this binary count the process's threads: they run one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    write!(
        c,
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer t\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut text = String::new();
    c.read_to_string(&mut text).unwrap();
    text.lines().find_map(|l| l.strip_prefix("Mcp-Session-Id: ")).expect("a session").to_owned()
}

#[test]
fn saturated_server_threads_obey_the_formula_and_all_go_at_stop() {
    let _serial = serial();
    let (c, u, l) = (8usize, 2usize, 8usize);
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    // Warm-up: the first server of a process leaves one lazily created OS thread behind (the platform's networking,
    // on Windows), and later servers return to the same count; it is counted in the baseline, not against the server.
    let warm = tt_mcp::http::start_with(
        h.clone(),
        HttpOptions { bind: "127.0.0.1:0".parse().unwrap(), token: "t".into(), server: Default::default() },
        Limits::default(),
    )
    .unwrap();
    let _ = initialize(warm.addr);
    warm.stop();
    drop(warm);
    std::thread::sleep(Duration::from_secs(3));
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
        write!(st, "GET /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer t\r\nMcp-Session-Id: {sid}\r\n\r\n")
            .unwrap();
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
        assert!(start.elapsed() < Duration::from_secs(10), "threads remain after stop: {after} > {baseline}");
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(h);
}

/// Ruling O-20, the ceiling K measured with a busy session: one session driven to every transient cap at once (8 jobs,
/// 4 `job_wait` waits, 2 server-side work threads held at their start, 1 roots refresh waiting for the client) owns
/// at most K = 20 threads above the baseline taken before it connected, and the next request of each capped kind
/// gets its typed `limit.exceeded` (`maxJobs`, `concurrentJobWaits`, `serverWork`); a second roots request is
/// coalesced into the running refresh (no new thread).
#[test]
fn a_session_at_every_transient_cap_owns_at_most_k_threads() {
    let _serial = serial();
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let set = h.invoke(tt_executor::Invocation::user(
        "prefs.set",
        json!({ "values": { "agent.policy.defaults": { "maxJobs": 8 } } }),
    ));
    assert!(set.error.is_none(), "{set:?}");
    // Warm-up of the process's lazily created threads (job table, file system), so the baseline holds them.
    let warm = support::Client::connect(&h, support::opts(&dir), json!({})).0;
    warm.close().unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let baseline = threads();
    // The test client adds one thread of its own (it reads the server's output); the session's protocol loop is the
    // other thread `Client::start` creates, and is the session's (K counts it).
    let harness = 1;
    let (mut c, _) = support::Client::connect(&h, support::opts(&dir), json!({ "roots": { "listChanged": true } }));
    c.drain(Duration::from_millis(500));
    // 8 long jobs (the session's maxJobs).
    let mut jobs = Vec::new();
    for i in 0..8 {
        let out = dir.join(format!("out{i}"));
        std::fs::create_dir_all(&out).unwrap();
        let r = c.run("render.range.run", json!({ "from": 1, "to": 2000, "width": 512, "directory": out }));
        jobs.push(r["result"]["jobId"].as_str().unwrap_or_else(|| panic!("job {i}: {r}")).to_owned());
    }
    // Server-side work held at its start.
    let release = Arc::new(AtomicBool::new(false));
    let held = Arc::new(Mutex::new(0usize));
    {
        let (release, held) = (release.clone(), held.clone());
        tt_persist::lock::hooks::set(Some(Arc::new(move |b, _| {
            if b == tt_persist::lock::Boundary::ServerWork {
                *held.lock().unwrap_or_else(PoisonError::into_inner) += 1;
                let until = Instant::now() + Duration::from_secs(120);
                while !release.load(Ordering::Acquire) && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        })));
    }
    let works: Vec<u64> = (0..MAX_SERVER_WORK)
        .map(|_| c.send("tools/call", json!({ "name": "list_commands", "arguments": {} })))
        .collect();
    let waits: Vec<u64> = jobs[..MAX_JOB_WAITS]
        .iter()
        .map(|j| c.send("tools/call", json!({ "name": "job_wait", "arguments": { "jobId": j, "timeoutMs": 120000 } })))
        .collect();
    // The roots refresh waits for the client's answer; a second request is coalesced into it.
    c.notify("notifications/roots/list_changed", json!({}));
    c.notify("notifications/roots/list_changed", json!({}));
    // The two server-work threads start one after the other (each call waits out its 5 s budget on `mcp-calls`).
    let started = Instant::now();
    while *held.lock().unwrap_or_else(PoisonError::into_inner) < MAX_SERVER_WORK {
        assert!(started.elapsed() < Duration::from_secs(30), "server work not held");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300));
    let measured = threads();
    let running = jobs
        .iter()
        .filter(|j| {
            h.jobs().list(Some(j.as_str()), None).first().is_some_and(|r| r.state == tt_executor::JobState::Running)
        })
        .count();
    eprintln!("baseline={baseline} measured={measured} (harness {harness}) K={SESSION_THREADS} running jobs={running}");
    assert_eq!(running, 8, "the jobs are still running at the measurement");
    assert!(
        measured <= baseline + harness + SESSION_THREADS,
        "{measured} > {baseline} + {harness} + {SESSION_THREADS}"
    );
    // The next request of each capped kind.
    let extra_wait =
        c.send("tools/call", json!({ "name": "job_wait", "arguments": { "jobId": jobs[4], "timeoutMs": 120000 } }));
    let extra_work = c.send("tools/call", json!({ "name": "list_commands", "arguments": {} }));
    let extra_job = c.send(
        "tools/call",
        json!({ "name": "run_command", "arguments": { "id": "render.range.run", "params": { "from": 1, "to": 2000, "width": 512, "directory": dir.join("out0") } } }),
    );
    let code = |r: &Option<Value>| {
        r.as_ref().map(|v| v["result"]["structuredContent"]["error"].clone()).unwrap_or(Value::Null)
    };
    let w = c.wait(extra_wait, Duration::from_secs(30));
    assert_eq!(code(&w)["code"], "limit.exceeded", "{w:?}");
    assert_eq!(code(&w)["details"]["limit"], "concurrentJobWaits", "{w:?}");
    let s = c.wait(extra_work, Duration::from_secs(60));
    assert_eq!(code(&s)["code"], "limit.exceeded", "{s:?}");
    assert_eq!(code(&s)["details"]["limit"], "serverWork", "{s:?}");
    let j = c.wait(extra_job, Duration::from_secs(60));
    assert_eq!(code(&j)["code"], "limit.exceeded", "{j:?}");
    assert_eq!(code(&j)["details"]["limit"], "maxJobs", "{j:?}");
    // Release everything.
    release.store(true, Ordering::Release);
    for id in &jobs {
        let _ = h.jobs().cancel(id);
    }
    // The held work and the waits answer once released (their answers may already have been read above).
    let _ = (works, waits);
    c.drain(Duration::from_secs(2));
    tt_persist::lock::hooks::set(None);
    let _ = c.close();

    let _ = std::fs::remove_dir_all(&dir);
}
