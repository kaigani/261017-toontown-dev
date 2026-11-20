//! Ruling D-68 item 4 (the roots race, since M3): a call that needs the file roots and arrives before the client has
//! answered the server's `roots/list` request waits for the answer, on the MCP call's thread, bounded by the call's
//! budget and at most 2 s. Answered in time, the call succeeds; never answered, it is refused at the bound with
//! `fs.outsideRoots`, as before.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]
mod support;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::*;

/// The client's root (`other`) as a `file://` URI.
fn uri(dir: &std::path::Path) -> String {
    format!("file:///{}", path(dir).replace('\\', "/").trim_start_matches('/'))
}

/// Initializes a client with the roots capability, then sends `initialized` and, at once, a call whose path lies in
/// the client's root only (not in the launch roots): the call reaches the server before the roots answer can.
fn race(c: &mut Client, target: &std::path::Path) -> u64 {
    c.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": { "roots": {} }, "clientInfo": { "name": "t", "version": "1" } }),
    );
    c.notify("notifications/initialized", json!({}));
    c.send(
        "tools/call",
        json!({ "name": "run_command", "arguments": { "id": "project.create", "params": { "name": "R", "path": path(target) } } }),
    )
}

#[test]
fn a_call_that_needs_the_roots_waits_for_a_late_answer_and_succeeds() {
    let dir = tmp();
    let other = tmp();
    let h = executor(&dir.join("user"));
    let mut c = Client::start(&h, opts(&dir));
    let u = uri(&other);
    // The client answers the roots request late: 600 ms, well inside the 2 s bound.
    c.respond_with(move |m, _| {
        if m == "roots/list" {
            std::thread::sleep(Duration::from_millis(600));
            json!({ "roots": [{ "uri": u, "name": "other" }] })
        } else {
            json!({})
        }
    });
    let id = race(&mut c, &other.join("R.animpkg"));
    let m = c.wait(id, Duration::from_secs(30)).expect("the call is answered");
    let env = &m["result"]["structuredContent"];
    assert_eq!(env["status"], "ok", "the call waited for the roots: {env}");
    assert!(other.join("R.animpkg").exists());
    assert!(c.asked.iter().any(|(m, _)| m == "roots/list"));
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn a_call_that_needs_the_roots_is_refused_at_the_bound_when_the_client_never_answers() {
    let dir = tmp();
    let other = tmp();
    let h = executor(&dir.join("user"));
    let mut c = Client::start(&h, opts(&dir));
    // The client never answers the roots request.
    c.respond_with(|m, _| if m == "roots/list" { Value::Null } else { json!({}) });
    let started = Instant::now();
    let id = race(&mut c, &other.join("R.animpkg"));
    let m = c.wait(id, Duration::from_secs(30)).expect("the call is answered at the bound");
    let waited = started.elapsed();
    let env = &m["result"]["structuredContent"];
    assert_eq!(env["error"]["code"], "fs.outsideRoots", "{env}");
    assert!(!other.join("R.animpkg").exists());
    // It waited for the roots (the bound is 2 s with the test executor's budget), and no longer than a bounded time.
    assert!(waited >= Duration::from_millis(1900), "waited {waited:?}");
    assert!(waited < Duration::from_secs(20), "waited {waited:?}");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn a_call_that_needs_no_roots_does_not_wait() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let mut c = Client::start(&h, opts(&dir));
    c.respond_with(|m, _| if m == "roots/list" { Value::Null } else { json!({}) });
    c.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": { "roots": {} }, "clientInfo": { "name": "t", "version": "1" } }),
    );
    c.notify("notifications/initialized", json!({}));
    let started = Instant::now();
    let id = c.send("tools/call", json!({ "name": "get_state", "arguments": {} }));
    let m = c.wait(id, Duration::from_secs(30)).expect("answered");
    assert!(m.get("error").is_none(), "{m}");
    // A read carries no path: it is answered without waiting for the roots (well under the 2 s bound).
    assert!(started.elapsed() < Duration::from_millis(1500), "{:?}", started.elapsed());
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
