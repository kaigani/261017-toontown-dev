//! M5 C5.1a round 1 (builder): the fixes of rulings E-21 to E-27 (`reviews/M5-C5.1a-r1-response.md`).
//! - E-21: Previous / Next Drawing at either end of the scene or the range, from outside the range, with `count` above
//!   1, through both commands and the key: nothing happens (09 line 135). What a handler panic does today:
//!   `c51_handler_panic.rs` (its own binary: the hook it sets is process-wide).
//! - E-22: every navigation form stops playback even when the frame does not change (09 line 138).
//! - E-23: a gesture commits only when its outcome in absolute values is what the preview showed.
//! - E-24: a range change on a scene that is not current raises its events.
//! - E-26: a reversed window is refused as `xsheet.exposures.get` refuses it.
//! - E-27: `timeline.currentFrame.set` clamps any integer (09 line 139); `app.frame.set` keeps its stated range.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c33_support;
mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const OTHER: &str = "scene:#2";

fn len(h: &ExecutorHandle) -> i64 {
    ok(h, "timeline.rows.list", json!({}))["sceneLength"].as_i64().unwrap()
}

fn frame(h: &ExecutorHandle) -> i64 {
    ok(h, "cell.selection.get", json!({}))["currentFrame"].as_i64().unwrap()
}

/// The frames at which Bob's exposed drawing changes (the marks Previous / Next Drawing visit): each run's start, and
/// the frame after a run where the cell turns blank.
fn bob_marks(h: &ExecutorHandle) -> Vec<i64> {
    let n = len(h);
    let sheet = ok(h, "xsheet.exposures.get", json!({"elementIds": [BOB]}));
    let runs = sheet["columns"][0]["exposures"].as_array().unwrap().clone();
    let mut marks: Vec<i64> = runs.iter().map(|r| r["from"].as_i64().unwrap()).collect();
    for r in &runs {
        let after = r["to"].as_i64().unwrap() + 1;
        if after <= n && !runs.iter().any(|x| x["from"].as_i64() == Some(after)) {
            marks.push(after);
        }
    }
    marks.sort_unstable();
    marks
}

// ----- E-21 ------------------------------------------------------------------------------------------------------------

fn next_drawing(h: &ExecutorHandle, count: i64, within: bool) -> Value {
    ok(
        h,
        "timeline.currentFrame.move",
        json!({"direction": "next", "unit": "drawing", "count": count, "withinRange": within, "elementId": BOB}),
    )
}

fn previous_drawing(h: &ExecutorHandle, count: i64, within: bool) -> Value {
    ok(
        h,
        "timeline.currentFrame.move",
        json!({"direction": "previous", "unit": "drawing", "count": count, "withinRange": within, "elementId": BOB}),
    )
}

#[test]
fn e21_drawing_steps_at_either_end_do_nothing() {
    let h = fixture();
    let n = len(&h);
    assert!(!bob_marks(&h).is_empty(), "the fixture's drawing has exposures");
    // The scene's last frame: Next Drawing does nothing, for one step and for several (09 line 135).
    for count in [1, 3] {
        ok(&h, "timeline.currentFrame.set", json!({"frame": n}));
        assert_eq!(next_drawing(&h, count, false), json!({"frame": n, "moved": false}), "count {count}");
        ok(&h, "timeline.currentFrame.set", json!({"frame": 1}));
        assert_eq!(previous_drawing(&h, count, false), json!({"frame": 1, "moved": false}), "count {count}");
    }
    // Inside the scene, several steps visit several changes and stop at the last one (never wrap).
    let marks = bob_marks(&h);
    ok(&h, "timeline.currentFrame.set", json!({"frame": 1}));
    let r = next_drawing(&h, 100_000, false);
    let last_mark = *marks.iter().max().unwrap();
    assert_eq!(r["frame"], last_mark.max(1));
    // The read answers after every call: the document thread is alive.
    ok(&h, "timeline.rows.list", json!({}));
}

