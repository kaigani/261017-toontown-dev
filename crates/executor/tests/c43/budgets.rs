//! Kernel-side budgets of the Camera view (plan §6; ruling D-5): the document-thread hold of a request (the scene
//! answer with its occurrences, a pick, a mapping) is at most 20 ms. Each figure is a whole round trip through the
//! executor handle, which bounds the hold from above, on an otherwise idle executor, measured with a monotonic clock.
//!
//! Ruling D-45 (D-41 made general): no CI assertion holds an absolute wall-clock budget the slowest runner meets with
//! less than a factor of four to spare (a macOS runner's pick median was 22.2 ms against the 20 ms hold, run
//! 43867592907). CI asserts the shape: each request's median on a 200-element scene against the same request on a
//! 20-element scene in the same process, within `RATIO`, under an absolute `CEILING` that only catches a gross
//! regression. The 20 ms budget itself is asserted by the workstation receipt `camera_budget_receipt` (ignored; debug
//! and release, 30 samples, 200 and 1,000 elements), regenerated at each handoff that touches the code it measures.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

use crate::common::*;

/// A scene of `n` drawing elements in a grid, each showing a small drawing at frame 1.
fn many(n: usize) -> ExecutorHandle {
    let h = fixture();
    // `element.create` makes at most 99 at a time.
    let mut ids = Vec::new();
    while ids.len() < n {
        let k = (n - ids.len()).min(99);
        let made = ok(&h, "element.create", json!({ "type": "drawing", "count": k }))["elementIds"].clone();
        ids.extend(made.as_array().unwrap().iter().cloned());
    }
    for (i, id) in ids.iter().enumerate() {
        ok(&h, "element.drawing.create", json!({ "elementId": id, "name": "1", "exposeFrom": 1, "exposeTo": 1 }));
        ok(&h, "app.context.set", json!({ "elementId": id, "frame": 1 }));
        ok(&h, "draw.rectangle.add", json!({ "from": { "x": -20, "y": -20 }, "to": { "x": 20, "y": 20 } }));
        let (x, y) = ((i % 40) as f64 * 0.3 - 6.0, (i / 40) as f64 * 0.3 - 3.5);
        ok(&h, "anim.transform.set", json!({ "nodeIds": [id], "target": "static", "position": { "x": x, "y": y } }));
    }
    ok(&h, "view.active.set", json!({ "viewId": "cameraView" }));
    h
}

fn click(x: f64, y: f64) -> Value {
    json!([
        { "type": "down", "x": x, "y": y, "modifiers": [] },
        { "type": "up", "x": x, "y": y, "modifiers": [] },
    ])
}

/// `reps` timed runs of `f`, sorted.
fn time(reps: usize, mut f: impl FnMut()) -> Vec<Duration> {
    let mut v: Vec<Duration> = (0..reps)
        .map(|_| {
            let s = Instant::now();
            f();
            s.elapsed()
        })
        .collect();
    v.sort();
    v
}

/// The requests the budget names, each timed `reps` times on `h`: (name, sorted samples).
fn measure(h: &ExecutorHandle, n: usize, reps: usize) -> Vec<(&'static str, Vec<Duration>)> {
    let scene = time(reps, || {
        let s = h.canvas_scene("cameraView").unwrap();
        assert!(s.entries.len() >= n);
    });
    // A press of the Select tool picks among every element (05 §3.5), then sets the selection.
    let mut at = 0.0;
    let pick = time(reps, || {
        at += 1.0;
        let env = h.pointer(
            Origin::User,
            json!({ "view": "cameraView", "tool": "select", "events": click(300.0 + at, 200.0) }),
        );
        assert!(env.is_ok(), "{env:?}");
    });
    // The pick's command alone, for the handoff's breakdown (the press's hold is the pick plus this).
    let command = time(reps, || {
        ok(h, "edit.selection.set", json!({ "context": "staging", "ids": [], "mode": "replace" }));
    });
    let hit = time(reps, || {
        ok(h, "inspect.hitTest", json!({ "space": "view", "view": "cameraView", "point": { "x": 480, "y": 270 } }));
    });
    let pts: Vec<Value> = (0..20).map(|i| json!({ "x": f64::from(i) * 0.25, "y": -1.0 })).collect();
    let mapping = time(reps, || {
        ok(h, "anim.view.coords.get", json!({ "viewId": "cameraView", "points": pts, "from": "scene" }));
    });
    vec![
        ("scene answer", scene),
        ("pick (select press)", pick),
        ("edit.selection.set alone", command),
        ("inspect.hitTest", hit),
        ("anim.view.coords.get", mapping),
    ]
}

