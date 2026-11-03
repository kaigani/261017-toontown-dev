//! M2 C2.4: the common mutation contract (as `c23_contracts.rs`) for every new C2.4 drawing mutation —
//! `draw.vectorize.run`, a job that commits each image as its own step. Dry run, exact ID sets, one revision and one
//! event per image, undo/redo, save/open, locked drawings, a stale `expectRev`, and a failing step (an atomic batch
//! refuses the job; a failing later image keeps the images before it).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions};

const BOB: &str = "scene:#1/element:\"Bob\"";

fn corpus(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("vectorize")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn job(h: &ExecutorHandle, id: &str, params: Value, options: InvokeOptions) -> Value {
    let started = with(h, id, params, options);
    assert!(started.is_ok(), "{started:?}");
    let r = ok(h, "job.wait", json!({ "jobId": started.result.unwrap()["jobId"], "timeoutMs": 60_000 }));
    if r.get("record").is_some() { r["record"].clone() } else { r }
}

/// Everything a vectorization may change: drawings (with content), palettes, Bob's bank and exposure.
fn readback(h: &ExecutorHandle) -> Value {
    let doc = h.snapshot().unwrap().unwrap();
    let bob = doc.elements().find(|e| e.name() == "Bob").unwrap();
    let bob = serde_json::to_value(bob).unwrap();
    let v = json!({
        "drawings": doc.drawings().map(|d| serde_json::to_value(d).unwrap()).collect::<Vec<_>>(),
        "palettes": doc.palettes().map(|p| serde_json::to_value(p).unwrap()).collect::<Vec<_>>(),
        "bob": bob,
        "elements": doc.elements().count(),
    });
    // Revision stamps move with every revision that touches an object, undo and redo included (10 REV-01): the
    // contract compares content.
    without_revs(v)
}

fn without_revs(v: Value) -> Value {
    match v {
        Value::Object(m) => {
            Value::Object(m.into_iter().filter(|(k, _)| k != "rev").map(|(k, x)| (k, without_revs(x))).collect())
        }
        Value::Array(a) => Value::Array(a.into_iter().map(without_revs).collect()),
        x => x,
    }
}

fn ids(v: &Value, key: &str) -> BTreeSet<String> {
    v[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().or_else(|| x["colourId"].as_str()).unwrap_or_default().to_owned())
        .collect()
}

fn prepared() -> ExecutorHandle {
    let h = fixture();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{"op": "replace", "path": "/exposure", "value": [[null, 24]]}] }),
    );
    h
}

