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
