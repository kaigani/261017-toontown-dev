//! Measures the C1.2 commit path at the size 10 §5 says the model MUST handle — one drawing layer with 100 000
//! strokes — and prints one JSON log (sizes, wall times in ms, patch sizes in bytes) to stdout.
//! Run: `cargo run --release -p tt-executor --example commit_scale > commit-scale.json`.
//!
//! Measured (C1.1 ticket: "measure full staged commits, the added committed-entry diff/application check and inverse
//! allocations", and isolate the touched-object diff from the whole-entry own-view diff):
//! - the full executor round trip of `doc.object.patch` on one stroke, its dry run, undo and redo;
//! - the model's staged commit alone (`Document::with_patched_entries`) and its parts: entry serialisation, the
//!   verified canonical diff (`diff_json`) and its application check (`apply` + equality);
//! - the touched-object change computation (two own views + their diff) against the whole-entry own-view diff;
//! - revision stamping; the size of the stored undo/redo patches for a property edit and for inserting one stroke;
//! - the two parts of the full staged integrity check (entry input path, document issues).
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used, missing_docs, reason = "measurement tool")]

use std::time::Instant;

use serde_json::{Map, Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation};
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::Drawing;
use tt_model::fixture::sample_parts;
use tt_model::object::from_json;

fn ms(t: Instant) -> f64 {
    (t.elapsed().as_secs_f64() * 1e6).round() / 1e3
}

fn time<T>(log: &mut Map<String, Value>, name: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    log.insert(name.to_owned(), json!(ms(t)));
    out
}

fn bytes(p: &[PatchOp]) -> usize {
    serde_json::to_string(p).map(|s| s.len()).unwrap_or(0)
}

fn op(kind: &'static str, path: &str, value: Option<Value>) -> PatchOp {
    PatchOp { op: kind, from: None, path: path.into(), range: None, value, old_value: None, elided: false }
}

