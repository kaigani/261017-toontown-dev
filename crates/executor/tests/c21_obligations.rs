//! M2 C2.1 obligations 2, 3 and 10 on a saved and reopened geometry fixture: drawing constructors run only through the
//! registry and the serialized executor; a later invalid geometry or reference step rolls the whole atomic batch back;
//! a successful atomic batch is one revision and one undo step whose undo and redo restore the canonical entries
//! byte for byte; a dry run predicts the real result and leaves the pre-state; revision exhaustion refuses; opaque
//! `meta`/`ext` data, stable IDs and drawing revisions are preserved; no derived field (zones) is installed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use c13_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Invocation, InvokeOptions, Origin, Status};
use tt_model::document::{Document, EntryKey};

const BOB: &str = "scene:#1/element:\"Bob\"";
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const INVISIBLE: &str = "st_01K742SG0S0NT23BWCZN64SFQ5";

fn canonical(doc: &Document) -> Vec<(EntryKey, String)> {
    doc.entry_keys()
        .into_iter()
        .map(|k| {
            let t = tt_model::canonical::to_canonical_string(&doc.entry_raw_json(&k).unwrap()).unwrap();
            (k, t)
        })
        .collect()
}

/// An object with every level of children in full.
fn deep(h: &ExecutorHandle, r: &str) -> Value {
    ok(h, "doc.object.get", json!({ "ref": r, "depth": -1 }))["object"].clone()
}

fn snap(h: &ExecutorHandle) -> Document {
    h.snapshot().unwrap().unwrap()
}

/// The fixture with drawing content added by commands, saved as a package and opened again in a fresh executor.
fn reopened() -> (ExecutorHandle, std::path::PathBuf) {
    let dir = tmp();
    let h = spawn(&dir.join("user1"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "points": [{"x": 0, "y": 0, "pressure": 0.3}, {"x": 30, "y": 20, "pressure": 1}, {"x": 60, "y": 0, "pressure": 0.5}] }),
    );
    ok(&h, "draw.text.add", json!({ "elementId": BOB, "frame": 1, "position": {"x": 0, "y": 100}, "text": "Saved" }));
    // Opaque data from another tool on a stroke (preserved verbatim, 10 §2.2).
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [format!("{D1}/{PENCIL}")], "props": { "ext": { "org.other.tool": { "weight": [1, 2, {"x": null}] } }, "meta": { "note": "keep" } } }),
    );
    let path = dir.join("Geometry.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path.to_string_lossy() }));
    drop(h);
    let h = spawn(&dir.join("user2"));
    job_ok(&h, "project.open", json!({ "path": path.to_string_lossy() }));
    (h, dir)
}

