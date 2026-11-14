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
//! **Admission (O-15; `reviews/M4-design-http-admission.md`).** The accept thread decides about each socket without
//! reading from it or waiting: at most [`Limits::max_connections`] connection workers (one thread and one socket each,
//! SSE streams included), of which at most [`Limits::max_unauthenticated`] may still be reading their request head
//! (the pending pool). A connection that meets a full pending pool is answered `503` with `Retry-After: 0`, and the
//! pending connection that has waited longest for its head is evicted, so idle sockets cannot keep a slot past the
//! next newcomer. Refusals are written without a thread, and every answered socket is closed gracefully by one closer
//! thread holding at most [`Limits::max_lingering`] sockets. The server holds at most `3 + max_connections + 32`
//! threads. An SSE subscriber that does not read is dropped: its queue holds [`Limits::sse_backlog`] messages and a
//! blocked write gives up after [`Limits::write_timeout`].
//!
//! **Sessions (O-7).** Each session runs the same protocol loop as stdio ([`crate::serve`]) over in-memory reader and
//! writer ends, so permissions, confirmations, the audit log and cancellation behave exactly as on stdio. A request
//! naming a session holds it in flight from the moment its head is authenticated until its answer (or stream) is
//! written. A session with no request in flight expires after [`Limits::session_idle`] (30 minutes); its id then
//! answers `404` and the client initialises again. Session ids come from the operating system's random source only
//! (O-6). A holder of the installation token can use any session id it learns: the token already grants every
//! session's powers (one user, one installation, ADR-0022), so the id is a routing key, not a credential.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
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
/// How often a quiet SSE stream checks that its client is still there.
const CLIENT_CHECK: Duration = Duration::from_secs(1);
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
    /// Connection workers at once (one thread and one socket each), SSE streams included.
    pub max_connections: usize,
    /// Workers still reading their request head (the pending pool; used as `min(this, max_connections)`).
    pub max_unauthenticated: usize,
    /// Answered sockets being closed gracefully by the closer thread; over it, the oldest is closed at once.
    pub max_lingering: usize,
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
            max_lingering: 128,
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
pub struct HttpServer {
    /// The bound address (the port actually used).
    pub addr: SocketAddr,
    shared: Arc<Shared>,
}

impl std::fmt::Debug for HttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpServer").field("addr", &self.addr).finish_non_exhaustive()
    }
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
        self.shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// What the server holds now (design note §3).
    #[must_use]
    pub fn stats(&self) -> Stats {
        let sh = &self.shared;
        Stats {
            workers: sh.workers.load(Ordering::Acquire),
            pending: sh.pending.lock().unwrap_or_else(PoisonError::into_inner).len(),
            lingering: sh.lingering.lock().unwrap_or_else(PoisonError::into_inner).len(),
            sessions: self.session_count(),
        }
    }

    /// Stops accepting connections, evicts the pending connections, closes the lingering sockets and ends every
    /// session (their agents are disconnected and their open streams end).
    pub fn stop(&self) {
        if self.shared.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        // Wake the accept loop.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        let evicted: Vec<Pending> =
            self.shared.pending.lock().unwrap_or_else(PoisonError::into_inner).drain(..).collect();
        for p in &evicted {
            p.evicted.store(true, Ordering::Release);
        }
        let drained: Vec<Session> =
            self.shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).drain().map(|(_, s)| s).collect();
        drop(drained);
        // The closer closes its sockets and the sweeper ends.
        let gate = self.shared.lingering.lock().unwrap_or_else(PoisonError::into_inner);
        self.shared.wake.notify_all();
        drop(gate);
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
}

/// One HTTP session: the input end of its protocol loop, its routes, its requests in flight and when the last one
/// ended.
struct Session {
    input: Sender<Vec<u8>>,
    routes: Arc<Mutex<Routes>>,
    last_seen: Instant,
    /// Requests and streams in flight now ([`InFlight`] guards).
    in_flight: usize,
}

