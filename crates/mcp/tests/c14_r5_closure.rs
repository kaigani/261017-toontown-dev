//! Final scoped C14-R4-01 reviewer probes.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod support;
use serde_json::{Value, json};
use support::*;
use tt_executor::{ExecutorHandle, Invocation};
fn options(dir: &std::path::Path) -> tt_mcp::ServerOptions {
    let mut o = opts(dir);
    o.policy = Some(tt_executor::agents::Policy::Autonomous);
    o
}
fn canonical(h: &ExecutorHandle) -> Vec<String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys()
        .iter()
        .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
        .collect()
}
fn rev(h: &ExecutorHandle) -> u64 {
    let d = h.snapshot().unwrap().unwrap();
    let k = d.entry_keys().into_iter().find(|k| matches!(k, tt_model::document::EntryKey::Scene(_))).unwrap();
    d.entry_raw_json(&k).unwrap()["rev"].as_u64().unwrap()
}
fn edit(n: u64) -> Value {
    json!({"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"value":n}}}})
}
fn run(h: &ExecutorHandle, dir: &std::path::Path, script: Value) -> Value {
    let p = dir.join("probe.json");
    std::fs::write(&p, serde_json::to_vec(&script).unwrap()).unwrap();
    serde_json::to_value(
        tt_mcp::script::run(h, &options(dir), tt_mcp::script::load(&p).unwrap(), None).unwrap().envelope,
    )
    .unwrap()
}
#[test]
fn guarded_group_has_one_revision_and_one_undo() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Group"}))).is_ok());
    let before = canonical(&h);
    let r = rev(&h);
    let dr = h.snapshot().unwrap().unwrap().document_rev();
    let out = run(
        &h,
        &dir,
        json!({"label":"outer","atomic":false,"steps":[{"id":"batch","params":{"label":"guarded","expectRev":{"scene:#1":r},"steps":[edit(1),edit(2)]}}]}),
    );
    assert_eq!(out["status"], "ok", "{out}");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), dr + 1);
    assert_eq!(rev(&h), r + 1);
    assert!(h.invoke(Invocation::user("edit.undo", json!({}))).is_ok());
    // Undo advances revisions; compare semantic content and the untouched entry set instead.
    let undone = h.snapshot().unwrap().unwrap();
    let k = undone.entry_keys().into_iter().find(|k| matches!(k, tt_model::document::EntryKey::Scene(_))).unwrap();
    assert!(undone.entry_raw_json(&k).unwrap()["meta"].get("value").is_none());
    assert!(h.invoke(Invocation::user("edit.redo", json!({}))).is_ok());
    assert_ne!(canonical(&h), before);
    assert_eq!(h.snapshot().unwrap().unwrap().entry_raw_json(&k).unwrap()["meta"]["value"], 2);
}
#[test]
fn sequential_step_guard_still_matches_then_conflicts() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Sequential"}))).is_ok());
    let r = rev(&h);
    let mut step = edit(1);
    step["params"]["expectRev"] = json!({"scene:#1":r});
    let script = json!({"label":"seq","atomic":false,"steps":[step]});
    assert_eq!(run(&h, &dir, script.clone())["status"], "ok");
    let before = canonical(&h);
    let out = run(&h, &dir, script);
    assert_eq!(out["error"]["details"]["error"]["code"], "rev.conflict", "{out}");
    assert_eq!(canonical(&h), before);
}
#[test]
fn exported_guard_replays_exactly_and_refuses_changed_revision() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Export"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let r = rev(&h);
    let (mut c, _) = Client::connect(&h, options(&dir), json!({}));
    assert_eq!(
        c.call("batch", json!({"label":"guarded","expectRev":{"scene:#1":r},"steps":[edit(1),edit(2)]}))["structuredContent"]
            ["status"],
        "ok"
    );
    let p = dir.join("export.json");
    assert_eq!(c.run("agent.activity.export", json!({"path":path(&p),"format":"batch"}))["status"], "ok");
    let script: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    assert_eq!(script["steps"][0]["params"]["expectRev"], json!({"scene:#1":r}));
    let fresh = executor(&dir.join("fresh"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    assert_eq!(run(&fresh, &dir, script.clone())["status"], "ok");
    assert_eq!(canonical(&fresh), canonical(&h));
    let before = canonical(&fresh);
    let out = run(&fresh, &dir, script);
    assert_eq!(out["error"]["details"]["error"]["code"], "rev.conflict", "{out}");
    assert_eq!(canonical(&fresh), before);
    c.close().unwrap();
}
#[test]
fn wrong_type_group_guard_is_invalid_and_executes_no_steps() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Invalid"}))).is_ok());
    let before = canonical(&h);
    for guard in [json!("wrong"), json!({"scene:#1":"wrong"}), json!({"scene:#1":true}), json!({"scene:#1":-1})] {
        let out = run(
            &h,
            &dir,
            json!({"label":"outer","atomic":false,"steps":[{"id":"batch","params":{"label":"bad","expectRev":guard,"steps":[edit(1),edit(2)]}}]}),
        );
        assert_eq!(out["error"]["details"]["error"]["code"], "arg.invalid", "{out}");
        assert_eq!(canonical(&h), before);
    }
}