#[test]
fn vectorize_run_keeps_the_common_contract() {
    let h = prepared();
    let params = json!({ "files": [corpus("e2-lineart.png")], "targetElementId": BOB, "startFrame": 2 });
    let (start, before) = (rev(&h), readback(&h));
    // Dry run: nothing starts, nothing changes.
    let dry = with(&h, "draw.vectorize.run", params.clone(), InvokeOptions { dry_run: true, ..Default::default() });
    assert_eq!(dry.result.unwrap()["jobId"], "dryRun");
    assert_eq!((rev(&h), readback(&h)), (start, before.clone()));
    // Stale expectRev on the target element: refused at submission.
    let mut stale = InvokeOptions::default();
    stale.expect_rev.insert(BOB.to_owned(), 0);
    let refused = with(&h, "draw.vectorize.run", params.clone(), stale);
    assert_eq!(refused.error.map(|e| e.code), Some("rev.conflict".to_owned()));
    assert_eq!((rev(&h), readback(&h)), (start, before.clone()));
    // A job cannot run inside an atomic batch: the whole batch is refused, its other step included.
    let failed = batch(
        &h,
        json!({ "label": "contract", "steps": [
        {"id": "draw.drawing.rename", "params": {"drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW", "name": "renamed"}},
        {"id": "draw.vectorize.run", "params": params.clone()}] }),
    );
    assert_eq!(failed.error.map(|e| e.code), Some("batch.jobNotAllowed".to_owned()));
    assert_eq!((rev(&h), readback(&h)), (start, before.clone()));
    // One image: one revision, one doc.changed and one drawing event, exactly the new objects.
    let rx = h.subscribe();
    let rec = job(&h, "draw.vectorize.run", params.clone(), InvokeOptions::default());
    assert_eq!(rec["state"], "succeeded", "{rec}");
    assert_eq!(rev(&h), start + 1);
    let events: Vec<_> = rx.try_iter().collect();
    assert_eq!(events.iter().filter(|e| e.name == "doc.changed").count(), 1, "one revision event");
    let drawing = rec["result"]["drawingIds"][0].as_str().unwrap().to_owned();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.name == "draw.drawing.changed" && e.payload["drawingId"] == drawing.as_str())
            .count(),
        1
    );
    let after = readback(&h);
    let new_drawings: BTreeSet<String> =
        ids(&after, "drawings").difference(&ids(&before, "drawings")).cloned().collect();
    assert_eq!(new_drawings, BTreeSet::from([drawing.clone()]), "exactly the reported drawing");
    let new_palettes: Vec<&Value> =
        after["palettes"].as_array().unwrap().iter().filter(|p| p["name"] == "Vectorization").collect();
    assert_eq!(new_palettes.len(), 1);
    assert_eq!(after["palettes"].as_array().unwrap().len(), before["palettes"].as_array().unwrap().len() + 1);
    assert_eq!(after["elements"], before["elements"], "no element created for an existing target");
    // Undo and redo.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(readback(&h), before, "undo restores everything");
    ok(&h, "edit.redo", json!({}));
    assert_eq!(readback(&h), after, "redo re-applies it");
    // Save and open.
    let dir = std::env::temp_dir().join(format!("tt-c24-contract-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let package = dir.join("C.animpkg");
    let saved = job(
        &h,
        "project.saveAs",
        json!({ "path": package.to_string_lossy(), "onExists": "overwrite" }),
        InvokeOptions::default(),
    );
    assert_eq!(saved["state"], "succeeded", "{saved}");
    let h2 = fixture();
    let opened = job(
        &h2,
        "project.open",
        json!({ "path": package.to_string_lossy(), "onUnsaved": "discard" }),
        InvokeOptions::default(),
    );
    assert_eq!(opened["state"], "succeeded", "{opened}");
    // What the vectorization created reads back the same: the drawing and the generated palette.
    let reopened = readback(&h2);
    let pick =
        |v: &Value, key: &str, id: &dyn Fn(&Value) -> bool| v[key].as_array().unwrap().iter().find(|x| id(x)).cloned();
    let is_new = |x: &Value| x["id"] == drawing.as_str();
    // Saving writes each drawing's colour-recovery copies (03 §11.1); everything else reads back unchanged.
    let content = |d: Option<Value>| {
        let mut d = d.unwrap();
        d.as_object_mut().unwrap().remove("colourRecovery");
        d
    };
    assert_eq!(content(pick(&reopened, "drawings", &is_new)), content(pick(&after, "drawings", &is_new)));
    let is_pal = |x: &Value| x["name"] == "Vectorization";
    assert!(pick(&after, "palettes", &is_pal).is_some());
    assert_eq!(pick(&reopened, "palettes", &is_pal), pick(&after, "palettes", &is_pal));
    drop(h2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn vectorize_never_edits_a_locked_drawing_and_a_failing_image_keeps_the_ones_before() {
    let h = prepared();
    let first = job(
        &h,
        "draw.vectorize.run",
        json!({ "files": [corpus("e2-lineart.png")], "targetElementId": BOB, "startFrame": 1 }),
        InvokeOptions::default(),
    );
    let old = first["result"]["drawingIds"][0].as_str().unwrap().to_owned();
    ok(&h, "draw.lock.set", json!({ "drawingId": old, "locked": true }));
    let locked_before = ok(&h, "draw.drawing.get", json!({ "drawingId": old }));
    // Replacing the locked drawing's cell puts a new drawing there; the locked one is untouched.
    let replaced = job(
        &h,
        "draw.vectorize.run",
        json!({ "files": [corpus("e2-lineart.png")], "targetElementId": BOB, "startFrame": 1, "onConflict": "replace" }),
        InvokeOptions::default(),
    );
    assert_eq!(replaced["state"], "succeeded", "{replaced}");
    assert_ne!(replaced["result"]["drawingIds"][0], old.as_str());
    assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": old })), locked_before);
    // A damaged second file fails the job after the first image committed; the first stays (each image is its own
    // step, as with cancellation).
    let v = rev(&h);
    let rec = job(
        &h,
        "draw.vectorize.run",
        json!({ "files": [corpus("e2-lineart.png"), corpus("truncated.png")], "targetElementId": BOB, "startFrame": 10 }),
        InvokeOptions::default(),
    );
    assert_eq!(rec["state"], "failed", "{rec}");
    assert_eq!(rec["error"]["code"], "vectorize.unsupportedFile");
    assert_eq!(rev(&h), v + 1, "the first image stays");
}