#[test]
fn a_later_invalid_step_rolls_the_whole_atomic_batch_back() {
    let (h, dir) = reopened();
    let before = canonical(&snap(&h));
    let rev0 = rev(&h);
    for bad in [
        json!({ "id": "draw.stroke.set", "params": { "drawingId": D1, "strokeId": PENCIL, "bezier": [{"x": 0, "y": 0}, {"x": 1, "y": 1}] } }),
        json!({ "id": "draw.stroke.set", "params": { "drawingId": D1, "strokeId": "st_missing", "thickness": 2 } }),
        json!({ "id": "draw.group.create", "params": { "drawingId": D1, "ids": ["nd_missing"] } }),
    ] {
        let env = batch(
            &h,
            json!({ "atomic": true, "label": "Three edits", "steps": [
                { "id": "draw.stroke.add", "params": { "drawingId": D1, "kind": "pencil", "points": [{"x": 0, "y": 0}, {"x": 10, "y": 10}] } },
                { "id": "draw.text.add", "params": { "drawingId": D1, "position": {"x": 5, "y": 5}, "text": "temp" } },
                bad.clone(),
            ] }),
        );
        assert_eq!(env.status, Status::Error, "{bad}");
        let e = env.error.unwrap();
        assert_eq!(e.code, "batch.stepFailed");
        assert_eq!(e.details["stepIndex"], 3, "the third step (1-based, ADR-0013 §6)");
        assert_eq!(canonical(&snap(&h)), before, "nothing of the batch remains: {bad}");
        assert_eq!(rev(&h), rev0);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_successful_atomic_batch_is_one_revision_and_one_exactly_invertible_undo_step() {
    let (h, dir) = reopened();
    let before = canonical(&snap(&h));
    let rev0 = rev(&h);
    let undo0 = ok(&h, "edit.history.get", json!({}))["entries"].as_array().unwrap().len();
    let env = batch(
        &h,
        json!({ "atomic": true, "label": "Ink pass", "steps": [
            { "id": "draw.stroke.add", "params": { "drawingId": D1, "kind": "brush", "points": [{"x": 0, "y": 0, "pressure": 0.2}, {"x": 40, "y": 30, "pressure": 1}, {"x": 80, "y": 10, "pressure": 0.4}] } },
            { "id": "draw.stroke.set", "params": { "drawingId": D1, "strokeId": PENCIL, "thickness": 6 } },
            { "id": "draw.object.delete", "params": { "drawingId": D1, "ids": [INVISIBLE] } },
            { "id": "draw.pivot.set", "params": { "drawingId": D1, "point": {"x": 1, "y": 2} } },
            { "id": "draw.text.add", "params": { "elementId": BOB, "frame": 7, "position": {"x": 0, "y": 0}, "text": "new cell" } },
        ] }),
    );
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
    assert_eq!(rev(&h), rev0 + 1, "one revision for the batch (REV-01)");
    let hist = ok(&h, "edit.history.get", json!({}));
    let entries = hist["entries"].as_array().unwrap();
    assert_eq!(entries.len(), undo0 + 1, "one undo step");
    assert_eq!(entries.last().unwrap()["label"], "Ink pass");
    // Every drawing changed reports its new revision (REV-02, 11 P6).
    let d1 = env.changes.iter().find(|c| c.id == D1).unwrap();
    assert_eq!(d1.rev, rev0 + 1);
    let after = canonical(&snap(&h));
    ok(&h, "edit.undo", json!({}));
    let undone = canonical(&snap(&h));
    // Content is the pre-state exactly; only revision stamps of the touched objects move forward (REV-03).
    let strip = |v: &[(EntryKey, String)]| -> Vec<(EntryKey, Value)> {
        v.iter()
            .map(|(k, t)| {
                let mut j: Value = serde_json::from_str(t).unwrap();
                strip_revs(&mut j);
                (k.clone(), j)
            })
            .collect()
    };
    assert_eq!(strip(&undone), strip(&before), "undo is the exact inverse of the content");
    ok(&h, "edit.redo", json!({}));
    assert_eq!(strip(&canonical(&snap(&h))), strip(&after), "redo restores the batch's content exactly");
    let _ = std::fs::remove_dir_all(dir);
}

fn strip_revs(v: &mut Value) {
    match v {
        Value::Object(m) => {
            if m.contains_key("type") {
                m.remove("rev");
            }
            m.remove("documentRev");
            m.remove("modifiedAt");
            m.values_mut().for_each(strip_revs);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_revs),
        _ => {}
    }
}

#[test]
fn a_dry_run_predicts_the_real_call_and_keeps_the_pre_state() {
    let (h, dir) = reopened();
    let before = canonical(&snap(&h));
    let hist = ok(&h, "edit.history.get", json!({}));
    let params = json!({ "drawingId": D1, "kind": "brush", "points": [{"x": 0, "y": 0}, {"x": 20, "y": 20}, {"x": 40, "y": 0}] });
    let dry = h.invoke(Invocation {
        command: "draw.stroke.add".into(),
        params: params.clone(),
        options: InvokeOptions { dry_run: true, ..Default::default() },
        origin: Origin::User,
    });
    assert_eq!(dry.status, Status::Ok, "{:?}", dry.error);
    assert!(dry.dry_run);
    assert_eq!(canonical(&snap(&h)), before, "the dry run changed nothing");
    assert_eq!(ok(&h, "edit.history.get", json!({})), hist);
    let real = run(&h, "draw.stroke.add", params);
    assert_eq!(dry.result, real.result, "the dry run predicts the IDs, layer and revision of the real call");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn revision_exhaustion_refuses_drawing_edits_and_changes_nothing() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    parts.document_rev = u64::MAX;
    let d = Document::from_parts(parts).unwrap();
    assert!(h.open_document(d, None, false).is_ok());
    let before = canonical(&snap(&h));
    for (id, params) in [
        ("draw.stroke.add", json!({ "drawingId": D1, "points": [{"x": 0, "y": 0}, {"x": 1, "y": 1}] })),
        ("draw.text.add", json!({ "drawingId": D1, "position": {"x": 0, "y": 0}, "text": "x" })),
        ("draw.stroke.add", json!({ "elementId": BOB, "frame": 7, "points": [{"x": 0, "y": 0}, {"x": 1, "y": 1}] })),
    ] {
        let e = err(&h, id, params);
        assert_eq!(e.code, "doc.limitExceeded", "{id}");
        assert_eq!(e.details["limit"], "documentRev");
    }
    assert_eq!(canonical(&snap(&h)), before);
    assert!(run(&h, "doc.state.get", json!({})).is_ok(), "the document thread still serves");
}

#[test]
fn opaque_data_ids_and_revisions_survive_edits_save_and_reopen() {
    let (h, dir) = reopened();
    let stroke = |h: &ExecutorHandle| get(h, &format!("{D1}/{PENCIL}"));
    // Reopened: the other tool's ext and the meta are there verbatim.
    assert_eq!(stroke(&h)["ext"]["org.other.tool"], json!({ "weight": [1, 2, {"x": null}] }));
    assert_eq!(stroke(&h)["meta"], json!({ "note": "keep" }));
    let drawing_ext = get(&h, D1)["ext"].clone();
    assert_eq!(drawing_ext["com.example.tool"], json!({ "stage": "clean-up", "pass": 2 }));
    // A geometry edit keeps the ID, the opaque data and every other stroke; the stroke and drawing revisions rise.
    let rev0 = rev(&h);
    let others: Vec<Value> =
        deep(&h, D1)["nodes"][0]["strokes"].as_array().unwrap().iter().filter(|s| s["id"] != PENCIL).cloned().collect();
    ok(
        &h,
        "draw.stroke.set",
        json!({ "drawingId": D1, "strokeId": PENCIL, "bezier": [{"x": 0, "y": 0}, {"x": 5, "y": 9}, {"x": 15, "y": 9}, {"x": 20, "y": 0}] }),
    );
    let s = stroke(&h);
    assert_eq!(s["id"], PENCIL);
    assert_eq!(s["ext"]["org.other.tool"], json!({ "weight": [1, 2, {"x": null}] }));
    assert_eq!(s["meta"], json!({ "note": "keep" }));
    assert_eq!(s["rev"], rev0 + 1);
    assert_eq!(get(&h, D1)["rev"], rev0 + 1, "the drawing revision follows its content (REV-02)");
    let still: Vec<Value> =
        deep(&h, D1)["nodes"][0]["strokes"].as_array().unwrap().iter().filter(|s| s["id"] != PENCIL).cloned().collect();
    assert_eq!(still, others);
    // No derived field is persisted: no zones anywhere, side fills untouched.
    let raw = snap(&h).entry_raw_json(&EntryKey::Drawing(tt_model::Id::parse(D1).unwrap())).unwrap();
    assert!(!raw.to_string().contains("\"zones\""));
    assert_eq!(s["sides"], json!({ "left": null, "right": null }));
    // Save, reopen: identical canonical entries.
    job_ok(&h, "project.save", json!({}));
    let before = canonical(&snap(&h));
    let path = dir.join("Geometry.animpkg").to_string_lossy().into_owned();
    drop(h);
    let h2 = spawn(&dir.join("user3"));
    job_ok(&h2, "project.open", json!({ "path": path }));
    let mut a = before;
    let mut b = canonical(&snap(&h2));
    for v in [&mut a, &mut b] {
        v.retain(|(k, _)| *k != EntryKey::Project);
    }
    assert_eq!(a, b, "the round trip is byte identical (11 §14.3)");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_patch_of_a_brush_spine_sweeps_the_outline_and_a_patch_of_the_outline_drops_the_spine() {
    let (h, dir) = reopened();
    let brush = "st_01K742SG0QD1F2HFZSX8R4V7WP";
    let key = format!("{D1}/{brush}");
    let before = get(&h, &key);
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [key], "ops": [{ "op": "replace", "path": "/spine/pressure", "value": [[0, 1], [1, 1]] }] }),
    );
    let after = get(&h, &key);
    assert_ne!(after["points"], before["points"], "the outline was swept again (02 §13.6)");
    assert!(after["spine"].is_object());
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [key], "ops": [{ "op": "replace", "path": "/points/0/0", "value": -1 }] }),
    );
    assert!(get(&h, &key)["spine"].is_null(), "an outline edited directly discards the spine (02 §3.2)");
    let e = err(
        &h,
        "doc.object.patch",
        json!({ "targets": [key], "ops": [{ "op": "replace", "path": "/points/0/0", "value": 1e300 }] }),
    );
    assert_eq!(e.code, "drawing.invalidGeometry");
    let _ = std::fs::remove_dir_all(dir);
}