#[test]
fn e21_drawing_steps_outside_the_playback_range_do_nothing() {
    let h = fixture();
    let n = len(&h);
    assert!(n >= 12, "the fixture's scene is long enough for a range inside it");
    ok(&h, "timeline.playbackRange.set", json!({"start": 4, "end": n - 4}));
    // Before the range: Previous Drawing within the range has nothing before (the panic of `lo..f` with f < lo).
    ok(&h, "timeline.currentFrame.set", json!({"frame": 2}));
    for count in [1, 2] {
        assert_eq!(previous_drawing(&h, count, true), json!({"frame": 2, "moved": false}));
    }
    // After the range: Next Drawing within the range has nothing after (the panic of `f + 1..=hi` with f >= hi).
    ok(&h, "timeline.currentFrame.set", json!({"frame": n - 1}));
    for count in [1, 2] {
        assert_eq!(next_drawing(&h, count, true), json!({"frame": n - 1, "moved": false}));
    }
    // At the range's ends.
    ok(&h, "timeline.currentFrame.set", json!({"frame": n - 4}));
    assert_eq!(next_drawing(&h, 1, true), json!({"frame": n - 4, "moved": false}));
    ok(&h, "timeline.currentFrame.set", json!({"frame": 4}));
    assert_eq!(previous_drawing(&h, 1, true), json!({"frame": 4, "moved": false}));
    ok(&h, "timeline.rows.list", json!({}));
}

#[test]
fn e21_the_shared_rule_through_app_frame_move_and_its_key() {
    let h = fixture();
    let n = len(&h);
    ok(&h, "xsheet.currentCell.set", json!({"elementId": BOB, "frame": n}));
    for count in [1, 2] {
        let r = ok(&h, "app.frame.move", json!({"direction": "next", "unit": "drawing", "count": count}));
        assert_eq!(r["frame"], n, "count {count}");
    }
    ok(&h, "app.frame.set", json!({"frame": 1}));
    let r = ok(&h, "app.frame.move", json!({"direction": "previous", "unit": "drawing", "count": 2}));
    assert_eq!(r["frame"], 1);
    // The key (Shift+. is Next Drawing, `keymap-default.json`): resolved through the keymap and run as the window runs it.
    ok(&h, "app.frame.set", json!({"frame": n}));
    let resolved = serde_json::to_value(h.resolve_key("Shift+.").unwrap()).unwrap();
    assert_eq!(resolved["commandId"], "app.frame.move", "{resolved}");
    let r = ok(&h, "app.frame.move", resolved["params"].clone());
    assert_eq!(r["frame"], n);
    assert_eq!(frame(&h), n);
}

// ----- E-22 ------------------------------------------------------------------------------------------------------------

/// Playback running at frame `f` without moving: a one-frame free range with Loop on.
fn playing_at(h: &ExecutorHandle, f: i64) {
    ok(h, "timeline.playbackRange.set", json!({"start": f, "end": f}));
    ok(h, "playback.loop.set", json!({"enabled": true}));
    ok(h, "timeline.currentFrame.set", json!({"frame": f}));
    ok(h, "playback.start", json!({}));
    assert_eq!(ok(h, "playback.state.get", json!({}))["playing"], true);
}

#[test]
fn e22_every_navigation_form_stops_playback_even_without_moving() {
    let h = fixture();
    let n = len(&h);
    let forms: Vec<(i64, &str, Value)> = vec![
        (5, "timeline.currentFrame.set", json!({"frame": 5})),
        (5, "timeline.currentFrame.move", json!({"direction": "first", "withinRange": true})),
        (5, "timeline.currentFrame.move", json!({"direction": "last", "withinRange": true})),
        (5, "timeline.currentFrame.move", json!({"direction": "next", "withinRange": true})),
        (5, "timeline.currentFrame.move", json!({"direction": "previous", "withinRange": true})),
        (n, "timeline.currentFrame.move", json!({"direction": "next", "unit": "drawing", "elementId": BOB})),
        (1, "timeline.currentFrame.move", json!({"direction": "previous", "unit": "drawing", "elementId": BOB})),
        (5, "app.frame.set", json!({"frame": 5})),
        (n, "app.frame.move", json!({"direction": "next"})),
        (1, "app.frame.move", json!({"direction": "previous"})),
        (n, "app.frame.move", json!({"direction": "next", "unit": "drawing"})),
        (5, "xsheet.currentCell.set", json!({"elementId": BOB, "frame": 5})),
    ];
    for (at, command, params) in forms {
        playing_at(&h, at);
        let events = h.subscribe();
        let r = ok(&h, command, params.clone());
        let stopped: Vec<_> = events.try_iter().filter(|e| e.name == "playback.stopped").collect();
        let state = ok(&h, "playback.state.get", json!({}));
        assert_eq!(state["playing"], false, "{command} {params}: {r}");
        assert_eq!(frame(&h), at, "{command} {params}: the frame did not change");
        assert_eq!(stopped.len(), 1, "{command} {params}");
        assert_eq!(stopped[0].payload["reason"], "navigation", "{command} {params}");
    }
}

// ----- E-23 ------------------------------------------------------------------------------------------------------------

