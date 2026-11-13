//! Independent request-envelope routing oracle, including response shape on the second client.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod support;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
fn drain(c: &mut support::Client, wanted: &[u64]) -> BTreeMap<u64, Value> {
    let end = Instant::now() + Duration::from_secs(60);
    let mut quiet = None;
    let mut messages = BTreeMap::new();
    loop {
        if quiet.is_none() && wanted.iter().all(|id| messages.contains_key(id)) {
            quiet = Some(Instant::now() + Duration::from_millis(700));
        }
        let until = quiet.unwrap_or(end);
        let Some(m) = c.next(until.saturating_duration_since(Instant::now())) else { break };
        assert_ne!(m["method"], "notifications/progress", "synchronous evaluate: {m}");
        if let Some(id) = m["id"].as_u64() {
            assert!(messages.insert(id, m).is_none(), "duplicate response id {id}");
        }
        assert!(Instant::now() < end, "delivery deadline");
    }
    assert_eq!(messages.keys().copied().collect::<Vec<_>>(), wanted);
    messages
}
fn send(c: &mut support::Client, command: &str, params: Value) -> u64 {
    c.send(
        "tools/call",
        json!({"name":"run_command","arguments":{"id":command,"params":params},"_meta":{"progressToken":"closure"}}),
    )
}
fn env(v: &Value) -> &Value {
    &v["result"]["structuredContent"]
}
#[test]
fn replies_are_unique_owned_typed_and_cancelled_requests_remain_silent() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut opts = support::opts(&dir);
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut a, _) = support::Client::connect(&h, opts.clone(), json!({}));
    let (mut b, _) = support::Client::connect(&h, opts, json!({}));
    let node = "scene:\"Intro\"/element:\"Bob\"";
    let frames: Vec<_> = (0..100_000).map(|i| 1.0 + f64::from(i) * 0.5).collect();
    let long = send(&mut a, "anim.evaluate", json!({"nodeIds":[node],"frames":frames,"include":["world"]}));
    let refused = send(&mut a, "anim.evaluate", json!({"nodeIds":vec![node;11],"frames":frames}));
    let read = send(&mut a, "anim.camera.list", json!({}));
    let other =
        send(&mut b, "anim.function.sample", json!({"nodeId":node,"channel":"rotation","from":1,"to":1,"step":1}));
    let got = drain(&mut a, &[long, refused, read]);
    assert_eq!(env(&got[&long])["status"], "ok");
    assert_eq!(env(&got[&long])["result"]["frames"].as_array().unwrap().len(), 100_000);
    assert_eq!(env(&got[&refused])["error"]["code"], "limit.exceeded");
    assert!(env(&got[&read])["result"]["cameras"].is_array());
    let got = drain(&mut b, &[other]);
    assert_eq!(env(&got[&other])["status"], "ok");
    assert_eq!(env(&got[&other])["result"]["samples"].as_array().unwrap().len(), 1);
    let cancelled = send(&mut a, "anim.evaluate", json!({"nodeIds":[node],"frames":frames,"include":["world"]}));
    std::thread::sleep(Duration::from_millis(50));
    a.notify("notifications/cancelled", json!({"requestId":cancelled}));
    let after = send(&mut a, "anim.camera.list", json!({}));
    let got = drain(&mut a, &[after]);
    assert!(env(&got[&after])["result"]["cameras"].is_array());
    a.close().unwrap();
    b.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
