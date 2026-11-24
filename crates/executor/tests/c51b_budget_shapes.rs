//! M5 C5.1b, plan §6.2: the CI shapes of two Timeline budgets (D-45: ratios and ceilings that catch a gross regression;
//! the budgets themselves are the workstation receipt, `examples/c51b_budgets.rs`).
//!
//! - `xsheet.exposures.get` for 60 rows x 200 frames: twice the rows about twice the time (linear about 2, quadratic about
//!   4), and a ceiling of 50 ms;
//! - a release to its committed state through the timing-gesture interpreter with 1,000 rows: a ceiling of 1 s.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

fn best(f: impl Fn()) -> Duration {
    (0..5)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap()
}

fn drawings(h: &ExecutorHandle, n: usize) -> Vec<String> {
    let made = ok(h, "element.create", json!({"type": "drawing", "count": n, "name": "Row"}));
    let ids: Vec<String> =
        made["elementIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    for id in &ids {
        ok(h, "element.drawing.create", json!({"elementId": id, "name": "1", "exposeFrom": 1, "exposeTo": 200}));
    }
    ids
}

#[test]
fn exposures_for_a_window_grow_with_the_rows() {
    let h = fixture();
    let ids = drawings(&h, 60);
    let read = |n: usize| {
        let window: Vec<&String> = ids.iter().take(n).collect();
        best(|| {
            ok(&h, "xsheet.exposures.get", json!({"elementIds": window, "from": 1, "to": 200}));
        })
    };
    let (half, full) = (read(30), read(60));
    let doubling = full.as_secs_f64() / half.max(Duration::from_micros(50)).as_secs_f64();
    eprintln!("xsheet.exposures.get 200 frames: 30 rows {half:?}, 60 rows {full:?}, doubling ratio {doubling:.2}");
    assert!(doubling <= 3.0, "exposures grow faster than the rows when they double: {doubling:.2}");
    assert!(full < Duration::from_millis(50), "60 rows x 200 frames over the ceiling: {full:?}");
}

#[test]
fn a_release_commits_within_the_ceiling_at_one_thousand_rows() {
    let h = fixture();
    let mut built = 0;
    while built < 990 {
        let n = (990 - built).min(99);
        ok(&h, "element.create", json!({"type": if built % 2 == 0 { "peg" } else { "drawing" }, "count": n}));
        built += n;
    }
    let rows: Vec<Value> = ok(&h, "timeline.rows.list", json!({}))["rows"].as_array().unwrap().clone();
    assert!(rows.len() >= 1000, "{} rows", rows.len());
    // The fixture's Fade (a bar over frames 1-48) dragged 3 frames, its row scrolled to the top.
    let fade = rows.iter().position(|r| r["name"] == "Fade").unwrap() as f64;
    let layout = json!({"surface": "tracks", "zoomLevel": 6, "firstFrame": 1, "scrollTop": fade * 22.0,
                        "platform": "windows"});
    let s = |x: f64| json!({"x": x, "y": 11.0, "modifiers": []});
    let samples = json!([s(320.0), s(340.0), s(368.0)]);
    let press =
        h.timing_gesture(Origin::User, json!({"phase": "press", "layout": layout, "sample": samples[0]})).unwrap();
    let t = Instant::now();
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    let took = t.elapsed();
    eprintln!("release to commit at {} rows: {took:?} ({})", rows.len(), out["plan"]);
    assert_eq!(out["plan"]["commands"][0][0], "element.timing.set", "{out}");
    assert!(out["envelopes"].as_array().unwrap().iter().all(|e| e["status"] == "ok"), "{out}");
    assert!(took < Duration::from_secs(1), "a release over the ceiling: {took:?}");
}
