//! M2 C2.1 obligation 6: the C1.4 replay contract with real drawing edits. An agent session drives drawing commands
//! through the MCP server — a single edit, an atomic multi-edit batch, undo/redo, checkpoint restore, a partially
//! failing sequential batch, a cached `requestId` retry, a dry run, discovery tools, a committed batch whose file
//! publication fails, and atomic groups with matching and mismatching `expectRev` — then exports its activity as a
//! batch script. Replaying that script through actual script mode into the same initial document reproduces every
//! canonical entry byte for byte, revisions included, and the same undo/redo history.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::{Value, json};
use support::*;
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const BOB: &str = "scene:#1/element:\"Bob\"";

fn autonomous(dir: &std::path::Path) -> tt_mcp::ServerOptions {
    let mut o = opts(dir);
    o.policy = Some(tt_executor::agents::Policy::Autonomous);
    o
}

fn canonical(h: &ExecutorHandle, created: &[String]) -> Vec<String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys()
        .iter()
        .map(|k| {
            let mut v = d.entry_raw_json(k).unwrap();
            // A drawing created by a command records the wall clock as its creation time (10 §4.8); that one member is
            // session time, everything else must replay exactly.
            if v.get("type").and_then(Value::as_str) == Some("drawing")
                && v.get("id").and_then(Value::as_str).is_some_and(|i| created.iter().any(|c| c == i))
            {
                v["createdAt"] = json!("<creation time>");
            }
            tt_model::canonical::to_canonical_string(&v).unwrap()
        })
        .collect()
}

/// The undo history without times.
fn history(h: &ExecutorHandle) -> Value {
    let mut v = h.invoke(tt_executor::Invocation::user("edit.history.get", json!({ "limit": 1000 }))).result.unwrap();
    for e in v["entries"].as_array_mut().unwrap() {
        e.as_object_mut().unwrap().remove("time");
    }
    v
}