impl Drop for Session {
    /// A session that ends (`DELETE`, expiry, the end of its protocol loop, server stop) ends its open streams: their
    /// queues close, so they stop waiting for messages that will not come.
    fn drop(&mut self) {
        let mut r = self.routes.lock().unwrap_or_else(PoisonError::into_inner);
        r.requests.clear();
        r.get = None;
    }
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

/// Reads the request head; returns it with the body bytes that arrived with it. Reads in slices of [`EVICT_POLL`] and
/// gives up ([`HeadError::Closed`]) as soon as `evicted` is set.
fn read_head(stream: &mut TcpStream, deadline: Instant, evicted: &AtomicBool) -> Result<(Head, Vec<u8>), HeadError> {
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
        if evicted.load(Ordering::Acquire) {
            return Err(HeadError::Closed);
        }
        if !arm(stream, deadline.min(Instant::now() + EVICT_POLL)) {
            return Err(HeadError::Timeout);
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Err(HeadError::Closed),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                if Instant::now() >= deadline {
                    return Err(HeadError::Timeout);
                }
            }
            Err(_) => return Err(HeadError::Closed),
        }
    }
}

/// Reads `len` body bytes incrementally (`first` already arrived with the head) before `deadline`, in slices of
/// [`EVICT_POLL`]: a server stop ends the read within one slice (C41-R3-02), and the worker then drops what kept its
/// session's protocol loop alive.
fn read_body(
    stream: &mut TcpStream,
    first: Vec<u8>,
    len: usize,
    deadline: Instant,
    stopped: &AtomicBool,
) -> Option<Vec<u8>> {
    let mut body = first;
    body.truncate(len);
    let mut chunk = vec![0u8; BODY_CHUNK.min(len.max(1))];
    while body.len() < len {
        if stopped.load(Ordering::Acquire) || !arm(stream, deadline.min(Instant::now() + EVICT_POLL)) {
            return None;
        }
        let want = (len - body.len()).min(chunk.len());
        match stream.read(&mut chunk[..want]) {
            Ok(0) => return None,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                if Instant::now() >= deadline {
                    return None;
                }
            }
            Err(_) => return None,
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
    write_events(stream, rx, until_response_for);
    // The end of the stream is the end of the answer: the client learns it from the server's FIN.
    let _ = stream.shutdown(std::net::Shutdown::Write);
}

fn write_events(stream: &mut TcpStream, rx: &Receiver<String>, until_response_for: Option<&str>) {
    let mut quiet = Duration::ZERO;
    loop {
        match rx.recv_timeout(CLIENT_CHECK) {
            Ok(line) => {
                quiet = Duration::ZERO;
                if stream
                    .write_all(
                        format!(
                            "event: message
data: {line}

"
                        )
                        .as_bytes(),
                    )
                    .is_err()
                {
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
                // A client that went away ends its stream (and releases its session's in-flight guard) within
                // CLIENT_CHECK, not at the next keep-alive write.
                if client_gone(stream) {
                    return;
                }
                quiet += CLIENT_CHECK;
                if quiet >= KEEP_ALIVE {
                    quiet = Duration::ZERO;
                    if stream
                        .write_all(
                            b": keep-alive

",
                        )
                        .is_err()
                    {
                        return;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Whether the client of a stream closed its side or the connection failed (a non-blocking peek sees the end or an
/// error; input it may still send is left unread).
fn client_gone(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return true;
    }
    let mut b = [0u8; 1];
    let gone = match stream.peek(&mut b) {
        Ok(0) => true,
        Ok(_) => false,
        Err(e) => e.kind() != std::io::ErrorKind::WouldBlock,
    };
    stream.set_nonblocking(false).is_err() || gone
}

/// A session id from the operating system's random source (O-6); `None` when the source fails.
fn new_session_id() -> Option<String> {
    tt_persist::fsx::secret_token(16).ok()
}

/// A connection whose request head is not complete yet (the pending pool, design note §4).
struct Pending {
    id: u64,
    since: Instant,
    /// Set when the connection is evicted (or the server stops): its worker, reading the head in slices of
    /// [`EVICT_POLL`], sees it within one slice and ends. (No second handle on the socket is kept: closing a duplicated
    /// socket handle resets the connection on Windows.)
    evicted: Arc<AtomicBool>,
}

/// How often a worker reading its head looks whether it was evicted.
const EVICT_POLL: Duration = Duration::from_millis(50);

/// A socket being closed gracefully (design note §5).
struct Lingering {
    stream: TcpStream,
    deadline: Instant,
    discarded: usize,
}

/// Shared state of the server's threads.
struct Shared {
    handle: ExecutorHandle,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    token: String,
    server_opts: ServerOptions,
    limits: Limits,
    max_body: usize,
    /// Connection workers alive (each owns one thread and one socket): at most `max_connections`.
    workers: AtomicUsize,
    /// The pending pool: at most `min(max_unauthenticated, max_connections)` entries. A leaf lock.
    pending: Mutex<Vec<Pending>>,
    next_id: AtomicU64,
    /// Sockets being closed by the closer thread: at most `max_lingering`. A leaf lock.
    lingering: Mutex<VecDeque<Lingering>>,
    /// Wakes the closer (a socket was handed over, or the server stops) and the sweeper (stop).
    wake: Condvar,
    stopped: AtomicBool,
}

impl Shared {
    fn pending_cap(&self) -> usize {
        self.limits.max_unauthenticated.min(self.limits.max_connections)
    }

    /// Hands a socket whose answer was written to the closer, which discards its remaining input until the client
    /// closes, 256 KiB or 1 s (design note §5). When the closer already holds `max_lingering` sockets, the oldest is
    /// closed first. Never blocks.
    fn linger(&self, stream: TcpStream) {
        if self.stopped.load(Ordering::Acquire) || stream.set_nonblocking(true).is_err() {
            return;
        }
        let mut set = self.lingering.lock().unwrap_or_else(PoisonError::into_inner);
        while !set.is_empty() && set.len() >= self.limits.max_lingering.max(1) {
            set.pop_front();
        }
        set.push_back(Lingering { stream, deadline: Instant::now() + LINGER_TIME, discarded: 0 });
        drop(set);
        self.wake.notify_all();
    }

    /// The worker `id` leaves the pending pool; `false` when it was evicted (its socket is shut down).
    fn leave_pending(&self, id: u64) -> bool {
        let mut pool = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(i) = pool.iter().position(|p| p.id == id) else { return false };
        pool.swap_remove(i);
        true
    }
}

/// A worker's slot in `max_connections`, given back when its thread ends (also on a panic).
struct WorkerSlot(Arc<Shared>);

impl Drop for WorkerSlot {
    fn drop(&mut self) {
        self.0.workers.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Counts of what the server holds now (design note §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Connection workers (threads with a socket): at most `max_connections`.
    pub workers: usize,
    /// Workers whose request head is not complete yet: at most `min(max_unauthenticated, max_connections)`.
    pub pending: usize,
    /// Sockets being closed gracefully: at most `max_lingering`.
    pub lingering: usize,
    /// Sessions: at most 32.
    pub sessions: usize,
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
    let mut server_opts = opts.server.clone();
    "attached".clone_into(&mut server_opts.run_mode);
    // The body limit follows the session's inline payload limit, as the stdio framing does (11 §6.5).
    let max_body = limits.max_body.unwrap_or_else(|| {
        handle.agent_status("").map_or(50 << 20, |s| usize::try_from(s.limits.max_inline_bytes).unwrap_or(usize::MAX))
    });
    let shared = Arc::new(Shared {
        handle,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        token: opts.token,
        server_opts,
        limits,
        max_body,
        workers: AtomicUsize::new(0),
        pending: Mutex::new(Vec::new()),
        next_id: AtomicU64::new(0),
        lingering: Mutex::new(VecDeque::new()),
        wake: Condvar::new(),
        stopped: AtomicBool::new(false),
    });
    let accept = shared.clone();
    std::thread::Builder::new().name("mcp-http-accept".into()).spawn(move || {
        for conn in listener.incoming() {
            if accept.stopped.load(Ordering::Acquire) {
                break;
            }
            if let Ok(stream) = conn {
                admit(&accept, stream);
            }
        }
    })?;
    let closer = shared.clone();
    std::thread::Builder::new().name("mcp-http-close".into()).spawn(move || close_loop(&closer))?;
    let sweeper = shared.clone();
    std::thread::Builder::new().name("mcp-http-sweep".into()).spawn(move || {
        let mut next = Instant::now() + sweeper.limits.sweep;
        let mut gate = sweeper.lingering.lock().unwrap_or_else(PoisonError::into_inner);
        while !sweeper.stopped.load(Ordering::Acquire) {
            let now = Instant::now();
            if now < next {
                gate = sweeper.wake.wait_timeout(gate, next - now).unwrap_or_else(PoisonError::into_inner).0;
                continue;
            }
            drop(gate);
            expire(&sweeper.sessions, sweeper.limits.session_idle);
            next = Instant::now() + sweeper.limits.sweep;
            gate = sweeper.lingering.lock().unwrap_or_else(PoisonError::into_inner);
        }
    })?;
    Ok(HttpServer { addr, shared })
}

/// Decides about one accepted socket on the accept thread, without reading from it or waiting (design note §2-§5):
/// admitted into the pending pool with a worker thread, or refused `503` and handed to the closer. A full pending pool
/// also evicts the pending connection that has waited longest for its head.
fn admit(shared: &Arc<Shared>, stream: TcpStream) {
    let accepted = Instant::now();
    let mut pool = shared.pending.lock().unwrap_or_else(PoisonError::into_inner);
    if pool.len() >= shared.pending_cap() {
        if let Some(oldest) = (0..pool.len()).min_by_key(|&i| pool[i].since) {
            pool.swap_remove(oldest).evicted.store(true, Ordering::Release);
        }
        drop(pool);
        return refuse(shared, stream);
    }
    let taken = shared
        .workers
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| (n < shared.limits.max_connections).then_some(n + 1))
        .is_ok();
    if !taken {
        drop(pool);
        return refuse(shared, stream);
    }
    let slot = WorkerSlot(shared.clone());
    let id = shared.next_id.fetch_add(1, Ordering::Relaxed);
    let evicted = Arc::new(AtomicBool::new(false));
    pool.push(Pending { id, since: accepted, evicted: evicted.clone() });
    drop(pool);
    let _ = stream.set_write_timeout(Some(shared.limits.write_timeout));
    let worker = shared.clone();
    let mut stream = stream;
    let spawned = std::thread::Builder::new().name("mcp-http-conn".into()).spawn(move || {
        let _slot = slot;
        let read_all = std::cell::Cell::new(false);
        serve_connection(&mut stream, &worker, accepted, (id, &evicted), &read_all);
        if evicted.load(Ordering::Acquire) {
            // Evicted before its head was complete: nothing was answered; the socket closes now.
            return;
        }
        if read_all.get() {
            // The whole request was read: the answer may end with the server's FIN.
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
        worker.linger(stream);
    });
    if spawned.is_err() {
        // The closure (with the socket and the slot) was dropped: the slot is back; forget the pending entry.
        shared.leave_pending(id);
    }
}

/// `503` for a connection over a cap: one non-blocking write of the short answer (nothing is read, nothing waits), then
/// the closer takes the socket (design note §5).
fn refuse(shared: &Shared, mut stream: TcpStream) {
    if stream.set_nonblocking(true).is_ok() {
        let _ = stream.write_all(
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\nRetry-After: 0\r\n\r\n",
        );
        shared.linger(stream);
    }
}

/// Bytes discarded at most after the answer, so an early refusal (sent before the request body was read) reaches the
/// client: closing a socket with unread input makes the operating system reset the connection, and a reset can drop
/// the answer before the client reads it.
const LINGER_BYTES: usize = 256 << 10;
/// How long the discarding lasts at most.
const LINGER_TIME: Duration = Duration::from_secs(1);
/// How often the closer looks at its sockets while it holds any.
const LINGER_POLL: Duration = Duration::from_millis(2);

/// The closer thread: discards the remaining input of every lingering socket through one fixed buffer until the client
/// closes, [`LINGER_BYTES`] were discarded or [`LINGER_TIME`] passed; then the socket closes. When the whole request was
/// read the worker has already sent the server's FIN; after an early refusal the connection stays open for the client
/// to close first, so the refusal is not lost to a reset and a client that shuts its socket down after reading it finds
/// it still connected. Idle (no CPU) while it holds no socket.
fn close_loop(shared: &Shared) {
    let mut sink = [0u8; 4096];
    let mut set = shared.lingering.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if shared.stopped.load(Ordering::Acquire) {
            set.clear();
            return;
        }
        if set.is_empty() {
            set = shared.wake.wait(set).unwrap_or_else(PoisonError::into_inner);
            continue;
        }
        let now = Instant::now();
        set.retain_mut(|l| {
            if now >= l.deadline {
                return false;
            }
            loop {
                match l.stream.read(&mut sink) {
                    Ok(0) => return false,
                    Ok(n) => {
                        l.discarded += n;
                        if l.discarded >= LINGER_BYTES {
                            return false;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return true,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => return false,
                }
            }
        });
        if !set.is_empty() {
            set = shared.wake.wait_timeout(set, LINGER_POLL).unwrap_or_else(PoisonError::into_inner).0;
        }
    }
}

/// Ends sessions with no request in flight, idle for longer than `idle` (O-7; design note §6).
fn expire(sessions: &Mutex<HashMap<String, Session>>, idle: Duration) {
    let expired: Vec<Session> = {
        let mut map = sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let ids: Vec<String> = map
            .iter()
            .filter(|(_, s)| s.in_flight == 0 && s.last_seen.elapsed() >= idle)
            .map(|(k, _)| k.clone())
            .collect();
        ids.iter().filter_map(|k| map.remove(k)).collect()
    };
    drop(expired);
}

/// A request in flight on a session (design note §6): taken under the session-map lock when the request's head is
/// authenticated, released by `Drop` (every return path, a client gone, a panic). While any is held the session does
/// not expire; its release starts the idle time.
struct InFlight {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    sid: String,
}

impl Drop for InFlight {
    fn drop(&mut self) {
        if let Some(s) = self.sessions.lock().unwrap_or_else(PoisonError::into_inner).get_mut(&self.sid) {
            s.in_flight = s.in_flight.saturating_sub(1);
            s.last_seen = Instant::now();
        }
    }
}

/// A request's in-flight guard with its session's input and routes.
type Entered = (InFlight, Sender<Vec<u8>>, Arc<Mutex<Routes>>);

/// Takes a request's in-flight guard on session `sid`, with the session's input and routes; `None` for an unknown
/// session.
fn enter(shared: &Shared, sid: &str) -> Option<Entered> {
    let mut map = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner);
    let s = map.get_mut(sid)?;
    s.in_flight += 1;
    s.last_seen = Instant::now();
    let guard = InFlight { sessions: shared.sessions.clone(), sid: sid.to_owned() };
    Some((guard, s.input.clone(), s.routes.clone()))
}

/// Serves one connection; `read_all` tells the caller whether the whole request was read (then the answer may end with
/// the server's FIN; after an early refusal the client closes first, see [`close_loop`]).
fn serve_connection(
    stream: &mut TcpStream,
    shared: &Shared,
    accepted: Instant,
    (id, evicted): (u64, &AtomicBool),
    read_all: &std::cell::Cell<bool>,
) {
    let head = read_head(stream, accepted + shared.limits.head_deadline, evicted);
    if !shared.leave_pending(id) {
        // Evicted for a newcomer (design note §4): nothing to answer.
        return;
    }
    let (head, first) = match head {
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
    read_all.set(head.content_length == 0 && first.is_empty());
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
    let sid = head.header("mcp-session-id").map(str::to_owned);
    match head.method.as_str() {
        "POST" => post(stream, shared, &head, first, sid, accepted, read_all),
        "GET" => {
            let Some(sid) = sid else { return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id") };
            let (tx, rx) = mpsc::sync_channel::<String>(shared.limits.sse_backlog);
            // The guard and the stream's installation are one critical section: a `DELETE` either comes before (the
            // session is unknown) or after (it clears the routes and the stream ends).
            let _guard = {
                let mut map = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(s) = map.get_mut(&sid) else {
                    drop(map);
                    return json_error(stream, "404 Not Found", "unknown session");
                };
                s.in_flight += 1;
                s.last_seen = Instant::now();
                let mut r = s.routes.lock().unwrap_or_else(PoisonError::into_inner);
                for line in r.backlog.drain(..) {
                    let _ = offer(&tx, &line);
                }
                r.get = Some(tx);
                drop(r);
                InFlight { sessions: shared.sessions.clone(), sid: sid.clone() }
            };
            stream_sse(stream, &[("Mcp-Session-Id", sid)], &rx, None);
        }
        "DELETE" => {
            let Some(sid) = sid else { return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id") };
            let removed = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(&sid);
            let found = removed.is_some();
            // Dropping the session clears its routes: its open streams end now.
            drop(removed);
            respond(stream, if found { "200 OK" } else { "404 Not Found" }, &[], b"");
        }
        _ => respond(stream, "405 Method Not Allowed", &[("Allow", "GET, POST, DELETE".to_owned())], b""),
    }
}

fn post(
    stream: &mut TcpStream,
    shared: &Shared,
    head: &Head,
    first: Vec<u8>,
    sid: Option<String>,
    accepted: Instant,
    read_all: &std::cell::Cell<bool>,
) {
    if head.content_length > shared.max_body {
        return json_error(stream, "413 Content Too Large", "the request body is larger than the inline payload limit");
    }
    // A named session must exist before its body is read, and is in flight from here (C41-R2-03).
    let entered = match &sid {
        Some(s) => match enter(shared, s) {
            Some(e) => Some(e),
            None => return json_error(stream, "404 Not Found", "unknown session"),
        },
        None => None,
    };
    let deadline = accepted + shared.limits.request_deadline;
    let Some(body) = read_body(stream, first, head.content_length, deadline, &shared.stopped) else {
        if shared.stopped.load(Ordering::Acquire) {
            // Stopping: the worker ends now; its session's input sender goes with it (C41-R3-02).
            return;
        }
        return json_error(stream, "408 Request Timeout", "the request body did not arrive in time");
    };
    read_all.set(true);
    let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
        return json_error(stream, "400 Bad Request", "the body is not one JSON-RPC message");
    };
    if !msg.is_object() {
        return json_error(stream, "400 Bad Request", "batched JSON-RPC messages are not accepted");
    }
    let is_init = msg.get("method").and_then(Value::as_str) == Some("initialize");
    let (sid, (_guard, input, routes)) = match (sid, entered, is_init) {
        (Some(s), Some(e), _) => (s, e),
        (None, _, true) => match open_session(shared) {
            Ok(opened) => opened,
            Err((status, why)) => return json_error(stream, status, why),
        },
        _ => return json_error(stream, "400 Bad Request", "missing Mcp-Session-Id"),
    };
    let is_request = msg.get("method").is_some() && msg.get("id").is_some();
    let mut line = serde_json::to_vec(&msg).unwrap_or_default();
    line.push(b'\n');
    if !is_request {
        // Notifications and the client's responses (elicitation answers): accepted.
        let _ = input.send(line);
        drop(input);
        return respond(stream, "202 Accepted", &[("Mcp-Session-Id", sid)], b"");
    }
    let id = id_text(msg.get("id").unwrap_or(&Value::Null));
    let progress = msg.get("params").and_then(|p| p.get("_meta")).and_then(|m| m.get("progressToken")).map(id_text);
    let (tx, rx) = mpsc::sync_channel::<String>(shared.limits.sse_backlog);
    routes.lock().unwrap_or_else(PoisonError::into_inner).requests.push((id.clone(), progress, tx));
    let sent = input.send(line).is_ok();
    drop(input);
    if !sent {
        routes.lock().unwrap_or_else(PoisonError::into_inner).requests.retain(|(rid, _, _)| *rid != id);
        return json_error(stream, "410 Gone", "the session has ended");
    }
    stream_sse(stream, &[("Mcp-Session-Id", sid)], &rx, Some(&id));
    // A client that went away before the answer: forget its stream (a cancelled request gets no response, 11 §4).
    routes.lock().unwrap_or_else(PoisonError::into_inner).requests.retain(|(rid, _, _)| *rid != id);
}

/// Opens a session for `initialize`: the cap check, the insertion and the first request's in-flight guard are one
/// critical section (O-7; design note §6).
fn open_session(shared: &Shared) -> Result<(String, Entered), (&'static str, &'static str)> {
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
        let session = Session { input: input.clone(), routes: routes.clone(), last_seen: Instant::now(), in_flight: 1 };
        map.insert(sid.clone(), session);
    }
    let guard = InFlight { sessions: shared.sessions.clone(), sid: sid.clone() };
    let reader = ChannelReader { rx, buf: Vec::new(), pos: 0 };
    let writer = RouterWriter { routes: routes.clone(), pending: Vec::new() };
    let (h, o, map, id2) = (shared.handle.clone(), shared.server_opts.clone(), shared.sessions.clone(), sid.clone());
    let spawned = std::thread::Builder::new().name("mcp-http-session".into()).spawn(move || {
        let _ = crate::serve(h, o, reader, writer);
        let ended = map.lock().unwrap_or_else(PoisonError::into_inner).remove(&id2);
        drop(ended);
    });
    if spawned.is_err() {
        let removed = shared.sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(&sid);
        drop(removed);
        return Err(("500 Internal Server Error", "cannot start the session"));
    }
    Ok((sid, (guard, input, routes)))
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
