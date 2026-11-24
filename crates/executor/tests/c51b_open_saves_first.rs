//! M5 ruling E-45: `project.open {onUnsaved: "save"}` of the open project's own path saves first, then reads what was
//! saved. Before it, the open's job read the file while the save was still pending (the save is written at the
//! invocation's commit, the job thread started inside the handler): the reopened document was the copy read before the
//! save, the disk held the new state, and the next save wrote the old over the new.
//!
//! For each kind of unsaved change (a document edit; a history position away from the saved one, the position itself
//! irrelevant to what is saved; session state saved with the project): the reopened document equals the saved file
//! entry for entry, the project is clean and writable (its own lock was released before the read), a second save keeps
//! the change, and the recovery data is consistent (the old session's copy gone, none offered, a new autosave holding
//! the reopened document).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use c13_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn entries(d: &tt_model::document::Document) -> BTreeMap<String, String> {
    d.entry_keys().iter().map(|k| (format!("{k:?}"), d.entry_canonical(k).unwrap())).collect()
}

fn live_entries(h: &ExecutorHandle) -> BTreeMap<String, String> {
    entries(&h.snapshot().unwrap().unwrap())
}

fn file_entries(p: &Path) -> BTreeMap<String, String> {
    entries(&tt_persist::load(p, &Default::default()).unwrap().doc)
}

/// Forces an autosave and returns the recovery id it names.
fn autosave(h: &ExecutorHandle) -> String {
    let events = h.subscribe();
    h.run_maintenance(true).unwrap();
    for _ in 0..150 {
        if let Ok(e) = events.recv_timeout(Duration::from_millis(200))
            && e.name == "project.autosaved"
        {
            return e.payload["recoveryId"].as_str().unwrap().to_owned();
        }
    }
    panic!("no project.autosaved");
}

/// A saved project whose scene carries `meta.base`; returns the handle, the target and the scene.
fn saved_project(dir: &Path) -> (ExecutorHandle, std::path::PathBuf, String) {
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Reopen" }));
    let target = dir.join("Reopen.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path(&target) }));
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "base": true } } }));
    job_ok(&h, "project.save", json!({}));
    (h, target, scene)
}

/// Reopens the open project's own path with `onUnsaved: "save"` and checks what every kind shares.
fn reopen_and_check(h: &ExecutorHandle, dir: &Path, target: &Path, kind: &str) {
    assert_eq!(ok(h, "doc.state.get", json!({}))["dirty"], true, "{kind}: the change is unsaved before the reopen");
    let old_recovery = autosave(h);
    let recovery = dir.join("user").join("Recovery");
    assert!(recovery.join(&old_recovery).exists(), "{kind}: the autosave wrote its copy");
    let before = live_entries(h);

    job_ok(h, "project.open", json!({ "path": path(target), "onUnsaved": "save" }));

    let saved = file_entries(target);
    let reopened = live_entries(h);
    assert_eq!(reopened, saved, "{kind}: the reopened document is the saved one");
    // What was unsaved is what was saved: the live document before the reopen, but for the save's `modifiedAt`.
    let without_project = |m: &BTreeMap<String, String>| {
        m.iter().filter(|(k, _)| !k.starts_with("Project")).map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()
    };
    assert_eq!(without_project(&reopened), without_project(&before), "{kind}: the unsaved change was kept");
    let st = ok(h, "doc.state.get", json!({}));
    assert_eq!(st["dirty"], false, "{kind}: {st}");
    assert_eq!(st["readOnly"], false, "{kind}: the project's own lock was released before the read: {st}");

    // Recovery: the old session's copy is gone, nothing is offered, and a new autosave holds the reopened document.
    assert!(!recovery.join(&old_recovery).exists(), "{kind}: the replaced session's recovery data was deleted");
    assert!(ok(h, "project.recovery.list", json!({})).as_array().unwrap().is_empty(), "{kind}");
    let scene = first(h, "scene");
    ok(h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "after": kind } } }));
    let new_recovery = autosave(h);
    assert_ne!(new_recovery, old_recovery, "{kind}");
    let copy = tt_persist::load(
        &tt_persist::recovery::project_folder(&recovery, &new_recovery),
        &tt_persist::LoadOptions { read_only: true, ..Default::default() },
    )
    .unwrap();
    assert_eq!(without_project(&entries(&copy.doc)), without_project(&live_entries(h)), "{kind}: the new copy");

    // The next save keeps the change (before E-45 it wrote the old copy over it).
    job_ok(h, "project.save", json!({}));
    assert_eq!(file_entries(target), live_entries(h), "{kind}: the next save");
}

#[test]
fn a_document_edit_is_saved_then_reopened() {
    let dir = tmp();
    let (h, target, scene) = saved_project(&dir);
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "edit": "unsaved" } } }));
    reopen_and_check(&h, &dir, &target, "document");
    assert_eq!(get(&h, &scene)["meta"]["edit"], "unsaved");
    assert_eq!(get(&h, &scene)["meta"]["base"], true);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_history_position_away_from_the_saved_one_is_saved_then_reopened() {
    let dir = tmp();
    let (h, target, scene) = saved_project(&dir);
    // Undo past the saved position: the project is modified, and the saved state is the undone one.
    ok(&h, "edit.undo", json!({}));
    assert!(get(&h, &scene)["meta"].get("base").is_none());
    reopen_and_check(&h, &dir, &target, "history position");
    assert!(get(&h, &scene)["meta"].get("base").is_none(), "the undone state was saved and reopened");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_state_is_saved_then_reopened() {
    let dir = tmp();
    let (h, target, scene) = saved_project(&dir);
    // A scene of 24 frames (a new project's scene is one frame long, and a free range is clamped to the scene).
    let drawing = ok(&h, "element.create", json!({ "type": "drawing" }))["elementIds"][0].as_str().unwrap().to_owned();
    ok(&h, "element.drawing.create", json!({ "elementId": drawing, "name": "1", "exposeFrom": 1, "exposeTo": 24 }));
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "edit": "with session" } } }));
    ok(&h, "timeline.playbackRange.set", json!({ "mode": "free", "start": 3, "end": 9 }));
    ok(&h, "timeline.currentFrame.set", json!({ "frame": 7 }));
    reopen_and_check(&h, &dir, &target, "session");
    let range: Value = ok(&h, "timeline.playbackRange.get", json!({}));
    assert_eq!(
        (range["mode"].clone(), range["start"].clone(), range["end"].clone()),
        (json!("free"), json!(3), json!(9))
    );
    assert_eq!(ok(&h, "app.state.get", json!({}))["frame"], 7);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}
