#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;

#[test]
fn a2_clamp_events_and_history() {
    let h = fixture();
    let len = ok(&h, "timeline.rows.list", json!({}))["sceneLength"].as_i64().unwrap();
    let history = ok(&h, "edit.history.get", json!({}));
    for frame in [0, -1, len, len + 1, 9999] {
        ok(&h, "timeline.currentFrame.set", json!({"frame": 2}));
        let rx = h.subscribe();
        let r = ok(&h, "timeline.currentFrame.set", json!({"frame":frame}));
        assert_eq!(r["frame"], frame.clamp(1, len), "04:1438");
        assert_eq!(rx.try_iter().filter(|e| e.name == "timeline.currentFrame.changed").count(), 1);
    }
    assert_eq!(ok(&h, "edit.history.get", json!({})), history, "04:1309");
}

#[test]
fn a3_alternating_frame_writers_and_explicit_timing_target() {
    let h = fixture();
    for (i, c) in
        ["app.frame.set", "timeline.currentFrame.set", "app.frame.set", "timeline.currentFrame.set"].iter().enumerate()
    {
        let f = i + 3;
        let rx = h.subscribe();
        ok(&h, c, json!({"frame":f}));
        assert_eq!(ok(&h, "timeline.rows.list", json!({}))["currentFrame"], f);
        assert_eq!(rx.try_iter().filter(|e| e.name == "timeline.currentFrame.changed").count(), 1, "SI-202");
    }
    let sel = ok(&h, "cell.selection.get", json!({}));
    let frame = ok(&h, "timeline.rows.list", json!({}))["currentFrame"].clone();
    ok(&h, "element.timing.set", json!({"elementIds":["scene:\"Intro\"/element:\"Bob-P\""],"startDelta":2}));
    assert_eq!(ok(&h, "cell.selection.get", json!({})), sel, "04:1268");
    assert_eq!(ok(&h, "timeline.rows.list", json!({}))["currentFrame"], frame);
}

#[test]
fn a4_reversed_window_must_not_kill_executor() {
    let h = fixture();
    let r = run(&h, "timeline.rows.list", json!({"from":20,"to":3}));
    assert!(r.is_ok() || r.error.is_some(), "04:1306 structured refusal: {r:?}");
    ok(&h, "timeline.rows.list", json!({}));
}

#[test]
fn a4_labels_and_window_keys() {
    let h = fixture();
    ok(&h, "timeline.timeDisplay.set", json!({"unit":"feetFrames"}));
    assert_eq!(
        ok(&h, "timeline.frameLabel.get", json!({"frames":[1,16,17,20,9999]}))["labels"],
        json!(["0+01", "1+00", "1+01", "1+04", "624+15"]),
        "04:644"
    );
    let rows = ok(&h, "timeline.rows.list", json!({"from":10,"to":20}));
    for row in rows["rows"].as_array().unwrap() {
        for k in row["keyframes"].as_array().unwrap() {
            assert!((10..=20).contains(&k["frame"].as_u64().unwrap()));
        }
    }
}

#[test]
fn a5_partial_invalid_and_operation_isolation() {
    let h = fixture();
    let read = |op| ok(&h, "timeline.transferOptions.get", json!({"operation":op}));
    let cut = read("cut");
    let drag = read("dragDrop");
    ok(&h, "prefs.set", json!({"values":{"edit.pasteSpecial.timeline":{"overwriteDrawings":true}}}));
    assert_eq!(read("paste")["options"]["overwriteDrawings"], true, "E-19");
    assert_eq!(read("paste")["options"]["updateColours"], false);
    let before = read("paste");
    assert!(
        run(
            &h,
            "timeline.transferOptions.set",
            json!({"operation":"paste","options":{"updateColours":true,"bogus":true}})
        )
        .error
        .is_some()
    );
    assert_eq!(read("paste"), before, "04:1306 atomic refusal");
    assert_eq!(read("cut"), cut);
    assert_eq!(read("dragDrop"), drag);
    ok(&h, "prefs.reset", json!({"keys":["edit.pasteSpecial.timeline"]}));
    assert_eq!(read("paste")["options"]["overwriteDrawings"], false);
}
