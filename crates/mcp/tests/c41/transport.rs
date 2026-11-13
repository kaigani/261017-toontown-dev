//! Admission, refusal and session lifetime of the HTTP transport: one test per invariant of
//! `reviews/M4-design-http-admission.md` (ruling O-15; C41-R2-01 to -03). Builder-owned.

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use tt_mcp::http::{HttpServer, Limits, Stats};

use super::{BEARER, initialize, limited};

/// Bound on waiting for an asynchronous state (far above a local step).
const BOUND: Duration = Duration::from_secs(10);

/// Waits until `pred` holds for the server's counts; fails after [`BOUND`].
fn until(srv: &HttpServer, what: &str, pred: impl Fn(Stats) -> bool) -> Stats {
    let started = Instant::now();
    loop {
        let s = srv.stats();
        if pred(s) {
            return s;
        }
        assert!(started.elapsed() < BOUND, "{what}: {s:?}");
        std::thread::yield_now();
    }
}

fn silent(srv: &HttpServer) -> TcpStream {
    TcpStream::connect(srv.addr).unwrap()
}

/// The status line of an answer to `request` on a fresh socket (the socket is kept open by the caller when wanted).
fn send(srv: &HttpServer, request: &str) -> (u16, TcpStream) {
    let mut s = TcpStream::connect(srv.addr).unwrap();
    s.set_read_timeout(Some(BOUND)).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut line = String::new();
    BufReader::new(s.try_clone().unwrap()).read_line(&mut line).unwrap();
    (line.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0), s)
}

fn delete_absent() -> String {
    format!("DELETE /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: {BEARER}\r\nMcp-Session-Id: absent\r\n\r\n")
}

/// Whether the server closed `s` (read sees the end or a reset) rather than keeping it open.
fn closed_by_server(s: &mut TcpStream) -> bool {
    s.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
    let mut b = [0u8; 64];
    loop {
        match s.read(&mut b) {
            Ok(0) => return true,
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return false,
            Err(_) => return true,
        }
    }
}

/// Opens a GET stream on `sid` and returns the socket after the answer head arrived.
fn get_stream(srv: &HttpServer, sid: &str) -> BufReader<TcpStream> {
    let mut s = TcpStream::connect(srv.addr).unwrap();
    s.set_read_timeout(Some(BOUND)).unwrap();
    write!(s, "GET /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: {BEARER}\r\nMcp-Session-Id: {sid}\r\n\r\n")
        .unwrap();
    let mut r = BufReader::new(s);
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    assert!(line.starts_with("HTTP/1.1 200"), "{line}");
    while line != "\r\n" {
        line.clear();
        r.read_line(&mut line).unwrap();
    }
    r
}

