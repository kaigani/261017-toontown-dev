//! M5 C5.1a (builder): the Timeline's kernel (`reviews/M5-design-timeline-kernel.md`): the current frame, the playback
//! range, the read model, the view, the transfer-option store, the one-state pairs (SI-202, SI-203; rulings E-3, E-4,
//! E-6), the `edit.*` / `cell.*` equivalence (E-5) and the timing-gesture interpreter with its change-under-drag rule
//! (E-7, E-10).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c33_support;
mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";

fn id(h: &ExecutorHandle, r: &str) -> String {
    get(h, r)["id"].as_str().unwrap().to_owned()
}

/// Events delivered after `f`.
fn events_of(h: &ExecutorHandle, f: impl FnOnce()) -> Vec<(String, Value)> {
    let rx = h.subscribe();
    f();
    rx.try_iter().map(|e| (e.name, e.payload)).collect()
}

/// The session and document state a pair of commands is compared on.
fn state(h: &ExecutorHandle) -> Value {
    json!({
        "app": ok(h, "app.state.get", json!({})),
        "cells": ok(h, "cell.selection.get", json!({})),
        "rows": ok(h, "timeline.rows.list", json!({})),
        "range": ok(h, "playback.range.get", json!({})),
        "prefs": ok(h, "prefs.get", json!({"keys": [
            "interface.timeMeasurement", "interface.background.timeline", "timing.timeline.singleCellSelection",
            "edit.pasteSpecial.timeline", "edit.cutSpecial.timeline", "edit.dragDrop.timeline"]})),
        "sheet": ok(h, "xsheet.exposures.get", json!({})),
        "peg": ok(h, "element.get", json!({"elementId": PEG})),
        "rev": rev(h),
    })
}

/// Ruling E-4's form: from the same state, `a` and `b` leave equal document and session state and raise equal events.
fn one_state(setup: &dyn Fn(&ExecutorHandle), a: (&str, Value), b: (&str, Value)) {
    let (ha, hb) = (fixture(), fixture());
    setup(&ha);
    setup(&hb);
    let ea = events_of(&ha, || {
        ok(&ha, a.0, a.1.clone());
    });
    let eb = events_of(&hb, || {
        ok(&hb, b.0, b.1.clone());
    });
    let (sa, sb) = (state(&ha), state(&hb));
    assert_eq!(sa, sb, "{} and {} leave different states", a.0, b.0);
    // Each command keeps its own registered undo label (design note §3); everything else must be equal.
    let names = |e: &[(String, Value)]| -> Vec<(String, Value)> {
        e.iter()
            .filter(|(n, _)| n != "agent.activity.recorded")
            .map(|(n, v)| {
                let mut v = v.clone();
                for k in ["label", "lastLabel"] {
                    if let Some(m) = v.as_object_mut()
                        && m.contains_key(k)
                    {
                        m.insert(k.into(), json!("(label)"));
                    }
                }
                (n.clone(), v)
            })
            .collect()
    };
    assert_eq!(names(&ea), names(&eb), "{} and {} raise different events", a.0, b.0);
}

// ----- the current frame and the playback range ------------------------------------------------------------------------

#[test]
fn the_current_frame_is_clamped_and_steps_as_04_and_09_say() {
    let h = fixture();
    let len = ok(&h, "timeline.rows.list", json!({}))["sceneLength"].as_u64().unwrap();
    assert_eq!(ok(&h, "timeline.currentFrame.set", json!({"frame": 5})), json!({"frame": 5, "clamped": false}));
    let r = ok(&h, "timeline.currentFrame.set", json!({"frame": 100_000}));
    assert_eq!(r, json!({"frame": len, "clamped": true}), "04 line 1438: clamped, not refused");
    assert_eq!(ok(&h, "timeline.currentFrame.move", json!({"direction": "first"}))["frame"], 1);
    let r = ok(&h, "timeline.currentFrame.move", json!({"direction": "previous"}));
    assert_eq!(r, json!({"frame": 1, "moved": false}), "09 line 137: never wraps");
    // Bob: 1 x6, blank x2, 2 x12, 1 x24: drawing changes at 7, 9, 21.
    ok(&h, "element.current.set", json!({"elementId": BOB}));
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(
            ok(&h, "timeline.currentFrame.move", json!({"direction": "next", "unit": "drawing"}))["frame"].clone(),
        );
    }
    assert_eq!(seen, vec![json!(7), json!(9), json!(21)]);
    ok(&h, "timeline.playbackRange.set", json!({"start": 10, "end": 20}));
    assert_eq!(ok(&h, "timeline.currentFrame.move", json!({"direction": "last", "withinRange": true}))["frame"], 20);
    assert_eq!(ok(&h, "timeline.currentFrame.move", json!({"direction": "first", "withinRange": true}))["frame"], 10);
    let r = ok(&h, "timeline.currentFrame.move", json!({"direction": "previous", "withinRange": true}));
    assert_eq!(r, json!({"frame": 10, "moved": false}));
}

