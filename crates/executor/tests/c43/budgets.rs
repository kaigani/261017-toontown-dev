//! Kernel-side budgets of the Camera view (plan §6; ruling D-5): the document-thread hold of a request (the scene
//! answer with its occurrences, a pick, a mapping) is at most 20 ms. Each figure is a whole round trip through the
//! executor handle, which bounds the hold from above, on an otherwise idle executor, measured with a monotonic clock.
//!
//! Ruling D-27: no assertion sits near its budget. CI asserts the median of 15 requests on a 200-element scene
//! (at most a quarter of the budget in a debug build on the workstation; the median absorbs scheduler spikes); the
//! 1,000-element figures are printed by the ignored probe and recorded in the handoff (release profile, workstation).

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

/// The median and the largest of 15 timed runs of `f`.
fn time(mut f: impl FnMut()) -> (Duration, Duration) {
    let mut v: Vec<Duration> = (0..15)
        .map(|_| {
            let s = Instant::now();
            f();
            s.elapsed()
        })
        .collect();
    v.sort();
    (v[v.len() / 2], v[v.len() - 1])
}

/// The four requests the budget names, timed on `h`: (name, median, max).
fn measure(h: &ExecutorHandle, n: usize) -> Vec<(&'static str, Duration, Duration)> {
    let scene = time(|| {
        let s = h.canvas_scene("cameraView").unwrap();
        assert!(s.entries.len() >= n);
    });
    // A press of the Select tool picks among every element (05 §3.5), then sets the selection.
    let mut at = 0.0;
    let pick = time(|| {
        at += 1.0;
        let env = h.pointer(
            Origin::User,
            json!({ "view": "cameraView", "tool": "select", "events": click(300.0 + at, 200.0) }),
        );
        assert!(env.is_ok(), "{env:?}");
    });
    // The pick's command alone, for the handoff's breakdown (the press's hold is the pick plus this).
    let command = time(|| {
        ok(h, "edit.selection.set", json!({ "context": "staging", "ids": [], "mode": "replace" }));
    });
    let hit = time(|| {
        ok(h, "inspect.hitTest", json!({ "space": "view", "view": "cameraView", "point": { "x": 480, "y": 270 } }));
    });
    let pts: Vec<Value> = (0..20).map(|i| json!({ "x": f64::from(i) * 0.25, "y": -1.0 })).collect();
    let mapping = time(|| {
        ok(h, "anim.view.coords.get", json!({ "viewId": "cameraView", "points": pts, "from": "scene" }));
    });
    vec![
        ("scene answer", scene.0, scene.1),
        ("pick (select press)", pick.0, pick.1),
        ("edit.selection.set alone", command.0, command.1),
        ("inspect.hitTest", hit.0, hit.1),
        ("anim.view.coords.get", mapping.0, mapping.1),
    ]
}

#[test]
fn the_camera_views_requests_hold_the_document_thread_within_budget() {
    let h = many(200);
    for (what, median, max) in measure(&h, 200) {
        eprintln!("200 elements: {what}: median {median:?}, max {max:?}");
        assert!(median <= Duration::from_millis(20), "{what}: median {median:?} over the 20 ms hold (plan §6)");
    }
}

#[test]
#[ignore = "probe: run with --ignored --nocapture (figures for the handoff)"]
fn probe_the_camera_view_on_many_elements() {
    for n in [200, 1000] {
        let h = many(n);
        for (what, median, max) in measure(&h, n) {
            eprintln!("{n} elements: {what}: median {median:?}, max {max:?}");
        }
    }
}
