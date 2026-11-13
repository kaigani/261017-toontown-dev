//! Streamable HTTP transport for attached mode (11 §4; M4 C4.1; ADR-0022).
//!
//! A small HTTP/1.1 server on `std::net` (no async runtime, so the headless dependency policy is unchanged) that
//! speaks the Streamable HTTP transport of the MCP revisions this server implements:
//! - one endpoint (`/mcp`), bound to a loopback address only (a non-loopback address is refused: [`StartError`]);
//! - every request needs `Authorization: Bearer <token>` (the per-installation token of the discovery file), a `Host`
//!   header naming the loopback, and, when an `Origin` header is present, a loopback origin (DNS-rebinding
//!   protection);
//! - `POST` carries one JSON-RPC message: a request is answered with an SSE stream that carries the messages for that
//!   request (progress, the server's own requests such as `elicitation/create`) and ends with its response;
//!   notifications and responses are accepted with `202`;
//! - `GET` opens the session's event stream for messages that belong to no request (resource updates, log messages,
//!   tool-list changes);
//! - `DELETE` ends the session;
//! - `Mcp-Session-Id` is assigned at `initialize` and required afterwards.
//!
//! **Request handling (C41-R1-03, C41-R1-05).** The request head is read first, bounded by [`MAX_HEAD`] bytes and the
//! head deadline, and parsed strictly: the request line must be `METHOD SP target SP HTTP/1.1`; lines end in CRLF
//! (a bare LF or CR is refused); header lines need a colon and may not be folded; `Authorization`, `Host`, `Origin`,
//! `Content-Length` and `Mcp-Session-Id` may appear once; `Transfer-Encoding` is refused; `Content-Length` is decimal
//! digits only. Every refusal is a `400` with no session or tool side effect. Then, before a single body byte is
//! read or reserved, the path (`404`), `Host` and `Origin` (`403`), the token (`401`), the method (`405`), the
//! declared length (`413`) and the session (`404`) are checked. The body is read incrementally, never reserved from
//! the declared length, under the session's inline payload limit (`maxInlineBytes`, 11 §6.5, default 50 MiB: the
//! largest legitimate message carries inline media; the stdio transport frames with the same limit) and an absolute
//! request deadline.
//!
//! **Admission.** At most [`Limits::max_connections`] connections are served at once (SSE streams included) and at
//! most [`Limits::max_unauthenticated`] of them may be still before their token check; a connection over either cap
//! is answered `503 Service Unavailable` with `Retry-After: 1` and closed without reading. An SSE subscriber that does
//! not read is dropped: its queue holds [`Limits::sse_backlog`] messages and a blocked write gives up after
//! [`Limits::write_timeout`].
//!
//! **Sessions (O-7).** Each session runs the same protocol loop as stdio ([`crate::serve`]) over in-memory reader and
//! writer ends, so permissions, confirmations, the audit log and cancellation behave exactly as on stdio. A session
//! with no open stream and no request in flight expires after [`Limits::session_idle`] (30 minutes); its id then
//! answers `404` and the client initialises again. Session ids come from the operating system's random source only
//! (O-6). A holder of the installation token can use any session id it learns: the token already grants every
//! session's powers (one user, one installation, ADR-0022), so the id is a routing key, not a credential.

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;
use tt_executor::ExecutorHandle;

use crate::ServerOptions;

/// The endpoint path.
pub const ENDPOINT: &str = "/mcp";
/// The largest request head (request line and headers).
pub const MAX_HEAD: usize = 16 << 10;
/// Sessions served at once.
const MAX_SESSIONS: usize = 32;
/// How long an SSE stream waits between messages before it sends a keep-alive comment.
const KEEP_ALIVE: Duration = Duration::from_secs(15);
/// Body bytes read per step (the body grows as bytes arrive; nothing is reserved from the declared length).
const BODY_CHUNK: usize = 64 << 10;
/// Headers that may appear once at most (C41-R1-03).
const SINGLE: &[&str] = &["authorization", "host", "origin", "content-length", "mcp-session-id"];

