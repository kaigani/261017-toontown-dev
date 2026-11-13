//! M3 C3.2 round 2 (builder): undo and redo emit the step's `anim.changed` again, with the same nodes, channels and
//! frame scope as the direct call (05 §19.4: one animation invalidation per undo step, a replayed step included;
//! C32-R2-02). The FOV alias removal (C32-R2-01) is covered in `c32_round1.rs` of both crates.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const CAMERA: &str = "scene:\"Intro\"/element:\"Camera\"";

/// The `anim.changed` payloads of one call.
fn changed(h: &ExecutorHandle, id: &str, params: Value) -> Vec<Value> {
    let rx = h.subscribe();
    if id == "batch" {
        let env = batch(h, params);
        assert_eq!(env.status, Status::Ok, "{env:?}");
    } else {
        ok(h, id, params);
    }
    rx.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload.clone()).collect()
}

/// Undo then redo of the last step each emit exactly what the direct call emitted.
fn replays_match(h: &ExecutorHandle, direct: &[Value], what: &str) {
    for cmd in ["edit.undo", "edit.redo"] {
        let replayed = changed(h, cmd, json!({}));
        assert_eq!(replayed, direct, "{what}: {cmd}");
    }
}

fn frames(ev: &Value) -> Option<(f64, f64)> {
    ev.get("frames").map(|f| (f["from"].as_f64().unwrap(), f["to"].as_f64().unwrap()))
}

#[test]
fn every_animation_action_replays_its_event_on_undo_and_redo() {
    let actions = [
        ("anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "rotation": 12 })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "frame": 18, "rotation": 37 })),
        ("anim.transform.set", json!({ "nodeIds": [PEG], "frame": 12, "rotation": 45, "position": { "x": 1 } })),
        ("anim.key.add", json!({ "nodeIds": [BOB], "frame": 6 })),
        ("anim.key.move", json!({ "nodeId": BOB, "fromFrame": 12, "toFrame": 14 })),
        ("anim.key.remove", json!({ "nodeIds": [BOB], "frame": 12 })),
        ("anim.segment.set", json!({ "nodeIds": [BOB], "frame": 1, "constant": true })),
        ("anim.segment.uniform.apply", json!({ "nodeId": BOB, "frame": 1, "side": "right" })),
        ("anim.function.key.add", json!({ "nodeId": BOB, "channel": "rotation", "frame": 6, "value": 3 })),
        ("anim.function.key.set", json!({ "nodeId": BOB, "channel": "rotation", "frame": 12, "value": 3 })),
        ("anim.function.key.remove", json!({ "nodeId": BOB, "channel": "rotation", "frames": [12] })),
        ("anim.function.key.duplicate", json!({ "nodeId": BOB, "channel": "rotation", "frame": 12, "toFrame": 13 })),
        ("anim.path.point.add", json!({ "nodeId": BOB, "kind": "key", "frame": 6 })),
        ("anim.camera.set", json!({ "cameraId": CAMERA, "fov": 60 })),
        ("anim.camera.set", json!({ "cameraId": CAMERA, "fovMode": "dynamic", "fov": 50, "frame": 1 })),
        ("doc.object.patch", json!({ "targets": [PEG], "props": { "transform": { "skew": 4 } } })),
    ];
    for (id, params) in actions {
        let h = fixture();
        let direct = changed(&h, id, params.clone());
        assert_eq!(direct.len(), 1, "{id} {params}: one event per undo step");
        replays_match(&h, &direct, &format!("{id} {params}"));
        // And again: the step keeps its payload through repeated replays.
        replays_match(&h, &direct, &format!("{id} {params}, second time"));
    }
}

#[test]
fn replayed_scopes_are_the_original_intervals_and_all_frame_forms() {
    let h = fixture();
    // Key add inside Bob's rotation keys 1, 12, 24: frames 12-24, on undo and redo too.
    let add = changed(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "frame": 18, "rotation": 37 }));
    assert_eq!(frames(&add[0]), Some((12.0, 24.0)));
    replays_match(&h, &add, "key add");
    // Move, duplicate and remove of that key, each with its own interval.
    let mv = changed(
        &h,
        "anim.key.move",
        json!({ "nodeId": BOB, "fromFrame": 18, "toFrame": 19, "channels": ["rotation"] }),
    );
    assert_eq!(frames(&mv[0]), Some((12.0, 24.0)));
    replays_match(&h, &mv, "key move");
    let dup = changed(
        &h,
        "anim.function.key.duplicate",
        json!({ "nodeId": BOB, "channel": "rotation", "frame": 19, "toFrame": 20 }),
    );
    assert_eq!(frames(&dup[0]), Some((19.0, 24.0)));
    replays_match(&h, &dup, "key duplicate");
    let rm = changed(&h, "anim.key.remove", json!({ "nodeIds": [BOB], "frame": 20, "channels": ["rotation"] }));
    assert_eq!(frames(&rm[0]), Some((19.0, 24.0)));
    replays_match(&h, &rm, "key remove");
    let seg = changed(
        &h,
        "anim.segment.set",
        json!({ "nodeIds": [BOB], "frame": 19, "channels": ["rotation"], "constant": true }),
    );
    assert_eq!(frames(&seg[0]), Some((12.0, 24.0)));
    replays_match(&h, &seg, "segment");
    // Statics cover every frame, on replay too.
    let stat = changed(&h, "anim.transform.set", json!({ "nodeIds": [PEG], "target": "static", "skew": 3 }));
    assert_eq!(frames(&stat[0]), None);
    replays_match(&h, &stat, "static");
    let cam = changed(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 70, "roll": 4 }));
    assert_eq!(frames(&cam[0]), None);
    replays_match(&h, &cam, "camera");
    // An atomic batch of a static and a keyed change: one all-frame event, replayed as one.
    let both = changed(
        &h,
        "batch",
        json!({ "label": "both", "atomic": true, "steps": [
            { "id": "anim.transform.set", "params": { "nodeIds": [BOB], "frame": 18, "rotation": 5 } },
            { "id": "anim.transform.set", "params": { "nodeIds": [PEG], "target": "static", "rotation": 9 } } ] }),
    );
    assert_eq!(both.len(), 1);
    assert_eq!(frames(&both[0]), None);
    replays_match(&h, &both, "batch");
}