#[test]
fn the_playback_range_follows_04_line_669_and_09_lines_185_to_188() {
    let h = fixture();
    let len = ok(&h, "timeline.rows.list", json!({}))["sceneLength"].as_u64().unwrap();
    assert_eq!(ok(&h, "timeline.playbackRange.get", json!({})), json!({"mode": "auto", "start": 1, "end": len}));
    let ev = events_of(&h, || {
        ok(&h, "timeline.playbackRange.set", json!({"start": 10, "end": 30}));
    });
    let names: Vec<&str> = ev.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"timeline.playbackRange.changed") && names.contains(&"playback.range.changed"),
        "{names:?}"
    );
    // A start past the end moves the end (09 line 186); clamped to the scene (04 line 669).
    let r = ok(&h, "playback.range.set", json!({"start": 40}));
    assert_eq!(r, json!({"mode": "free", "start": 40, "end": 40, "adjusted": true}));
    let r = ok(&h, "playback.range.set", json!({"end": 100_000}));
    assert_eq!((r["end"].clone(), r["adjusted"].clone()), (json!(len), json!(true)));
    // Automatic keeps the last free values; free again restores them (09 line 188).
    assert_eq!(
        ok(&h, "playback.range.mode.set", json!({"mode": "auto"})),
        json!({"mode": "auto", "start": 1, "end": len})
    );
    assert_eq!(
        ok(&h, "playback.range.mode.set", json!({"mode": "free"})),
        json!({"mode": "free", "start": 40, "end": len})
    );
    assert_eq!(
        ok(&h, "timeline.rows.list", json!({}))["playbackRange"],
        json!({"mode": "free", "start": 40, "end": len})
    );
}

