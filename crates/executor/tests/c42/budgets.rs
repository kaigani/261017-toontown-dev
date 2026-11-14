//! Budgets of the kernel side (plan §6): document-thread holds of the view commands, the gesture-to-command path of
//! `pointer`, and the mapping function. Each call is one document-thread request, so its round trip from this thread
//! bounds its hold from above (the executor is otherwise idle). Measured with a monotonic clock; bounds are at least
//! twice the workstation's debug-build values; the numbers are printed for the handoff.

use std::time::{Duration, Instant};

use serde_json::json;
use tt_executor::shell::{self, Point, Size, ViewTransform, ViewType};

use crate::common::*;
use crate::equivalence::{drag, pointer, ready};

fn p95(mut v: Vec<Duration>) -> (Duration, Duration) {
    v.sort();
    let i = (v.len() * 95).div_ceil(100).saturating_sub(1);
    (v[i], *v.last().unwrap())
}

#[test]
fn view_commands_hold_the_document_thread_briefly() {
    let h = ready();
    let calls = [
        ("view.zoom.set", json!({ "viewId": "drawingView", "steps": 1 })),
        ("view.rotation.set", json!({ "viewId": "drawingView", "steps": 1 })),
        ("view.pan.set", json!({ "viewId": "drawingView", "by": { "x": 7, "y": -3 } })),
        ("view.zoom.set", json!({ "viewId": "drawingView", "steps": -1 })),
        ("draw.viewMapping.get", json!({ "view": "drawing" })),
    ];
    let mut times = Vec::new();
    for _ in 0..40 {
        for (id, p) in &calls {
            let t = Instant::now();
            ok(&h, id, p.clone());
            times.push(t.elapsed());
        }
    }
    let (p, max) = p95(times);
    eprintln!("view commands: p95 {p:?}, max {max:?} (200 calls)");
    assert!(p <= Duration::from_millis(20), "plan §6: a view command holds the document thread at most 20 ms ({p:?})");
}

#[test]
fn a_gesture_reaches_its_command_quickly() {
    let h = ready();
    // A 100-sample Grabber drag: grouping, the mapping at the press, `to_command` and a cheap `view.pan.set`.
    let pts: Vec<(f64, f64)> = (0..100).map(|i| (900.0 + f64::from(i), 500.0 + f64::from(i) * 0.5)).collect();
    let mut nav = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let env = pointer(&h, "grabber", drag(&pts));
        nav.push(t.elapsed());
        assert!(env.is_ok(), "{env:?}");
    }
    // A 100-sample Brush stroke into the sample drawing: the same path plus `draw.stroke.add` (fit, planar map).
    let mut ink = Vec::new();
    for k in 0..10 {
        let pts: Vec<(f64, f64)> =
            (0..100).map(|i| (800.0 + f64::from(i) * 3.0, 300.0 + f64::from(k) * 40.0 + f64::from(i % 7))).collect();
        let t = Instant::now();
        let env = pointer(&h, "brush", drag(&pts));
        ink.push(t.elapsed());
        assert!(env.is_ok(), "{env:?}");
    }
    let ((np, nmax), (ip, imax)) = (p95(nav), p95(ink));
    eprintln!("gesture to command (100-sample Grabber): p95 {np:?}, max {nmax:?}");
    eprintln!("gesture to committed stroke (100-sample Brush): p95 {ip:?}, max {imax:?}");
    assert!(np <= Duration::from_millis(20), "the gesture path itself within the 20 ms hold ({np:?})");
    assert!(ip <= Duration::from_millis(250), "a 100-sample brush gesture committed ({ip:?})");
}

#[test]
fn the_mapping_function_is_cheap_and_round_trips() {
    let size = Size { width: 1920.0, height: 1080.0 };
    let ppu = shell::pixels_per_unit(ViewType::Drawing, 1920.0);
    let t = ViewTransform { zoom: 2.5, rotation_degrees: 37.0, centre: Point { x: 120.0, y: -40.0 } };
    let n = 100_000u32;
    let start = Instant::now();
    let mut worst = 0.0f64;
    for i in 0..n {
        let p = Point { x: f64::from(i % 1920), y: f64::from(i / 1920) };
        let d = shell::view_to_doc(&t, size, ppu, p);
        let q = shell::doc_to_view(&t, size, ppu, d);
        worst = worst.max((q.x - p.x).abs().max((q.y - p.y).abs()));
    }
    let took = start.elapsed();
    eprintln!(
        "mapping: {n} round trips in {took:?} ({:.0} ns each), worst error {worst:e} px",
        took.as_nanos() as f64 / f64::from(n)
    );
    assert!(worst < 1e-9, "{worst}");
    assert!(took < Duration::from_millis(200), "{took:?}");
}

/// The latency of a user query submitted while `id` runs from another thread, once the call is on the document thread
/// (or its job is running): the minimum of three samples, so one scheduler stall cannot decide.
fn query_latency_during(id: &'static str, params: serde_json::Value, wait_for_job: bool) -> Duration {
    let mut samples = Vec::new();
    for _ in 0..3 {
        let h = std::sync::Arc::new(ready());
        crate::equivalence::painted_rect(&h);
        let h2 = h.clone();
        let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let s2 = started.clone();
        let p = params.clone();
        let worker = std::thread::spawn(move || {
            s2.store(true, std::sync::atomic::Ordering::SeqCst);
            h2.invoke(tt_executor::Invocation::user(id, p))
        });
        while !started.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::yield_now();
        }
        if wait_for_job {
            // The capture's job is running: the document thread has answered the snapshot.
            let t = Instant::now();
            while t.elapsed() < Duration::from_secs(5) {
                let jobs = ok(&h, "job.list", json!({}));
                if jobs.to_string().contains(id) {
                    break;
                }
                std::thread::yield_now();
            }
        } else {
            // Give the call a moment to reach the document thread (yielding, no sleep inside the measured bound).
            let t = Instant::now();
            while t.elapsed() < Duration::from_millis(10) {
                std::thread::yield_now();
            }
        }
        let t = Instant::now();
        ok(&h, "view.transform.get", json!({ "viewId": "drawingView" }));
        samples.push(t.elapsed());
        assert!(worker.join().unwrap().is_ok());
    }
    samples.into_iter().min().unwrap()
}

#[test]
fn a_capture_never_holds_the_document_thread_beyond_the_hold_budget() {
    let shot = query_latency_during("inspect.screenshot", json!({}), true);
    eprintln!("a user query during a 1920 x 1080 capture waited {shot:?}");
    assert!(shot <= Duration::from_millis(20), "design note 5 S5: the capture runs off the document thread ({shot:?})");
    // For the record (M11 C11.1): the renders sharing the rasteriser still hold the document thread while they run.
    let render = query_latency_during(
        "draw.drawing.render",
        json!({ "width": 1920, "height": 1080, "region": { "x": -960, "y": -540, "width": 1920, "height": 1080 } }),
        false,
    );
    let frame = query_latency_during("render.frame.run", json!({ "scale": 1.0 }), false);
    eprintln!(
        "a user query during draw.drawing.render 1920 x 1080 waited {render:?}; during render.frame.run 1920 x 1080 {frame:?}"
    );
}