fn visible_row(h: &ExecutorHandle, name: &str) -> usize {
    ok(h, "timeline.rows.list", json!({}))["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["shownInTimeline"] == true)
        .position(|r| r["name"] == name)
        .unwrap()
}

fn layout() -> Value {
    json!({"surface": "tracks", "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"})
}

fn at(r: usize, x: f64, upper: bool) -> Value {
    json!({"x": x, "y": r as f64 * 22.0 + if upper { 5.0 } else { 16.0 }, "modifiers": []})
}

/// Press, a change while the drag is in progress, release: the release must cancel and change nothing more.
fn cancelled_by(h: &ExecutorHandle, samples: Value, change: &dyn Fn(&ExecutorHandle)) {
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout(), "sample": samples[0].clone()}))
        .unwrap();
    change(h);
    let before = rev(h);
    let rows_before = ok(h, "timeline.rows.list", json!({}));
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["cancelled"], true, "{out}");
    assert_eq!(out["error"]["code"], "timeline.targetChanged");
    assert_eq!(rev(h), before, "nothing ran");
    assert_eq!(ok(h, "timeline.rows.list", json!({})), rows_before);
}

/// The reviewer's input (C51A-R1S-02): equal relative commands (`startDelta: 2`) before and after a concurrent change
/// of the bar's start resolve to different starts.
#[test]
fn e23_the_reviewers_bar_body_input_cancels() {
    let h = fixture();
    ok(&h, "element.timing.set", json!({"elementIds": [PEG], "start": 1, "duration": 50}));
    ok(&h, "element.select", json!({"elementIds": [PEG], "mode": "replace"}));
    let r = visible_row(&h, "Bob-P");
    let samples = json!([at(r, 160.0, false), at(r, 192.0, false)]);
    // Without the change the drag commits start 3 (the preview).
    let g = fixture();
    ok(&g, "element.timing.set", json!({"elementIds": [PEG], "start": 1, "duration": 50}));
    ok(&g, "element.select", json!({"elementIds": [PEG], "mode": "replace"}));
    let press =
        g.timing_gesture(Origin::User, json!({"phase": "press", "layout": layout(), "sample": samples[0]})).unwrap();
    g.timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples.clone()}))
        .unwrap();
    assert_eq!(ok(&g, "element.get", json!({"elementId": PEG}))["start"], 3);
    // With the start set to 2 meanwhile: the same command would give start 4; cancelled.
    cancelled_by(&h, samples, &|h| {
        ok(h, "element.timing.set", json!({"elementIds": [PEG], "start": 2}));
    });
    assert_eq!(ok(&h, "element.get", json!({"elementId": PEG}))["start"], 2);
}