#[test]
fn the_playback_range_is_saved_in_session_json() {
    let dir = std::env::temp_dir().join(format!("tt-c51-{}", tt_executor::random_token(8)));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Range.animpkg");
    let h = fixture();
    ok(&h, "timeline.playbackRange.set", json!({"start": 5, "end": 9}));
    let job = ok(&h, "project.saveAs", json!({"path": path.to_string_lossy()}))["jobId"].clone();
    ok(&h, "job.wait", json!({"jobId": job, "timeoutMs": 60_000}));
    ok(&h, "timeline.playbackRange.set", json!({"mode": "auto"}));
    let job = ok(&h, "project.open", json!({"path": path.to_string_lossy(), "onUnsaved": "discard"}))["jobId"].clone();
    ok(&h, "job.wait", json!({"jobId": job, "timeoutMs": 60_000}));
    assert_eq!(ok(&h, "timeline.playbackRange.get", json!({}))["start"], 5, "SI-204: per scene in session.json");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn frame_labels_follow_the_time_display() {
    let h = fixture();
    assert_eq!(ok(&h, "timeline.frameLabel.get", json!({"frames": [1, 20]}))["labels"], json!(["1", "20"]));
    ok(&h, "timeline.timeDisplay.set", json!({"unit": "feetFrames"}));
    assert_eq!(
        ok(&h, "timeline.frameLabel.get", json!({"frames": [1, 16, 20]}))["labels"],
        json!(["0+01", "1+00", "1+04"])
    );
}

// ----- one state (SI-202, SI-203; E-3, E-4, E-6) ------------------------------------------------------------------------

#[test]
fn every_pair_of_si_202_writes_one_state_and_raises_the_same_events() {
    let none = |_: &ExecutorHandle| {};
    one_state(&none, ("app.frame.set", json!({"frame": 5})), ("timeline.currentFrame.set", json!({"frame": 5})));
    let at3 = |h: &ExecutorHandle| {
        ok(h, "timeline.currentFrame.set", json!({"frame": 3}));
    };
    one_state(
        &at3,
        ("app.frame.move", json!({"direction": "next"})),
        ("timeline.currentFrame.move", json!({"direction": "next"})),
    );
    one_state(
        &at3,
        ("app.frame.move", json!({"direction": "next", "unit": "drawing"})),
        ("timeline.currentFrame.move", json!({"direction": "next", "unit": "drawing"})),
    );
    one_state(
        &none,
        ("app.currentElement.set", json!({"elementId": PEG})),
        ("element.current.set", json!({"elementId": PEG})),
    );
    one_state(
        &none,
        ("app.currentElement.move", json!({"direction": "next", "order": "xsheet"})),
        ("element.current.move", json!({"direction": "next", "order": "xsheet"})),
    );
    one_state(
        &none,
        ("timeline.playbackRange.set", json!({"start": 3, "end": 9})),
        ("playback.range.set", json!({"start": 3, "end": 9})),
    );
    let free = |h: &ExecutorHandle| {
        ok(h, "playback.range.set", json!({"start": 3, "end": 9}));
    };
    one_state(
        &free,
        ("timeline.playbackRange.set", json!({"mode": "auto"})),
        ("playback.range.mode.set", json!({"mode": "auto"})),
    );
}

#[test]
fn e3_loops_by_either_command_are_one_state() {
    let none = |_: &ExecutorHandle| {};
    one_state(
        &none,
        ("anim.loop.set", json!({"nodeId": PEG, "loops": 3})),
        ("element.timing.set", json!({"elementIds": [PEG], "loops": 3})),
    );
}

#[test]
fn e6_the_preference_commands_write_the_01_keys_as_prefs_set_does() {
    let none = |_: &ExecutorHandle| {};
    one_state(
        &none,
        ("timeline.timeDisplay.set", json!({"unit": "feetFrames"})),
        ("prefs.set", json!({"values": {"interface.timeMeasurement": "feetAndFrames"}})),
    );
    one_state(
        &none,
        ("timeline.backgroundColour.set", json!({"colour": "#303030FF"})),
        ("prefs.set", json!({"values": {"interface.background.timeline": "#303030FF"}})),
    );
    one_state(
        &none,
        ("timeline.singleCellSelection.set", json!({"enabled": false})),
        ("prefs.set", json!({"values": {"timing.timeline.singleCellSelection": false}})),
    );
    one_state(
        &none,
        (
            "timeline.transferOptions.set",
            json!({"operation": "paste", "options": {"exposure": false, "pegKeys": {"skew": false}}}),
        ),
        (
            "prefs.set",
            json!({"values": {"edit.pasteSpecial.timeline": {"createDeleteExposure": false, "skewKeys": false}}}),
        ),
    );
}

// ----- the transfer-option store (SI-201, E-8) ---------------------------------------------------------------------------

#[test]
fn one_store_serves_both_vocabularies_and_omitted_cell_members() {
    let h = fixture();
    let stored = ok(&h, "timeline.transferOptions.get", json!({"operation": "dragDrop"}))["options"].clone();
    assert_eq!(stored["exposure"], true);
    ok(&h, "timeline.transferOptions.set", json!({"operation": "dragDrop", "options": {"exposure": false}}));
    let pref =
        ok(&h, "prefs.get", json!({"keys": ["edit.dragDrop.timeline"]}))["values"]["edit.dragDrop.timeline"].clone();
    assert_eq!(pref["createDeleteExposure"], false, "04's `exposure` is 01's `createDeleteExposure`");
    assert_eq!(
        err(&h, "timeline.transferOptions.set", json!({"operation": "cut", "options": {"nope": true}})).code,
        "arg.unknown"
    );
    // `cell.range.move` takes omitted members from the drag-and-drop set (04 line 1366): the same result as the
    // explicit option on a fresh fixture.
    let move_params = json!({"sourceElementIds": [BOB], "from": 1, "to": 2, "targetFrame": 30, "mode": "move"});
    ok(&h, "cell.range.move", move_params.clone());
    let g = fixture();
    let mut explicit = move_params;
    explicit["options"] = json!({"exposure": false});
    ok(&g, "cell.range.move", explicit);
    assert_eq!(ok(&h, "xsheet.exposures.get", json!({})), ok(&g, "xsheet.exposures.get", json!({})));
}

#[test]
fn edit_special_remembers_into_the_store() {
    let h = fixture();
    let cell = format!("{BOB}/cell:#1");
    ok(
        &h,
        "edit.cutSpecial",
        json!({"context": "xsheet", "objectIds": [cell], "options": {"createDeleteExposure": false}, "remember": true}),
    );
    assert_eq!(ok(&h, "timeline.transferOptions.get", json!({"operation": "cut"}))["options"]["exposure"], false);
}

// ----- E-5: the window's `edit.*` against the explicit-target `cell.*` forms -------------------------------------------

/// The document, clipboard and selection after `op` from the same cell selection in `context`.
fn after_edit(context: &str, op: (&str, Value)) -> Value {
    let h = fixture();
    ok(&h, "cell.select", json!({"elementIds": [BOB], "from": 1, "to": 3}));
    ok(&h, "xsheet.currentCell.set", json!({"elementId": BOB, "frame": 1}));
    let focus = if context == "xsheet" { "xsheet" } else { "timeline" };
    ok(&h, "window.panel.set", json!({"panelId": focus, "visible": true, "focus": true}));
    if op.0.contains("paste") || op.0 == "cell.clipboard.apply" {
        // A paste: frames 1-3 copied, then the cell at frame 30 selected and current.
        ok(&h, "edit.copy", json!({"context": context}));
        ok(&h, "cell.select", json!({"elementIds": [BOB], "from": 30, "to": 30}));
        ok(&h, "xsheet.currentCell.set", json!({"elementId": BOB, "frame": 30}));
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
    })
}

