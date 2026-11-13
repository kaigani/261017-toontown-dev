//! Independent round 2 transport attacks.
use std::error::Error;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};
use tt_mcp::http::{HttpOptions, HttpServer, Limits};

fn server(limits: Limits) -> Result<HttpServer, Box<dyn Error>> {
    Ok(tt_mcp::http::start_with(
        tt_executor::ExecutorHandle::spawn(Default::default())?,
        HttpOptions { bind: "127.0.0.1:0".parse()?, token: "review".into(), server: Default::default() },
        limits,
    )?)
}
fn socket(s: &HttpServer) -> Result<TcpStream, Box<dyn Error>> {
    let c = TcpStream::connect(s.addr)?;
    c.set_read_timeout(Some(Duration::from_secs(4)))?;
    Ok(c)
}
fn status(s: &HttpServer, request: &str) -> Result<String, Box<dyn Error>> {
    let mut c = socket(s)?;
    c.write_all(request.as_bytes())?;
    let mut line = String::new();
    BufReader::new(c).read_line(&mut line)?;
    Ok(line)
}
fn request(extra: &str) -> String {
    format!("DELETE /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer review\r\n{extra}\r\n")
}
#[test]
fn ipv6_bind_edges() {
    for a in ["[::]:0", "[::2]:0", "[::ffff:127.0.0.1]:0"] {
        assert!(!tt_mcp::http::bind_allowed(&a.parse().expect("fixture")));
    }
    assert!(tt_mcp::http::bind_allowed(&"[::1]:0".parse().expect("fixture")));
}
#[test]
fn strict_header_edges() -> Result<(), Box<dyn Error>> {
    let s = server(Default::default())?;
    for extra in [
        "hOsT: localhost\r\n",
        "Host : localhost\r\n",
        "Content-Length: +0\r\n",
        "Content-Length: 18446744073709551616\r\n",
        "tRaNsFeR-EnCoDiNg: identity\r\n",
    ] {
        assert!(status(&s, &request(extra))?.contains("400"), "{extra}");
    }
    for target in ["http://localhost/mcp", "*"] {
        assert!(status(&s, &format!("OPTIONS {target} HTTP/1.1\r\nHost: localhost\r\n\r\n"))?.contains("400"));
    }
    Ok(())
}
#[test]
fn authority_edges() -> Result<(), Box<dyn Error>> {
    let s = server(Default::default())?;
    for host in ["localhost:65536", "localhost:", "::1"] {
        assert!(status(&s, &request("").replace("Host: localhost", &format!("Host: {host}")))?.contains("403"));
    }
    for host in ["LOCALHOST.", "localhost:0", "[::1]:65535"] {
        assert!(
            status(&s, &request("Mcp-Session-Id: absent\r\n").replace("Host: localhost", &format!("Host: {host}")))?
                .contains("404")
        );
    }
    for origin in ["http://localhost/path", "file://localhost", "null"] {
        assert!(status(&s, &request(&format!("Origin: {origin}\r\n")))?.contains("403"));
    }
    Ok(())
}
#[test]
fn short_body_has_absolute_deadline() -> Result<(), Box<dyn Error>> {
    let s = server(Limits { request_deadline: Duration::from_millis(150), ..Default::default() })?;
    let start = Instant::now();
    let line = status(
        &s,
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer review\r\nContent-Length: 2\r\n\r\n{",
    )?;
    assert!(line.contains("408"), "{line}");
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(status(&s, "POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Length: 999999999\r\n\r\n")?.contains("401"));
    Ok(())
}
#[test]
fn pipelined_request_is_not_dispatched() -> Result<(), Box<dyn Error>> {
    let s = server(Default::default())?;
    let mut c = socket(&s)?;
    c.write_all(format!("{}{}", request(""), request("")).as_bytes())?;
    c.shutdown(std::net::Shutdown::Write)?;
    let mut answer = String::new();
    c.read_to_string(&mut answer)?;
    assert_eq!(answer.matches("HTTP/1.1").count(), 1);
    Ok(())
}
#[test]
fn idle_unauthenticated_clients_leave_authenticated_service_available() -> Result<(), Box<dyn Error>> {
    let s = server(Limits { max_unauthenticated: 1, head_deadline: Duration::from_secs(2), ..Default::default() })?;
    let idle = socket(&s)?;
    let start = Instant::now();
    loop {
        let line = status(&s, &request("Mcp-Session-Id: absent\r\n"))?;
        if line.contains("503") {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(1), "idle client never admitted");
    }
    let line = status(&s, &request("Mcp-Session-Id: absent\r\n"))?;
    drop(idle);
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(line.contains("404"), "valid credential starved by idle client: {line}");
    Ok(())
}

fn initialize(s: &HttpServer) -> Result<String, Box<dyn Error>> {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"review","version":"1"}}}"#;
    let mut c = socket(s)?;
    write!(
        c,
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer review\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )?;
    let mut answer = String::new();
    c.read_to_string(&mut answer)?;
    Ok(answer)
}

#[test]
fn concurrent_initialization_obeys_session_cap() -> Result<(), Box<dyn Error>> {
    let s = server(Limits { max_connections: 128, max_unauthenticated: 64, ..Default::default() })?;
    let results = std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..40).map(|_| scope.spawn(|| initialize(&s).expect("initialize socket"))).collect();
        jobs.into_iter().map(|j| j.join().expect("worker")).collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|r| r.starts_with("HTTP/1.1 200")).count(), 32);
    assert_eq!(results.iter().filter(|r| r.starts_with("HTTP/1.1 503")).count(), 8);
    assert_eq!(s.session_count(), 32);
    Ok(())
}

