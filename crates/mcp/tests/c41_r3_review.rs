//! Independent round-three admission and lifetime probes.
use std::error::Error;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};
use tt_mcp::http::{HttpOptions, HttpServer, Limits, Stats};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
fn server(limits: Limits) -> Result<HttpServer> {
    Ok(tt_mcp::http::start_with(
        tt_executor::ExecutorHandle::spawn(Default::default())?,
        HttpOptions { bind: "127.0.0.1:0".parse()?, token: "review".into(), server: Default::default() },
        limits,
    )?)
}
fn socket(s: &HttpServer) -> Result<TcpStream> {
    let c = TcpStream::connect(s.addr)?;
    c.set_read_timeout(Some(Duration::from_secs(3)))?;
    Ok(c)
}
fn until(s: &HttpServer, pred: impl Fn(Stats) -> bool) -> Result {
    let start = Instant::now();
    while !pred(s.stats()) {
        if start.elapsed() > Duration::from_secs(3) {
            return Err(format!("state timeout: {:?}", s.stats()).into());
        }
        std::thread::yield_now();
    }
    Ok(())
}
fn head(method: &str, sid: &str, len: usize) -> String {
    let session = if sid.is_empty() { String::new() } else { format!("Mcp-Session-Id: {sid}\r\n") };
    format!(
        "{method} /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer review\r\n{session}Content-Length: {len}\r\n\r\n"
    )
}
fn exchange(s: &HttpServer, method: &str, sid: &str, body: &str) -> Result<String> {
    let mut c = socket(s)?;
    c.write_all(format!("{}{body}", head(method, sid, body.len())).as_bytes())?;
    let mut text = String::new();
    c.read_to_string(&mut text)?;
    Ok(text)
}
fn initialize(s: &HttpServer) -> Result<String> {
    let text = exchange(
        s,
        "POST",
        "",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"review","version":"1"}}}"#,
    )?;
    assert!(text.starts_with("HTTP/1.1 200"), "{text}");
    text.lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
                .map(|(_, v)| v.trim().to_owned())
        })
        .ok_or_else(|| "no session".into())
}
fn partial(s: &HttpServer, sid: &str) -> Result<TcpStream> {
    let mut c = socket(s)?;
    c.write_all(format!("{}{{", head("POST", sid, 100)).as_bytes())?;
    Ok(c)
}
#[test]
fn stop_releases_authenticated_body_workers() -> Result {
    let s = server(Limits {
        request_deadline: Duration::from_secs(15),
        write_timeout: Duration::from_millis(100),
        ..Default::default()
    })?;
    let sid = initialize(&s)?;
    until(&s, |st| st.workers == 0)?;
    let body = partial(&s, &sid)?;
    until(&s, |st| st.workers == 1 && st.pending == 0)?;
    s.stop();
    let result = until(&s, |st| st.workers == 0 && st.sessions == 0);
    drop(body);
    result
}
#[test]
fn delete_during_body_does_not_resurrect_request_routes() -> Result {
    let s = server(Default::default())?;
    let sid = initialize(&s)?;
    until(&s, |st| st.workers == 0)?;
    let body = r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#;
    let mut c = socket(&s)?;
    c.write_all(format!("{}{}", head("POST", &sid, body.len()), &body[..1]).as_bytes())?;
    until(&s, |st| st.workers == 1 && st.pending == 0)?;
    assert!(exchange(&s, "DELETE", &sid, "")?.starts_with("HTTP/1.1 200"));
    assert_eq!(s.session_count(), 0);
    c.write_all(&body.as_bytes()[1..])?;
    let mut text = String::new();
    let result = c.read_to_string(&mut text);
    drop(c);
    assert!(result.is_ok(), "deleted request never ended: {result:?}; {text}");
    Ok(())
}
#[test]
fn many_guards_release_and_expire_after_last_disconnect() -> Result {
    let idle = Duration::from_millis(250);
    let s = server(Limits { session_idle: idle, sweep: Duration::from_millis(10), ..Default::default() })?;
    let sid = initialize(&s)?;
    let mut held = Vec::new();
    for _ in 0..12 {
        held.push(partial(&s, &sid)?);
    }
    until(&s, |st| st.workers == 12 && st.pending == 0)?;
    let start = Instant::now();
    while start.elapsed() < idle * 2 {
        assert_eq!(s.session_count(), 1);
        std::thread::yield_now();
    }
    drop(held);
    until(&s, |st| st.workers == 0)?;
    let start = Instant::now();
    until(&s, |st| st.sessions == 0)?;
    assert!(start.elapsed() >= Duration::from_millis(180));
    Ok(())
}
#[test]
fn trickling_oldest_is_evicted_without_answer() -> Result {
    let s = server(Limits { max_unauthenticated: 1, ..Default::default() })?;
    let mut oldest = socket(&s)?;
    oldest.write_all(b"P")?;
    until(&s, |st| st.pending == 1)?;
    let mut new = socket(&s)?;
    let mut line = String::new();
    BufReader::new(&mut new).read_line(&mut line)?;
    assert!(line.starts_with("HTTP/1.1 503"));
    let mut out = [0; 1];
    let result = oldest.read(&mut out);
    assert!(
        matches!(result, Ok(0)) || result.as_ref().is_err_and(|e| e.kind() == std::io::ErrorKind::ConnectionReset),
        "{result:?}"
    );
    Ok(())
}

