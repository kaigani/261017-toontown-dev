//! C1.2 executor obligations: transactions and revision stamping (REV-01…06, drawing content included), undo/redo as
//! transactions with stable step ids and the verified patches of C11-R6-01, atomic multi-entry rollback, `expectRev`,
//! `dryRun`, `requestId`, checkpoints, `doc.diff`, events and the history bound.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{InvokeOptions, Status};

#[test]
fn a_patch_is_one_transaction_with_stamped_revisions_and_an_undo_step() {
    let h = fixture();
    let r0 = rev(&h);
    let scene = first(&h, "scene");
    let before = h.snapshot().unwrap().unwrap();
    let env = run(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "status": "layout" } } }));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.document_rev, Some(r0 + 1), "REV-01: one transaction, +1");
    assert_eq!(env.changes.len(), 1);
    let c = &env.changes[0];
    assert_eq!((c.id.as_str(), c.op.as_str(), c.rev), (scene.as_str(), "modified", r0 + 1));
    let patch = c.patch.as_ref().unwrap();
    assert_eq!(
        (patch[0]["op"].as_str(), patch[0]["path"].as_str()),
        (Some("add"), Some("/meta")),
        "empty meta is omitted, so it is added whole"
    );
    assert_eq!(obj_rev(&h, &scene), r0 + 1, "REV-02: the object carries the transaction's revision");
    let undo = env.undo.clone().unwrap();
    assert_eq!(undo.step_id, "u-1");
    assert_eq!(undo.label, "Set meta");
    assert_eq!(env.result.as_ref().unwrap()["documentRev"], json!(r0 + 1));
    // Copy-on-write: the earlier snapshot is untouched; unchanged entries are shared.
    assert_eq!(before.document_rev(), r0);
    assert!(before.locate(&scene).unwrap().rev < r0 + 1);
    let after = h.snapshot().unwrap().unwrap();
    let palette = after.palettes().next().unwrap().id().clone();
    assert!(before.same_entry(&after, &tt_model::document::EntryKey::Palette(palette)));
    // A patch that changes nothing commits nothing.
    let env = run(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "status": "layout" } } }));
    assert!(env.is_ok());
    assert!(env.changes.is_empty() && env.undo.is_none());
    assert_eq!(rev(&h), r0 + 1);
}

#[test]
fn drawing_content_edits_stamp_the_drawing() {
    let h = fixture();
    let stroke = first(&h, "stroke");
    let (drawing, _) = stroke.split_once('/').unwrap();
    let drawing = drawing.to_owned();
    let swatch = first(&h, "swatch");
    let node = h.snapshot().unwrap().unwrap().locate(&stroke).unwrap().parent.clone().unwrap();
    let node_rev = obj_rev(&h, &node);
    let r0 = rev(&h);
    let env = run(&h, "doc.object.patch", json!({ "targets": [stroke], "props": { "colourId": swatch } }));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(obj_rev(&h, &stroke), r0 + 1);
    assert_eq!(obj_rev(&h, &drawing), r0 + 1, "REV-02: a content edit changes the drawing's rev");
    assert_eq!(obj_rev(&h, &node), node_rev, "the layer's own view did not change");
    // Undo is a new transaction: content back, revisions forward (REV-03).
    let env = run(&h, "edit.undo", json!({}));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.document_rev, Some(r0 + 2));
    assert_eq!(obj_rev(&h, &stroke), r0 + 2);
    assert_eq!(obj_rev(&h, &drawing), r0 + 2);
    assert_ne!(get(&h, &stroke)["colourId"], json!(swatch));
}

