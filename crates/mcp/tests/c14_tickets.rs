//! C14-R1-16 sub-items ticketed (round-1 response): each test states the requirement and fails today; it is ignored
//! with its reason and owner, and runs with `--ignored`. The other sub-items (`get_state` = `app.state.get`,
//! `doc.undone` / `doc.redone`) are fixed and tested in `protocol.rs` and the reviewer's suite.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::json;
use support::*;

/// 11 §8: "The envelope's `result.effectiveParams` lists the values actually used".
#[test]
#[ignore = "C14-R1-16: result.effectiveParams changes the result contract of every command (719 result schemas, the \
            generated IPC types) and its defaults include the GUI's last-used values, which arrive with the M4 \
            dialogs; owner: builder, M4 C4.1 (traceability row 11 §8)"]
fn every_result_lists_its_effective_params() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let env = c.run("project.create", json!({ "name": "E", "path": path(&dir.join("E.animpkg")) }));
    assert!(env["result"]["effectiveParams"].is_object(), "project.create: {env}");
    let found = c.run("doc.object.find", json!({ "type": "scene" }));
    assert_eq!(found["result"]["effectiveParams"]["limit"], 100, "doc.object.find: {found}");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// 11 §13: "`describe_command` includes at least one example request and its result; docs resources include a worked
/// end-to-end example per namespace".
#[test]
#[ignore = "C14-R1-16: executed example results need each namespace's commands, most of which arrive with later \
            milestones (M2-M9); the kernel namespaces (project, doc, edit, checkpoint, job, agent, app) get recorded \
            worked examples with the M2 agent-workflow follow-up; owner: builder, before M2 C2.1 (traceability row \
            11 §13)"]
fn namespace_docs_carry_a_worked_example_with_executed_results() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let doc = c.request("resources/read", json!({ "uri": "app://docs/doc" }));
    let text = doc["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("## Worked example"), "no worked end-to-end example in the doc namespace");
    let d = c.call("describe_command", json!({ "id": "doc.object.get" }));
    assert!(d["structuredContent"]["result"]["examples"][0].get("result").is_some(), "no executed example result");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
