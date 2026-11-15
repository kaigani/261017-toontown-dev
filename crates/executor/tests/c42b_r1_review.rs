//! Independent scene and budget probes, C4.2b round 1.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;

fn ready() -> tt_executor::ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"", "frame":21}));
    h
}

#[test]
fn onion_uses_current_exposure_occurrence() {
    let h = ready();
    ok(&h, "drawAid.onionSkin.set", json!({"enabled":true,"previous":1,"next":1}));
    let s = h.canvas_scene("drawingView").unwrap();
    let onions: Vec<_> = s.entries.iter().filter(|e| e.role == "onion").collect();
    assert_eq!(onions.len(), 1, "last run has one previous and no next drawing");
    assert_eq!(
        onions[0].tint,
        Some([255, 0, 0, 255]),
        "02 section 10.2: at the second occurrence of drawing 1, drawing 2 is previous, not next"
    );
}

#[test]
fn scene_and_load_are_repeatable_pure_queries() {
    let h = ready();
    let before = rev(&h);
    let history = ok(&h, "edit.history.get", json!({}));
    let events = h.subscribe();
    let a = h.canvas_scene("drawingView").unwrap();
    let p = h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
    assert_eq!(a, h.canvas_scene("drawingView").unwrap());
    assert_eq!(p, h.canvas_drawing(None, 1, 0, Default::default()).unwrap());
    assert_eq!(before, rev(&h));
    assert_eq!(history, ok(&h, "edit.history.get", json!({})));
    assert!(events.try_recv().is_err());
    let current = a.entries.iter().find(|e| e.role == "current").unwrap();
    assert_eq!(current.rev, p.rev);
    assert_eq!(current.drawing_id, p.drawing_id);
}

#[test]
fn large_drawing_budget_receipt() {
    use std::time::Instant;
    let h = std::sync::Arc::new(ready());
    let t = Instant::now();
    for k in 0..100 {
        let points: Vec<_> = (0..500)
            .map(|i| {
                json!({
                    "x": -600.0 + f64::from(i)*2.4,
                    "y": -400.0 + f64::from(k)*8.0 + (f64::from(i)*0.3).sin()*6.0,
                    "pressure":0.6
                })
            })
            .collect();
        ok(&h, "draw.stroke.add", json!({"kind":"brush","points":points,"fit":false,"layer":"top"}));
    }
    eprintln!("fixture 50,000 points: {:?}", t.elapsed());
    let mut scene = Vec::new();
    for _ in 0..200 {
        let t = Instant::now();
        h.canvas_scene("drawingView").unwrap();
        scene.push(t.elapsed());
    }
    scene.sort();
    eprintln!("scene 200 samples: best {:?}, median {:?}, worst {:?}", scene[0], scene[100], scene[199]);
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other = h.clone();
    let g = gate.clone();
    let worker = std::thread::spawn(move || {
        g.wait();
        for _ in 0..400 {
            other.canvas_scene("drawingView").unwrap();
        }
    });
    gate.wait();
    let mut user = Vec::new();
    for _ in 0..200 {
        let t = Instant::now();
        ok(&h, "view.transform.get", json!({"viewId":"drawingView"}));
        user.push(t.elapsed());
    }
    worker.join().unwrap();
    user.sort();
    eprintln!("user under repeated scene requests 200 samples: median {:?}, worst {:?}", user[100], user[199]);
    let mut loads = Vec::new();
    for _ in 0..3 {
        let t = Instant::now();
        let p = h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
        loads.push(t.elapsed());
        eprintln!("load: {:?}, {} bytes", t.elapsed(), p.bytes.len());
        assert!(p.bytes.len() <= 64 << 20);
    }
    eprintln!("best load {:?}", loads.iter().min().unwrap());
    let t = Instant::now();
    ok(
        &h,
        "draw.stroke.add",
        json!({"kind":"brush","points":[{"x":0,"y":0},{"x":10,"y":1}],"fit":false,"layer":"top"}),
    );
    eprintln!("small edit commit {:?}", t.elapsed());
    let t = Instant::now();
    h.canvas_scene("drawingView").unwrap();
    let p = h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
    eprintln!("small edit scene + full load {:?}, {} bytes", t.elapsed(), p.bytes.len());
    let events: Vec<_> = (0..10_000)
        .map(|i| {
            json!({
                "type":if i==0 {"down"} else if i==9999 {"up"} else {"move"},
                "x":400.0+f64::from(i%1000)*1.1,"y":200.0+f64::from(i/1000)*60.0
            })
        })
        .collect();
    let t = Instant::now();
    let e = h.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":"brush","events":events}));
    assert!(e.is_ok(), "{e:?}");
    let commit = t.elapsed();
    h.canvas_scene("drawingView").unwrap();
    let p = h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
    eprintln!(
        "10k gesture on large drawing: commit {commit:?}, through scene + load {:?}, {} bytes",
        t.elapsed(),
        p.bytes.len()
    );
}