#[test]
fn undo_and_redo_replay_verified_patches_and_keep_step_ids() {
    let h = fixture();
    let scene = first(&h, "scene");
    let original = get(&h, &scene);
    for (i, v) in ["a", "b", "c"].iter().enumerate() {
        let env = run(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "v": v } } }));
        assert_eq!(env.undo.unwrap().step_id, format!("u-{}", i + 1));
    }
    let r = rev(&h);
    let env = run(&h, "edit.undo", json!({ "steps": 2 }));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.document_rev, Some(r + 2), "each undone step is its own transaction");
    assert_eq!(env.result.as_ref().unwrap()["undone"], json!(["Set meta", "Set meta"]));
    assert_eq!(env.result.as_ref().unwrap()["position"], 1);
    assert_eq!(get(&h, &scene)["meta"]["v"], "a");
    // ifTop protects later steps (11 §7.7).
    let e = err(&h, "edit.undo", json!({ "ifTop": "u-3" }));
    assert_eq!(e.code, "undo.notTop");
    assert_eq!(e.details["top"]["stepId"], "u-1");
    let env = run(&h, "edit.redo", json!({ "steps": 2 }));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(get(&h, &scene)["meta"]["v"], "c");
    // Undo/redo cycles reproduce the states exactly, again and again (the redo returned by undo is replayed).
    for _ in 0..3 {
        assert!(run(&h, "edit.undo", json!({ "steps": 3 })).is_ok());
        let mut now = get(&h, &scene);
        let mut orig = original.clone();
        for v in [&mut now, &mut orig] {
            v.as_object_mut().unwrap().remove("rev");
        }
        assert_eq!(now, orig);
        assert!(run(&h, "edit.redo", json!({ "steps": 3 })).is_ok());
        assert_eq!(get(&h, &scene)["meta"]["v"], "c");
    }
    let hist = ok(&h, "edit.history.get", json!({}));
    let ids: Vec<&str> = hist["entries"].as_array().unwrap().iter().map(|e| e["stepId"].as_str().unwrap()).collect();
    assert_eq!(ids, ["u-1", "u-2", "u-3"], "replays keep the step ids");
    assert_eq!(hist["position"], 3);
    // A new change discards the redo steps.
    assert!(run(&h, "edit.undo", json!({})).is_ok());
    let env = run(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "v": "d" } } }));
    assert_eq!(env.undo.unwrap().step_id, "u-4");
    assert_eq!(err(&h, "edit.redo", json!({})).code, "edit.nothingToRedo");
    // history.move walks the history as transactions.
    let r = rev(&h);
    let env = run(&h, "edit.history.move", json!({ "position": 0 }));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.document_rev, Some(r + 3));
    assert_eq!(err(&h, "edit.undo", json!({})).code, "edit.nothingToUndo");
}

