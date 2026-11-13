//! M4 C4.1 builder tests of `tt-mcp`, one test binary for the checkpoint (orchestrator ruling D-2): the Streamable
//! HTTP transport of attached mode (ADR-0022; 11 §4) over a real loopback socket. Refusals (token, Origin, Host,
//! unknown session, wrong method), the session id on `initialize`, answers as SSE, tool calls, two sessions on one
//! executor, and DELETE ending a session. The official SDK client runs against the desktop app in the `desktop gui` CI
//! job (`scripts/mcp-conformance.py --http`). Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::{Value, json};
use support::*;
use tt_mcp::ServerOptions;
use tt_mcp::http::{HttpOptions, HttpServer};

const TOKEN: &str = "c41-test-token";
/// Socket read bound: far above a local answer (a stuck server fails the test instead of hanging it).
const READ_BOUND: Duration = Duration::from_secs(60);

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The JSON-RPC messages of an SSE body.
    fn messages(&self) -> Vec<Value> {
        self.body.lines().filter_map(|l| l.strip_prefix("data: ")).map(|d| serde_json::from_str(d).unwrap()).collect()
    }
}

/// One HTTP/1.1 exchange (the server closes every connection after its answer).
fn exchange(addr: SocketAddr, method: &str, headers: &[(&str, &str)], body: Option<&Value>) -> Reply {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(READ_BOUND)).unwrap();
    let body = body.map(|b| serde_json::to_vec(b).unwrap()).unwrap_or_default();
    let mut req = format!("{method} /mcp HTTP/1.1\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).unwrap();
    s.write_all(&body).unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8(raw).unwrap();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let mut lines = head.lines();
    let status = lines.next().and_then(|l| l.split(' ').nth(1)).and_then(|c| c.parse().ok()).unwrap();
    let headers =
        lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned())).collect();
    Reply { status, headers, body: body.to_owned() }
}

fn server() -> (HttpServer, std::path::PathBuf) {
    let dir = tmp();
    let h = executor(&dir);
    let srv = tt_mcp::http::start(
        h,
        HttpOptions { bind: "127.0.0.1:0".parse().unwrap(), token: TOKEN.to_owned(), server: ServerOptions::default() },
    )
    .unwrap();
    (srv, dir)
}

const BEARER: &str = "Bearer c41-test-token";

fn post(srv: &HttpServer, sid: Option<&str>, msg: &Value) -> Reply {
    let host = srv.addr.to_string();
    let mut headers = vec![
        ("Host", host.as_str()),
        ("Authorization", BEARER),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
    ];
    if let Some(s) = sid {
        headers.push(("Mcp-Session-Id", s));
    }
    exchange(srv.addr, "POST", &headers, Some(msg))
}

fn initialize(srv: &HttpServer) -> String {
    let r = post(
        srv,
        None,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": tt_mcp::DEFAULT_PROTOCOL, "capabilities": {},
            "clientInfo": {"name": "c41-http-test", "version": "1"}}}),
    );
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.header("content-type"), Some("text/event-stream"));
    let sid = r.header("mcp-session-id").expect("a session id on initialize").to_owned();
    let answer = r.messages().into_iter().find(|m| m["id"] == 1).unwrap();
    assert_eq!(answer["result"]["protocolVersion"], tt_mcp::DEFAULT_PROTOCOL, "{answer}");
    let n = post(srv, Some(&sid), &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    assert_eq!(n.status, 202);
    sid
}

#[test]
fn refusals_come_before_any_session_work() {
    let (srv, _dir) = server();
    let host = srv.addr.to_string();
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}});
    // No token, a wrong token.
    let r = exchange(srv.addr, "POST", &[("Host", &host)], Some(&init));
    assert_eq!(r.status, 401);
    assert_eq!(r.header("www-authenticate"), Some("Bearer"));
    let r = exchange(srv.addr, "POST", &[("Host", &host), ("Authorization", "Bearer nope")], Some(&init));
    assert_eq!(r.status, 401);
    // A non-local Origin or Host (DNS rebinding), even with the token.
    let r = exchange(
        srv.addr,
        "POST",
        &[("Host", &host), ("Authorization", BEARER), ("Origin", "http://evil.example")],
        Some(&init),
    );
    assert_eq!(r.status, 403);
    let r = exchange(srv.addr, "POST", &[("Host", "evil.example:80"), ("Authorization", BEARER)], Some(&init));
    assert_eq!(r.status, 403);
    // A request other than initialize without a session; an unknown session; a batch; a wrong method.
    let r = post(&srv, None, &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert_eq!(r.status, 400);
    let r = post(&srv, Some("not-a-session"), &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert_eq!(r.status, 404);
    let r = post(&srv, None, &json!([init]));
    assert_eq!(r.status, 400);
    let r = exchange(srv.addr, "PUT", &[("Host", &host), ("Authorization", BEARER)], None);
    assert_eq!(r.status, 405);
    srv.stop();
}

#[test]
fn sessions_answer_over_sse_share_the_executor_and_end_on_delete() {
    let (srv, _dir) = server();
    let a = initialize(&srv);
    let b = initialize(&srv);
    assert_ne!(a, b, "each initialize opens its own session");
    // Tools are listed and called over the session's SSE answers.
    let r = post(&srv, Some(&a), &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert_eq!(r.status, 200);
    let tools = &r.messages().into_iter().find(|m| m["id"] == 2).unwrap()["result"]["tools"];
    assert!(tools.as_array().unwrap().iter().any(|t| t["name"] == "run_command"), "{tools}");
    let r = post(
        &srv,
        Some(&a),
        &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "run_command", "arguments": {"id": "project.create", "params": {"name": "Over HTTP"}}}}),
    );
    let answer = r.messages().into_iter().find(|m| m["id"] == 3).unwrap();
    assert_ne!(answer["result"]["isError"], true, "{answer}");
    // The other session sees the same document: one executor behind every session.
    let r = post(
        &srv,
        Some(&b),
        &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
            "name": "run_command", "arguments": {"id": "project.info.get", "params": {}}}}),
    );
    let answer = r.messages().into_iter().find(|m| m["id"] == 4).unwrap();
    assert!(answer.to_string().contains("Over HTTP"), "{answer}");
    // DELETE ends a session; it is unknown afterwards; the other one goes on.
    let host = srv.addr.to_string();
    let del = |sid: &str| {
        exchange(srv.addr, "DELETE", &[("Host", &host), ("Authorization", BEARER), ("Mcp-Session-Id", sid)], None)
    };
    assert_eq!(del(&a).status, 200);
    assert_eq!(del(&a).status, 404);
    let r = post(&srv, Some(&a), &json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list"}));
    assert_eq!(r.status, 404);
    let r = post(&srv, Some(&b), &json!({"jsonrpc": "2.0", "id": 6, "method": "tools/list"}));
    assert_eq!(r.status, 200);
    srv.stop();
}
