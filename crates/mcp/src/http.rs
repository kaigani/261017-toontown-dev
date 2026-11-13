//! Streamable HTTP transport for attached mode (11 §4; M4 C4.1; ADR-0022).
//!
//! A small HTTP/1.1 server on `std::net` (no async runtime, so the headless dependency policy is unchanged) that
//! speaks the Streamable HTTP transport of the MCP revisions this server implements:
//! - one endpoint (`/mcp`), loopback only by default;
//! - every request needs `Authorization: Bearer <token>` (the per-installation token of the discovery file) and, when
//!   an `Origin` header is present, a loopback origin; the `Host` header must name the loopback address (DNS-rebinding
//!   protection);
//! - `POST` carries one JSON-RPC message: a request is answered with an SSE stream that carries the messages for that
//!   request (progress, the server's own requests such as `elicitation/create`) and ends with its response;
//!   notifications and responses are accepted with `202`;
//! - `GET` opens the session's event stream for messages that belong to no request (resource updates, log messages,
//!   tool-list changes);
//! - `DELETE` ends the session;
//! - `Mcp-Session-Id` is assigned at `initialize` and required afterwards.
//!
//! Each session runs the same protocol loop as stdio ([`crate::serve`]) over in-memory reader and writer ends, so
//! sessions, permissions, confirmations, the audit log and cancellation behave exactly as on stdio. Several clients
//! connect at once; each has its own agent session over the one executor (11 §3).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;
use tt_executor::ExecutorHandle;

use crate::ServerOptions;

/// The endpoint path.
pub const ENDPOINT: &str = "/mcp";
/// The largest request body accepted (above the 50 MB inline payload limit of 11 §6.5).
const MAX_BODY: usize = 64 << 20;
/// The largest header block.
const MAX_HEADERS: usize = 64 << 10;
/// Sessions served at once.
const MAX_SESSIONS: usize = 32;
/// How long an SSE stream waits between messages before it sends a keep-alive comment.
const KEEP_ALIVE: Duration = Duration::from_secs(15);
/// Socket read timeout (a stalled client cannot hold a connection thread forever).
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Options of the HTTP server.
#[derive(Debug, Clone)]
pub struct HttpOptions {
    /// Address to bind (`127.0.0.1:0` picks a free port).
    pub bind: SocketAddr,
    /// The bearer token clients must present.
    pub token: String,
    /// Options of each session's protocol loop (`run_mode` is forced to `attached`).
    pub server: ServerOptions,
}

/// A running server.
#[derive(Debug)]
pub struct HttpServer {
    /// The bound address (the port actually used).
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl HttpServer {
    /// The endpoint URL.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}{ENDPOINT}", self.addr)
    }

    /// Stops accepting connections and ends every session (their agents are disconnected).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        // Wake the accept loop.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        let drained: Vec<Session> =
            self.sessions.lock().unwrap_or_else(PoisonError::into_inner).drain().map(|(_, s)| s).collect();
        drop(drained);
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Where outgoing messages of one session go.
#[derive(Default)]
struct Routes {
    /// Open request streams by JSON-RPC id (canonical text of the id), newest last.
    requests: Vec<(String, Option<String>, Sender<String>)>,
    /// The session's GET stream.
    get: Option<Sender<String>>,
    /// Messages for no open stream, delivered to the next GET stream (bounded).
    backlog: Vec<String>,
}

/// One HTTP session: the input end of its protocol loop and its routes.
struct Session {
    input: Sender<Vec<u8>>,
    routes: Arc<Mutex<Routes>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session")
    }
}

/// The protocol loop's input: one JSON-RPC message per line, EOF when the session ends.
struct ChannelReader {
    rx: Receiver<Vec<u8>>,
    buf: Vec<u8>,
    pos: usize,
}

impl Read for ChannelReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let available = self.fill_buf()?;
        let n = available.len().min(out.len());
        out[..n].copy_from_slice(&available[..n]);
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for ChannelReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos >= self.buf.len() {
            match self.rx.recv() {
                Ok(bytes) => {
                    self.buf = bytes;
                    self.pos = 0;
                }
                Err(_) => return Ok(&[]),
            }
        }
        Ok(&self.buf[self.pos..])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.buf.len());
    }
}