fn median(v: &[Duration]) -> Duration {
    v[v.len() / 2]
}

/// The CI form (ruling D-45): a 200-element figure at most `RATIO` times the 20-element one (taken at least `FLOOR`:
/// a figure of a few tenths of a millisecond is the timer's), and never above `CEILING`.
/// Measured on the workstation (debug, three runs; the ratio against the figure taken at least `FLOOR`): the pick, which
/// visits every element, 2.2 to 2.3 (2.0 ms on 20 elements, 4.5 to 4.8 ms on 200); the scene answer 1.2 to 1.4;
/// `inspect.hitTest` 1.7 to 1.8; the others below 1. The worst runner figure seen, a macOS pick median of 22.2 ms on
/// 200 elements (run 43867592907), is 4.5 times below the ceiling.
const RATIO: u32 = 20;
const FLOOR: Duration = Duration::from_millis(1);
const CEILING: Duration = Duration::from_millis(100);

#[test]
fn the_camera_views_requests_keep_their_shape() {
    let small = many(20);
    let large = many(200);
    // Each scene measured twice, alternating, and the smaller median of the two kept (a runner's scheduler spike in
    // one pass does not decide the shape).
    let mut figures: Vec<(&'static str, Duration, Duration)> = Vec::new();
    let (s1, l1, s2, l2) =
        (measure(&small, 20, 15), measure(&large, 200, 15), measure(&small, 20, 15), measure(&large, 200, 15));
    for k in 0..s1.len() {
        let s = median(&s1[k].1).min(median(&s2[k].1));
        let l = median(&l1[k].1).min(median(&l2[k].1));
        figures.push((s1[k].0, s, l));
    }
    for (what, s, l) in &figures {
        let ratio = l.as_secs_f64() / s.max(&FLOOR).as_secs_f64();
        eprintln!("{what}: 20 elements {s:?}, 200 elements {l:?}, ratio {ratio:.1}");
    }
    for (what, s, l) in figures {
        assert!(
            l <= s.max(FLOOR) * RATIO && l <= CEILING,
            "{what}: 200 elements {l:?} against 20 elements {s:?} (at most {RATIO} x, ceiling {CEILING:?}; ruling D-45)"
        );
    }
}

/// Plan §6's budget on the workstation (ruling D-45's receipt): each request's median of 30 holds the document thread
/// at most 20 ms on 200 and on 1,000 elements. Run in debug and release with `--ignored --nocapture`;
/// `TT_CAMERA_RECEIPT` names the file the receipt is written to (`reviews/evidence/M4-C4.3b/camera-budget/`).
#[test]
#[ignore = "workstation receipt (ruling D-45): run with --ignored --nocapture, debug and release"]
fn camera_budget_receipt() {
    let r2 = |d: Duration| (d.as_secs_f64() * 100_000.0).round() / 100.0;
    let mut rows = serde_json::Map::new();
    let mut over = Vec::new();
    for n in [200, 1000] {
        let h = many(n);
        for (what, v) in measure(&h, n, 30) {
            let at = |q: f64| v[((q * v.len() as f64).ceil() as usize).saturating_sub(1).min(v.len() - 1)];
            rows.insert(
                format!("{n} elements: {what}"),
                json!({ "n": v.len(), "median": r2(median(&v)), "p95": r2(at(0.95)), "max": r2(at(1.0)),
                        "samples": v.iter().map(|d| r2(*d)).collect::<Vec<_>>() }),
            );
            if median(&v) > Duration::from_millis(20) {
                over.push(format!("{n} elements: {what}: median {:?}", median(&v)));
            }
        }
    }
    let receipt = json!({
        "kind": "toontown.c43.camera-budget",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "budgetMs": 20,
        "figures": rows,
    });
    eprintln!("{}", serde_json::to_string_pretty(&receipt).unwrap());
    if let Ok(path) = std::env::var("TT_CAMERA_RECEIPT") {
        std::fs::write(&path, serde_json::to_string_pretty(&receipt).unwrap() + "\n").unwrap();
    }
    assert!(over.is_empty(), "plan §6's 20 ms hold: {over:?}");
}