#[test]
fn e5_edit_commands_equal_the_cell_forms_with_the_target_omitted() {
    // Ruling E-5: each `edit.*` command must give exactly the document, clipboard and selection of the matching
    // `cell.*` command with its target omitted, in both contexts. The differences found are brought to the orchestrator
    // before either command changes; until the ruling this test pins exactly them, so any other difference fails.
    let mut differences = Vec::new();
    for context in ["xsheet", "timeline"] {
        for (a, b) in [
            (("edit.copy", json!({})), ("cell.clipboard.set", json!({}))),
            (("edit.cut", json!({})), ("cell.clipboard.set", json!({"cut": true}))),
            (("edit.delete", json!({})), ("cell.range.delete", json!({}))),
            (("edit.paste", json!({})), ("cell.clipboard.apply", json!({}))),
        ] {
            let (x, y) = (after_edit(context, a.clone()), after_edit(context, b.clone()));
            for key in ["status", "error", "sheet", "clipboard", "selection"] {
                if x[key] != y[key] {
                    let what = if key == "clipboard" {
                        let fields: Vec<String> = x[key]
                            .as_object()
                            .map(|m| m.keys().filter(|k| x[key][k.as_str()] != y[key][k.as_str()]).cloned().collect())
                            .unwrap_or_default();
                        format!("clipboard {}", fields.join(","))
                    } else {
                        key.to_owned()
                    };
                    differences.push(format!("{context} {} / {}: {what}", a.0, b.0));
                }
            }
        }
    }
    let expected = vec![
        // Ruling E-14: Cut is to follow 04 §13.16 (`cell.clipboard.set {cut: true}`: removed and shifted with the
        // cut-special exposure option on); `edit.cut` still clears, because the reviewer test
        // `c31_r2_variants::r05_generic_cut_clears_notes_and_retimes_remaining_notes` pins its clearing: left as it is
        // until the orchestrator answers.
        "xsheet edit.cut / cell.clipboard.set: sheet".to_owned(),
        // A plain paste overwrites the destination cells; `cell.clipboard.apply` inserts them (the stored paste set's
        // create/delete exposure option, on by default, 04 §11.14). Brought to the orchestrator with the E-13 report.
        "xsheet edit.paste / cell.clipboard.apply: sheet".to_owned(),
        "timeline edit.cut / cell.clipboard.set: sheet".to_owned(),
        "timeline edit.paste / cell.clipboard.apply: sheet".to_owned(),
    ];

    assert_eq!(differences, expected, "E-5 differences (to the orchestrator before either changes)");
}

// ----- the read model ---------------------------------------------------------------------------------------------------

#[test]
fn rows_list_carries_bars_keyframes_and_child_markers() {
    let h = fixture();
    let r = ok(&h, "timeline.rows.list", json!({}));
    let rows = r["rows"].as_array().unwrap();
    let peg = rows.iter().find(|x| x["name"] == "Bob-P").unwrap();
    let skew: Vec<&Value> =
        peg["keyframes"].as_array().unwrap().iter().filter(|k| k["channels"] == json!(["skew"])).collect();
    assert_eq!(skew.len(), 2, "the peg's skew keys at its bar's frames 1 and 48: {peg}");
    assert!(peg["bar"]["start"].as_u64().unwrap() >= 1);
    let bob = rows.iter().find(|x| x["name"] == "Bob").unwrap();
    assert_eq!(bob["bar"]["start"], 1, "a cell-based row's bar is its exposed extent");
    if bob["parentId"] == peg["elementId"] {
        let bob_frames: Vec<Value> = bob["keyframes"].as_array().unwrap().iter().map(|k| k["frame"].clone()).collect();
        for f in &bob_frames {
            assert!(peg["childKeyframeFrames"].as_array().unwrap().contains(f), "04 line 630: {f} on the parent");
        }
    }
    let windowed = ok(&h, "timeline.rows.list", json!({"from": 2, "to": 10}));
    for row in windowed["rows"].as_array().unwrap() {
        for k in row["keyframes"].as_array().unwrap() {
            assert!((2..=10).contains(&k["frame"].as_u64().unwrap()), "{k}");
        }
    }
}