#[test]
fn idle_session_expires_with_short_configured_clock() -> Result<(), Box<dyn Error>> {
    let s = server(Limits {
        session_idle: Duration::from_millis(100),
        sweep: Duration::from_millis(10),
        ..Default::default()
    })?;
    let answer = initialize(&s)?;
    let sid = answer.lines().find_map(|l| l.strip_prefix("Mcp-Session-Id: ")).ok_or("missing session")?;
    let began = Instant::now();
    while s.session_count() != 0 && began.elapsed() < Duration::from_secs(2) {
        std::hint::spin_loop();
    }
    assert_eq!(s.session_count(), 0);
    assert!(status(&s, &request(&format!("Mcp-Session-Id: {sid}\r\n")))?.contains("404"));
    Ok(())
}

#[test]
fn credential_format_and_uniqueness() -> Result<(), Box<dyn Error>> {
    let a = tt_persist::fsx::secret_token(24)?;
    let b = tt_persist::fsx::secret_token(24)?;
    assert!(tt_persist::fsx::is_secret_token(&a, 24));
    assert_ne!(a, b);
    assert!(!tt_persist::fsx::is_secret_token(&"X".repeat(48), 24));
    Ok(())
}

#[test]
fn session_is_not_expired_while_authenticated_body_is_in_flight() -> Result<(), Box<dyn Error>> {
    let s = server(Limits {
        session_idle: Duration::from_millis(300),
        sweep: Duration::from_millis(10),
        request_deadline: Duration::from_secs(3),
        ..Default::default()
    })?;
    let init = initialize(&s)?;
    let sid = init.lines().find_map(|l| l.strip_prefix("Mcp-Session-Id: ")).ok_or("missing session")?;
    let body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let mut c = socket(&s)?;
    write!(
        c,
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer review\r\nMcp-Session-Id: {sid}\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        &body[..1]
    )?;
    let began = Instant::now();
    while began.elapsed() < Duration::from_millis(600) {
        std::hint::spin_loop();
    }
    c.write_all(&body.as_bytes()[1..])?;
    let mut answer = String::new();
    BufReader::new(c).read_line(&mut answer)?;
    assert!(answer.contains("202"), "session expired during body reception: {answer}");
    Ok(())
}

#[cfg(windows)]
#[test]
fn refused_connections_do_not_escape_thread_cap() -> Result<(), Box<dyn Error>> {
    fn threads() -> Result<usize, Box<dyn Error>> {
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).Threads.Count", std::process::id())])
            .output()?;
        Ok(String::from_utf8(output.stdout)?.trim().parse()?)
    }
    let s = server(Limits { max_connections: 1, ..Default::default() })?;
    let before = threads()?;
    let began = Instant::now();
    let mut held = Vec::new();
    for _ in 0..64 {
        let mut c = socket(&s)?;
        c.write_all(b"DELETE /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n")?;
        let mut r = BufReader::new(c);
        let mut line = String::new();
        r.read_line(&mut line)?;
        assert!(line.contains("401") || line.contains("503"), "{line}");
        held.push(r);
    }
    let after = threads()?;
    let elapsed = began.elapsed();
    drop(held);
    assert!(elapsed < Duration::from_secs(1), "probe missed linger window: {elapsed:?}");
    assert!(
        after <= before + 16,
        "cap=1 but refusal burst added {} threads in {elapsed:?}",
        after.saturating_sub(before)
    );
    Ok(())
}

mod support;
mod part_c1_keys {
    use super::support::Client;
    use serde_json::json;
    use tt_executor::{ExecutorHandle, host::ShellReport};
    #[test]
    fn disabled_key_is_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        let (mut c, _) = Client::connect(&h, Default::default(), json!({}));
        let r = c.call("key", json!({"keys":"Primary+Z"}));
        assert_eq!(r["structuredContent"]["error"]["code"], "command.disabled", "{r}");
        Ok(())
    }
    #[test]
    fn modal_key_is_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        h.report_shell(serde_json::from_value::<ShellReport>(
            json!({"modal":{"kind":"dialog","title":"Preferences"}}),
        )?);
        let (mut c, _) = Client::connect(&h, Default::default(), json!({}));
        let r = c.call("key", json!({"keys":"Primary+Z"}));
        let code = r["structuredContent"]["error"]["code"].as_str();
        assert!(matches!(code, Some("ui.busy" | "ui.modalOpen")), "{r}");
        Ok(())
    }
    #[test]
    fn stale_inspect_is_bounded_and_keeps_last_tree() -> Result<(), Box<dyn std::error::Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        h.report_shell(serde_json::from_value::<ShellReport>(
            json!({"attached":true,"uiTree":{"id":"main","label":"last"}}),
        )?);
        let (mut c, _) = Client::connect(&h, Default::default(), json!({}));
        let start = std::time::Instant::now();
        let r = c.call("inspect_ui", json!({}));
        assert!((std::time::Duration::from_millis(1950)..std::time::Duration::from_secs(3)).contains(&start.elapsed()));
        assert_eq!(r["structuredContent"]["result"]["stale"], true, "{r}");
        assert_eq!(r["structuredContent"]["result"]["root"]["label"], "last");
        Ok(())
    }
}
