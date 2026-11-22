#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
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

#[test]
fn large_body_is_fast_and_absolute_deadline_survives_slices() -> Result {
    let s = server(Limits { request_deadline: Duration::from_millis(300), ..Default::default() })?;
    let sid = initialize(&s)?;
    let body = format!("{}{}", r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#, " ".repeat(1024 * 1024));
    let start = Instant::now();
    let reply = exchange(&s, "POST", &sid, &body)?;
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert!(start.elapsed() < Duration::from_secs(1));
    eprintln!("1 MiB authenticated body in {:?}", start.elapsed());
    let mut c = socket(&s)?;
    let start = Instant::now();
    c.write_all(head("POST", &sid, 100).as_bytes())?;
    let mut reply = String::new();
    BufReader::new(c).read_line(&mut reply)?;
    assert!(reply.contains("408"), "{reply}");
    assert!(start.elapsed() < Duration::from_secs(1));
    eprintln!("absolute 300ms body deadline in {:?}", start.elapsed());
    s.stop();
    until(&s, |st| st.workers == 0 && st.sessions == 0)?;
    Ok(())
}