#[test]
fn atomic_batches_commit_several_entries_as_one_step_or_roll_back_entirely() {
    let h = fixture();
    let scene = first(&h, "scene");
    let palette = first(&h, "palette");
    let element = first(&h, "element");
    let r0 = rev(&h);
    let env = batch(
        &h,
        json!({ "label": "Tag things", "steps": [
        { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "t": 1 } } } },
        { "id": "doc.object.patch", "params": { "targets": [palette], "props": { "meta": { "t": 2 } } } },
        { "id": "doc.object.get", "params": { "ref": palette } },
        { "id": "doc.object.patch", "params": { "targets": [element], "props": { "role": "foreground" } }, "as": "role" }
    ] }),
    );
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.document_rev, Some(r0 + 1), "one transaction");
    assert_eq!(env.undo.as_ref().unwrap().label, "Tag things");
    assert_eq!(env.changes.len(), 3);
    let steps = env.steps.as_ref().unwrap();
    assert_eq!(steps[2].result.as_ref().unwrap()["object"]["meta"]["t"], 2, "queries see uncommitted state");
    for k in [&scene, &palette, &element] {
        assert_eq!(obj_rev(&h, k), r0 + 1);
    }
    // Undo of a multi-entry step restores every entry in one transaction.
    let env = run(&h, "edit.undo", json!({}));
    assert_eq!(env.document_rev, Some(r0 + 2));
    assert_eq!(env.changes.len(), 3);
    assert!(get(&h, &scene)["meta"].get("t").is_none());
    assert_eq!(get(&h, &element)["role"], "normal");

    // A failing step rolls back everything, UI state included.
    let r1 = rev(&h);
    let snap = h.snapshot().unwrap().unwrap();
    let env = batch(
        &h,
        json!({ "label": "Broken", "steps": [
        { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "t": 3 } } } },
        { "id": "window.fullScreen.set", "params": { "enabled": true } },
        { "id": "doc.object.patch", "params": { "targets": [palette], "props": { "meta": { "t": 4 } } } },
        { "id": "doc.object.patch", "params": { "targets": [scene], "ops": [{ "op": "replace", "path": "/id", "value": "sc_X" }] } }
    ] }),
    );
    assert_eq!(env.status, Status::Error);
    let e = env.error.unwrap();
    assert_eq!(e.code, "batch.stepFailed");
    assert_eq!(e.details["stepIndex"], 4);
    assert_eq!(e.details["error"]["code"], "doc.readOnlyProperty");
    assert_eq!(rev(&h), r1);
    let now = h.snapshot().unwrap().unwrap();
    for k in [&scene, &palette] {
        assert_eq!(now.locate(k).unwrap().rev, snap.locate(k).unwrap().rev);
    }
    assert!(get(&h, &scene)["meta"].get("t").is_none());
    assert_eq!(ok(&h, "app.state.get", json!({}))["fullScreen"], false);
    assert_eq!(ok(&h, "edit.history.get", json!({}))["total"], 1);
}

#[test]
fn model_level_staged_commit_of_several_entries_is_atomic() {
    // The executor commits through Document::with_patched_entries: an integrity error in the final state rejects
    // every entry's patch (no partial commit).
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let scene = doc.scenes().next().unwrap().id().clone();
    let element = doc.elements().next().unwrap().id().clone();
    let op = |path: &str, value: Value| tt_model::change::PatchOp {
        op: "replace",
        from: None,
        path: path.into(),
        range: None,
        value: Some(value),
        old_value: None,
        elided: false,
    };
    let err = doc
        .with_patched_entries(&[
            (tt_model::document::EntryKey::Scene(scene.clone()), vec![op("/meta", json!({ "ok": true }))]),
            (
                tt_model::document::EntryKey::Element(element),
                vec![op("/drawingBankId", json!("db_00000000000000000000000000"))],
            ),
        ])
        .unwrap_err();
    assert!(!err.is_empty());
    assert!(doc.scene(&scene).unwrap().meta().is_empty(), "the source document never changes");
}

#[test]
fn expect_rev_conflicts_are_typed() {
    let h = fixture();
    let scene = first(&h, "scene");
    let current = obj_rev(&h, &scene);
    let mut opts = InvokeOptions::default();
    opts.expect_rev.insert(scene.clone(), current + 7);
    let env = with(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "x": 1 } } }), opts);
    let e = env.error.unwrap();
    assert_eq!(e.code, "rev.conflict");
    assert_eq!(e.details["expected"], current + 7);
    assert_eq!(e.details["actual"], current);
    let e =
        err(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "x": 1 } }, "expectRev": 999 }));
    assert_eq!(e.code, "rev.conflict");
    let mut opts = InvokeOptions::default();
    opts.expect_rev.insert(scene.clone(), current);
    assert!(with(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "x": 1 } } }), opts).is_ok());
    let now = obj_rev(&h, &scene);
    assert!(
        run(
            &h,
            "doc.object.patch",
            json!({ "targets": [scene], "props": { "meta": { "x": 2 } }, "expectRev": { scene.clone(): now } })
        )
        .is_ok()
    );
}

