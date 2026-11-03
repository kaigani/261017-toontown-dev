//! M2 C2.3 round 3 (C23-R3-01): a call whose caller timed out is refused at the acceptance boundary whatever it
//! changes, and the session state it had changed is restored in full. Controls beside the reviewer's proving test
//! (`c23_r3_selection_cancel`): a previous non-empty selection, the point selection, the current frame and the mode
//! survive a cancelled selection, alone and inside an atomic batch; the same work is accepted afterwards.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::Duration;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle};

const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

/// The seed-1 fixture with `n` simple disjoint pencil strokes in one layer of `D` (selecting them all crosses a
/// 200 ms budget through real work), under `budget`.
fn crowded(n: usize, budget: Duration) -> ExecutorHandle {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let index = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut drawing = serde_json::to_value(&parts.drawings[index]).unwrap();
    let template = drawing["nodes"][0]["strokes"][1].clone();
    let mut layer = drawing["nodes"][0].clone();
    layer["strokes"] = json!(
        (0..n)
            .map(|i| {
                let mut s = template.clone();
                s["id"] = json!(format!("st_round3_{i}"));
                s["points"] = json!([[i * 3, 2000, 0, 0, 0, 0], [i * 3 + 1, 2001, 0, 0, 0, 0]]);
                s
            })
            .collect::<Vec<_>>()
    );
    drawing["nodes"] = json!([layer]);
    parts.drawings[index] = tt_model::object::from_json(&drawing).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let h = ExecutorHandle::spawn(ExecutorConfig { budget }).unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    h
}

/// The session state a cancelled call must not change: the app state (selection, frame, mode, element), the drawing
/// selection with the point selection, and the revision. Retried until the worker answers.
fn session(h: &ExecutorHandle) -> Value {
    for _ in 0..100 {
        let a = run(h, "app.state.get", json!({}));
        let s = run(h, "draw.selection.get", json!({ "drawingId": D }));
        if let (Some(a), Some(s)) = (a.result, s.result) {
            return json!({ "app": a, "points": s["selectedPoints"], "ids": s["selectedIds"], "rev": rev(h) });
        }
    }
    panic!("the worker did not answer");
}

fn prepare(h: &ExecutorHandle) {
    ok(h, "draw.selection.set", json!({ "drawingId": D, "ids": ["st_round3_7", "st_round3_8"] }));
    ok(h, "draw.pointSelection.set", json!({ "drawingId": D, "points": [{ "strokeId": "st_round3_7", "anchor": 1 }] }));
    ok(h, "app.frame.set", json!({ "frame": 2 }));
    ok(h, "app.mode.set", json!({ "mode": "drawing" }));
}

#[test]
fn a_cancelled_selection_keeps_the_previous_selection_points_frame_and_mode() {
    let h = crowded(3000, Duration::from_millis(200));
    prepare(&h);
    let before = session(&h);
    assert_eq!(before["ids"], json!(["st_round3_7", "st_round3_8"]));
    assert_eq!(before["points"].as_array().map(Vec::len), Some(1));
    for mode in ["replace", "add", "toggle"] {
        let e = run(&h, "draw.selection.set", json!({ "drawingId": D, "by": "all", "mode": mode }));
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{mode}: {e:?}");
        assert_eq!(session(&h), before, "{mode}: the cancelled selection changed nothing");
    }
    // A slow call that ends in a different mode and frame inside one atomic batch: refused as a whole.
    let e = batch(
        &h,
        json!({ "label": "r3", "atomic": true, "steps": [
            {"id": "app.frame.set", "params": {"frame": 5}},
            {"id": "draw.pointSelection.set", "params": {"drawingId": D, "points": []}},
            {"id": "draw.selection.set", "params": {"drawingId": D, "by": "all"}},
        ] }),
    );
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{e:?}");
    assert_eq!(session(&h), before, "the cancelled batch changed nothing");
}

#[test]
fn the_same_selection_is_accepted_within_its_budget() {
    let h = crowded(3000, Duration::from_secs(60));
    prepare(&h);
    let r = ok(&h, "draw.selection.set", json!({ "drawingId": D, "by": "all" }));
    assert_eq!(r["selectedIds"].as_array().map(Vec::len), Some(3000));
    let after = session(&h);
    assert_eq!(after["ids"].as_array().map(Vec::len), Some(3000));
    assert_eq!(after["points"].as_array().map(Vec::len), Some(1), "the point selection is kept");
}
