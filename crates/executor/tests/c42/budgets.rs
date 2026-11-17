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
    crate::common::timing::record("c42/budgets.rs:40", p, Duration::from_millis(20));
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
    crate::common::timing::record("c42/budgets.rs:68", np, Duration::from_millis(20));
    assert!(np <= Duration::from_millis(20), "the gesture path itself within the 20 ms hold ({np:?})");
    crate::common::timing::record("c42/budgets.rs:69", ip, Duration::from_millis(250));
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
    crate::common::timing::record("c42/budgets.rs:92", took, Duration::from_millis(200));
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
    crate::common::timing::record("c42/budgets.rs:142", shot, Duration::from_millis(20));
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

/// Ruling D-13 (b): where the time of a 10,000-sample Brush gesture goes. A measurement receipt, not a bound
/// (`cargo test -p tt-executor --test c42 -- --ignored --nocapture long_stroke_split`).
#[test]
#[ignore = "measurement receipt for ruling D-13; prints the split, asserts nothing about time"]
fn long_stroke_split() {
    let n = 10_000u32;
    let pts: Vec<(f64, f64)> =
        (0..n).map(|i| (400.0 + f64::from(i % 1000) * 1.1, 200.0 + f64::from(i / 1000) * 60.0)).collect();
    let best = |f: &dyn Fn() -> Duration| (0..3).map(|_| f()).min().unwrap();
    let pointer_total = best(&|| {
        let h = ready();
        let t = Instant::now();
        assert!(pointer(&h, "brush", drag(&pts)).is_ok());
        t.elapsed()
    });
    let conversion = best(&|| {
        let h = ready();
        let t = Instant::now();
        assert!(pointer(&h, "grabber", drag(&pts)).is_ok());
        t.elapsed()
    });
    let params = {
        let h = ready();
        pointer(&h, "brush", drag(&pts)).result.unwrap()["commands"][1]["params"].clone()
    };
    let direct = |p: serde_json::Value, dry: bool| {
        best(&|| {
            let h = ready();
            let mut inv = tt_executor::Invocation::user("draw.stroke.add", p.clone());
            inv.options.dry_run = dry;
            let t = Instant::now();
            assert!(h.invoke(inv).is_ok());
            t.elapsed()
        })
    };
    let direct_fit = direct(params.clone(), false);
    let mut unfit = params.clone();
    unfit["fit"] = json!(false);
    let direct_unfit = direct(unfit, false);
    let dry_fit = direct(params, true);
    eprintln!(
        "10,000 samples: pointer Brush {pointer_total:?}; direct draw.stroke.add {direct_fit:?} (fit) / {direct_unfit:?} (no fit); dry run (handler, no commit) {dry_fit:?}; pointer Grabber (grouping and conversion, trivial command) {conversion:?}"
    );
}

/// C42B-R1-04's kernel half (plan §6 release-to-committed, large; release profile): on the reviewer's `d1-long` fixture
/// (100 brush strokes of 500 points through `draw.stroke.add` with the default layer), thirty samples each, every one
/// undone so all commit to the same 50,000-point drawing: the reviewer's 9,998-event Brush gesture, a short Brush
/// gesture and a short Pencil gesture (the window's gestures); and, on the reviewer's receipt fixture (the same strokes
/// all in one layer, `layer: "top"`), a 2-point brush stroke appended to that layer through `draw.stroke.add`. Each
/// sample is the commit and the canvas's part (`canvas_scene` and `canvas_drawing`, the reload). Prints the
/// percentiles; asserts nothing about time.
#[test]
#[ignore = "measurement receipt for C42B-R1-04 (release profile); prints the percentiles, asserts nothing about time"]
fn strokes_on_the_large_drawing() {
    let fixture = |layer: Option<&str>| {
        let h = ready();
        for k in 0..100 {
            let points: Vec<serde_json::Value> = (0..500)
                .map(|i| json!({ "x": -600.0 + f64::from(i) * 2.4, "y": -400.0 + f64::from(k) * 8.0 + f64::from(i % 20) * 0.3, "pressure": 0.6 }))
                .collect();
            let mut p = json!({ "kind": "brush", "points": points, "fit": false });
            if let Some(l) = layer {
                p["layer"] = json!(l);
            }
            ok(&h, "draw.stroke.add", p);
        }
        h
    };
    let pct = |mut v: Vec<Duration>| {
        v.sort();
        (v[v.len() / 2], v[(v.len() * 95).div_ceil(100) - 1], v[v.len() - 1])
    };
    let measure = |h: &tt_executor::ExecutorHandle, label: &str, commit_once: &dyn Fn() -> bool| {
        let (mut commit, mut total) = (Vec::new(), Vec::new());
        for _ in 0..30 {
            let t = Instant::now();
            assert!(commit_once(), "{label}");
            commit.push(t.elapsed());
            h.canvas_scene("drawingView").unwrap();
            h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
            total.push(t.elapsed());
            ok(h, "edit.undo", json!({}));
        }
        eprintln!("{label}, 30 samples (p50, p95, max): commit {:?}; through the reload {:?}", pct(commit), pct(total));
    };
    let h = fixture(None);
    let n = 9_998u32;
    let long: Vec<(f64, f64)> =
        (0..n).map(|i| (960.0 - 180.0 + f64::from(i) * 240.0 / f64::from(n - 1), 540.0 + 140.0)).collect();
    measure(&h, "9,998-event Brush gesture on the 50,000-point drawing", &|| pointer(&h, "brush", drag(&long)).is_ok());
    let short: Vec<(f64, f64)> = (0..20).map(|i| (700.0 + f64::from(i) * 4.0, 300.0 + f64::from(i % 5))).collect();
    measure(&h, "20-event Brush gesture on the 50,000-point drawing", &|| pointer(&h, "brush", drag(&short)).is_ok());
    measure(&h, "20-event Pencil gesture on the 50,000-point drawing", &|| pointer(&h, "pencil", drag(&short)).is_ok());
    let top = fixture(Some("top"));
    let small =
        json!({ "kind": "brush", "points": [{ "x": 0, "y": 0 }, { "x": 10, "y": 1 }], "fit": false, "layer": "top" });
    measure(&top, "2-point draw.stroke.add into the one 50,000-point layer (layer: top)", &|| {
        top.invoke(tt_executor::Invocation::user("draw.stroke.add", small.clone())).is_ok()
    });
}

