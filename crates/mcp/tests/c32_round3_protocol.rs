//! M3 C3.2 round 3 (builder; C32-R3-01 c): request-owned delivery of `anim.evaluate` through MCP. Each request's result
//! or error is answered to that request id only (JSON-RPC; 11 §4); `anim.evaluate` is a synchronous command, so it
//! sends no `notifications/progress` (progress belongs to jobs, `job_wait`, 11 §7); a request the client cancels gets no
//! answer while the other requests keep theirs (11 §4: no response to a cancelled request; a document-thread call is
//! bounded by the executor's budget). A second client sees none of the first client's answers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const WAIT: Duration = Duration::from_secs(60);

fn evaluate(c: &mut support::Client, params: Value, token: &str) -> u64 {
    c.send(
        "tools/call",
        json!({ "name": "run_command", "arguments": { "id": "anim.evaluate", "params": params },
                "_meta": { "progressToken": token } }),
    )
}

fn envelope(m: &Value) -> Value {
    m["result"]["structuredContent"].clone()
}

/// Every message the client receives for `d` after its expected answers arrived (or `WAIT` passed): the answers by
/// request id (unlike `Client::wait`, no response is dropped on the way) and the notifications.
fn collect(c: &mut support::Client, ids: &[u64], d: Duration) -> (BTreeMap<u64, Value>, Vec<Value>) {
    let deadline = Instant::now() + WAIT;
    let mut answers = BTreeMap::new();
    let mut notes = Vec::new();
    let mut quiet_until: Option<Instant> = None;
    loop {
        if quiet_until.is_none() && ids.iter().all(|i| answers.contains_key(i)) {
            quiet_until = Some(Instant::now() + d);
        }
        let until = quiet_until.unwrap_or(deadline);
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let Some(m) = c.next(left) else { break };
        if m.get("method").is_some() {
            notes.push(m);
        } else if let Some(id) = m.get("id").and_then(Value::as_u64) {
            assert!(answers.insert(id, m).is_none(), "one answer per request");
        }
    }
    (answers, notes)
}

fn run_request(id: &str) -> Value {
    json!({ "name": "run_command", "arguments": { "id": id, "params": {} } })
}

#[test]
fn long_and_refused_evaluations_answer_their_own_requests_only() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut opts = support::opts(&dir);
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut a, _) = support::Client::connect(&h, opts.clone(), json!({}));
    let (mut b, _) = support::Client::connect(&h, opts, json!({}));
    let long: Vec<f64> = (0..100_000).map(|i| 1.0 + f64::from(i) * 0.5).collect();
    let oversized: Vec<f64> = (0..100_000).map(|i| 1.0 + f64::from(i)).collect();
    // Pipelined: a long range, a refused one and a read, before any answer; the other client reads meanwhile.
    let id_long = evaluate(&mut a, json!({ "nodeIds": [BOB], "frames": long, "include": ["world"] }), "p-long");
    let id_refused = evaluate(&mut a, json!({ "nodeIds": vec![BOB; 11], "frames": oversized }), "p-refused");
    let id_read = a.send("tools/call", run_request("anim.camera.list"));
    let b_read = b.send("tools/call", run_request("doc.state.get"));
    let (got, notes) = collect(&mut a, &[id_long, id_refused, id_read], Duration::from_millis(300));
    assert_eq!(got.keys().copied().collect::<Vec<_>>(), vec![id_long, id_refused, id_read], "exactly these answers");
    // Each answer is its own request's.
    let env = envelope(&got[&id_long]);
    assert_eq!(env["status"], "ok", "{env}");
    assert_eq!(env["result"]["frames"].as_array().unwrap().len(), 100_000);
    let env = envelope(&got[&id_refused]);
    assert_eq!(
        (env["status"].as_str(), env["error"]["code"].as_str()),
        (Some("error"), Some("limit.exceeded")),
        "{env}"
    );
    let env = envelope(&got[&id_read]);
    assert_eq!(env["status"], "ok");
    assert!(env["result"]["cameras"].is_array(), "{env}");
    // No progress for synchronous commands, under either token.
    assert!(!notes.iter().any(|n| n["method"] == "notifications/progress"), "{notes:?}");
    // The other client got its read and nothing of client A's.
    let (got_b, notes_b) = collect(&mut b, &[b_read], Duration::from_millis(300));
    assert_eq!(got_b.keys().copied().collect::<Vec<_>>(), vec![b_read], "client B gets only its own answer");
    assert_eq!(envelope(&got_b[&b_read])["status"], "ok");
    assert!(!notes_b.iter().any(|n| n["method"] == "notifications/progress"));
    // A cancelled long evaluation gets no answer; the next request gets its own. The document thread runs calls in
    // order, so the cancelled evaluation has finished by the time the next read answers.
    let frames: Vec<f64> = (0..100_000).map(|i| 1.0 + f64::from(i) * 0.25).collect();
    let id_cancelled =
        evaluate(&mut a, json!({ "nodeIds": [BOB], "frames": frames, "include": ["world"] }), "p-cancel");
    std::thread::sleep(Duration::from_millis(50));
    a.notify("notifications/cancelled", json!({ "requestId": id_cancelled, "reason": "test" }));
    let id_after = a.send("tools/call", run_request("anim.camera.list"));
    let (got, notes) = collect(&mut a, &[id_after], Duration::from_millis(500));
    assert_eq!(got.keys().copied().collect::<Vec<_>>(), vec![id_after], "a cancelled request gets no answer: {got:?}");
    assert_eq!(envelope(&got[&id_after])["status"], "ok");
    assert!(!notes.iter().any(|n| n["method"] == "notifications/progress"));
    a.close().unwrap();
    b.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