#[test]
fn steps_without_animation_replay_no_animation_event_and_direct_edits_are_not_doubled() {
    let h = fixture();
    let rx = h.subscribe();
    ok(&h, "element.rename", json!({ "elementId": PEG, "name": "Bob-Peg" }));
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.redo", json!({}));
    assert!(!rx.try_iter().any(|e| e.name == "anim.changed"), "no animation changed");
    // Three edits, three undos, three redos: nine events, one each.
    let rx = h.subscribe();
    for f in [16, 17, 18] {
        ok(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "frame": f, "rotation": f }));
    }
    for _ in 0..3 {
        ok(&h, "edit.undo", json!({}));
    }
    for _ in 0..3 {
        ok(&h, "edit.redo", json!({}));
    }
    let names: Vec<_> = rx.try_iter().map(|e| e.name).collect();
    assert_eq!(names.iter().filter(|n| *n == "anim.changed").count(), 9, "{names:?}");
    // Each replay's event follows its `doc.changed`, as a direct edit's does.
    let mut last_doc = None;
    for (i, n) in names.iter().enumerate() {
        if n == "doc.changed" {
            last_doc = Some(i);
        }
        if n == "anim.changed" {
            assert!(last_doc.is_some_and(|d| d < i), "{names:?}");
        }
    }
}

#[test]
fn replay_survives_save_and_a_reopened_project_records_new_steps() {
    let h = fixture();
    let add = changed(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "frame": 18, "rotation": 37 }));
    let dir = tmp();
    let out = dir.join("replay.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": out }));
    // Saving keeps the history: undo and redo still announce the step.
    replays_match(&h, &add, "after save");
    // A reopened project starts a fresh history (01 §6.1); its new steps replay their events.
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    job_ok(&h, "project.open", json!({ "path": out }));
    let mv = changed(
        &h,
        "anim.key.move",
        json!({ "nodeId": BOB, "fromFrame": 18, "toFrame": 19, "channels": ["rotation"] }),
    );
    assert_eq!(frames(&mv[0]), Some((12.0, 24.0)));
    replays_match(&h, &mv, "after open");
    let cam = changed(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 30 }));
    replays_match(&h, &cam, "camera after open");
}