/// The protocol loop's output: lines routed to the stream they belong to.
struct RouterWriter {
    routes: Arc<Mutex<Routes>>,
    pending: Vec<u8>,
}

fn id_text(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn route(routes: &Mutex<Routes>, line: &str) {
    let Ok(msg) = serde_json::from_str::<Value>(line) else { return };
    let mut r = routes.lock().unwrap_or_else(PoisonError::into_inner);
    let is_response = msg.get("id").is_some() && (msg.get("result").is_some() || msg.get("error").is_some());
    if is_response {
        let id = id_text(msg.get("id").unwrap_or(&Value::Null));
        if let Some(pos) = r.requests.iter().position(|(rid, _, _)| *rid == id) {
            let (_, _, tx) = r.requests.remove(pos);
            let _ = tx.send(line.to_owned());
        }
        return;
    }
    // Progress belongs to the request that asked for it.
    if msg.get("method").and_then(Value::as_str) == Some("notifications/progress")
        && let Some(token) = msg.get("params").and_then(|p| p.get("progressToken"))
    {
        let token = id_text(token);
        if let Some((_, _, tx)) = r.requests.iter().find(|(_, t, _)| t.as_deref() == Some(token.as_str())) {
            let _ = tx.send(line.to_owned());
            return;
        }
    }
    // A server request (elicitation) goes to the newest open request stream; other notifications to the GET stream.
    let is_request = msg.get("id").is_some() && msg.get("method").is_some();
    if is_request && let Some((_, _, tx)) = r.requests.last() {
        let _ = tx.send(line.to_owned());
        return;
    }
    if let Some(get) = &r.get
        && get.send(line.to_owned()).is_ok()
    {
        return;
    }
    if is_request && r.backlog.len() < 256 {
        r.backlog.push(line.to_owned());
    }
}

impl Write for RouterWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(data);
        while let Some(i) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=i).collect();
            if let Ok(text) = std::str::from_utf8(&line[..line.len() - 1]) {
                route(&self.routes, text);
            }
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A parsed request.
struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut head = String::new();
    let mut total = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        total += n;
        if total > MAX_HEADERS {
            return None;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        head.push_str(&line);
    }
    let mut lines = head.lines();
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_owned();
    let path = first.next()?.split('?').next().unwrap_or("").to_owned();
    let mut headers = HashMap::new();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_owned());
        }
    }
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if len > MAX_BODY {
        return None;
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).ok()?;
    Some(Request { method, path, headers, body })
}

fn respond(stream: &mut TcpStream, status: &str, headers: &[(&str, String)], body: &[u8]) {
    let mut out = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    for (k, v) in headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("\r\n");
    let _ = stream.write_all(out.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn json_error(stream: &mut TcpStream, status: &str, message: &str) {
    let body = serde_json::json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32600, "message": message } });
    respond(stream, status, &[("Content-Type", "application/json".to_owned())], body.to_string().as_bytes());
}

fn loopback_host(value: &str) -> bool {
    let host = value.trim_start_matches("http://").trim_start_matches("https://");
    let host = host.split('/').next().unwrap_or("");
    let name = match host.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => h,
        _ => host,
    };
    matches!(name, "127.0.0.1" | "localhost" | "[::1]")
}