fn main() {
    let mut log = Map::new();
    log.insert("profile".into(), json!(if cfg!(debug_assertions) { "debug" } else { "release" }));
    log.insert("os".into(), json!(std::env::consts::OS));
    log.insert("arch".into(), json!(std::env::consts::ARCH));
    let mut parts = sample_parts(1).unwrap();
    let drawing_idx = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut d: Value = serde_json::to_value(&parts.drawings[drawing_idx]).unwrap();
    let strokes: Vec<Value> = (0..100_000)
        .map(|i| {
            let x = f64::from(i % 1000) * 1.5;
            let y = f64::from(i / 1000) * 2.25;
            json!({"id": format!("s{i}"), "type": "stroke", "rev": 1, "kind": "pencil", "closed": false,
                   "points": [[x, y, 0, 0, 0.5, 0.25], [x + 1.0, y + 0.5, -0.5, 0, 0, 0], [x + 2.0, y, 0, 0, 0, 0]],
                   "colourId": null, "fillMatrix": null, "lineStyle": {"thickness": 1, "lineType": "solid"},
                   "spine": null, "sides": {"left": null, "right": null}})
        })
        .collect();
    d["nodes"][0]["strokes"] = Value::Array(strokes);
    parts.drawings[drawing_idx] = from_json::<Drawing>(&d).unwrap();
    let drawing = parts.drawings[drawing_idx].id().clone();
    let doc = time(&mut log, "documentAssembleAndCheckMs", || Document::from_parts(parts).unwrap());
    log.insert("objectCount".into(), json!(doc.object_count()));
    let entry = EntryKey::Drawing(drawing.clone());
    let stroke = format!("{drawing}/s50000");
    let swatch = doc.palettes().next().unwrap().swatches()[0].colour_id().as_str().to_owned();

    // ----- model-level parts of one staged commit ------------------------------------------------------------------
    let raw = time(&mut log, "entryRawJsonMs", || doc.entry_raw_json(&entry).unwrap());
    log.insert("entryJsonBytes".into(), json!(serde_json::to_string(&raw).map(|s| s.len()).unwrap_or(0)));
    let (_, _, at) = time(&mut log, "typedObjectPointerMs", || doc.object_pointer(&stroke).unwrap());
    let patch = vec![op("replace", &format!("{at}/colourId"), Some(json!(swatch)))];
    let (next, commits) = time(&mut log, "stagedCommitWithPatchedEntriesMs", || {
        doc.with_patched_entries(&[(entry.clone(), patch.clone())]).unwrap()
    });
    log.insert("propertyEditUndoBytes".into(), json!(bytes(&commits[0].undo)));
    log.insert("propertyEditRedoBytes".into(), json!(bytes(&commits[0].redo)));
    let after = next.entry_raw_json(&entry).unwrap();
    let diff = time(&mut log, "committedEntryDiffJsonMs", || {
        change::diff_json(&after, &raw, "", tt_model::ObjectType::Drawing)
    });
    let applied = time(&mut log, "committedEntryApplyCheckMs", || change::apply(&after, &diff).unwrap());
    time(&mut log, "committedEntryEqualityCheckMs", || assert!(applied == raw));
    time(&mut log, "touchedObjectChangeMs", || {
        let a = change::own_view(&doc, &stroke).unwrap();
        let b = change::own_view(&next, &stroke).unwrap();
        change::diff_json(&a.1, &b.1, "", a.0)
    });
    time(&mut log, "wholeEntryOwnViewDiffMs", || change::diff(&doc, &next).unwrap());
    time(&mut log, "stampRevisionsMs", || next.with_revisions(doc.document_rev() + 1, std::slice::from_ref(&stroke)).unwrap());
    // The two parts of the full staged integrity guarantee kept by the C1.1 ruling: the entry's full input path
    // (depth, normalisation, schema, typed parse, object rules, frame limits) and the document-wide issues().
    time(&mut log, "stagedEntryInputPathMs", || from_json::<Drawing>(&after).unwrap());
    time(&mut log, "documentIssuesMs", || next.issues().len());
    // Inserting one stroke: the verified undo/redo are one operation each (ID-keyed list diff).
    let (_, mut s) = doc.object_raw_json(&stroke).unwrap();
    s["id"] = json!("s_extra");
    let list = at.rsplit_once('/').unwrap().0.to_owned();
    let (_, inserted) = time(&mut log, "stagedCommitInsertStrokeMs", || {
        doc.with_patched_entries(&[(entry.clone(), vec![op("add", &format!("{list}/-"), Some(s.clone()))])]).unwrap()
    });
    log.insert("insertStrokeUndoBytes".into(), json!(bytes(&inserted[0].undo)));
    log.insert("insertStrokeRedoBytes".into(), json!(bytes(&inserted[0].redo)));

    // ----- the executor round trip ----------------------------------------------------------------------------------
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: std::time::Duration::from_secs(600) }).unwrap();
    time(&mut log, "executorOpenDocumentMs", || assert!(h.open_document(doc.clone(), None, false).is_ok()));
    let params = json!({ "targets": [stroke], "props": { "colourId": swatch } });
    let env = time(&mut log, "executorPatchDryRunMs", || {
        h.invoke(Invocation::user(
            "doc.object.patch",
            json!({ "targets": [stroke], "props": { "colourId": swatch }, "dryRun": true }),
        ))
    });
    assert!(env.is_ok(), "{:?}", env.error);
    let env =
        time(&mut log, "executorPatchCommitMs", || h.invoke(Invocation::user("doc.object.patch", params.clone())));
    assert!(env.is_ok(), "{:?}", env.error);
    log.insert("executorPatchChanges".into(), json!(env.changes.len()));
    let env = time(&mut log, "executorUndoMs", || h.invoke(Invocation::user("edit.undo", json!({}))));
    assert!(env.is_ok(), "{:?}", env.error);
    let env = time(&mut log, "executorRedoMs", || h.invoke(Invocation::user("edit.redo", json!({}))));
    assert!(env.is_ok(), "{:?}", env.error);
    let env = time(&mut log, "executorQueryStrokeMs", || {
        h.invoke(Invocation::user("doc.object.get", json!({ "ref": stroke })))
    });
    assert!(env.is_ok(), "{:?}", env.error);
    let env = time(&mut log, "executorDiffSinceOpenMs", || {
        h.invoke(Invocation::user("doc.diff", json!({ "sinceRev": doc.document_rev(), "detail": "summary" })))
    });
    assert!(env.is_ok(), "{:?}", env.error);
    println!("{}", serde_json::to_string_pretty(&Value::Object(log)).unwrap());
}