#[test]
fn the_view_answers_its_state_and_reveals_a_row() {
    let h = fixture();
    let v = ok(&h, "timeline.view.set", json!({}));
    assert_eq!((v["zoomLevel"].clone(), v["firstFrame"].clone()), (json!(6), json!(1)));
    assert_eq!(v["lastFrame"], 60.0, "960 px at 16 px a frame");
    let v = ok(&h, "timeline.view.set", json!({"zoomLevel": 9, "firstFrame": 30}));
    assert_eq!((v["firstFrame"].clone(), v["lastFrame"].clone()), (json!(30), json!(49.0)));
    assert_eq!(err(&h, "timeline.view.set", json!({"zoomLevel": 99})).code, "arg.invalid");
    let ev = events_of(&h, || {
        ok(&h, "timeline.view.set", json!({"revealElementId": BOB}));
    });
    let notice = ev.iter().find(|(n, _)| n == "shell.timelineView").expect("the window follows an agent's call (E-11)");
    assert_eq!(notice.1["revealElementId"], id(&h, BOB));
}

// ----- the timing-gesture interpreter (E-7, E-10) ----------------------------------------------------------------------

/// Visible rows: Timeline order without the descendants of collapsed rows.
fn visible(h: &ExecutorHandle) -> Vec<Value> {
    let rows = ok(h, "timeline.rows.list", json!({}))["rows"].as_array().unwrap().clone();
    let mut out = Vec::new();
    let mut hidden_below: Option<u64> = None;
    for r in rows {
        let d = r["depth"].as_u64().unwrap();
        if let Some(h) = hidden_below {
            if d > h {
                continue;
            }
            hidden_below = None;
        }
        if r["collapsed"] == true {
            hidden_below = Some(d);
        }
        out.push(r);
    }
    out
}

fn row_of(h: &ExecutorHandle, name: &str) -> usize {
    visible(h).iter().position(|r| r["name"] == name).unwrap()
}

fn layout(surface: &str) -> Value {
    json!({"surface": surface, "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"})
}

/// A sample at the centre of frame `f`'s cell, in row `r` (lower half of the row unless `upper`).
fn at(r: usize, f: u32, upper: bool, mods: &[&str]) -> Value {
    json!({"x": f64::from(f - 1) * 16.0 + 8.0, "y": r as f64 * 22.0 + if upper { 5.0 } else { 16.0 }, "modifiers": mods})
}

fn plan(h: &ExecutorHandle, surface: &str, samples: Value) -> Value {
    h.timing_replay(Origin::User, layout(surface), samples, false).unwrap()["plan"].clone()
}

/// The gesture through the window's path (press, release) on one fixture, the direct command on another: equal states,
/// and the replay's plan equal to the expected command (D-23: written from the spec, not from the output).
fn three_way(surface: &str, samples: Value, expected: Value, setup: &dyn Fn(&ExecutorHandle)) {
    let (hw, hd) = (fixture(), fixture());
    setup(&hw);
    setup(&hd);
    assert_eq!(plan(&hw, surface, samples.clone()), expected, "the interpreter's plan");
    let first = samples[0].clone();
    let press =
        hw.timing_gesture(Origin::User, json!({"phase": "press", "layout": layout(surface), "sample": first})).unwrap();
    let out = hw
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["plan"], expected, "the window's commit: {out}");
    for (command, params) in
        expected["commands"].as_array().cloned().unwrap_or_default().iter().map(|c| (c[0].clone(), c[1].clone()))
    {
        ok(&hd, command.as_str().unwrap(), params);
    }
    assert_eq!(state(&hw), state(&hd), "window and direct commands agree exactly (D-32)");
}

fn cmds(list: &[(&str, Value)]) -> Value {
    json!({"kind": "commands", "commands": list.iter().map(|(c, p)| json!([c, p])).collect::<Vec<_>>()})
}