#[test]
fn e23_one_cancellation_per_gesture_kind() {
    // Bar end (duration, anchor start): the loops change meanwhile; the command is the same, the outcome is not.
    let h = fixture();
    ok(&h, "element.timing.set", json!({"elementIds": [PEG], "start": 1, "duration": 20}));
    let r = visible_row(&h, "Bob-P");
    let end_x = 20.0 * 16.0;
    cancelled_by(&h, json!([at(r, end_x, false), at(r, end_x + 48.0, false)]), &|h| {
        ok(h, "element.timing.set", json!({"elementIds": [PEG], "loops": 2}));
    });
    // Bar start (anchor end): a keyframe added meanwhile (the keys are scaled with the bar).
    let h = fixture();
    ok(&h, "element.timing.set", json!({"elementIds": [PEG], "start": 5, "duration": 20}));
    let r = visible_row(&h, "Bob-P");
    let start_x = 4.0 * 16.0;
    cancelled_by(&h, json!([at(r, start_x, false), at(r, start_x + 32.0, false)]), &|h| {
        ok(h, "anim.key.add", json!({"nodeIds": [PEG], "frame": 15}));
    });
    // Loop handle: a keyframe added meanwhile.
    let h = fixture();
    ok(&h, "element.timing.set", json!({"elementIds": [PEG], "start": 1, "duration": 10}));
    let r = visible_row(&h, "Bob-P");
    cancelled_by(&h, json!([at(r, 160.0, true), at(r, 330.0, true)]), &|h| {
        ok(h, "anim.key.add", json!({"nodeIds": [PEG], "frame": 3}));
    });
    // Keyframe move: another key of the element moved meanwhile.
    let h = fixture();
    ok(&h, "element.timing.set", json!({"elementIds": [PEG], "start": 1, "duration": 40}));
    ok(&h, "anim.key.add", json!({"nodeIds": [PEG], "frame": 10}));
    ok(&h, "anim.key.add", json!({"nodeIds": [PEG], "frame": 20}));
    let r = visible_row(&h, "Bob-P");
    cancelled_by(&h, json!([at(r, 9.0 * 16.0 + 8.0, false), at(r, 12.0 * 16.0 + 8.0, false)]), &|h| {
        ok(h, "anim.key.move", json!({"nodeId": PEG, "fromFrame": 20, "toFrame": 30}));
    });
    // Cell move: a note added to a grabbed cell meanwhile (the content grabbed, E-28).
    let h = fixture();
    ok(&h, "cell.select", json!({"elementIds": [BOB], "from": 1, "to": 2}));
    let r = visible_row(&h, "Bob");
    cancelled_by(&h, json!([at(r, 8.0, false), at(r, 29.0 * 16.0 + 8.0, false)]), &|h| {
        ok(h, "cell.note.set", json!({"elementId": BOB, "frame": 2, "text": "added under the drag"}));
    });
    // Row drop onto a peg (attach): the peg gets another child meanwhile, so the children after the drop differ.
    let h = fixture();
    ok(&h, "element.create", json!({"type": "drawing", "name": "Mover"}));
    ok(&h, "element.create", json!({"type": "drawing", "name": "Sibling"}));
    let (mover, peg) = (visible_row(&h, "Mover"), visible_row(&h, "Bob-P"));
    let layers = |r: usize, frac: f64| json!({"x": 40.0, "y": (r as f64 + frac) * 22.0, "modifiers": []});
    let press = h
        .timing_gesture(
            Origin::User,
            json!({"phase": "press", "layout": {"surface": "layers", "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"}, "sample": layers(mover, 0.5)}),
        )
        .unwrap();
    let sibling = ok(&h, "timeline.rows.list", json!({}))["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "Sibling")
        .unwrap()["elementId"]
        .clone();
    ok(&h, "element.parent.set", json!({"elementIds": [sibling], "parentId": PEG}));
    let before = rev(&h);
    let out = h
        .timing_gesture(
            Origin::User,
            json!({"phase": "release", "grabId": press["grabId"], "samples": [layers(mover, 0.5), layers(peg, 0.5)]}),
        )
        .unwrap();
    assert_eq!(out["error"]["code"], "timeline.targetChanged", "{out}");
    assert_eq!(rev(&h), before);
}

/// Ruling E-28 (C51A-R1-04): a row inserted above the grabbed bar does not cancel: the target is the id resolved at
/// the press, and its timing is unchanged.
#[test]
fn e28_targets_are_fixed_by_id_at_the_press() {
    let h = fixture();
    ok(&h, "element.timing.set", json!({"elementIds": [PEG], "start": 1, "duration": 30}));
    let r = visible_row(&h, "Bob-P");
    let samples = json!([at(r, 152.0, false), at(r, 200.0, false)]);
    let press =
        h.timing_gesture(Origin::User, json!({"phase": "press", "layout": layout(), "sample": samples[0]})).unwrap();
    ok(&h, "element.create", json!({"type": "peg", "name": "Inserted"}));
    let rows = ok(&h, "timeline.rows.list", json!({}))["rows"].as_array().unwrap().clone();
    let new = rows.iter().find(|x| x["name"] == "Inserted").unwrap()["elementId"].clone();
    let mut root: Vec<Value> =
        rows.iter().filter(|x| x["depth"] == 0 && x["elementId"] != new).map(|x| x["elementId"].clone()).collect();
    root.insert(0, new);
    ok(&h, "element.order.set", json!({"order": "timeline", "elementIds": root}));
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert!(out.get("cancelled").is_none(), "{out}");
    assert_eq!(ok(&h, "element.get", json!({"elementId": PEG}))["start"], 4);
}

// ----- E-24 ------------------------------------------------------------------------------------------------------------

#[test]
fn e24_a_range_change_on_another_scene_raises_its_events_without_navigating() {
    let h = fixture();
    let sel = ok(&h, "cell.selection.get", json!({}));
    let rows = ok(&h, "timeline.rows.list", json!({}));
    let events = h.subscribe();
    let r = ok(&h, "playback.range.set", json!({"sceneId": OTHER, "start": 2, "end": 3}));
    let got: Vec<_> = events.try_iter().collect();
    let pr: Vec<_> = got.iter().filter(|e| e.name == "playback.range.changed").collect();
    assert_eq!(pr.len(), 1, "{got:?}");
    assert_eq!(pr[0].payload["mode"], "free");
    assert_eq!((pr[0].payload["start"].clone(), pr[0].payload["end"].clone()), (r["start"].clone(), r["end"].clone()));
    assert_ne!(pr[0].payload["sceneId"], rows["sceneId"], "the scene named, not the current one");
    let tl: Vec<_> = got.iter().filter(|e| e.name == "timeline.playbackRange.changed").collect();
    assert_eq!(tl.len(), 1);
    assert_eq!(tl[0].payload, json!({"mode": "free", "start": r["start"], "end": r["end"]}));
    assert!(got.iter().all(|e| e.name != "timeline.currentFrame.changed" && e.name != "cell.selection.changed"));
    assert_eq!(ok(&h, "cell.selection.get", json!({})), sel);
    assert_eq!(ok(&h, "timeline.rows.list", json!({}))["sceneId"], rows["sceneId"]);
    // The mode form too; an unchanged range raises nothing.
    let events = h.subscribe();
    ok(&h, "playback.range.mode.set", json!({"sceneId": OTHER, "mode": "auto"}));
    assert_eq!(events.try_iter().filter(|e| e.name == "playback.range.changed").count(), 1);
    let events = h.subscribe();
    ok(&h, "playback.range.mode.set", json!({"sceneId": OTHER, "mode": "auto"}));
    assert_eq!(events.try_iter().filter(|e| e.name.contains("ange.changed")).count(), 0);
}

// ----- E-26 and E-27 -----------------------------------------------------------------------------------------------------

#[test]
fn e26_a_reversed_window_is_refused_as_the_exposure_sheet_refuses_it() {
    let h = fixture();
    let a = err(&h, "timeline.rows.list", json!({"from": 20, "to": 3}));
    let b = err(&h, "xsheet.exposures.get", json!({"from": 20, "to": 3}));
    assert_eq!((a.code.as_str(), a.details["pointer"].clone()), ("arg.invalid", json!("/to")));
    assert_eq!((a.code.clone(), a.details["pointer"].clone()), (b.code, b.details["pointer"].clone()));
    // A window past the scene end with `to` omitted is empty, not refused (the default `to` is at least `from`).
    let n = len(&h);
    let r = ok(&h, "timeline.rows.list", json!({"from": n + 50}));
    assert!(r["rows"].as_array().unwrap().iter().all(|row| row["keyframes"].as_array().unwrap().is_empty()));
    ok(&h, "timeline.rows.list", json!({}));
}

#[test]
fn e27_the_timeline_frame_clamps_any_integer_and_app_frame_set_keeps_its_range() {
    let h = fixture();
    let n = len(&h);
    for (asked, got) in [(0, 1), (-5, 1), (i64::from(i32::MIN), 1), (n + 1, n), (i64::from(i32::MAX), n)] {
        let r = ok(&h, "timeline.currentFrame.set", json!({"frame": asked}));
        assert_eq!(r, json!({"frame": got, "clamped": true}), "{asked}");
    }
    // `app.frame.set`: spec A line 57 states its range `1–scene length`; refused outside it (M1 builder tests pin it).
    assert_eq!(err(&h, "app.frame.set", json!({"frame": 0})).code, "arg.invalid");
    assert_eq!(err(&h, "app.frame.set", json!({"frame": n + 1})).code, "arg.invalid");
    // The other schemas the round's observation named: accepted and clamped.
    ok(&h, "timeline.currentFrame.set", json!({"frame": 3}));
    let r = ok(&h, "timeline.currentFrame.move", json!({"direction": "next", "count": 0}));
    assert_eq!(r, json!({"frame": 4, "moved": true}), "count below 1 counts as 1");
    ok(&h, "timeline.currentFrame.move", json!({"direction": "next", "count": -7}));
    ok(&h, "timeline.view.set", json!({"firstFrame": -20}));
    assert_eq!(ok(&h, "timeline.view.set", json!({}))["firstFrame"], 1);
    let r = ok(&h, "playback.start", json!({"fromFrame": -3}));
    assert_eq!(r["frame"], 1);
    ok(&h, "playback.stop", json!({}));
}

/// A fixture's preparation.
type Setup = dyn Fn(&ExecutorHandle);

// ----- E-25 and the E-5 extension -----------------------------------------------------------------------------------------

/// Every member of an option set (01 §6.3's values), with `flip` members changed.
fn members(flip: &[(&str, bool)]) -> Value {
    let mut m = json!({
        "overwriteDrawings": true, "createDeleteExposure": true, "extendExposures": false, "updateColours": true,
        "keyframeSubstitution": false, "motionKeys": true, "rotationKeys": true, "scaleKeys": true, "pivotKeys": true,
        "velocityKeys": true, "skewKeys": true, "boundaryKeys": false
    });
    for (k, v) in flip {
        m[*k] = json!(v);
    }
    m
}

/// The last frame of Bob's exposure.
fn bob_end(h: &ExecutorHandle) -> i64 {
    let sheet = ok(h, "xsheet.exposures.get", json!({"elementIds": [BOB]}));
    sheet["columns"][0]["exposures"].as_array().unwrap().iter().map(|r| r["to"].as_i64().unwrap()).max().unwrap()
}

/// The state after `op` with every member of both sets stored as `stored`, from Bob's cells 1-2 (pasted at `dest`
/// frames past Bob's last exposure) in `context`; `setup` prepares the fixture first.
fn after(op: (&str, Value), stored: &Value, context: &str, dest: i64, setup: &dyn Fn(&ExecutorHandle)) -> Value {
    let h = fixture();
    setup(&h);
    ok(&h, "prefs.set", json!({"values": {"edit.cutSpecial.timeline": stored, "edit.pasteSpecial.timeline": stored}}));
    ok(&h, "window.panel.set", json!({"panelId": context, "visible": true, "focus": true}));
    ok(&h, "cell.select", json!({"elementIds": [BOB], "from": 1, "to": 2}));
    ok(&h, "xsheet.currentCell.set", json!({"elementId": BOB, "frame": 1}));
    if op.0.contains("paste") || op.0 == "cell.clipboard.apply" {
        ok(&h, "edit.copy", json!({"context": context}));
        let at = bob_end(&h) + dest;
        ok(&h, "cell.select", json!({"elementIds": [BOB], "from": at, "to": at}));
        ok(&h, "xsheet.currentCell.set", json!({"elementId": BOB, "frame": at}));
    }
    let mut params = op.1.clone();
    if op.0.starts_with("edit.") {
        params["context"] = json!(context);
    }
    let env = run(&h, op.0, params);
    json!({
        "status": format!("{:?}", env.status),
        "error": env.error.as_ref().map(|e| e.code.clone()),
        "sheet": ok(&h, "xsheet.exposures.get", json!({})),
        "clipboard": ok(&h, "edit.clipboard.get", json!({})),
        "selection": ok(&h, "cell.selection.get", json!({})),
        "banks": ok(&h, "element.drawing.list", json!({"elementId": BOB})),
    })
}

/// Ruling E-25 (C51A-R1S-04): a structured paste past the last exposure extends it, as `edit.pasteSpecial` does.
#[test]
fn e25_the_structured_paste_extends_exposure_automatically() {
    let stored = members(&[("extendExposures", true)]);
    let none = |_: &ExecutorHandle| {};
    for context in ["xsheet", "timeline"] {
        let a = after(("edit.pasteSpecial", json!({})), &stored, context, 4, &none);
        let b = after(("cell.clipboard.apply", json!({})), &stored, context, 4, &none);
        assert_eq!(a, b, "{context}");
        assert_eq!(a["status"], "Ok");
    }
    // The gap is held by the last drawing: Bob's last run now reaches the pasted block.
    let h = fixture();
    let end = bob_end(&h);
    let b = after(("cell.clipboard.apply", json!({})), &stored, "xsheet", 4, &none);
    let runs =
        b["sheet"]["columns"].as_array().unwrap().iter().find(|c| c["name"] == "Bob").unwrap()["exposures"].clone();
    let covered: Vec<(i64, i64)> =
        runs.as_array().unwrap().iter().map(|r| (r["from"].as_i64().unwrap(), r["to"].as_i64().unwrap())).collect();
    assert!(
        covered.iter().any(|(a, b)| *a <= end && *b >= end + 3),
        "frames up to the pasted block are held: {covered:?}"
    );
}

/// Ruling E-25: the E-5 equality extended to the members it did not exercise, each flipped from 01's values with every
/// member stored, with a clone group and with a locked drawing.
#[test]
fn e25_the_e5_equality_holds_for_every_member_with_clones_and_locked_drawings() {
    let mut variants: Vec<(String, Value)> = vec![
        ("autoExtend".into(), members(&[("extendExposures", true)])),
        ("keyframeSubstitution".into(), members(&[("keyframeSubstitution", true)])),
        ("boundaryKeys".into(), members(&[("boundaryKeys", true)])),
        ("noExposure".into(), members(&[("createDeleteExposure", false)])),
    ];
    for k in ["motionKeys", "rotationKeys", "scaleKeys", "pivotKeys", "velocityKeys", "skewKeys"] {
        variants.push((format!("{k} off"), members(&[(k, false)])));
    }
    let none = |_: &ExecutorHandle| {};
    let clone = |h: &ExecutorHandle| {
        ok(h, "element.clone", json!({"elementId": BOB}));
    };
    let locked = |h: &ExecutorHandle| {
        let drawings = ok(h, "element.drawing.list", json!({"elementId": BOB}));
        let list = drawings["drawings"].as_array().cloned().unwrap_or_default();
        let ids: Vec<Value> = list.iter().map(|d| d["id"].clone()).collect();
        ok(h, "element.drawingLock.set", json!({"drawingIds": ids, "locked": true}));
    };
    let setups: [(&str, &Setup); 3] = [("plain", &none), ("clone group", &clone), ("locked", &locked)];
    let mut differences = Vec::new();
    for (name, stored) in &variants {
        for (setup_name, setup) in &setups {
            for context in ["xsheet", "timeline"] {
                for dest in [-1, 4] {
                    for (a, b) in [
                        (("edit.pasteSpecial", json!({})), ("cell.clipboard.apply", json!({}))),
                        (("edit.cutSpecial", json!({})), ("cell.clipboard.set", json!({"cut": true}))),
                    ] {
                        let (x, y) = (
                            after(a.clone(), stored, context, dest, *setup),
                            after(b.clone(), stored, context, dest, *setup),
                        );
                        if x != y {
                            let keys: Vec<&str> = ["status", "error", "sheet", "clipboard", "selection", "banks"]
                                .into_iter()
                                .filter(|k| x[*k] != y[*k])
                                .collect();
                            differences.push(format!(
                                "{name} / {setup_name} / {context} / {dest}: {} vs {}: {keys:?} ({} / {})",
                                a.0, b.0, x["error"], y["error"]
                            ));
                        }
                    }
                }
            }
        }
    }
    // One difference, sent to the orchestrator under E-17 item 4 and changed nowhere (the response, E-25): with the
    // exposure option off a paste overwrites the destination cells; `cell.clipboard.apply` refuses when a destination
    // cell exposes a locked drawing (its M3 rule), `edit.pasteSpecial` refuses only when it would overwrite a locked
    // drawing's content (`c12_r3_contracts::fresh_cell_paste_checks_target_type_and_locked_drawing_before_commit`) and
    // here reuses the same drawings, so it pastes.
    let expected: Vec<String> = ["xsheet", "timeline"]
        .iter()
        .map(|c| {
            format!(
                "noExposure / locked / {c} / -1: edit.pasteSpecial vs cell.clipboard.apply: [\"status\", \"error\"] (null / \"drawing.locked\")"
            )
        })
        .collect();
    assert_eq!(differences, expected, "special = structured with every member stored, but for the stated difference");
}

// ----- the `xsheet` composition: pixels and the camera (the observation on SI-107) --------------------------------------

/// The `xsheet` composition at frame 1, decoded.
fn xsheet_image(h: &ExecutorHandle) -> c33_support::Img {
    c33_support::render(h, json!({"composition": "xsheet", "frame": 1, "scale": 0.1}))
}

#[test]
fn the_xsheet_composition_draws_the_front_column_at_the_centre_and_ignores_the_camera() {
    let h = fixture();
    let order = |h: &ExecutorHandle| -> Vec<Value> {
        ok(h, "element.list", json!({"order": "xsheet"}))["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].clone())
            .collect()
    };
    let colours = c33_support::solids(&h, 2);
    let (ca, cb) = (colours[0].1, colours[1].1);
    assert_ne!(ca, cb);
    let a = json!(c33_support::square(&h, "SquareA", 100.0, &colours[0].0));
    let b = json!(c33_support::square(&h, "SquareB", 100.0, &colours[1].0));
    // Only the two squares, B to the right (in front, 04 line 56).
    for e in order(&h) {
        if e != a && e != b {
            ok(&h, "element.visibility.set", json!({"elementIds": [e], "scope": "xsheet", "visible": false}));
        }
    }
    let mut ab: Vec<Value> = order(&h).into_iter().filter(|e| *e != a && *e != b).collect();
    ab.extend([a.clone(), b.clone()]);
    ok(&h, "element.order.set", json!({"order": "xsheet", "elementIds": ab.clone()}));
    // Drawing space maps to the output with the origin at the centre (render.rs `xsheet_pixels`): the centre pixel is the
    // front square's colour; a corner, outside both squares (half-size 100 du), is not.
    let img = xsheet_image(&h);
    let (cx, cy) = (img.w / 2, img.h / 2);
    assert_eq!(img.at(cx, cy), cb, "B in front");
    assert_ne!(img.at(0, 0), cb);
    assert_ne!(img.at(0, 0), ca);
    let mut ba = ab.clone();
    let n = ba.len();
    ba.swap(n - 1, n - 2);
    ok(&h, "element.order.set", json!({"order": "xsheet", "elementIds": ba}));
    assert_eq!(xsheet_image(&h).at(cx, cy), ca, "A in front after the swap");
    ok(&h, "element.visibility.set", json!({"elementIds": [a], "scope": "xsheet", "visible": false}));
    assert_eq!(xsheet_image(&h).at(cx, cy), cb, "A hidden in the Exposure Sheet: B shows");
    // A camera move changes the camera composition and not the `xsheet` one (no camera transform).
    let before_xsheet = xsheet_image(&h).png;
    let camera_render =
        |h: &ExecutorHandle| ok(h, "render.frame.run", json!({"frame": 1, "scale": 0.1}))["sha256"].clone();
    let before_camera = camera_render(&h);
    let camera = ok(&h, "element.list", json!({}))["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "camera")
        .unwrap()["id"]
        .clone();
    ok(&h, "anim.transform.set", json!({"nodeIds": [camera], "target": "static", "position": {"x": 3, "y": 2}}));
    assert_ne!(camera_render(&h), before_camera, "the camera moved");
    assert_eq!(xsheet_image(&h).png, before_xsheet, "the xsheet composition does not follow the camera");
}

// ----- remembered paste-special options across a restart (named for round 2) ---------------------------------------------

/// Rulings E-19 and E-8: `edit.pasteSpecial` with `remember` stores the members it is given in the user's preferences;
/// after a restart on the same user folders both families read them, the members never given keep each family's own
/// default, and the cut and drag-and-drop sets are untouched.
#[test]
fn remembered_paste_special_members_survive_a_restart_and_govern_both_families() {
    use tt_executor::{ExecutorOptions, PersistOptions, UserDirs};
    let root = std::env::temp_dir().join(format!("tt-c51-r1-{}", tt_executor::random_token(8)));
    let dirs = UserDirs::under(&root);
    let spawn = || {
        let h = ExecutorHandle::spawn_with_options(ExecutorOptions {
            persist: PersistOptions { user_dirs: Some(dirs.clone()), ..Default::default() },
            ..Default::default()
        })
        .unwrap();
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        h
    };
    let h = spawn();
    let cut = ok(&h, "timeline.transferOptions.get", json!({"operation": "cut"}));
    let drag = ok(&h, "timeline.transferOptions.get", json!({"operation": "dragDrop"}));
    ok(&h, "cell.select", json!({"elementIds": [BOB], "from": 1, "to": 1}));
    ok(&h, "edit.copy", json!({"context": "xsheet"}));
    let end = bob_end(&h) + 3;
    ok(&h, "cell.select", json!({"elementIds": [BOB], "from": end, "to": end}));
    ok(&h, "xsheet.currentCell.set", json!({"elementId": BOB, "frame": end}));
    // Three members given: overwrite (at the 04 family's value), update colours (at 01's), automatic extension.
    let given = json!({"overwriteDrawings": false, "updateColours": true, "extendExposures": true});
    ok(&h, "edit.pasteSpecial", json!({"context": "xsheet", "options": given, "remember": true}));
    drop(h);
    let h = spawn();
    let paste_04 = ok(&h, "timeline.transferOptions.get", json!({"operation": "paste"}))["options"].clone();
    let paste_01 =
        ok(&h, "prefs.get", json!({"keys": ["edit.pasteSpecial.timeline"]}))["values"]["edit.pasteSpecial.timeline"]
            .clone();
    for (family, set, overwrite, update, extend) in [
        ("04", &paste_04, "overwriteDrawings", "updateColours", "autoExtend"),
        ("01", &paste_01, "overwriteDrawings", "updateColours", "extendExposures"),
    ] {
        assert_eq!(set[overwrite], false, "{family}: {set}");
        assert_eq!(set[update], true, "{family}: {set}");
        assert_eq!(set[extend], true, "{family}: {set}");
    }
    // Members never given keep the defaults both tables agree on.
    assert_eq!(paste_04["exposure"], true);
    assert_eq!(paste_01["createDeleteExposure"], true);
    assert_eq!(ok(&h, "timeline.transferOptions.get", json!({"operation": "cut"})), cut, "cut untouched");
    assert_eq!(ok(&h, "timeline.transferOptions.get", json!({"operation": "dragDrop"})), drag, "drag untouched");
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}