#[test]
fn dry_runs_report_changes_and_change_nothing() {
    let h = fixture();
    let scene = first(&h, "scene");
    let r0 = rev(&h);
    let opts = InvokeOptions { dry_run: true, ..InvokeOptions::default() };
    let env =
        with(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "d": 1 } } }), opts.clone());
    assert!(env.is_ok() && env.dry_run);
    assert_eq!(env.changes[0].rev, r0 + 1, "the revision it would get");
    assert!(env.undo.is_none());
    assert_eq!(rev(&h), r0);
    assert!(get(&h, &scene)["meta"].get("d").is_none());
    // The command's own dryRun parameter is the same.
    let env =
        run(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "d": 1 } }, "dryRun": true }));
    assert!(env.dry_run && env.changes.len() == 1);
    assert_eq!(rev(&h), r0);
    // UI commands and batches too.
    let env = with(&h, "window.fullScreen.set", json!({ "enabled": true }), opts.clone());
    assert!(env.is_ok() && env.dry_run);
    assert_eq!(ok(&h, "app.state.get", json!({}))["fullScreen"], false);
    let env = batch(
        &h,
        json!({ "label": "x", "dryRun": true, "steps": [{ "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "d": 2 } } } }] }),
    );
    assert!(env.is_ok() && env.dry_run && env.changes.len() == 1);
    assert_eq!(rev(&h), r0);
    // Undo as a dry run.
    assert!(run(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "d": 3 } } })).is_ok());
    let env = with(&h, "edit.undo", json!({}), opts);
    assert!(env.is_ok() && env.dry_run && env.changes.len() == 1);
    assert_eq!(get(&h, &scene)["meta"]["d"], 3);
    assert_eq!(ok(&h, "edit.history.get", json!({}))["position"], 1);
}

#[test]
fn request_ids_make_calls_idempotent_per_session() {
    let h = fixture();
    let scene = first(&h, "scene");
    let r0 = rev(&h);
    let opts = InvokeOptions { request_id: Some("req-1".into()), ..InvokeOptions::default() };
    let a = with(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 1 } } }), opts.clone());
    let b = with(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 2 } } }), opts.clone());
    assert_eq!(a, b, "a repeated requestId returns the original result");
    assert_eq!(rev(&h), r0 + 1);
    assert_eq!(get(&h, &scene)["meta"]["n"], 1);
    // Another session's identical id is another request.
    let env = h.invoke(tt_executor::Invocation {
        command: "doc.object.patch".into(),
        params: json!({ "targets": [scene], "props": { "meta": { "n": 3 } } }),
        options: opts,
        origin: agent(&[tt_registry::Scope::Read, tt_registry::Scope::Edit]),
    });
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(rev(&h), r0 + 2);
}

#[test]
fn checkpoints_restore_as_one_undoable_step_without_lowering_revisions() {
    let h = fixture();
    let scene = first(&h, "scene");
    let palette = first(&h, "palette");
    ok(&h, "checkpoint.create", json!({ "name": "base" }));
    assert_eq!(err(&h, "checkpoint.create", json!({ "name": "base" })).code, "checkpoint.exists");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "k": 1 } } }));
    ok(&h, "doc.object.patch", json!({ "targets": [palette], "props": { "meta": { "k": 2 } } }));
    let revs_before: Vec<u64> = [&scene, &palette].iter().map(|k| obj_rev(&h, k)).collect();
    let r = rev(&h);
    let env = run(&h, "checkpoint.restore", json!({ "name": "base" }));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.document_rev, Some(r + 1));
    assert_eq!(env.undo.as_ref().unwrap().label, "Restore checkpoint base");
    assert_eq!(env.changes.len(), 2);
    assert!(get(&h, &scene)["meta"].get("k").is_none());
    for (k, before) in [&scene, &palette].iter().zip(revs_before) {
        assert!(obj_rev(&h, k) > before, "REV-03: revisions never decrease");
    }
    // Content-equal objects keep their (higher) current revision.
    let snap = h.snapshot().unwrap().unwrap();
    for loc in snap.locations() {
        assert!(loc.rev <= snap.document_rev());
    }
    let env = run(&h, "edit.undo", json!({}));
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(get(&h, &scene)["meta"]["k"], 1);
    assert!(run(&h, "edit.redo", json!({})).is_ok());
    assert!(get(&h, &scene)["meta"].get("k").is_none());
    let list = ok(&h, "checkpoint.list", json!({}));
    assert_eq!(list[0]["name"], "base");
    assert_eq!(list[0]["origin"], "user");
    ok(&h, "checkpoint.delete", json!({ "name": "base" }));
    assert_eq!(err(&h, "checkpoint.delete", json!({ "name": "base" })).code, "checkpoint.notFound");
}

