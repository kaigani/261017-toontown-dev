//! M5 C5.1a (builder): the rest of the timing-gesture D-23 table (`reviews/M5-design-timeline-kernel.md` §5.3: loop
//! handle, stretch from start, keyframe drag, row reorder, attach, another level, back to the root), the shape of
//! `timeline.rows.list` at 1,000 elements (D-45) and the real playback clock observed by its states (E-1).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{BatchRequest, ExecutorHandle, Origin};

/// The fixture's visible rows (Timeline order; no row is collapsed in the sample document): `(id, name)`.
fn rows(h: &ExecutorHandle) -> Vec<(String, String)> {
    ok(h, "timeline.rows.list", json!({}))["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["elementId"].as_str().unwrap().to_owned(), r["name"].as_str().unwrap().to_owned()))
        .collect()
}

fn row(h: &ExecutorHandle, name: &str) -> (usize, String) {
    let r = rows(h);
    let i = r.iter().position(|(_, n)| n == name).unwrap();
    (i, r[i].0.clone())
}

fn layout(surface: &str) -> Value {
    json!({"surface": surface, "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"})
}

fn s(x: f64, y: f64) -> Value {
    json!({"x": x, "y": y, "modifiers": []})
}

/// The state compared between the window's path and the direct command.
fn state(h: &ExecutorHandle) -> Value {
    json!({
        "rows": ok(h, "timeline.rows.list", json!({})),
        "app": ok(h, "app.state.get", json!({})),
        "history": ok(h, "edit.history.get", json!({}))["entries"].as_array().map(|e| e.len()),
        "rev": rev(h),
    })
}

/// The interpreter's plan equals the expectation written from the spec (D-23); the window's press and release commit
/// it; the direct commands give the same state exactly (D-32).
fn three_way(surface: &str, samples: Value, expected: Value) {
    let (hw, hd) = (fixture(), fixture());
    let plan = hw.timing_replay(Origin::User, layout(surface), samples.clone(), false).unwrap()["plan"].clone();
    assert_eq!(plan, expected, "the interpreter's plan");
    let press = hw
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout(surface), "sample": samples[0]}))
        .unwrap();
    let out = hw
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["plan"], expected, "the window's commit: {out}");
    for env in out["envelopes"].as_array().unwrap() {
        assert_eq!(env["status"], "ok", "{env}");
    }
    match expected["kind"].as_str().unwrap() {
        "commands" => {
            for c in expected["commands"].as_array().unwrap() {
                ok(&hd, c[0].as_str().unwrap(), c[1].clone());
            }
        }
        "batch" => {
            let steps: Vec<Value> =
                expected["steps"].as_array().unwrap().iter().map(|c| json!({"id": c[0], "params": c[1]})).collect();
            let req: BatchRequest =
                serde_json::from_value(json!({"label": expected["label"], "atomic": true, "steps": steps})).unwrap();
            let env = hd.batch(req, Origin::User);
            assert!(env.is_ok(), "{env:?}");
        }
        _ => {}
    }
    assert_eq!(state(&hw), state(&hd), "window and direct commands agree exactly (D-32)");
}

fn one(command: &str, params: Value) -> Value {
    json!({"kind": "commands", "commands": [[command, params]]})
}

const RH: f64 = 22.0;
const CW: f64 = 16.0;

#[test]
fn d23_loop_handle_and_stretch_from_start() {
    let h = fixture();
    let (fade, id) = row(&h, "Fade");
    // 04 line 720: the loop handle (the upper half of a loopable bar's end) adjusts the loops: the bar's 48-frame cycle
    // dragged to 96 frames is two loops.
    let y = fade as f64 * RH + 5.0;
    three_way(
        "tracks",
        json!([s(48.0 * CW, y), s(96.0 * CW, y)]),
        one("element.timing.set", json!({"elementIds": [id], "loops": 2})),
    );
    // 04 line 719: the start dragged changes start and duration keeping the end (keyframes scaled).
    let y = fade as f64 * RH + 16.0;
    three_way(
        "tracks",
        json!([s(2.0, y), s(2.0 + 4.0 * CW, y)]),
        one("element.timing.set", json!({"elementIds": [id], "duration": 44, "anchor": "end", "keyframes": "scale"})),
    );
}