/// Why the server did not start.
#[derive(Debug)]
pub enum StartError {
    /// The address is not a loopback address: remote binding is refused (ADR-0022 decision 2; C41-R1-02).
    RemoteBind(SocketAddr),
    /// The socket could not be bound or the accept thread not started.
    Io(std::io::Error),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RemoteBind(a) => write!(f, "remote binding is refused: {a} is not a loopback address (ADR-0022)"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StartError {}

impl From<std::io::Error> for StartError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Whether an address may be bound: IPv4 `127.0.0.0/8` or IPv6 `::1`.
#[must_use]
pub fn bind_allowed(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

/// Options of the HTTP server.
#[derive(Debug, Clone)]
pub struct HttpOptions {
    /// Address to bind (`127.0.0.1:0` picks a free port); loopback only.
    pub bind: SocketAddr,
    /// The bearer token clients must present.
    pub token: String,
    /// Options of each session's protocol loop (`run_mode` is forced to `attached`).
    pub server: ServerOptions,
}

/// Resource bounds of the server (defaults: [`Limits::default`]).
#[derive(Debug, Clone)]
pub struct Limits {
    /// Connections served at once, SSE streams included.
    pub max_connections: usize,
    /// Connections not yet past their token check.
    pub max_unauthenticated: usize,
    /// The request head must arrive within this time of the connection being accepted.
    pub head_deadline: Duration,
    /// The whole request (head and body) must arrive within this time of the connection being accepted.
    pub request_deadline: Duration,
    /// A blocked write to a client gives up after this time.
    pub write_timeout: Duration,
    /// Messages queued for one SSE stream before its subscriber is dropped.
    pub sse_backlog: usize,
    /// A session with no open stream and no request in flight expires after this time.
    pub session_idle: Duration,
    /// How often expired sessions are looked for.
    pub sweep: Duration,
    /// The largest request body (`None`: the session's `maxInlineBytes`, 11 §6.5).
    pub max_body: Option<usize>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_connections: 64,
            max_unauthenticated: 16,
            head_deadline: Duration::from_secs(10),
            request_deadline: Duration::from_secs(60),
            write_timeout: Duration::from_secs(10),
            sse_backlog: 1024,
            session_idle: Duration::from_secs(30 * 60),
            sweep: Duration::from_secs(60),
            max_body: None,
        }
    }
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

    /// The number of live sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner).len()
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
    requests: Vec<(String, Option<String>, SyncSender<String>)>,
    /// The session's GET stream.
    get: Option<SyncSender<String>>,
    /// Messages for no open stream, delivered to the next GET stream (bounded).
    backlog: Vec<String>,
    /// Streams being written now (request and GET streams).
    streams: usize,
}

/// One HTTP session: the input end of its protocol loop, its routes and when it was last used.
struct Session {
    input: Sender<Vec<u8>>,
    routes: Arc<Mutex<Routes>>,
    last_seen: Instant,
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

/// Queues a message for a stream; `false` when the subscriber is gone or has stopped reading (its queue is full).
fn offer(tx: &SyncSender<String>, line: &str) -> bool {
    match tx.try_send(line.to_owned()) {
        Ok(()) => true,
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
    }
}

fn route(routes: &Mutex<Routes>, line: &str) {
    let Ok(msg) = serde_json::from_str::<Value>(line) else { return };
    let mut r = routes.lock().unwrap_or_else(PoisonError::into_inner);
    let is_response = msg.get("id").is_some() && (msg.get("result").is_some() || msg.get("error").is_some());
    if is_response {
        let id = id_text(msg.get("id").unwrap_or(&Value::Null));
        if let Some(pos) = r.requests.iter().position(|(rid, _, _)| *rid == id) {
            let (_, _, tx) = r.requests.remove(pos);
            let _ = offer(&tx, line);
        }
        return;
    }
    // Progress belongs to the request that asked for it.
    if msg.get("method").and_then(Value::as_str) == Some("notifications/progress")
        && let Some(token) = msg.get("params").and_then(|p| p.get("progressToken"))
    {
        let token = id_text(token);
        if let Some(pos) = r.requests.iter().position(|(_, t, _)| t.as_deref() == Some(token.as_str())) {
            if !offer(&r.requests[pos].2, line) {
                // A request stream that does not read is dropped.
                r.requests.remove(pos);
            }
            return;
        }
    }
    // A server request (elicitation) goes to the newest open request stream; other notifications to the GET stream.
    let is_request = msg.get("id").is_some() && msg.get("method").is_some();
    if is_request && let Some((_, _, tx)) = r.requests.last() {
        if !offer(tx, line) {
            r.requests.pop();
        }
        return;
    }
    if let Some(get) = &r.get {
        if offer(get, line) {
            return;
        }
        // The GET subscriber stopped reading or went away: dropped; messages wait for the next one.
        r.get = None;
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

/// A parsed request head.
#[derive(Debug)]
struct Head {
    method: String,
    path: String,
    /// Header names in lower case, in order.
    headers: Vec<(String, String)>,
    content_length: usize,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

/// Why a request head was refused.
#[derive(Debug, PartialEq, Eq)]
enum HeadError {
    /// The client closed the connection before a complete head.
    Closed,
    /// The head did not arrive within the head deadline.
    Timeout,
    /// Larger than [`MAX_HEAD`].
    TooLarge,
    /// Malformed or ambiguous (C41-R1-03).
    Malformed(&'static str),
}

/// RFC 9110 `tchar`.
fn tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

/// Parses a request head (the bytes before the blank line, without it).
fn parse_head(raw: &[u8]) -> Result<Head, HeadError> {
    // Only visible ASCII, space, tab and CRLF line ends.
    for (i, &b) in raw.iter().enumerate() {
        match b {
            b'\n' if i == 0 || raw[i - 1] != b'\r' => return Err(HeadError::Malformed("bare LF line ending")),
            b'\r' if raw.get(i + 1) != Some(&b'\n') => return Err(HeadError::Malformed("bare CR")),
            b'\r' | b'\n' | b'\t' | 0x20..=0x7e => {}
            _ => return Err(HeadError::Malformed("control or non-ASCII byte in the head")),
        }
    }
    let text = std::str::from_utf8(raw).map_err(|_| HeadError::Malformed("head is not ASCII"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = request_line.split(' ').collect();
    let [method, target, version] = parts.as_slice() else {
        return Err(HeadError::Malformed("request line is not METHOD SP target SP HTTP/1.1"));
    };
    if method.is_empty() || method.len() > 16 || !method.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(HeadError::Malformed("bad method"));
    }
    if !target.starts_with('/') || target.contains('\t') {
        return Err(HeadError::Malformed("bad request target"));
    }
    if *version != "HTTP/1.1" {
        return Err(HeadError::Malformed("only HTTP/1.1 is served"));
    }
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in lines {
        if line.is_empty() {
            return Err(HeadError::Malformed("empty header line"));
        }
        if line.starts_with([' ', '\t']) {
            return Err(HeadError::Malformed("folded header line"));
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(HeadError::Malformed("header line without a colon"));
        };
        if name.is_empty() || !name.bytes().all(tchar) {
            return Err(HeadError::Malformed("bad header name"));
        }
        let name = name.to_ascii_lowercase();
        if name == "transfer-encoding" {
            return Err(HeadError::Malformed("Transfer-Encoding is not accepted"));
        }
        if SINGLE.contains(&name.as_str()) && headers.iter().any(|(k, _)| *k == name) {
            return Err(HeadError::Malformed("duplicate security or framing header"));
        }
        headers.push((name, value.trim_matches([' ', '\t']).to_owned()));
    }
    let content_length = match headers.iter().find(|(k, _)| k == "content-length") {
        None => 0,
        Some((_, v)) => {
            if v.is_empty() || v.len() > 19 || !v.bytes().all(|b| b.is_ascii_digit()) {
                return Err(HeadError::Malformed("Content-Length is not a decimal number"));
            }
            v.parse::<usize>().map_err(|_| HeadError::Malformed("Content-Length out of range"))?
        }
    };
    let path = target.split('?').next().unwrap_or("").to_owned();
    Ok(Head { method: (*method).to_owned(), path, headers, content_length })
}

/// Sets the socket's read timeout to what is left before `deadline`; `false` when nothing is left.
fn arm(stream: &TcpStream, deadline: Instant) -> bool {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return false;
    }
    stream.set_read_timeout(Some(left)).is_ok()
}

/// Reads the request head; returns it with the body bytes that arrived with it.
fn read_head(stream: &mut TcpStream, deadline: Instant) -> Result<(Head, Vec<u8>), HeadError> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(end + 4);
            buf.truncate(end);
            return parse_head(&buf).map(|h| (h, rest));
        }
        if buf.len() > MAX_HEAD {
            return Err(HeadError::TooLarge);
        }
        if !arm(stream, deadline) {
            return Err(HeadError::Timeout);
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Err(HeadError::Closed),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                return Err(HeadError::Timeout);
            }
            Err(_) => return Err(HeadError::Closed),
        }
    }
}

/// Reads `len` body bytes incrementally (`first` already arrived with the head) before `deadline`.
fn read_body(stream: &mut TcpStream, first: Vec<u8>, len: usize, deadline: Instant) -> Option<Vec<u8>> {
    let mut body = first;
    body.truncate(len);
    let mut chunk = vec![0u8; BODY_CHUNK.min(len.max(1))];
    while body.len() < len {
        if !arm(stream, deadline) {
            return None;
        }
        let want = (len - body.len()).min(chunk.len());
        match stream.read(&mut chunk[..want]) {
            Ok(0) | Err(_) => return None,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    Some(body)
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

/// The host part of an authority (`host [":" port]`, RFC 9110 §7.2), lower-cased; `None` when it is not one: empty,
/// whitespace or control characters, a scheme, path, query, fragment or userinfo, a bad port, a bad IPv6 literal.
fn authority_host(a: &str) -> Option<String> {
    if a.is_empty() || !a.bytes().all(|b| (0x21..=0x7e).contains(&b)) || a.contains(['/', '?', '#', '@', '\\']) {
        return None;
    }
    let (host, port) = if let Some(rest) = a.strip_prefix('[') {
        let (inside, after) = rest.split_once(']')?;
        inside.parse::<Ipv6Addr>().ok()?;
        let port = if after.is_empty() { None } else { Some(after.strip_prefix(':')?) };
        (format!("[{}]", inside.to_ascii_lowercase()), port)
    } else {
        match a.split_once(':') {
            Some((h, p)) => (h.to_ascii_lowercase(), Some(p)),
            None => (a.to_ascii_lowercase(), None),
        }
    };
    if let Some(p) = port
        && (p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) || p.parse::<u32>().ok()? > 65_535)
    {
        return None;
    }
    if host.is_empty() {
        return None;
    }
    Some(host)
}

/// Whether a normalised host names the loopback: `localhost` (also in its fully qualified form `localhost.`, the same
/// DNS name), `127.0.0.1`, or the IPv6 loopback in any spelling (`[::1]`, `[0:0:0:0:0:0:0:1]`).
fn loopback_name(host: &str) -> bool {
    if host == "localhost" || host == "localhost." {
        return true;
    }
    if let Some(inside) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return inside.parse::<Ipv6Addr>().is_ok_and(|ip| ip.is_loopback());
    }
    host.parse::<Ipv4Addr>().is_ok_and(|ip| ip == Ipv4Addr::LOCALHOST)
}

/// The `Host` header names the loopback (C41-R1-06): an authority only, compared case-insensitively.
fn host_ok(value: &str) -> bool {
    authority_host(value).is_some_and(|h| loopback_name(&h))
}

/// The `Origin` header is a loopback origin: `http` or `https`, `://`, a loopback authority, nothing after it. The
/// opaque origin `null` is refused.
fn origin_ok(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else { return false };
    (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) && host_ok(rest)
}

/// Constant-time comparison of the presented token.
fn token_ok(presented: Option<&str>, token: &str) -> bool {
    let Some(p) = presented.and_then(|v| v.strip_prefix("Bearer ")) else { return false };
    let (a, b) = (p.as_bytes(), token.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Writes an SSE stream from `rx` until the response for `until_response_for` was written, the subscriber goes away
/// or a write blocks longer than the write timeout.
fn stream_sse(
    stream: &mut TcpStream,
    extra: &[(&str, String)],
    rx: &Receiver<String>,
    until_response_for: Option<&str>,
) {
    let mut head = String::from(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n",
    );
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

/// A session id from the operating system's random source (O-6); `None` when the source fails.
fn new_session_id() -> Option<String> {
    tt_persist::fsx::secret_token(16).ok()
}

/// Shared state of the connection threads.
struct Shared {
    handle: ExecutorHandle,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    token: String,
    server_opts: ServerOptions,
    limits: Limits,
    max_body: usize,
    open: AtomicUsize,
    unauthenticated: AtomicUsize,
}

/// Counts a connection while it lives.
struct Counted<'a>(&'a AtomicUsize);

impl Drop for Counted<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Takes a slot under `cap`; `None` when the cap is reached.
fn admit(counter: &AtomicUsize, cap: usize) -> Option<Counted<'_>> {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| (n < cap).then_some(n + 1))
        .ok()
        .map(|_| Counted(counter))
}

/// Starts the server with the default [`Limits`].
///
/// # Errors
/// [`StartError::RemoteBind`] for a non-loopback address (nothing is bound); [`StartError::Io`] when the port is taken.
pub fn start(handle: ExecutorHandle, opts: HttpOptions) -> Result<HttpServer, StartError> {
    start_with(handle, opts, Limits::default())
}

/// Starts the server with explicit resource bounds.
///
/// # Errors
/// As [`start`].
pub fn start_with(handle: ExecutorHandle, opts: HttpOptions, limits: Limits) -> Result<HttpServer, StartError> {
    if !bind_allowed(&opts.bind) {
        return Err(StartError::RemoteBind(opts.bind));
    }
    let listener = TcpListener::bind(opts.bind)?;
    let addr = listener.local_addr()?;
    if !bind_allowed(&addr) {
        return Err(StartError::RemoteBind(addr));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let sessions: Arc<Mutex<HashMap<String, Session>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut server_opts = opts.server.clone();
    "attached".clone_into(&mut server_opts.run_mode);
    // The body limit follows the session's inline payload limit, as the stdio framing does (11 §6.5).
    let max_body = limits.max_body.unwrap_or_else(|| {
        handle.agent_status("").map_or(50 << 20, |s| usize::try_from(s.limits.max_inline_bytes).unwrap_or(usize::MAX))
    });
    let shared = Arc::new(Shared {
        handle,
        sessions: sessions.clone(),
        token: opts.token,
        server_opts,
        limits,
        max_body,
        open: AtomicUsize::new(0),
        unauthenticated: AtomicUsize::new(0),
    });
    let (stop2, accept) = (stop.clone(), shared.clone());
    std::thread::Builder::new().name("mcp-http-accept".into()).spawn(move || {
        for conn in listener.incoming() {
            if stop2.load(Ordering::Acquire) {
                break;
            }
            let Ok(mut stream) = conn else { continue };
            let _ = stream.set_write_timeout(Some(accept.limits.write_timeout));
            let accepted = Instant::now();
            let shared = accept.clone();
            let _ = std::thread::Builder::new().name("mcp-http-conn".into()).spawn(move || {
                let Some(_open) = admit(&shared.open, shared.limits.max_connections) else {
                    return busy(&mut stream);
                };
                let Some(unauth) = admit(&shared.unauthenticated, shared.limits.max_unauthenticated) else {
                    return busy(&mut stream);
                };
                serve_connection(&mut stream, &shared, accepted, unauth);
                linger_close(&mut stream);
            });
        }
    })?;
    let (stop3, sweeper) = (stop.clone(), shared);
    std::thread::Builder::new().name("mcp-http-sweep".into()).spawn(move || {
        while !stop3.load(Ordering::Acquire) {
            std::thread::sleep(sweeper.limits.sweep);
            expire(&sweeper.sessions, sweeper.limits.session_idle);
        }
    })?;
    Ok(HttpServer { addr, stop, sessions })
}

/// `503` for a connection over a cap (nothing is read).
fn busy(stream: &mut TcpStream) {
    respond(stream, "503 Service Unavailable", &[("Retry-After", "1".to_owned())], b"");
    linger_close(stream);
}

/// Bytes discarded at most after the answer, so an early refusal (sent before the request body was read) reaches the
/// client: closing a socket with unread input makes the operating system reset the connection, and a reset can drop
/// the answer before the client reads it.
const LINGER_BYTES: usize = 256 << 10;
/// How long the discarding lasts at most.
const LINGER_TIME: Duration = Duration::from_secs(1);

/// Ends a connection gracefully: no more output, then up to [`LINGER_BYTES`] of remaining input discarded through a
/// fixed buffer within [`LINGER_TIME`] (nothing is reserved or parsed), then the socket closes.
fn linger_close(stream: &mut TcpStream) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let deadline = Instant::now() + LINGER_TIME;
    let mut sink = [0u8; 4096];
    let mut discarded = 0usize;
    while discarded < LINGER_BYTES && arm(stream, deadline) {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => discarded += n,
        }
    }
}

/// Ends sessions with no open stream and no request in flight, idle for longer than `idle` (O-7).
fn expire(sessions: &Mutex<HashMap<String, Session>>, idle: Duration) {
    let mut map = sessions.lock().unwrap_or_else(PoisonError::into_inner);
    map.retain(|_, s| {
        let r = s.routes.lock().unwrap_or_else(PoisonError::into_inner);
        r.streams > 0 || !r.requests.is_empty() || s.last_seen.elapsed() < idle
    });
}

fn serve_connection(stream: &mut TcpStream, shared: &Shared, accepted: Instant, unauth: Counted<'_>) {
    let head_deadline = accepted + shared.limits.head_deadline;
    let (head, first) = match read_head(stream, head_deadline) {
        Ok(h) => h,
        Err(HeadError::Closed) => return,
        Err(HeadError::Timeout) => {
            return json_error(stream, "408 Request Timeout", "the request head did not arrive in time");
        }
        Err(HeadError::TooLarge) => {
            return json_error(stream, "431 Request Header Fields Too Large", "request head too large");
        }
        Err(HeadError::Malformed(why)) => return json_error(stream, "400 Bad Request", why),
    };
    if head.path != ENDPOINT {
        return respond(stream, "404 Not Found", &[], b"");
    }
    // DNS-rebinding and cross-origin protection (11 §4): a loopback Host, and a loopback Origin when present.
    if !head.header("host").is_some_and(host_ok) || head.header("origin").is_some_and(|o| !origin_ok(o)) {
        return json_error(stream, "403 Forbidden", "non-local origin or host");
    }
    if !token_ok(head.header("authorization"), &shared.token) {
        return respond(stream, "401 Unauthorized", &[("WWW-Authenticate", "Bearer".to_owned())], b"");
    }
    drop(unauth);
    let sid = head.header("mcp-session-id").map(str::to_owned);
    match head.method.as_str() {
        "POST" => post(stream, shared, &head, first, sid, accepted),
        "GET" => {
            let Some(sid) = sid else { return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id") };
            let (tx, rx) = mpsc::sync_channel::<String>(shared.limits.sse_backlog);
            let routes = {
                let mut map = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(s) = map.get_mut(&sid) else {
                    drop(map);
                    return json_error(stream, "404 Not Found", "unknown session");
                };
                s.last_seen = Instant::now();
                let mut r = s.routes.lock().unwrap_or_else(PoisonError::into_inner);
                for line in r.backlog.drain(..) {
                    let _ = offer(&tx, &line);
                }
                r.get = Some(tx);
                r.streams += 1;
                drop(r);
                s.routes.clone()
            };
            stream_sse(stream, &[("Mcp-Session-Id", sid.clone())], &rx, None);
            end_stream(shared, &sid, &routes);
        }
        "DELETE" => {
            let Some(sid) = sid else { return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id") };
            let removed = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(&sid);
            respond(stream, if removed.is_some() { "200 OK" } else { "404 Not Found" }, &[], b"");
        }
        _ => respond(stream, "405 Method Not Allowed", &[("Allow", "GET, POST, DELETE".to_owned())], b""),
    }
}

/// A stream ended: its session counts it no more and starts its idle time now.
fn end_stream(shared: &Shared, sid: &str, routes: &Mutex<Routes>) {
    {
        let mut r = routes.lock().unwrap_or_else(PoisonError::into_inner);
        r.streams = r.streams.saturating_sub(1);
    }
    if let Some(s) = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).get_mut(sid) {
        s.last_seen = Instant::now();
    }
}

fn post(stream: &mut TcpStream, shared: &Shared, head: &Head, first: Vec<u8>, sid: Option<String>, accepted: Instant) {
    if head.content_length > shared.max_body {
        return json_error(stream, "413 Content Too Large", "the request body is larger than the inline payload limit");
    }
    // A named session must exist before its body is read.
    if let Some(s) = &sid
        && !shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).contains_key(s)
    {
        return json_error(stream, "404 Not Found", "unknown session");
    }
    let Some(body) = read_body(stream, first, head.content_length, accepted + shared.limits.request_deadline) else {
        return json_error(stream, "408 Request Timeout", "the request body did not arrive in time");
    };
    let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
        return json_error(stream, "400 Bad Request", "the body is not one JSON-RPC message");
    };
    if !msg.is_object() {
        return json_error(stream, "400 Bad Request", "batched JSON-RPC messages are not accepted");
    }
    let is_init = msg.get("method").and_then(Value::as_str) == Some("initialize");
    let sid = match (sid, is_init) {
        (Some(s), _) => s,
        (None, true) => match open_session(shared) {
            Ok(s) => s,
            Err((status, why)) => return json_error(stream, status, why),
        },
        (None, false) => return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id"),
    };
    let (input, routes) = {
        let mut map = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(s) = map.get_mut(&sid) else {
            drop(map);
            return json_error(stream, "404 Not Found", "unknown session");
        };
        s.last_seen = Instant::now();
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
    let (tx, rx) = mpsc::sync_channel::<String>(shared.limits.sse_backlog);
    {
        let mut r = routes.lock().unwrap_or_else(PoisonError::into_inner);
        r.requests.push((id.clone(), progress, tx));
        r.streams += 1;
    }
    if input.send(line).is_err() {
        end_stream(shared, &sid, &routes);
        return json_error(stream, "410 Gone", "the session has ended");
    }
    stream_sse(stream, &[("Mcp-Session-Id", sid.clone())], &rx, Some(&id));
    // A client that went away before the answer: forget its stream (a cancelled request gets no response, 11 §4).
    routes.lock().unwrap_or_else(PoisonError::into_inner).requests.retain(|(rid, _, _)| *rid != id);
    end_stream(shared, &sid, &routes);
}

/// Opens a session for `initialize`: the cap check and the insertion are one critical section (O-7).
fn open_session(shared: &Shared) -> Result<String, (&'static str, &'static str)> {
    let Some(sid) = new_session_id() else {
        return Err(("503 Service Unavailable", "the operating system's random source failed: no session id"));
    };
    let (input, rx) = mpsc::channel::<Vec<u8>>();
    let routes = Arc::new(Mutex::new(Routes::default()));
    {
        let mut map = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= MAX_SESSIONS {
            return Err(("503 Service Unavailable", "too many sessions"));
        }
        map.insert(sid.clone(), Session { input, routes: routes.clone(), last_seen: Instant::now() });
    }
    let reader = ChannelReader { rx, buf: Vec::new(), pos: 0 };
    let writer = RouterWriter { routes, pending: Vec::new() };
    let (h, o, map, id2) = (shared.handle.clone(), shared.server_opts.clone(), shared.sessions.clone(), sid.clone());
    let spawned = std::thread::Builder::new().name("mcp-http-session".into()).spawn(move || {
        let _ = crate::serve(h, o, reader, writer);
        map.lock().unwrap_or_else(PoisonError::into_inner).remove(&id2);
    });
    if spawned.is_err() {
        shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(&sid);
        return Err(("500 Internal Server Error", "cannot start the session"));
    }
    Ok(sid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_origins_and_tokens() {
        for good in ["127.0.0.1:4000", "localhost", "LOCALHOST:1", "localhost.", "[::1]:8", "[0:0:0:0:0:0:0:1]"] {
            assert!(host_ok(good), "{good}");
        }
        for bad in [
            "evil.example:4000",
            "localhost/foreign",
            "http://localhost",
            "user@localhost",
            "127.0.0.1.evil.example",
            "localhost:99999",
            "localhost:",
            "[::2]",
            "",
            "local host",
        ] {
            assert!(!host_ok(bad), "{bad}");
        }
        assert!(origin_ok("http://localhost:1420") && origin_ok("HTTPS://127.0.0.1"));
        for bad in ["null", "localhost", "http://localhost/", "file://localhost", "http://evil.example"] {
            assert!(!origin_ok(bad), "{bad}");
        }
        assert!(token_ok(Some("Bearer abc"), "abc"));
        assert!(!token_ok(Some("Bearer abd"), "abc"));
        assert!(!token_ok(Some("abc"), "abc"));
        assert!(!token_ok(None, "abc"));
    }

    #[test]
    fn heads_are_parsed_strictly() {
        let ok = parse_head(b"POST /mcp?x=1 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 12");
        assert!(ok.as_ref().is_ok_and(|h| h.path == "/mcp" && h.content_length == 12), "{ok:?}");
        for bad in [
            &b"POST /mcp HTTP/1.1\nHost: localhost"[..],
            b"POST /mcp HTTP/1.1\r\nHost: localhost\n",
            b"POST  /mcp HTTP/1.1",
            b"POST /mcp HTTP/1.0",
            b"post /mcp HTTP/1.1",
            b"POST mcp HTTP/1.1",
            b"POST /mcp HTTP/1.1\r\nHost localhost",
            b"POST /mcp HTTP/1.1\r\nHost: localhost\r\n folded",
            b"POST /mcp HTTP/1.1\r\nHost : localhost",
            b"POST /mcp HTTP/1.1\r\nContent-Length: -1",
            b"POST /mcp HTTP/1.1\r\nContent-Length: +1",
            b"POST /mcp HTTP/1.1\r\nContent-Length: 1x",
            b"POST /mcp HTTP/1.1\r\nTransfer-Encoding: identity",
            b"POST /mcp HTTP/1.1\r\nHost: a\r\nhost: b",
            b"POST /mcp HTTP/1.1\r\nMcp-Session-Id: a\r\nMcp-Session-Id: a",
        ] {
            assert!(matches!(parse_head(bad), Err(HeadError::Malformed(_))), "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn responses_progress_and_server_requests_are_routed() {
        let routes = Mutex::new(Routes::default());
        let (a, ra) = mpsc::sync_channel(8);
        let (b, rb) = mpsc::sync_channel(8);
        let (g, rg) = mpsc::sync_channel(8);
        {
            let mut r = routes.lock().unwrap_or_else(PoisonError::into_inner);
            r.requests.push(("1".into(), Some("\"p1\"".into()), a));
            r.requests.push(("2".into(), None, b));
            r.get = Some(g);
        }
        route(
            &routes,
            r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"p1","progress":1}}"#,
        );
        route(&routes, r#"{"jsonrpc":"2.0","id":9,"method":"elicitation/create","params":{}}"#);
        route(
            &routes,
            r#"{"jsonrpc":"2.0","method":"notifications/resources/updated","params":{"uri":"app://state"}}"#,
        );
        route(&routes, r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
        assert!(ra.try_recv().unwrap_or_default().contains("progress"));
        assert!(ra.try_recv().unwrap_or_default().contains("result"));
        assert!(rb.try_recv().unwrap_or_default().contains("elicitation"));
        assert!(rg.try_recv().unwrap_or_default().contains("resources/updated"));
    }

    #[test]
    fn a_subscriber_that_does_not_read_is_dropped() {
        let routes = Mutex::new(Routes::default());
        let (g, _rg) = mpsc::sync_channel(2);
        routes.lock().unwrap_or_else(PoisonError::into_inner).get = Some(g);
        let note = r#"{"jsonrpc":"2.0","method":"notifications/resources/updated","params":{"uri":"app://state"}}"#;
        route(&routes, note);
        route(&routes, note);
        assert!(routes.lock().unwrap_or_else(PoisonError::into_inner).get.is_some(), "two fit the backlog");
        route(&routes, note);
        assert!(routes.lock().unwrap_or_else(PoisonError::into_inner).get.is_none(), "the third finds it full");
    }
}