/// The large drawing of plan §6: 50,000 points of brush strokes, built through commands (100 strokes of 500 points).
fn large() -> tt_executor::ExecutorHandle {
    let h = ready();
    for k in 0..100 {
        let points: Vec<serde_json::Value> = (0..500)
            .map(|i| {
                let x = -600.0 + f64::from(i) * 2.4;
                let y = -400.0 + f64::from(k) * 8.0 + (f64::from(i) * 0.3).sin() * 6.0;
                json!({ "x": x, "y": y, "pressure": 0.6 })
            })
            .collect();
        ok(&h, "draw.stroke.add", json!({ "kind": "brush", "points": points, "fit": false, "layer": "top" }));
    }
    h
}

#[test]
fn the_canvas_holds_the_document_thread_briefly_and_its_large_payload_fits() {
    let h = std::sync::Arc::new(large());
    // `canvas_scene` is one document-thread read: its round trip bounds its hold.
    let mut scene = Vec::new();
    for _ in 0..40 {
        let t = Instant::now();
        let _ = h.canvas_scene("drawingView").unwrap();
        scene.push(t.elapsed());
    }
    let (p, max) = p95(scene);
    eprintln!("canvas_scene on the large drawing: p95 {p:?}, max {max:?} (40 calls)");
    crate::common::timing::record("c42/budgets.rs:285", p, Duration::from_millis(20));
    assert!(p <= Duration::from_millis(20), "plan §6: the scene read holds the document thread at most 20 ms ({p:?})");
    // The payload: built off the document thread. While it is built, a user query still answers within the hold bound
    // (only the snapshot is taken on the document thread; design note 1 §6).
    let t = Instant::now();
    let builder = {
        let h = h.clone();
        std::thread::spawn(move || h.canvas_drawing(None, 1, 0, tt_executor::canvas::PayloadOptions::default()))
    };
    let mut queries = Vec::new();
    while !builder.is_finished() {
        let q = Instant::now();
        ok(&h, "app.state.get", json!({}));
        queries.push(q.elapsed());
    }
    let payload = builder.join().unwrap().unwrap();
    let produced = t.elapsed();
    let (qp, qmax) = p95(queries.clone());
    eprintln!(
        "large payload: {} bytes in {produced:?}; {} user queries during it: p95 {qp:?}, max {qmax:?}",
        payload.bytes.len(),
        queries.len()
    );
    assert!(
        payload.bytes.len() <= 64 << 20,
        "plan §6: at most 64 MB for 50,000 points ({} bytes)",
        payload.bytes.len()
    );
    crate::common::timing::record("c42/budgets.rs:312", qp, Duration::from_millis(20));
    assert!(qp <= Duration::from_millis(20), "plan §6: the payload's document-thread hold stays within 20 ms ({qp:?})");
    let d = tt_render::canvas::decode(&payload.bytes).unwrap();
    let vertices: usize = d
        .records
        .iter()
        .map(|r| match r {
            tt_render::canvas::Record::Solid { vertices, .. }
            | tt_render::canvas::Record::Gradient { vertices, .. } => vertices.len(),
            tt_render::canvas::Record::Bitmap { .. } => 4,
        })
        .sum();
    eprintln!("large payload: {} records, {vertices} vertices", d.records.len());
    assert!(vertices > 50_000, "the strokes are all there ({vertices} vertices)");
}
