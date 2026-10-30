//! C1.3 obligation 7 through the real MCP path: the guarded load (scopes and the unsaved-work rule of the load
//! transaction), the frozen gate of newer-minor entries, atomic batches with a deferred save (one committed revision,
//! one commit record, one audit entry, the file holds that revision), and staged integrity (a batch that fails after
//! its save step writes nothing).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use support::*;
use tt_executor::agents::Policy;

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

fn wait_job(c: &mut Client, env: &Value) -> Value {
    let job = env["result"]["jobId"].as_str().unwrap_or_else(|| panic!("not a job: {env}")).to_owned();
    let r = c.call("job_wait", json!({ "jobId": job, "timeoutMs": 60000 }))["structuredContent"].clone();
    let rec = r["result"].get("record").cloned().unwrap_or_else(|| r["result"].clone());
    assert!(rec["state"].is_string(), "{r}");
    rec
}

fn doc_changed_since(c: &mut Client, seq: u64) -> (usize, u64) {
    let r = c.request("resources/read", json!({ "uri": format!("app://events?after={seq}") }));
    let body: Value = serde_json::from_str(r["contents"][0]["text"].as_str().unwrap()).unwrap();
    let n = body["events"].as_array().unwrap().iter().filter(|e| e["type"] == "doc.changed").count();
    (n, body["lastSeq"].as_u64().unwrap())
}

#[test]
fn guarded_load_and_frozen_gate_through_mcp() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    // A folder project of format 1.3 whose element uses a loop type this version does not know.
    let folder = dir.join("Newer");
    {
        let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
        let env = c.run(
            "project.create",
            json!({ "name": "Newer", "path": format!("{}{}", path(&folder), std::path::MAIN_SEPARATOR) }),
        );
        assert_eq!(env["status"], "ok", "{env}");
        assert_eq!(c.run("project.close", json!({}))["status"], "ok");
        c.close().unwrap();
    }
    let mut m: Value = serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    m["formatVersion"] = json!("1.3");
    std::fs::write(folder.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    let el_file = walk(&folder).into_iter().find(|p| p.components().any(|c| c.as_os_str() == "elements")).unwrap();
    let mut el: Value = serde_json::from_slice(&std::fs::read(&el_file).unwrap()).unwrap();
    el["loopType"] = json!("pingPong");
    std::fs::write(&el_file, serde_json::to_vec_pretty(&el).unwrap()).unwrap();
    let eid = el["id"].as_str().unwrap().to_owned();
    // Observe has no files.read: the guarded load refuses before reading anything.
    let mut o = opts(&dir);
    o.policy = Some(Policy::Observe);
    let (mut c, _) = Client::connect(&h, o, json!({}));
    assert_eq!(c.run("project.open", json!({ "path": path(&folder) }))["error"]["code"], "permission.denied");
    c.close().unwrap();
    // Assist opens it (a job); the frozen entry refuses edits, dry runs included; other objects stay editable.
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let started = c.run("project.open", json!({ "path": path(&folder) }));
    assert_eq!(wait_job(&mut c, &started)["state"], "succeeded");
    for dry in [false, true] {
        let r = c.call("set_properties", json!({ "targets": [eid], "props": { "meta": { "x": 1 } }, "dryRun": dry }));
        assert_eq!(r["structuredContent"]["error"]["code"], "doc.unknownValue", "dry {dry}: {r}");
    }
    let scene = c.call("find_objects", json!({ "type": "scene", "limit": 1 }))["structuredContent"]["result"]["items"]
        [0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "e": 1 } } }))["structuredContent"]["status"],
        "ok"
    );
    // The load transaction's unsaved-work rule: replacing a modified project fails, and discarding is not an agent's.
    let again = c.run("project.open", json!({ "path": path(&folder) }));
    assert_eq!(again["error"]["code"], "project.unsavedChanges", "{again}");
    let discard = c.run("project.open", json!({ "path": path(&folder), "onUnsaved": "discard" }));
    assert_eq!(discard["error"]["code"], "permission.denied", "{discard}");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn atomic_batch_with_a_deferred_save_and_staged_integrity_through_mcp() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let pkg = dir.join("Deferred.animpkg");
    let env = c.run("project.create", json!({ "name": "Deferred", "path": path(&pkg) }));
    let scene = env["result"]["sceneId"].as_str().unwrap().to_owned();
    let (_, seq) = doc_changed_since(&mut c, 0);
    let audit_before = c.request("resources/read", json!({ "uri": "app://activity" }))["contents"][0]["text"]
        .as_str()
        .unwrap()
        .lines()
        .count();
    // Edit and save in one atomic batch: one revision, one commit record, one audit entry; the file holds it.
    let r = c.call(
        "batch",
        json!({ "label": "Edit and save", "steps": [
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "saved": 1 } } } },
            { "id": "project.close", "params": { "onUnsaved": "save" } },
        ] }),
    )["structuredContent"]
        .clone();
    assert_eq!(r["status"], "ok", "{r}");
    let committed = r["documentRev"].as_u64().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let (commits, seq2) = doc_changed_since(&mut c, seq);
    assert_eq!(commits, 1, "one commit record per transaction");
    let audit = c.request("resources/read", json!({ "uri": "app://activity" }))["contents"][0]["text"]
        .as_str()
        .unwrap()
        .lines()
        .count();
    assert_eq!(audit - audit_before, 1, "one audit entry per transaction");
    let saved = tt_persist::load(&pkg, &Default::default()).unwrap();
    assert_eq!(saved.doc.document_rev(), committed, "the file holds the committed revision");
    // A batch that fails after its save step: rolled back, and the save (a deferred effect) never happens.
    let reopened = c.run("project.open", json!({ "path": path(&pkg) }));
    assert_eq!(wait_job(&mut c, &reopened)["state"], "succeeded");
    let (_, seq2) = doc_changed_since(&mut c, seq2);
    let before = std::fs::read(&pkg).unwrap();
    let failed = c.call(
        "batch",
        json!({ "label": "Fails after saving", "steps": [
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "saved": 2 } } } },
            { "id": "project.close", "params": { "onUnsaved": "save" } },
            { "id": "doc.object.patch", "params": { "targets": ["el_missing"], "props": {} } },
        ] }),
    )["structuredContent"]
        .clone();
    assert_eq!(failed["status"], "error", "{failed}");
    assert_eq!(std::fs::read(&pkg).unwrap(), before, "staged integrity: nothing written");
    let (commits, _) = doc_changed_since(&mut c, seq2);
    assert_eq!(commits, 0, "no commit record for a rolled-back batch");
    let state = c.call("get_state", json!({ "include": ["project"] }))["structuredContent"]["result"].clone();
    assert_eq!(state["project"]["name"], "Deferred", "the close was rolled back too: {state}");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
