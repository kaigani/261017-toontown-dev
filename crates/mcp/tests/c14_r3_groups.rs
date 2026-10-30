//! C14-R3-02 builder closure: atomic group steps (ADR-0015 §13). Replay keeps transaction and undo boundaries through
//! undo and redo; a group nested in an atomic batch or a group, or a group that is not atomic, is refused with a typed
//! error before anything runs.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]
mod support;
use serde_json::{Value, json};
use support::*;
use tt_executor::{BatchRequest, ExecutorHandle, Invocation, Origin};

fn autonomous_opts(dir: &std::path::Path) -> tt_mcp::ServerOptions {
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

fn two_edits() -> Value {
    json!({"label":"two edits", "atomic":true, "steps":[
        {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"first":1}}}},
        {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"second":2}}}}
    ]})
}

/// Undo then redo after an atomic batch: the exported script reproduces the document byte for byte, and the batch
/// is exported as one atomic group step.
#[test]
fn undo_then_redo_after_an_atomic_batch_replays_the_same_document() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Replay"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, autonomous_opts(&dir), json!({}));
    let e = c.call("batch", two_edits());
    assert_eq!(e["structuredContent"]["status"], "ok", "{e}");
    assert_eq!(c.run("edit.undo", json!({}))["status"], "ok");
    assert_eq!(c.run("edit.redo", json!({}))["status"], "ok");
    let target = dir.join("replay.json");
    assert_eq!(c.run("agent.activity.export", json!({"path":path(&target),"format":"batch"}))["status"], "ok");
    let script: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    let groups: Vec<&Value> = script["steps"].as_array().unwrap().iter().filter(|s| s["id"] == "batch").collect();
    assert_eq!(groups.len(), 1, "{script}");
    assert_eq!(groups[0]["params"]["atomic"], true);
    assert_eq!(groups[0]["params"]["label"], "two edits");
    assert_eq!(groups[0]["params"]["steps"].as_array().unwrap().len(), 2);
    let fresh = executor(&dir.join("fresh-user"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    let outcome = tt_mcp::script::run(&fresh, &opts(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert!(outcome.ok, "{:?}", outcome.envelope);
    assert_eq!(canonical(&fresh), canonical(&h));
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A group is one transaction and one undo step, exactly like `batch {atomic: true}`: the same revisions, and after
/// one undo the same document; two separate edits followed by one undo give a different document.
#[test]
fn a_group_is_one_undo_step_like_an_atomic_batch() {
    let dir = tmp();
    let origin = executor(&dir.join("origin"));
    assert!(origin.invoke(Invocation::user("project.create", json!({"name":"Replay"}))).is_ok());
    let initial = origin.snapshot().unwrap().unwrap();
    // Each variant starts from the same saved document; the result is (revision, after, after undo, after redo).
    let run = |name: &str, f: &dyn Fn(&ExecutorHandle) -> Option<u64>| {
        let h = executor(&dir.join(name));
        assert!(h.open_document(initial.clone(), None, false).is_ok());
        let rev = f(&h);
        let after = canonical(&h);
        assert!(h.invoke(Invocation::user("edit.undo", json!({}))).is_ok());
        let undone = canonical(&h);
        assert!(h.invoke(Invocation::user("edit.redo", json!({}))).is_ok());
        (rev, after, undone, canonical(&h))
    };
    let batch = |h: &ExecutorHandle, req: Value| {
        let env = h.batch(serde_json::from_value::<BatchRequest>(req).unwrap(), Origin::User);
        assert!(env.is_ok(), "{env:?}");
        env.document_rev
    };
    let group = run("group", &|h| {
        batch(h, json!({"label": "outer", "atomic": false, "steps": [{"id": "batch", "params": two_edits()}]}))
    });
    let atomic = run("atomic", &|h| batch(h, two_edits()));
    let separate = run("separate", &|h| {
        let steps = two_edits()["steps"].clone();
        let mut rev = None;
        for s in steps.as_array().unwrap() {
            let env = h.invoke(Invocation::user(s["id"].as_str().unwrap(), s["params"].clone()));
            assert!(env.is_ok(), "{env:?}");
            rev = env.document_rev;
        }
        rev
    });
    assert_eq!(group, atomic, "a group and an atomic batch: same revisions and documents through undo and redo");
    assert_ne!(group.0, separate.0, "two separate edits take two revisions, a group one");
    assert_ne!(group.2, separate.2, "one undo removes the whole group but only the last of two separate edits");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Nested groups and non-atomic groups are refused with typed errors and change nothing.
#[test]
fn nested_and_non_atomic_groups_are_refused() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Groups"}))).is_ok());
    let before = canonical(&h);
    let edit = json!({"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"x":1}}}});
    let cases = [
        // A group inside an atomic batch.
        (
            json!({"label": "outer", "atomic": true, "steps": [edit, {"id": "batch", "params": two_edits()}]}),
            "batch.nestedGroup",
        ),
        // A group inside a group (the outer group is atomic).
        (
            json!({"label": "outer", "atomic": false, "steps": [{"id": "batch", "params": {"label": "inner", "steps": [edit, {"id": "batch", "params": two_edits()}]}}]}),
            "batch.nestedGroup",
        ),
        // A group that is not atomic.
        (
            json!({"label": "outer", "atomic": false, "steps": [{"id": "batch", "params": {"label": "inner", "atomic": false, "steps": [edit]}}]}),
            "arg.invalid",
        ),
        // A group whose params are not a batch.
        (json!({"label": "outer", "atomic": false, "steps": [{"id": "batch", "params": {"steps": 3}}]}), "arg.invalid"),
    ];
    for (req, code) in cases {
        let r: BatchRequest = serde_json::from_value(req.clone()).unwrap();
        let env = h.batch(r, Origin::User);
        let json = serde_json::to_value(&env).unwrap();
        let error = if json["error"]["code"] == "batch.stepFailed" {
            &json["error"]["details"]["error"]
        } else {
            &json["error"]
        };
        assert_eq!(error["code"], code, "{req} -> {json}");
        assert_eq!(canonical(&h), before, "{req}: nothing changed");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
