//! Scoped round-four closure probes.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod support;
use serde_json::{Value, json};
use support::*;
use tt_executor::{ExecutorHandle, Invocation};
fn canonical(h: &ExecutorHandle) -> Vec<String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys()
        .iter()
        .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
        .collect()
}
fn edit(n: u64) -> Value {
    json!({"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"value":n}}}})
}
fn options(dir: &std::path::Path) -> tt_mcp::ServerOptions {
    let mut o = opts(dir);
    o.policy = Some(tt_executor::agents::Policy::Autonomous);
    o
}
#[test]
fn sequential_last_publication_failure_after_two_edits() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Replay"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, options(&dir), json!({}));
    let blocked = dir.join("blocked.json");
    std::fs::create_dir(&blocked).unwrap();
    let r=c.call("batch",json!({"label":"two then publish","atomic":false,"steps":[edit(1),edit(2),{"id":"doc.export.json","params":{"scope":"scenes","refs":["scene:#1"],"path":path(&blocked),"onExists":"overwrite"}}]}));
    assert_eq!(r["structuredContent"]["status"], "error");
    let log = c.run("agent.activity.list", json!({"limit":100}));
    let batch = log["result"].as_array().unwrap().iter().find(|e| e["commandId"] == "batch").unwrap();
    assert_eq!(batch["status"], "error");
    assert_eq!(batch["committedSteps"], json!([0, 1, 2]));
    let target = dir.join("script.json");
    assert_eq!(c.run("agent.activity.export", json!({"path":path(&target),"format":"batch"}))["status"], "ok");
    std::fs::remove_dir(&blocked).unwrap();
    let fresh = executor(&dir.join("fresh"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    assert!(tt_mcp::script::run(&fresh, &options(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap().ok);
    assert_eq!(canonical(&h), canonical(&fresh));
    c.close().unwrap();
}
#[test]
fn two_atomic_batches_and_sequential_batch_keep_history() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Replay"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, options(&dir), json!({}));
    for (n, atomic) in [(1, true), (3, true), (5, false)] {
        assert_eq!(
            c.call("batch", json!({"label":format!("batch {n}"),"atomic":atomic,"steps":[edit(n),edit(n+1)]}))["structuredContent"]
                ["status"],
            "ok"
        );
    }
    let target = dir.join("script.json");
    assert_eq!(c.run("agent.activity.export", json!({"path":path(&target),"format":"batch"}))["status"], "ok");
    let script: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    assert_eq!(script["steps"].as_array().unwrap().iter().filter(|s| s["id"] == "batch").count(), 2);
    let fresh = executor(&dir.join("fresh"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    assert!(tt_mcp::script::run(&fresh, &options(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap().ok);
    assert_eq!(canonical(&h), canonical(&fresh));
    for _ in 0..4 {
        assert!(h.invoke(Invocation::user("edit.undo", json!({}))).is_ok());
        assert!(fresh.invoke(Invocation::user("edit.undo", json!({}))).is_ok());
        assert_eq!(canonical(&h), canonical(&fresh));
    }
    for _ in 0..4 {
        assert!(h.invoke(Invocation::user("edit.redo", json!({}))).is_ok());
        assert!(fresh.invoke(Invocation::user("edit.redo", json!({}))).is_ok());
        assert_eq!(canonical(&h), canonical(&fresh));
    }
    c.close().unwrap();
}
#[test]
fn group_expect_rev_is_supported_and_conflicts_atomically() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Group"}))).is_ok());
    let before = canonical(&h);
    let target = dir.join("group.json");
    std::fs::write(&target,serde_json::to_vec(&json!({"label":"outer","atomic":false,"steps":[{"id":"batch","params":{"label":"group","expectRev":{"scene:#1":999},"steps":[edit(1),edit(2)]}}]})).unwrap()).unwrap();
    let out = tt_mcp::script::run(&h, &options(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert_eq!(canonical(&h), before);
    let env = serde_json::to_value(out.envelope).unwrap();
    assert_eq!(env["error"]["details"]["error"]["code"], "rev.conflict", "{env}");
}
#[test]
fn group_accepts_a_matching_expect_rev() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Group"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let scene = initial.entry_keys().into_iter().find(|k| matches!(k, tt_model::document::EntryKey::Scene(_))).unwrap();
    let rev = initial.entry_raw_json(&scene).unwrap()["rev"].as_u64().unwrap();
    let target = dir.join("group-matching.json");
    std::fs::write(&target, serde_json::to_vec(&json!({"label":"outer","atomic":false,"steps":[{"id":"batch","params":{"label":"group","expectRev":{"scene:#1":rev},"steps":[edit(1),edit(2)]}}]})).unwrap()).unwrap();
    let out = tt_mcp::script::run(&h, &options(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert!(out.ok, "a matching expectRev must run: {:?}", out.envelope);
}
