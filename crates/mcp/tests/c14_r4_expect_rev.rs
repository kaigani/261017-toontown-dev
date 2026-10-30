//! C14-R4-01 (C14-R3-02) builder closure: `expectRev` guards one transaction (ADR-0015 §13). The batch tool and the
//! atomic group step share it: a matching precondition runs the steps as one transaction, a mismatch refuses the whole
//! batch with `rev.conflict` and runs nothing; the activity export carries the precondition into the replay script; a
//! sequential batch refuses it.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]
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

fn edit(n: u64) -> Value {
    json!({"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"value":n}}}})
}

fn scene_rev(h: &ExecutorHandle) -> u64 {
    let d = h.snapshot().unwrap().unwrap();
    let scene = d.entry_keys().into_iter().find(|k| matches!(k, tt_model::document::EntryKey::Scene(_))).unwrap();
    d.entry_raw_json(&scene).unwrap()["rev"].as_u64().unwrap()
}

/// The batch tool: a stale `expectRev` refuses the atomic batch with `rev.conflict` (not `batch.stepFailed`) and runs
/// no step; the current one runs it as one transaction. The export writes the precondition into the group, and the
/// script replays into the same initial document byte for byte.
#[test]
fn the_batch_tool_and_its_replayed_group_honour_expect_rev() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Rev"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let before = canonical(&h);
    let rev = scene_rev(&h);
    let (mut c, _) = Client::connect(&h, options(&dir), json!({}));

    let stale = c.call("batch", json!({"label":"stale","expectRev":{"scene:#1": rev + 7},"steps":[edit(1),edit(2)]}));
    let env = &stale["structuredContent"];
    assert_eq!(env["status"], "error", "{stale}");
    assert_eq!(env["error"]["code"], "rev.conflict", "{stale}");
    assert_eq!(env["error"]["details"]["currentRev"], rev);
    assert_eq!(env["steps"], json!([]), "no step runs");
    assert_eq!(canonical(&h), before, "nothing changed");

    let ok = c.call("batch", json!({"label":"current","expectRev":{"scene:#1": rev},"steps":[edit(1),edit(2)]}));
    assert_eq!(ok["structuredContent"]["status"], "ok", "{ok}");
    assert_eq!(ok["structuredContent"]["steps"].as_array().unwrap().len(), 2);

    let target = dir.join("replay.json");
    assert_eq!(c.run("agent.activity.export", json!({"path":path(&target),"format":"batch"}))["status"], "ok");
    let script: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    let groups: Vec<&Value> = script["steps"].as_array().unwrap().iter().filter(|s| s["id"] == "batch").collect();
    assert_eq!(groups.len(), 1, "only the committed batch replays: {script}");
    assert_eq!(groups[0]["params"]["label"], "current");
    assert_eq!(groups[0]["params"]["expectRev"], json!({"scene:#1": rev}), "{script}");

    let fresh = executor(&dir.join("fresh"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    let out = tt_mcp::script::run(&fresh, &options(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert!(out.ok, "{:?}", out.envelope);
    assert_eq!(canonical(&fresh), canonical(&h));
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// In a script, a group whose precondition no longer holds (an earlier step changed the object) is refused whole;
/// the steps before it stay committed and, with `stopOnError`, nothing after it runs.
#[test]
fn a_group_after_an_edit_to_its_guarded_object_is_refused_whole() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Rev"}))).is_ok());
    let rev = scene_rev(&h);
    let target = dir.join("script.json");
    let script = json!({"label":"outer","atomic":false,"steps":[
        edit(1),
        {"id":"batch","params":{"label":"group","expectRev":{"scene:#1": rev},"steps":[edit(2),edit(3)]}},
        edit(4)
    ]});
    std::fs::write(&target, serde_json::to_vec(&script).unwrap()).unwrap();
    let out = tt_mcp::script::run(&h, &options(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert!(!out.ok);
    let env = serde_json::to_value(out.envelope).unwrap();
    assert_eq!(env["error"]["code"], "batch.stepFailed", "{env}");
    assert_eq!(env["error"]["details"]["stepIndex"], 2, "{env}");
    assert_eq!(env["error"]["details"]["error"]["code"], "rev.conflict", "{env}");
    let steps = env["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2, "stopOnError: the step after the group does not run");
    assert_eq!(steps[0]["status"], "ok");
    let d = h.snapshot().unwrap().unwrap();
    let scene = d.entry_keys().into_iter().find(|k| matches!(k, tt_model::document::EntryKey::Scene(_))).unwrap();
    assert_eq!(d.entry_raw_json(&scene).unwrap()["meta"]["value"], 1, "only the first edit committed");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `expectRev` on a sequential batch is refused with a typed error before anything runs: it would guard only the
/// first of several commits.
#[test]
fn a_sequential_batch_refuses_expect_rev() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Rev"}))).is_ok());
    let before = canonical(&h);
    let rev = scene_rev(&h);
    let (mut c, _) = Client::connect(&h, options(&dir), json!({}));
    let r = c.call("batch", json!({"label":"seq","atomic":false,"expectRev":{"scene:#1": rev},"steps":[edit(1)]}));
    assert_eq!(r["structuredContent"]["error"]["code"], "arg.invalid", "{r}");
    assert_eq!(r["structuredContent"]["error"]["details"]["pointer"], "/expectRev", "{r}");
    assert_eq!(canonical(&h), before);
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