#[test]
fn drawing_edits_replay_byte_for_byte_through_script_mode() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, autonomous(&dir), json!({}));
    let ok = |c: &mut Client, id: &str, params: Value| {
        let e = c.run(id, params.clone());
        assert_eq!(e["status"], "ok", "{id} {params}: {e}");
        e
    };
    // 1. A single drawing edit.
    ok(
        &mut c,
        "draw.stroke.add",
        json!({ "drawingId": D1, "points": [{"x": 0, "y": 0, "pressure": 0.4}, {"x": 30, "y": 10, "pressure": 1}, {"x": 60, "y": 0, "pressure": 0.3}] }),
    );
    // 2. An atomic multi-edit transaction (one revision, one undo step).
    let atomic = c.call("batch", json!({ "label": "Ink and caption", "atomic": true, "steps": [
        { "id": "draw.stroke.add", "params": { "drawingId": D1, "kind": "pencil", "points": [{"x": -10, "y": -10}, {"x": 10, "y": 10}] } },
        { "id": "draw.text.add", "params": { "drawingId": D1, "position": {"x": 0, "y": 50}, "text": "Replay" } },
        { "id": "draw.stroke.set", "params": { "drawingId": D1, "strokeId": "$1.result.strokeIds.0", "thickness": 4 } },
    ] }));
    assert_eq!(atomic["structuredContent"]["status"], "ok", "{atomic}");
    // 3. Undo and redo.
    ok(&mut c, "edit.undo", json!({}));
    ok(&mut c, "edit.redo", json!({}));
    // 4. Checkpoint restore.
    ok(&mut c, "checkpoint.create", json!({ "name": "before-pivot" }));
    ok(&mut c, "draw.pivot.set", json!({ "drawingId": D1, "point": {"x": 9, "y": 9} }));
    ok(&mut c, "checkpoint.restore", json!({ "name": "before-pivot" }));
    // 5. A sequential batch that fails after committing its first step.
    let partial = c.call(
        "batch",
        json!({ "label": "partial", "atomic": false, "stopOnError": true, "steps": [
        { "id": "draw.stroke.set", "params": { "drawingId": D1, "strokeId": PENCIL, "thickness": 7 } },
        { "id": "draw.stroke.set", "params": { "drawingId": D1, "strokeId": PENCIL, "bezier": [{"x": 0, "y": 0}] } },
        { "id": "draw.pivot.set", "params": { "drawingId": D1, "point": {"x": 1, "y": 1} } },
    ] }),
    );
    assert_eq!(partial["structuredContent"]["status"], "error", "{partial}");
    // 6. A cached requestId retry (executed once; replay leaves the retry out).
    let first = c.call("run_command", json!({ "id": "draw.stroke.add", "requestId": "rq-1", "params": { "drawingId": D1, "kind": "invisible", "points": [{"x": 0, "y": 200}, {"x": 50, "y": 200}], "fit": false } }));
    let again = c.call("run_command", json!({ "id": "draw.stroke.add", "requestId": "rq-1", "params": { "drawingId": D1, "kind": "invisible", "points": [{"x": 0, "y": 200}, {"x": 50, "y": 200}], "fit": false } }));
    assert_eq!(first["structuredContent"], again["structuredContent"], "the retry returns the first envelope");
    // 7. A dry run and 8. discovery (both left out of the replay).
    let dry = c.call("run_command", json!({ "id": "draw.stroke.add", "dryRun": true, "params": { "drawingId": D1, "points": [{"x": 0, "y": 0}, {"x": 1, "y": 1}] } }));
    assert_eq!(dry["structuredContent"]["status"], "ok");
    c.call("list_commands", json!({ "namespace": "draw" }));
    c.call("describe_command", json!({ "id": "draw.stroke.add" }));
    // 9. A committed batch whose file cannot be published (a directory blocks the target).
    let blocked = dir.join("blocked.json");
    std::fs::create_dir(&blocked).unwrap();
    let publish = c.call("batch", json!({ "label": "export after drawing", "atomic": true, "steps": [
        { "id": "draw.stroke.add", "params": { "drawingId": D1, "kind": "pencil", "points": [{"x": 70, "y": 0}, {"x": 90, "y": 20}] } },
        { "id": "doc.export.json", "params": { "scope": "scenes", "refs": ["scene:#1"], "path": path(&blocked), "onExists": "overwrite" } },
    ] }));
    assert_eq!(publish["structuredContent"]["error"]["details"]["reason"], "publish", "{publish}");
    // 10. Atomic groups guarded by expectRev: a matching guard commits, a mismatching one changes nothing.
    let drev = h.snapshot().unwrap().unwrap().locate(D1).unwrap().rev;
    let guarded = c.call(
        "batch",
        json!({ "label": "guarded", "atomic": true, "expectRev": { D1: drev }, "steps": [
        { "id": "draw.pivot.set", "params": { "drawingId": D1, "point": {"x": 2, "y": 3} } },
        { "id": "draw.stroke.set", "params": { "drawingId": D1, "strokeId": PENCIL, "thickness": 3 } },
    ] }),
    );
    assert_eq!(guarded["structuredContent"]["status"], "ok", "{guarded}");
    let stale = c.call(
        "batch",
        json!({ "label": "stale", "atomic": true, "expectRev": { D1: drev }, "steps": [
        { "id": "draw.pivot.set", "params": { "drawingId": D1, "point": {"x": 4, "y": 5} } },
    ] }),
    );
    assert_eq!(stale["structuredContent"]["error"]["code"], "rev.conflict", "{stale}");
    // 11. Text edits and a drawing created in an empty cell.
    let created = ok(
        &mut c,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 7, "kind": "brush", "points": [{"x": 0, "y": 0}, {"x": 20, "y": 5}] }),
    );
    let created_id = created["result"]["drawingId"].as_str().unwrap().to_owned();
    let text = ok(
        &mut c,
        "draw.text.add",
        json!({ "drawingId": created_id, "position": {"x": 0, "y": 0}, "text": "Cell seven" }),
    );
    ok(
        &mut c,
        "draw.textFormat.set",
        json!({ "drawingId": created_id, "textId": text["result"]["textId"], "start": 0, "length": 4, "bold": true }),
    );
    let target = dir.join("replay.json");
    let exported = c.run("agent.activity.export", json!({ "path": path(&target), "format": "batch" }));
    assert_eq!(exported["status"], "ok", "{exported}");
    let omitted: Vec<Value> =
        exported["result"]["omitted"].as_array().unwrap().iter().map(|o| o["reason"].clone()).collect();
    for reason in ["cachedRetry", "dryRun", "notACommand", "notCompleted"] {
        assert!(omitted.iter().any(|r| r == reason), "{reason} in {omitted:?}");
    }
    // The exported script keeps atomic batches as groups, the guarded one with its expectRev.
    let script: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    let groups: Vec<&Value> = script["steps"].as_array().unwrap().iter().filter(|s| s["id"] == "batch").collect();
    assert!(groups.iter().any(|g| g["params"]["label"] == "Ink and caption"));
    assert!(groups.iter().any(|g| g["params"]["expectRev"][D1] == drev), "{script}");
    std::fs::remove_dir(&blocked).unwrap();
    let fresh = executor(&dir.join("fresh-user"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    let outcome = tt_mcp::script::run(&fresh, &autonomous(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert!(outcome.ok, "{:?}", outcome.envelope);
    let created = vec![created_id];
    assert_eq!(canonical(&fresh, &created), canonical(&h, &created), "replay reproduces every canonical entry");
    assert_eq!(fresh.snapshot().unwrap().unwrap().document_rev(), h.snapshot().unwrap().unwrap().document_rev());
    let (a, b) = (history(&h), history(&fresh));
    assert_eq!(
        a["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["label"].clone(), e["documentRev"].clone()))
            .collect::<Vec<_>>(),
        b["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["label"].clone(), e["documentRev"].clone()))
            .collect::<Vec<_>>(),
        "the same undo steps at the same revisions"
    );
    c.close().unwrap();
    drop(h);
    drop(fresh);
    let _ = std::fs::remove_dir_all(&dir);
}