#[test]
fn d23_keyframe_drag_moves_the_keys_of_that_frame() {
    let h = fixture();
    let (peg, id) = row(&h, "Bob-P");
    // 04 line 621, 05 §10.1 "Move keyframe": the peg's key at frame 48 dragged ten frames earlier.
    let y = peg as f64 * RH + 11.0;
    let x = 47.0 * CW + CW / 2.0;
    three_way(
        "tracks",
        json!([s(x, y), s(x - 10.0 * CW, y)]),
        one("anim.key.move", json!({"nodeId": id, "fromFrame": 48, "toFrame": 38})),
    );
}

#[test]
fn d23_row_drops_reorder_attach_and_change_level() {
    let h = fixture();
    let r = rows(&h);
    let (bg, bg_id) = row(&h, "Background");
    let (peg, peg_id) = row(&h, "Bob-P");
    let (bob, bob_id) = row(&h, "Bob");
    let root: Vec<String> = ok(&h, "timeline.rows.list", json!({}))["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["depth"] == 0)
        .map(|r| r["elementId"].as_str().unwrap().to_owned())
        .collect();
    let press = s(40.0, bg as f64 * RH + 11.0);
    // 04 line 478: a row dropped between two rows of its level reorders the level (the upper quarter of Bob-P's row
    // drops before it).
    let mut order: Vec<String> = root.iter().filter(|x| **x != bg_id).cloned().collect();
    let at = order.iter().position(|x| *x == peg_id).unwrap();
    order.insert(at, bg_id.clone());
    three_way(
        "layers",
        json!([press, s(40.0, peg as f64 * RH + 2.0)]),
        one("element.order.set", json!({"order": "timeline", "elementIds": order})),
    );
    // 04 §9.2 line 487: dropped onto a peg row (its middle half), the row is attached.
    three_way(
        "layers",
        json!([press, s(40.0, peg as f64 * RH + 11.0)]),
        one("element.parent.set", json!({"elementIds": [bg_id], "parentId": peg_id})),
    );
    // Between rows of another level (before Bob, Bob-P's first child): attached at that index.
    three_way(
        "layers",
        json!([press, s(40.0, bob as f64 * RH + 2.0)]),
        one("element.parent.set", json!({"elementIds": [bg_id], "parentId": peg_id, "index": 0})),
    );
    // A child dropped below the last row goes to the end of the root level: detached, then the root order (one step).
    let mut root_after: Vec<String> = root.clone();
    root_after.push(bob_id.clone());
    three_way(
        "layers",
        json!([s(40.0, bob as f64 * RH + 11.0), s(40.0, r.len() as f64 * RH + 5.0)]),
        json!({"kind": "batch", "label": "Move Rows", "steps": [
            ["element.parent.remove", {"elementIds": [bob_id]}],
            ["element.order.set", {"order": "timeline", "elementIds": root_after}],
        ]}),
    );
}

/// D-45: the read model's cost at 1,000 elements against 10, in one process (a ratio), with a ceiling that only catches a
/// gross regression; the budget itself (20 ms p95 for a 120-frame window, release) is a workstation receipt.
///
/// The three scenes (the fixture, about 500 rows, about 1,000 rows) are built first and measured in turns, ten rounds,
/// the best of each: a burst of load on the runner (the other tests of this binary run beside it) then falls on every
/// side of the ratio. Measured one after the other, the 1,000-row side of run 45400525009 (macOS) took 14.3 ms against
/// 4.7 ms on the workstation while its 500-row side took 4.2 ms against 2.5: a doubling ratio of 3.41 for a read that is
/// linear (1.75 to 1.89 on the workstation). Every sample is printed when an assertion fails.
#[test]
fn rows_list_stays_linear_at_one_thousand_elements() {
    let small = fixture();
    let half_h = fixture();
    let h = fixture();
    let fill = |h: &ExecutorHandle, rows: usize| {
        let mut built = 0;
        while built < rows {
            let n = (rows - built).min(99);
            ok(
                h,
                "element.create",
                json!({"type": if built % 2 == 0 { "peg" } else { "drawing" }, "count": n, "name": format!("Row{built}")}),
            );
            built += n;
        }
    };
    fill(&half_h, 495);
    fill(&h, 990);
    let once = |h: &ExecutorHandle| -> Duration {
        let t = Instant::now();
        ok(h, "timeline.rows.list", json!({"from": 1, "to": 120}));
        t.elapsed()
    };
    let mut samples: [Vec<Duration>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for _ in 0..10 {
        for (k, handle) in [&small, &half_h, &h].into_iter().enumerate() {
            samples[k].push(once(handle));
        }
    }
    let best = |k: usize| samples[k].iter().copied().min().unwrap_or(Duration::MAX);
    let (base, half, large) = (best(0).max(Duration::from_millis(1)), best(1), best(2));
    let rows_large = ok(&h, "timeline.rows.list", json!({}))["rows"].as_array().unwrap().len();
    assert!(rows_large >= 1000, "{rows_large} rows");
    let shown = format!("samples (fixture, ~500 rows, ~1,000 rows): {samples:?}");
    let ratio = large.as_secs_f64() / base.as_secs_f64();
    eprintln!("timeline.rows.list: {rows_large} rows {large:?}, 20 rows {base:?}, ratio {ratio:.1}");
    assert!(ratio <= 150.0, "rows.list grows faster than the rows: ratio {ratio:.1}; {shown}");
    // Twice the rows, about twice the time (linear about 2, quadratic about 4): run 45139823182 found the read growing with
    // the square of the rows (the scene length computed per row).
    let doubling = large.as_secs_f64() / half.max(Duration::from_micros(100)).as_secs_f64();
    eprintln!("timeline.rows.list: about 500 rows {half:?}, 1000 rows {large:?}, doubling ratio {doubling:.2}");
    assert!(doubling <= 3.0, "rows.list grows faster than the rows when they double: {doubling:.2}; {shown}");
    assert!(large < Duration::from_secs(2), "a gross regression: {large:?}; {shown}");
}

/// E-1: the real clock plays the range and stops at its end; observed by the events it raises, waiting for states, never
/// asserting a duration (D-45). The bound only guards a hang.
#[test]
fn the_real_clock_plays_the_range_and_stops_at_its_end() {
    let h = fixture();
    ok(&h, "timeline.playbackRange.set", json!({"start": 2, "end": 6}));
    ok(&h, "playback.rate.set", json!({"fps": 120}));
    ok(&h, "timeline.currentFrame.set", json!({"frame": 2}));
    let rx = h.subscribe();
    let started = ok(&h, "playback.start", json!({}));
    assert_eq!((started["playing"].clone(), started["frame"].clone()), (json!(true), json!(2)));
    let guard = Instant::now() + Duration::from_secs(60);
    let mut frames = Vec::new();
    let stopped = loop {
        let left = guard.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "playback never stopped: frames {frames:?}");
        let Ok(e) = rx.recv_timeout(left) else { continue };
        if e.name == "timeline.currentFrame.changed" && e.payload["source"] == "playback" {
            frames.push(e.payload["frame"].as_u64().unwrap());
        }
        if e.name == "playback.stopped" {
            break e.payload;
        }
    };
    assert_eq!((stopped["reason"].clone(), stopped["frame"].clone()), (json!("end"), json!(6)), "09 line 115");
    assert!(frames.windows(2).all(|w| w[0] < w[1]), "frames advance: {frames:?}");
    assert_eq!(frames.last().copied(), Some(6), "the last played frame is announced: {frames:?}");
    let st = ok(&h, "playback.state.get", json!({}));
    assert_eq!((st["playing"].clone(), st["frame"].clone()), (json!(false), json!(6)));
    // A navigation stops a playing clock first (09 line 138).
    ok(&h, "playback.loop.set", json!({"enabled": true}));
    let rx = h.subscribe();
    ok(&h, "playback.start", json!({}));
    ok(&h, "timeline.currentFrame.set", json!({"frame": 3}));
    let reason = rx.try_iter().find(|e| e.name == "playback.stopped").map(|e| e.payload["reason"].clone());
    assert_eq!(reason, Some(json!("navigation")));
}