#[test]
fn doc_diff_reports_net_changes_since_a_revision_or_checkpoint() {
    let h = fixture();
    let scene = first(&h, "scene");
    let palette = first(&h, "palette");
    let r0 = rev(&h);
    ok(&h, "checkpoint.create", json!({ "name": "c0" }));
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "a": 1 } } }));
    ok(&h, "doc.object.patch", json!({ "targets": [palette], "props": { "meta": { "b": 1 } } }));
    ok(&h, "edit.undo", json!({}));
    let d = ok(&h, "doc.diff", json!({ "sinceRev": r0 }));
    assert_eq!(d["fromRev"], r0);
    assert_eq!(d["toRev"], r0 + 3);
    let ids: Vec<&str> = d["changes"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, [scene.as_str()], "net: the palette change was undone");
    let d = ok(&h, "doc.diff", json!({ "sinceRev": r0, "untilRev": r0 + 2, "detail": "summary" }));
    assert_eq!(d["changes"].as_array().unwrap().len(), 2);
    assert!(d["changes"][0].get("patch").is_none());
    let d = ok(&h, "doc.diff", json!({ "sinceCheckpoint": "c0", "types": ["palette"] }));
    assert!(d["changes"].as_array().unwrap().is_empty());
    let e = err(&h, "doc.diff", json!({ "sinceRev": r0 - 1 }));
    assert_eq!(e.code, "doc.diffUnavailable");
    assert_eq!(e.details["oldestRev"], r0);
    let st = ok(&h, "doc.state.get", json!({}));
    assert_eq!(st["historyFromRev"], r0);
    assert_eq!(st["documentRev"], r0 + 3);
    assert_eq!(st["dirty"], true);
}

#[test]
fn history_keeps_at_least_200_steps() {
    let h = fixture();
    let scene = first(&h, "scene");
    for i in 0..205 {
        ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "i": i } } }));
    }
    let hist = ok(&h, "edit.history.get", json!({ "limit": 1000 }));
    assert_eq!(hist["total"], 205);
    assert!(tt_executor::MAX_UNDO_STEPS >= 200);
    let env = run(&h, "edit.undo", json!({ "steps": 205 }));
    assert!(env.is_ok());
    assert!(get(&h, &scene)["meta"].get("i").is_none());
}

#[test]
fn events_follow_commits() {
    let h = fixture();
    let rx = h.subscribe();
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "e": 1 } } }));
    ok(&h, "app.frame.move", json!({ "direction": "next" }));
    let events: Vec<tt_executor::Event> = rx.try_iter().collect();
    let names: Vec<&str> = events.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"doc.changed"), "{names:?}");
    assert!(names.contains(&"edit.history.changed"), "{names:?}");
    assert!(names.contains(&"app.state.changed"), "{names:?}");
    let changed = events.iter().find(|e| e.name == "doc.changed").unwrap();
    assert_eq!(changed.payload["undoStepId"], "u-1");
    assert_eq!(changed.payload["origin"], "user");
    assert_eq!(changed.payload["changes"][0]["id"], json!(scene));
    let state = events.iter().rev().find(|e| e.name == "app.state.changed").unwrap();
    assert!(state.payload["changed"].as_array().unwrap().contains(&json!("frame")));
}