/// Obligations 5 and 7: no C2.1 drawing command produces a file or asks for a confirmation, so the staged-file and
/// confirmation contracts of C1.4 are unchanged by C2.1 (the first file-producing drawing command is
/// `draw.drawing.render {path}`, M2 C2.3; the first confirming one `draw.feather.apply`, C2.3). Their scopes are
/// `read` or `edit` only.
#[test]
fn c21_drawing_commands_write_no_files_and_ask_no_confirmation() {
    let registry = tt_executor::registry().unwrap();
    let c21 = [
        "draw.drawing.get",
        "draw.current.get",
        "draw.pivot.set",
        "draw.object.delete",
        "draw.group.create",
        "draw.group.delete",
        "draw.stroke.add",
        "draw.stroke.set",
        "draw.content.add",
        "draw.text.add",
        "draw.text.set",
        "draw.textFormat.set",
    ];
    for id in c21 {
        let e = registry.get(id).unwrap();
        assert_eq!(e.status(), tt_registry::Status::Implemented, "{id}");
        let c = e.command.unwrap();
        assert!(c.decl.confirm.is_empty(), "{id} asks for confirmation");
        let scopes = e.scopes();
        assert!(scopes == [tt_registry::Scope::Read] || scopes == [tt_registry::Scope::Edit], "{id}: {scopes:?}");
        let params = e.params_schema().to_string();
        assert!(!params.contains("\"path\""), "{id} takes no file path");
    }
    let implemented: Vec<&str> = registry
        .entries()
        .filter(|e| e.id().starts_with("draw.") && e.status() == tt_registry::Status::Implemented)
        .map(|e| e.id())
        .collect();
    assert_eq!(implemented.len(), c21.len(), "{implemented:?}");
}