/// Constant-time comparison of the presented token.
fn token_ok(presented: Option<&String>, token: &str) -> bool {
    let Some(p) = presented.and_then(|v| v.strip_prefix("Bearer ")) else { return false };
    let (a, b) = (p.as_bytes(), token.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Writes an SSE stream from `rx` until `done` says the stream is complete or the client goes away.
fn stream_sse(stream: &mut TcpStream, extra: &[(&str, String)], rx: &Receiver<String>, until_response_for: Option<&str>) {
    let mut head = String::from("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n");
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    loop {
        match rx.recv_timeout(KEEP_ALIVE) {
            Ok(line) => {
                if stream.write_all(format!("event: message\ndata: {line}\n\n").as_bytes()).is_err() {
                    return;
                }
                let _ = stream.flush();
                if let Some(id) = until_response_for
                    && let Ok(v) = serde_json::from_str::<Value>(&line)
                    && v.get("id").map(id_text).as_deref() == Some(id)
                    && (v.get("result").is_some() || v.get("error").is_some())
                {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stream.write_all(b": keep-alive\n\n").is_err() {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn new_session_id() -> String {
    tt_persist::fsx::random_token(16)
}

/// Starts the server.
///
/// # Errors
/// The bind error (the port is taken, or the address is not local).
pub fn start(handle: ExecutorHandle, opts: HttpOptions) -> std::io::Result<HttpServer> {
    let listener = TcpListener::bind(opts.bind)?;
    let addr = listener.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let sessions: Arc<Mutex<HashMap<String, Session>>> = Arc::new(Mutex::new(HashMap::new()));
    let (stop2, sessions2) = (stop.clone(), sessions.clone());
    let token = opts.token.clone();
    let mut server_opts = opts.server.clone();
    "attached".clone_into(&mut server_opts.run_mode);
    std::thread::Builder::new().name("mcp-http-accept".into()).spawn(move || {
        for conn in listener.incoming() {
            if stop2.load(Ordering::Acquire) {
                break;
            }
            let Ok(mut stream) = conn else { continue };
            let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
            let (handle, sessions, token, server_opts) =
                (handle.clone(), sessions2.clone(), token.clone(), server_opts.clone());
            let _ = std::thread::Builder::new().name("mcp-http-conn".into()).spawn(move || {
                serve_connection(&mut stream, &handle, &sessions, &token, &server_opts);
            });
        }
    })?;
    Ok(HttpServer { addr, stop, sessions })
}

fn serve_connection(
    stream: &mut TcpStream,
    handle: &ExecutorHandle,
    sessions: &Arc<Mutex<HashMap<String, Session>>>,
    token: &str,
    server_opts: &ServerOptions,
) {
    let Some(req) = read_request(stream) else {
        json_error(stream, "400 Bad Request", "malformed or oversized request");
        return;
    };
    if req.path != ENDPOINT {
        respond(stream, "404 Not Found", &[], b"");
        return;
    }
    // DNS-rebinding and cross-origin protection (11 §4): loopback Host and Origin only.
    if !req.headers.get("host").is_some_and(|h| loopback_host(h)) || req.headers.get("origin").is_some_and(|o| !loopback_host(o)) {
        json_error(stream, "403 Forbidden", "non-local origin or host");
        return;
    }
    if !token_ok(req.headers.get("authorization"), token) {
        respond(stream, "401 Unauthorized", &[("WWW-Authenticate", "Bearer".to_owned())], b"");
        return;
    }
    let sid = req.headers.get("mcp-session-id").cloned();
    match req.method.as_str() {
        "POST" => post(stream, handle, sessions, server_opts, sid, &req.body),
        "GET" => {
            let Some(sid) = sid else { return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id") };
            let (tx, rx) = mpsc::channel::<String>();
            {
                let map = sessions.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(s) = map.get(&sid) else {
                    drop(map);
                    return json_error(stream, "404 Not Found", "unknown session");
                };
                let mut r = s.routes.lock().unwrap_or_else(PoisonError::into_inner);
                for line in r.backlog.drain(..) {
                    let _ = tx.send(line);
                }
                r.get = Some(tx);
            }
            stream_sse(stream, &[("Mcp-Session-Id", sid)], &rx, None);
        }
        "DELETE" => {
            let Some(sid) = sid else { return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id") };
            let removed = sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(&sid);
            respond(stream, if removed.is_some() { "200 OK" } else { "404 Not Found" }, &[], b"");
        }
        _ => respond(stream, "405 Method Not Allowed", &[("Allow", "GET, POST, DELETE".to_owned())], b""),
    }
}

fn post(
    stream: &mut TcpStream,
    handle: &ExecutorHandle,
    sessions: &Arc<Mutex<HashMap<String, Session>>>,
    server_opts: &ServerOptions,
    sid: Option<String>,
    body: &[u8],
) {
    let Ok(msg) = serde_json::from_slice::<Value>(body) else {
        return json_error(stream, "400 Bad Request", "the body is not one JSON-RPC message");
    };
    if !msg.is_object() {
        return json_error(stream, "400 Bad Request", "batched JSON-RPC messages are not accepted");
    }
    let is_init = msg.get("method").and_then(Value::as_str) == Some("initialize");
    let sid = match (sid, is_init) {
        (Some(s), _) => s,
        (None, true) => {
            if sessions.lock().unwrap_or_else(PoisonError::into_inner).len() >= MAX_SESSIONS {
                return json_error(stream, "503 Service Unavailable", "too many sessions");
            }
            let sid = new_session_id();
            let (input, rx) = mpsc::channel::<Vec<u8>>();
            let routes = Arc::new(Mutex::new(Routes::default()));
            let reader = ChannelReader { rx, buf: Vec::new(), pos: 0 };
            let writer = RouterWriter { routes: routes.clone(), pending: Vec::new() };
            let (h, o, map, id2) = (handle.clone(), server_opts.clone(), sessions.clone(), sid.clone());
            let spawned = std::thread::Builder::new().name("mcp-http-session".into()).spawn(move || {
                let _ = crate::serve(h, o, reader, writer);
                map.lock().unwrap_or_else(PoisonError::into_inner).remove(&id2);
            });
            if spawned.is_err() {
                return json_error(stream, "500 Internal Server Error", "cannot start the session");
            }
            sessions.lock().unwrap_or_else(PoisonError::into_inner).insert(sid.clone(), Session { input, routes });
            sid
        }
        (None, false) => return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id"),
    };
    let (input, routes) = {
        let map = sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(s) = map.get(&sid) else {
            drop(map);
            return json_error(stream, "404 Not Found", "unknown session");
        };
        (s.input.clone(), s.routes.clone())
    };
    let is_request = msg.get("method").is_some() && msg.get("id").is_some();
    let mut line = serde_json::to_vec(&msg).unwrap_or_default();
    line.push(b'\n');
    if !is_request {
        // Notifications and the client's responses (elicitation answers): accepted.
        let _ = input.send(line);
        return respond(stream, "202 Accepted", &[("Mcp-Session-Id", sid)], b"");
    }
    let id = id_text(msg.get("id").unwrap_or(&Value::Null));
    let progress = msg.get("params").and_then(|p| p.get("_meta")).and_then(|m| m.get("progressToken")).map(id_text);
    let (tx, rx) = mpsc::channel::<String>();
    routes.lock().unwrap_or_else(PoisonError::into_inner).requests.push((id.clone(), progress, tx));
    if input.send(line).is_err() {
        return json_error(stream, "410 Gone", "the session has ended");
    }
    stream_sse(stream, &[("Mcp-Session-Id", sid)], &rx, Some(&id));
    // A client that went away before the answer: forget its stream (a cancelled request gets no response, 11 §4).
    routes.lock().unwrap_or_else(PoisonError::into_inner).requests.retain(|(rid, _, _)| *rid != id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_and_tokens() {
        assert!(loopback_host("127.0.0.1:4000"));
        assert!(loopback_host("http://localhost:1"));
        assert!(loopback_host("[::1]:8"));
        assert!(!loopback_host("evil.example:4000"));
        assert!(!loopback_host("http://127.0.0.1.evil.example"));
        assert!(token_ok(Some(&"Bearer abc".to_owned()), "abc"));
        assert!(!token_ok(Some(&"Bearer abd".to_owned()), "abc"));
        assert!(!token_ok(Some(&"abc".to_owned()), "abc"));
        assert!(!token_ok(None, "abc"));
    }

    #[test]
    fn responses_progress_and_server_requests_are_routed() {
        let routes = Mutex::new(Routes::default());
        let (a, ra) = mpsc::channel();
        let (b, rb) = mpsc::channel();
        let (g, rg) = mpsc::channel();
        {
            let mut r = routes.lock().unwrap_or_else(PoisonError::into_inner);
            r.requests.push(("1".into(), Some("\"p1\"".into()), a));
            r.requests.push(("2".into(), None, b));
            r.get = Some(g);
        }
        route(&routes, r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"p1","progress":1}}"#);
        route(&routes, r#"{"jsonrpc":"2.0","id":9,"method":"elicitation/create","params":{}}"#);
        route(&routes, r#"{"jsonrpc":"2.0","method":"notifications/resources/updated","params":{"uri":"app://state"}}"#);
        route(&routes, r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
        assert!(ra.try_recv().unwrap_or_default().contains("progress"));
        assert!(ra.try_recv().unwrap_or_default().contains("result"));
        assert!(rb.try_recv().unwrap_or_default().contains("elicitation"));
        assert!(rg.try_recv().unwrap_or_default().contains("resources/updated"));
    }
}