/// I-1: under idle and refused bursts the server never holds more workers, pending connections or lingering sockets
/// than its caps (threads `3 + C + S`, sockets `2 + C + L`), and gives everything back afterwards.
#[test]
fn admission_bounds_hold_under_idle_and_refused_bursts() {
    let srv = limited(Limits { max_connections: 4, max_unauthenticated: 2, max_lingering: 8, ..Default::default() });
    let mut held = Vec::new();
    for i in 0..40 {
        if i % 2 == 0 {
            held.push(silent(&srv));
        } else {
            let mut s = TcpStream::connect(srv.addr).unwrap();
            s.write_all(b"DELETE /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            held.push(s);
        }
        let s = srv.stats();
        assert!(s.workers <= 4 && s.pending <= 2 && s.lingering <= 8 && s.sessions <= 32, "{s:?}");
    }
    let s = until(&srv, "the burst settles within the caps", |s| s.workers <= 4);
    assert!(s.pending <= 2 && s.lingering <= 8, "{s:?}");
    drop(held);
    until(&srv, "everything is given back", |s| s.workers == 0 && s.pending == 0 && s.lingering == 0);
}

/// I-2: a newcomer that meets a full pending pool is refused `503` and the connection that has waited longest for its
/// head is evicted, not a younger one; the younger one is still served.
#[test]
fn a_full_pending_pool_evicts_the_connection_waiting_longest() {
    let srv = limited(Limits { max_unauthenticated: 2, ..Default::default() });
    let mut oldest = silent(&srv);
    until(&srv, "the first silent connection is pending", |s| s.pending == 1);
    let mut younger = silent(&srv);
    until(&srv, "both are pending", |s| s.pending == 2);
    let (status, _newcomer) = send(&srv, &delete_absent());
    assert_eq!(status, 503);
    assert!(closed_by_server(&mut oldest), "the oldest pending connection is evicted");
    assert!(!closed_by_server(&mut younger), "the younger one keeps its slot");
    younger.set_read_timeout(Some(BOUND)).unwrap();
    younger.write_all(delete_absent().as_bytes()).unwrap();
    let mut line = String::new();
    BufReader::new(younger).read_line(&mut line).unwrap();
    assert!(line.starts_with("HTTP/1.1 404"), "{line}");
}

/// I-3: a token holder that meets a pool full of idle sockets is served on its retry, however often the idle sockets
/// are replenished.
#[test]
fn a_token_holder_is_served_on_retry_while_idle_sockets_are_replenished() {
    let srv = limited(Limits { max_unauthenticated: 2, ..Default::default() });
    let mut idle = Vec::new();
    for round in 0..5 {
        // Replenished: the previous round's idle sockets go, two new ones fill the pool.
        idle.clear();
        until(&srv, "the previous idle sockets are gone", |s| s.pending == 0);
        idle.push(silent(&srv));
        until(&srv, "the first idle socket is pending", |s| s.pending == 1);
        idle.push(silent(&srv));
        until(&srv, "the pool is full of idle sockets", |s| s.pending == 2);
        let mut attempts = 0;
        loop {
            attempts += 1;
            let (status, _s) = send(&srv, &delete_absent());
            if status == 404 {
                break;
            }
            assert_eq!(status, 503, "round {round}");
            assert!(attempts < 2, "round {round}: the retry after a refusal finds room");
        }
    }
}

/// I-4: refusals create no worker (C full of an authenticated stream: no eviction, every newcomer `503`), and the
/// lingering sockets stay within L, the oldest closed first.
#[test]
fn refusals_create_no_worker_and_lingering_is_capped() {
    let srv = limited(Limits { max_connections: 1, max_lingering: 4, ..Default::default() });
    let sid = initialize(&srv);
    until(&srv, "the initialize exchanges are done", |s| s.workers == 0);
    let mut stream = get_stream(&srv, &sid);
    until(&srv, "the stream holds the only worker", |s| s.workers == 1 && s.pending == 0);
    let mut refused = Vec::new();
    for _ in 0..12 {
        let (status, s) = send(&srv, &delete_absent());
        assert_eq!(status, 503);
        refused.push(s);
        let st = srv.stats();
        assert_eq!(st.workers, 1, "a refusal never creates a worker: {st:?}");
        assert!(st.lingering <= 4, "{st:?}");
    }
    // The oldest refused sockets were closed when the cap was reached; the newest still linger.
    assert!(closed_by_server(&mut refused[0]));
    assert!(!closed_by_server(&mut refused[11]));
    // The authenticated stream was never evicted.
    assert!(!closed_by_server(stream.get_mut()));
}

/// I-5: a session with a request in flight is not expired, a stream or a body arriving slowly alike; its idle time
/// starts when the request ends.
#[test]
fn a_streaming_request_keeps_its_session_and_idle_time_starts_at_its_end() {
    let idle = Duration::from_millis(300);
    let srv = limited(Limits { session_idle: idle, sweep: Duration::from_millis(10), ..Default::default() });
    let sid = initialize(&srv);
    let stream = get_stream(&srv, &sid);
    let began = Instant::now();
    while began.elapsed() < idle * 3 {
        assert_eq!(srv.session_count(), 1, "a session with an open stream does not expire");
        std::thread::yield_now();
    }
    drop(stream);
    // A body that arrives slowly holds the session too; its idle time starts at the answer.
    let body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let mut s = TcpStream::connect(srv.addr).unwrap();
    s.set_read_timeout(Some(BOUND)).unwrap();
    write!(
        s,
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: {BEARER}\r\nMcp-Session-Id: {sid}\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        &body[..1]
    )
    .unwrap();
    let began = Instant::now();
    while began.elapsed() < idle * 3 {
        assert_eq!(srv.session_count(), 1, "a session receiving a body does not expire");
        std::thread::yield_now();
    }
    s.write_all(&body.as_bytes()[1..]).unwrap();
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).unwrap();
    assert!(line.starts_with("HTTP/1.1 202"), "{line}");
    let answered = Instant::now();
    until(&srv, "the session expires once idle", |s| s.sessions == 0);
    assert!(answered.elapsed() >= idle - Duration::from_millis(20), "idle time starts at the answer");
}

/// I-6: a client that goes away mid-body releases its guard: the session then expires.
#[test]
fn a_client_that_leaves_mid_body_releases_the_guard() {
    let srv = limited(Limits {
        session_idle: Duration::from_millis(200),
        sweep: Duration::from_millis(10),
        ..Default::default()
    });
    let sid = initialize(&srv);
    let mut s = TcpStream::connect(srv.addr).unwrap();
    write!(
        s,
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: {BEARER}\r\nMcp-Session-Id: {sid}\r\nContent-Length: 100\r\n\r\n{{"
    )
    .unwrap();
    drop(s);
    until(&srv, "the session expires after its client left", |s| s.sessions == 0 && s.workers == 0);
}

/// I-7: `DELETE` and server stop end a session's open streams at once.
#[test]
fn delete_and_stop_end_open_streams() {
    let srv = limited(Limits::default());
    let sid = initialize(&srv);
    let mut stream = get_stream(&srv, &sid);
    let (status, _s) = send(
        &srv,
        &format!("DELETE /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: {BEARER}\r\nMcp-Session-Id: {sid}\r\n\r\n"),
    );
    assert_eq!(status, 200);
    let began = Instant::now();
    let mut rest = Vec::new();
    let _ = stream.read_to_end(&mut rest);
    assert!(began.elapsed() < Duration::from_secs(5), "DELETE ends the session's stream");
    let sid = initialize(&srv);
    let mut stream = get_stream(&srv, &sid);
    srv.stop();
    let began = Instant::now();
    let _ = stream.read_to_end(&mut rest);
    assert!(began.elapsed() < Duration::from_secs(5), "stop ends the session's stream");
    until(&srv, "stop gives everything back", |s| s.workers == 0 && s.sessions == 0 && s.lingering == 0);
}
