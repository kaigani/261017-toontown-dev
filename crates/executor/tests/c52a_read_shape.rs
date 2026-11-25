//! M5 C5.2a: the Exposure Sheet's read at scale (the C5.2 plan §11; D-45). The window reads the cells in view: a window
//! of 40 columns by 60 frames (`xsheet.exposures.get {elementIds, from, to}`) on scenes of 40, about 500 and about 1,000
//! columns, each 9,999 frames long with a run of exposures in every column. The read's cost follows the window, not the
//! scene: against the 40-column scene and when the columns double, a ratio near 1 (asserted as at most linear), with a
//! ceiling that only catches a gross regression. The budget itself (5 ms p95 for the window, release) is a workstation
//! receipt. The scenes are measured in turns, ten rounds, the best of each (the lesson of `c51_gestures`'s ratio), and
//! every sample is printed when an assertion fails.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const FRAMES: u32 = 9_999;

/// A new project's scene with `columns` drawing elements, each exposing drawings in runs over 9,999 frames; the IDs.
fn scene(columns: usize) -> (ExecutorHandle, Vec<String>) {
    let h = fixture_with(tt_executor::ExecutorConfig { budget: Duration::from_secs(120) });
    ok(&h, "project.create", json!({ "name": "Sheet at scale", "onUnsaved": "discard" }));
    let mut ids: Vec<String> = Vec::new();
    while ids.len() < columns {
        let n = (columns - ids.len()).min(99);
        let made =
            ok(&h, "element.create", json!({ "type": "drawing", "count": n, "name": format!("C{}", ids.len()) }));
        ids.extend(made["elementIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()));
    }
    // Runs of 12 frames cycling four drawings, written 100 columns per call (created with the call).
    let runs: Vec<Value> = (0..FRAMES.div_ceil(12))
        .map(|k| {
            let from = k * 12 + 1;
            json!({ "drawing": format!("{}", k % 4 + 1), "from": from, "to": (from + 11).min(FRAMES) })
        })
        .collect();
    for chunk in ids.chunks(100) {
        let cols: Vec<Value> =
            chunk.iter().map(|e| json!({ "elementId": e, "from": 1, "to": FRAMES, "exposures": runs })).collect();
        ok(&h, "xsheet.exposures.set", json!({ "columns": cols, "createMissingDrawings": true }));
    }
    (h, ids)
}

#[test]
fn the_sheets_window_read_follows_the_window_not_the_scene() {
    let built = Instant::now();
    let scenes = [scene(40), scene(500), scene(1000)];
    eprintln!("scenes built in {:?}", built.elapsed());
    // The window in view: 40 columns from the middle of the scene, frames 5,000 to 5,059.
    let window = |ids: &[String]| -> Value {
        let mid = ids.len() / 2;
        let lo = mid.saturating_sub(20);
        json!({ "elementIds": ids[lo..lo + 40], "from": 5000, "to": 5059 })
    };
    let read = |(h, ids): &(ExecutorHandle, Vec<String>)| -> Duration {
        let t = Instant::now();
        let sheet = ok(h, "xsheet.exposures.get", window(ids));
        let took = t.elapsed();
        assert_eq!(sheet["columns"].as_array().unwrap().len(), 40);
        took
    };
    let mut samples: [Vec<Duration>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for _ in 0..10 {
        for (k, s) in scenes.iter().enumerate() {
            samples[k].push(read(s));
        }
    }
    let best = |k: usize| samples[k].iter().copied().min().unwrap_or(Duration::MAX);
    let (base, half, large) = (best(0).max(Duration::from_micros(200)), best(1), best(2));
    let shown = format!("samples (40, 500, 1,000 columns): {samples:?}");
    let ratio = large.as_secs_f64() / base.as_secs_f64();
    let doubling = large.as_secs_f64() / half.max(Duration::from_micros(200)).as_secs_f64();
    eprintln!(
        "xsheet.exposures.get 40 x 60: 40 columns {base:?}, 500 {half:?}, 1,000 {large:?}; ratio {ratio:.2}, doubling {doubling:.2}"
    );
    // At most linear in the scene's columns (25 times the columns); the read follows the window, about 1.
    assert!(ratio <= 25.0, "the window's read grows with the scene: ratio {ratio:.2}; {shown}");
    assert!(
        doubling <= 2.5,
        "the window's read grows faster than the columns when they double: {doubling:.2}; {shown}"
    );
    assert!(large < Duration::from_secs(2), "a gross regression: {large:?}; {shown}");
}