#[test]
fn d23_ruler_and_range_markers() {
    let none = |_: &ExecutorHandle| {};
    // 04 line 662: click in the ruler sets the current frame (ui; no undo step).
    three_way(
        "ruler",
        json!([at(0, 12, false, &[])]),
        cmds(&[("timeline.currentFrame.set", json!({"frame": 12}))]),
        &none,
    );
    // 04 line 606: dragging scrubs; the release commits the last frame.
    three_way(
        "ruler",
        json!([at(0, 3, false, &[]), at(0, 9, false, &[]), at(0, 15, false, &[])]),
        cmds(&[("timeline.currentFrame.set", json!({"frame": 15}))]),
        &none,
    );
    // 04 line 608: a free range's markers drag (ui; no undo step).
    let free = |h: &ExecutorHandle| {
        ok(h, "timeline.playbackRange.set", json!({"start": 5, "end": 20}));
    };
    three_way(
        "ruler",
        json!([at(0, 5, false, &[]), at(0, 8, false, &[])]),
        cmds(&[("timeline.playbackRange.set", json!({"start": 8}))]),
        &free,
    );
}

#[test]
fn d23_row_clicks_and_drags() {
    let none = |_: &ExecutorHandle| {};
    let h = fixture();
    let bob_row = row_of(&h, "Bob");
    let bob = id(&h, BOB);
    // 04 line 685: a click on a row name selects it and makes it current (no undo step).
    three_way(
        "layers",
        json!([at(bob_row, 1, false, &[])]),
        cmds(&[
            ("element.select", json!({"elementIds": [bob], "mode": "replace"})),
            ("element.current.set", json!({"elementId": bob})),
        ]),
        &none,
    );
    // 04 line 686: the toggle modifier toggles.
    three_way(
        "layers",
        json!([at(bob_row, 1, false, &["ctrl"])]),
        cmds(&[("element.select", json!({"elementIds": [bob], "mode": "toggle"}))]),
        &none,
    );
}

#[test]
fn d23_bar_drags_make_one_step_with_the_commands_label() {
    let h = fixture();
    let peg_row = row_of(&h, "Bob-P");
    let peg = id(&h, PEG);
    let bar = visible(&h)[peg_row]["bar"].clone();
    let (start, end) = (bar["start"].as_u64().unwrap() as u32, bar["end"].as_u64().unwrap() as u32);
    let none = |_: &ExecutorHandle| {};
    // 04 line 717: dragging the body moves the bar (startDelta), one undo step.
    let mid = (start + end) / 2;
    three_way(
        "tracks",
        json!([at(peg_row, mid, false, &[]), at(peg_row, mid + 4, false, &[])]),
        cmds(&[("element.timing.set", json!({"elementIds": [peg], "startDelta": 4}))]),
        &none,
    );
    // 04 line 718: the end stretches the duration with keyframes scaled.
    let xe = f64::from(end) * 16.0; // the right edge of the last cell
    let samples = json!([
        {"x": xe, "y": peg_row as f64 * 22.0 + 16.0, "modifiers": []},
        {"x": xe - 64.0, "y": peg_row as f64 * 22.0 + 16.0, "modifiers": []}
    ]);
    three_way(
        "tracks",
        samples,
        cmds(&[(
            "element.timing.set",
            json!({"elementIds": [peg], "duration": i64::from(end - start + 1) - 4, "anchor": "start", "keyframes": "scale"}),
        )]),
        &none,
    );
    let g = fixture();
    let before = ok(&g, "edit.history.get", json!({}))["entries"].as_array().unwrap().len();
    let press = g
        .timing_gesture(
            Origin::User,
            json!({"phase": "press", "layout": layout("tracks"), "sample": at(peg_row, mid, false, &[])}),
        )
        .unwrap();
    g.timing_gesture(
        Origin::User,
        json!({"phase": "release", "grabId": press["grabId"], "samples": [at(peg_row, mid, false, &[]), at(peg_row, mid + 2, false, &[])]}),
    )
    .unwrap();
    let hist = ok(&g, "edit.history.get", json!({}));
    assert_eq!(hist["entries"].as_array().unwrap().len(), before + 1, "one step from press to release (04 line 723)");
}

