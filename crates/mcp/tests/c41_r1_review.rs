//! Independent C4.1 round 1 transport boundary regressions.
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tt_executor::ExecutorHandle;
use tt_mcp::http::{HttpOptions, HttpServer};

fn start(bind: &str) -> Result<HttpServer, Box<dyn Error>> {
    let handle = ExecutorHandle::spawn(Default::default())?;
    Ok(tt_mcp::http::start(
        handle,
        HttpOptions { bind: bind.parse()?, token: "review-fixture-only".into(), server: Default::default() },
    )?)
}

#[test]
fn remote_bind_is_refused() -> Result<(), Box<dyn Error>> {
    let result = start("0.0.0.0:0");
    assert!(result.is_err(), "ADR-0022 requires remote binding to be refused");
    Ok(())
}

fn status(addr: SocketAddr, extra: &str) -> Result<String, Box<dyn Error>> {
    let mut socket = TcpStream::connect(addr)?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(socket, "DELETE /mcp HTTP/1.1\r\n{extra}Mcp-Session-Id: absent\r\nContent-Length: 0\r\n\r\n")?;
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line)?;
    Ok(line)
}

#[test]
fn duplicate_authorization_is_refused() -> Result<(), Box<dyn Error>> {
    let server = start("127.0.0.1:0")?;
    let response = status(
        server.addr,
        "Host: localhost\r\nAuthorization: Bearer invalid\r\nAuthorization: Bearer review-fixture-only\r\n",
    )?;
    assert!(
        response.contains("400") || response.contains("401"),
        "duplicate credentials reached session lookup: {response}"
    );
    Ok(())
}

#[test]
fn duplicate_foreign_host_is_refused() -> Result<(), Box<dyn Error>> {
    let server = start("127.0.0.1:0")?;
    let response = status(
        server.addr,
        "Host: foreign.invalid\r\nHost: localhost\r\nAuthorization: Bearer review-fixture-only\r\n",
    )?;
    assert!(
        response.contains("400") || response.contains("403"),
        "foreign duplicate Host reached session lookup: {response}"
    );
    Ok(())
}

#[test]
fn ambiguous_transfer_framing_is_refused() -> Result<(), Box<dyn Error>> {
    let server = start("127.0.0.1:0")?;
    let response = status(
        server.addr,
        "Host: localhost\r\nAuthorization: Bearer review-fixture-only\r\nTransfer-Encoding: chunked\r\n",
    )?;
    assert!(response.contains("400"), "ambiguous framing reached session lookup: {response}");
    Ok(())
}

/// An unauthenticated header must not reserve a maximum-sized body while waiting for bytes.
#[test]
fn unauthenticated_large_body_is_refused_before_reading_payload() -> Result<(), Box<dyn Error>> {
    let server = start("127.0.0.1:0")?;
    let mut socket = TcpStream::connect(server.addr)?;
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(socket, "POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Length: 67108864\r\n\r\n")?;
    let began = std::time::Instant::now();
    let mut response = String::new();
    let result = BufReader::new(socket.try_clone()?).read_line(&mut response);
    // Release the one test connection even when the server is still waiting for the body.
    socket.shutdown(std::net::Shutdown::Both)?;
    assert!(
        result.is_ok() && response.contains("401"),
        "unauthenticated request held awaiting payload for {:?}: {result:?}, {response:?}",
        began.elapsed()
    );
    Ok(())
}

mod part_c_http {
    use super::*;

    fn request(
        addr: SocketAddr,
        method: &str,
        host: Option<&str>,
        origin: Option<&str>,
    ) -> Result<String, Box<dyn Error>> {
        let mut socket = TcpStream::connect(addr)?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        write!(
            socket,
            "{method} /mcp HTTP/1.1\r\nAuthorization: Bearer review-fixture-only\r\nMcp-Session-Id: absent\r\nContent-Length: 2\r\n"
        )?;
        if let Some(h) = host {
            write!(socket, "Host: {h}\r\n")?;
        }
        if let Some(o) = origin {
            write!(socket, "Origin: {o}\r\n")?;
        }
        write!(socket, "\r\n{{}}")?;
        let mut line = String::new();
        BufReader::new(socket).read_line(&mut line)?;
        Ok(line)
    }

    #[test]
    fn authority_matrix() -> Result<(), Box<dyn Error>> {
        let server = start("127.0.0.1:0")?;
        for method in ["POST", "GET", "DELETE"] {
            for bad in ["localhost.evil.example", "127.0.0.1.evil.example", "evil.example:80@127.0.0.1", "", "null"] {
                assert!(request(server.addr, method, Some(bad), None)?.contains("403"));
                assert!(request(server.addr, method, Some("localhost"), Some(bad))?.contains("403"));
            }
            assert!(request(server.addr, method, None, None)?.contains("403"));
            for good in ["localhost", "127.0.0.1", "[::1]", "[::1]:123"] {
                assert!(request(server.addr, method, Some(good), None)?.contains("404"));
            }
        }
        Ok(())
    }

    #[test]
    fn dns_host_case_is_insensitive() -> Result<(), Box<dyn Error>> {
        let server = start("127.0.0.1:0")?;
        for method in ["POST", "GET", "DELETE"] {
            let r = request(server.addr, method, Some("LOCALHOST"), None)?;
            assert!(r.contains("404"), "valid loopback authority refused: {method}: {r}");
        }
        Ok(())
    }

    #[test]
    fn host_cannot_contain_url_path() -> Result<(), Box<dyn Error>> {
        let server = start("127.0.0.1:0")?;
        for method in ["POST", "GET", "DELETE"] {
            let r = request(server.addr, method, Some("localhost/foreign"), None)?;
            assert!(r.contains("400") || r.contains("403"), "invalid authority accepted: {method}: {r}");
        }
        Ok(())
    }
}
