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

mod part_b_scene {
    use super::*;
    #[test]
    fn defaults_grid_background_context_and_camera_only_top() {
        let h = ready();
        let s = h.canvas_scene("drawingView").unwrap();
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].role, "current");
        assert_eq!(s.background, [240, 240, 240, 255]);
        assert!(!s.grid.visible);
        for ty in ["normal", "field12", "field16"] {
            for overlay in [false, true] {
                ok(&h, "drawAid.grid.set", json!({"visible":true,"type":ty,"overlay":overlay}));
                let s = h.canvas_scene("drawingView").unwrap();
                assert_eq!(s.grid.ty, ty);
                assert_eq!(s.grid.overlay, overlay);
            }
        }
        ok(&h, "drawAid.background.set", json!({"colour":"#102030FF"}));
        assert_eq!(h.canvas_scene("drawingView").unwrap().background, [16,32,48,255]);
        let before = h.canvas_scene("drawingView").unwrap();
        for enabled in [true, false] {
            ok(&h, "drawAid.currentOnTop.set", json!({"enabled":enabled}));
            assert_eq!(h.canvas_scene("drawingView").unwrap(), before);
        }
        ok(&h, "app.context.set", json!({"frame":10}));
        let next = h.canvas_scene("drawingView").unwrap();
        assert_ne!(next.entries[0].drawing_id, s.entries[0].drawing_id);
        let payload = h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
        assert_eq!(next.entries[0].rev, payload.rev);
        assert_eq!(next.entries[0].drawing_id, payload.drawing_id);

    }
}

mod part_b_attached {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tt_executor::{AttachedIo, CapturedPng, CommandError, Envelope, Origin};
    #[derive(Debug)]
    struct Io {
        answer: Option<Envelope>,
        requests: Mutex<Vec<serde_json::Value>>,
    }
    impl AttachedIo for Io {
        fn route_pointer(&self, _: &str, _: &Origin, request: &serde_json::Value, _: Duration) -> Option<Envelope> {
            self.requests.lock().unwrap().push(request.clone());
            self.answer.clone()
        }
        fn capture(&self, _: &str, _: Option<[f64;4]>, _: u32, _: Duration) -> Result<CapturedPng, CommandError> {
            Err(CommandError::new("view.notFound", "no shown view"))
        }
    }
    #[test]
    fn routed_envelopes_scope_and_refusal_purity() {
        let h = ready();
        let request = json!({"view":"drawing", "events":[]});
        let answer = Envelope::ok(json!({"commands":[{"status":"ok","result":{"probe":7}}],"hover":null}), Some(19));
        let io = Arc::new(Io {answer:Some(answer.clone()),requests:Mutex::default()});
        h.set_attached_io(Some(io.clone()));
        let before = rev(&h);
        let history = ok(&h,"edit.history.get",json!({}));
        let selection = ok(&h,"draw.selection.get",json!({}));
        let all = agent(&tt_registry::Scope::ALL);
        let got = h.pointer(all.clone(),request.clone());
        assert_eq!(serde_json::to_value(got).unwrap(),serde_json::to_value(answer).unwrap());
        assert_eq!(*io.requests.lock().unwrap(),vec![request.clone()]);
        assert!(!h.pointer(agent(&[]),request.clone()).is_ok());
        assert_eq!(io.requests.lock().unwrap().len(),1);
        for code in ["ui.busy","command.timeout","render.failed"] {
            let answer = Envelope::error(CommandError::with_details(code,"interrupted",json!({"reason":"userInput"})),None);
            let io = Arc::new(Io {answer:Some(answer.clone()),requests:Mutex::default()});
            h.set_attached_io(Some(io));
            assert_eq!(serde_json::to_value(h.pointer(all.clone(),request.clone())).unwrap(),serde_json::to_value(answer).unwrap());
            assert_eq!(rev(&h),before);
            assert_eq!(ok(&h,"edit.history.get",json!({})),history);
            assert_eq!(ok(&h,"draw.selection.get",json!({})),selection);
        }
        h.set_wait_for_user(Duration::from_millis(40));
        h.report_shell(serde_json::from_str(r#"{"modal":{"kind":"gesture","title":"drawing"}}"#).unwrap());
        let start = std::time::Instant::now();
        assert_eq!(h.pointer(all.clone(), request.clone()).error.unwrap().code, "ui.busy");
        assert!(start.elapsed() >= Duration::from_millis(30));
        let start = std::time::Instant::now();
        let e = h.invoke(tt_executor::Invocation { command:"drawAid.grid.set".into(), params:json!({"visible":true}), options:Default::default(), origin:all.clone() });
        assert_eq!(e.error.unwrap().code,"ui.busy");
        assert!(start.elapsed() >= Duration::from_millis(30));
        let start = std::time::Instant::now();
        let q = h.invoke(tt_executor::Invocation { command:"drawAid.state.get".into(), params:json!({}), options:Default::default(), origin:all.clone() });
        assert!(q.is_ok());
        assert!(start.elapsed() < Duration::from_secs(1));
        h.report_shell(serde_json::from_str(r#"{"modal":null}"#).unwrap());
        let io = Arc::new(Io {answer:None,requests:Mutex::default()});
        h.set_attached_io(Some(io));
        assert!(h.pointer(all,request).is_ok());
    }
}