#[test]
fn d23_cell_clicks_and_selection_drags() {
    let h = fixture();
    let bob_row = row_of(&h, "Bob");
    let bob = id(&h, BOB);
    let none = |_: &ExecutorHandle| {};
    // 04 line 687 (single-cell selection on, the default): the cell, and it becomes current (04 line 662).
    three_way(
        "tracks",
        json!([at(bob_row, 4, false, &[])]),
        cmds(&[
            ("cell.select", json!({"elementIds": [bob], "from": 4, "to": 4})),
            ("xsheet.currentCell.set", json!({"elementId": bob, "frame": 4})),
        ]),
        &none,
    );
    // 04 line 688: a drag across cells selects the frame range.
    three_way(
        "tracks",
        json!([at(bob_row, 2, false, &[]), at(bob_row, 6, false, &[])]),
        cmds(&[
            ("cell.select", json!({"elementIds": [bob], "from": 2, "to": 6})),
            ("xsheet.currentCell.set", json!({"elementId": bob, "frame": 2})),
        ]),
        &none,
    );
    // Single-cell selection off: the track (04 line 687).
    let off = |h: &ExecutorHandle| {
        ok(h, "prefs.set", json!({"values": {"timing.timeline.singleCellSelection": false}}));
    };
    let len = ok(&h, "timeline.rows.list", json!({}))["sceneLength"].clone();
    three_way(
        "tracks",
        json!([at(bob_row, 4, false, &[])]),
        cmds(&[
            ("cell.select", json!({"elementIds": [bob], "from": 1, "to": len, "extent": "track"})),
            ("xsheet.currentCell.set", json!({"elementId": bob, "frame": 4})),
        ]),
        &off,
    );
    // 04 line 751: selected cells dragged move (the copy modifier copies), one undo step.
    let selected = |h: &ExecutorHandle| {
        ok(h, "cell.select", json!({"elementIds": [BOB], "from": 1, "to": 2}));
    };
    three_way(
        "tracks",
        json!([at(bob_row, 1, false, &[]), at(bob_row, 31, false, &[])]),
        cmds(&[(
            "cell.range.move",
            json!({"sourceElementIds": [bob], "from": 1, "to": 2, "targetFrame": 31, "mode": "move"}),
        )]),
        &selected,
    );
    three_way(
        "tracks",
        json!([at(bob_row, 1, false, &[]), at(bob_row, 31, false, &["ctrl"])]),
        cmds(&[(
            "cell.range.move",
            json!({"sourceElementIds": [bob], "from": 1, "to": 2, "targetFrame": 31, "mode": "copy"}),
        )]),
        &selected,
    );
}

#[test]
fn d23_gestures_that_must_make_no_step() {
    let h = fixture();
    let peg_row = row_of(&h, "Bob-P");
    // A drag that comes back to where it started moves nothing.
    let mid = 10;
    let p = plan(
        &h,
        "tracks",
        json!([at(peg_row, mid, false, &[]), at(peg_row, mid + 3, false, &[]), at(peg_row, mid, false, &[])]),
    );
    assert_eq!(p, json!({"kind": "nothing"}));
    // A press below the last row.
    let p = plan(&h, "layers", json!([{"x": 4.0, "y": 22.0 * 500.0, "modifiers": []}]));
    assert_eq!(p, json!({"kind": "nothing"}));
    // An abort sends nothing.
    let before = rev(&h);
    let press = h
        .timing_gesture(
            Origin::User,
            json!({"phase": "press", "layout": layout("tracks"), "sample": at(peg_row, mid, false, &[])}),
        )
        .unwrap();
    h.timing_gesture(Origin::User, json!({"phase": "abort", "grabId": press["grabId"]})).unwrap();
    let e = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": []}))
        .unwrap_err();
    assert_eq!(e.code, "timeline.targetChanged");
    assert_eq!(rev(&h), before);
}

#[test]
fn e10_a_change_under_the_drag_cancels_or_commits_by_what_the_preview_showed() {
    let h = fixture();
    let peg_row = row_of(&h, "Bob-P");
    let mid = 10;
    let press_at = at(peg_row, mid, false, &[]);
    let samples = json!([press_at.clone(), at(peg_row, mid + 3, false, &[])]);
    // An unrelated change (Bob renamed by an agent) between press and release: the drag commits.
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout("tracks"), "sample": press_at.clone()}))
        .unwrap();
    let agent = agent(&[tt_registry::Scope::Edit, tt_registry::Scope::Read]);
    let env = h.invoke(tt_executor::Invocation {
        command: "element.rename".into(),
        params: json!({"elementId": BOB, "name": "Bobby"}),
        options: Default::default(),
        origin: agent.clone(),
    });
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert!(out.get("cancelled").is_none(), "{out}");
    // The peg moved by an agent while the person drags it: what the preview showed is no longer the command.
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout("tracks"), "sample": press_at.clone()}))
        .unwrap();
    let env = h.invoke(tt_executor::Invocation {
        command: "element.timing.set".into(),
        params: json!({"elementIds": [PEG], "startDelta": 30}),
        options: Default::default(),
        origin: agent,
    });
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let before = rev(&h);
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["cancelled"], true, "{out}");
    assert_eq!(out["error"]["code"], "timeline.targetChanged");
    assert_eq!(rev(&h), before, "nothing ran");
}

// ----- the `xsheet` composition (SI-107) ------------------------------------------------------------------------------