#[cfg(windows)]
fn threads() -> Result<usize> {
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).Threads.Count", std::process::id())])
        .output()?;
    Ok(String::from_utf8(output.stdout)?.trim().parse()?)
}
#[cfg(windows)]
#[test]
fn saturated_pools_obey_thread_formula_and_unread_refusals_do_not_stall() -> Result {
    // Keep the executor alive across the baseline so its own threads are excluded.
    let h = tt_executor::ExecutorHandle::spawn(Default::default())?;
    let baseline = threads()?;
    let s = tt_mcp::http::start_with(
        h.clone(),
        HttpOptions { bind: "127.0.0.1:0".parse()?, token: "review".into(), server: Default::default() },
        Limits { max_connections: 8, max_unauthenticated: 2, max_lingering: 8, ..Default::default() },
    )?;
    let mut ids = Vec::new();
    for _ in 0..32 {
        ids.push(initialize(&s)?);
    }
    let mut streams = Vec::new();
    for i in 0..6 {
        let mut c = socket(&s)?;
        c.write_all(head("GET", &ids[i], 0).as_bytes())?;
        let mut r = BufReader::new(c);
        let mut line = String::new();
        r.read_line(&mut line)?;
        assert!(line.contains("200"));
        streams.push(r);
    }
    let pending = vec![socket(&s)?, socket(&s)?];
    until(&s, |st| st.workers == 8 && st.pending == 2)?;
    let measured = threads()?;

    let mut refused = Vec::new();
    let start = Instant::now();
    for _ in 0..64 {
        refused.push(socket(&s)?);
    }
    let mut served = false;
    while start.elapsed() < Duration::from_secs(3) {
        let mut c = socket(&s)?;
        c.write_all(head("DELETE", "absent", 0).as_bytes())?;
        let mut line = String::new();
        BufReader::new(c).read_line(&mut line)?;
        if line.contains("404") {
            served = true;
            break;
        }
    }
    assert!(served, "unread refusals blocked authenticated service");
    let st = s.stats();
    assert!(st.workers <= 8 && st.pending <= 2 && st.lingering <= 8 && st.sessions == 32, "{st:?}");
    eprintln!(
        "baseline={baseline}; saturated={measured}; formula={}; service={:?}; stats={st:?}",
        baseline + 43,
        start.elapsed()
    );
    s.stop();
    drop((streams, pending, refused));
    until(&s, |st| st.workers == 0 && st.sessions == 0 && st.lingering == 0)?;
    let start = Instant::now();
    loop {
        let after = threads()?;
        if after <= baseline {
            break;
        }
        if start.elapsed() > Duration::from_secs(3) {
            return Err(format!("threads remain: baseline={baseline} after={after}").into());
        }
    }
    drop(h);
    assert!(measured <= baseline + 3 + 8 + 32, "baseline={baseline} measured={measured}");
    Ok(())
}