#[test]
fn the_xsheet_composition_follows_column_order_and_exposure_sheet_visibility() {
    let h = fixture();
    let render = |h: &ExecutorHandle| {
        ok(h, "render.frame.run", json!({"composition": "xsheet", "frame": 1, "scale": 0.1}))["sha256"].clone()
    };
    let order = |h: &ExecutorHandle| -> Vec<Value> {
        ok(h, "element.list", json!({"order": "xsheet"}))["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].clone())
            .collect()
    };
    // Two overlapping squares of different colours at the origin.
    let c = c33_support::solids(&h, 2);
    let a = json!(c33_support::square(&h, "SquareA", 100.0, &c[0].0));
    let b = json!(c33_support::square(&h, "SquareB", 100.0, &c[1].0));
    let mut ab: Vec<Value> = order(&h).into_iter().filter(|e| *e != a && *e != b).collect();
    ab.extend([a.clone(), b.clone()]);
    ok(&h, "element.order.set", json!({"order": "xsheet", "elementIds": ab.clone()}));
    let b_in_front = render(&h);
    assert_eq!(render(&h), b_in_front, "deterministic (09 §11.1 item 2)");
    // Right is front (04 line 56): swapping the two columns changes which square shows.
    let mut ba = ab.clone();
    let n = ba.len();
    ba.swap(n - 1, n - 2);
    ok(&h, "element.order.set", json!({"order": "xsheet", "elementIds": ba}));
    let a_in_front = render(&h);
    assert_ne!(a_in_front, b_in_front, "the column order is the stacking order");
    // A column hidden in the Exposure Sheet is left out (04 §1.3): A hidden, the front square is gone.
    ok(&h, "element.visibility.set", json!({"elementIds": [a], "scope": "xsheet", "visible": false}));
    let a_hidden = render(&h);
    assert_ne!(a_hidden, a_in_front, "hiding the front column changes the composition");
    ok(&h, "element.visibility.set", json!({"elementIds": [a], "scope": "xsheet", "visible": true}));
    assert_eq!(render(&h), a_in_front, "shown again: as before");
}

// ----- E-14: Delete on cells; E-16: category colours --------------------------------------------------------------------

#[test]
fn e14_delete_clears_in_the_exposure_sheet_and_follows_the_cut_option_in_the_timeline() {
    let sheet = |context: &str, cut_exposure: Option<bool>| {
        let h = fixture();
        if let Some(on) = cut_exposure {
            ok(&h, "timeline.transferOptions.set", json!({"operation": "cut", "options": {"exposure": on}}));
        }
        ok(&h, "cell.select", json!({"elementIds": [BOB], "from": 1, "to": 2}));
        ok(&h, "window.panel.set", json!({"panelId": context, "visible": true, "focus": true}));
        ok(&h, "edit.delete", json!({"context": context}));
        ok(&h, "xsheet.exposures.get", json!({"elementIds": [BOB]}))["columns"][0]["exposures"][0].clone()
    };
    // Bob: drawing 1 on frames 1-6. The Exposure Sheet clears frames 1-2 (04 §13.17, §18 #8).
    assert_eq!(sheet("xsheet", None)["from"], 3);
    // The Timeline follows the cut-special create/delete exposure option: on (the default) removes and shifts.
    let shifted = sheet("timeline", None);
    assert_eq!((shifted["from"].clone(), shifted["to"].clone()), (json!(1), json!(4)));
    assert_eq!(sheet("timeline", Some(false))["from"], 3, "off: cleared");
}

#[test]
fn e16_category_colours_live_in_the_01_keys() {
    let h = fixture();
    let def = ok(&h, "prefs.get", json!({"keys": ["interface.elementColours.peg"]}))["values"]["interface.elementColours.peg"].clone();
    assert_eq!(ok(&h, "element.typeColour.get", json!({}))["colours"]["pegs"], def, "01 §7.3's default is reported");
    assert_eq!(ok(&h, "element.get", json!({"elementId": PEG}))["effectiveMarkerColour"], def);
    let none = |_: &ExecutorHandle| {};
    one_state(
        &none,
        ("element.typeColour.set", json!({"category": "pegs", "colour": "#102030FF"})),
        ("prefs.set", json!({"values": {"interface.elementColours.peg": "#102030FF"}})),
    );
    // "skeletons" writes the skeleton and bone keys together (SI-203).
    ok(&h, "element.typeColour.set", json!({"category": "skeletons", "colour": "#405060"}));
    let v = ok(
        &h,
        "prefs.get",
        json!({"keys": ["interface.elementColours.skeleton", "interface.elementColours.bone"]}),
    )["values"]
        .clone();
    assert_eq!(v["interface.elementColours.skeleton"], "#405060FF");
    assert_eq!(v["interface.elementColours.bone"], "#405060FF");
}
