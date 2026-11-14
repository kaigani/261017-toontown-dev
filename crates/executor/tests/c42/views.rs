//! Views and the one mapping (design note 6, invariants V1-V5; SI-063, SI-069, SI-122, SI-123, SI-128).

use serde_json::{Value, json};
use tt_executor::Origin;
use tt_registry::Scope;

use crate::common::*;
use crate::equivalence::{BOB, D1, ready};

fn apply(m: &Value, x: f64, y: f64) -> (f64, f64) {
    let f = |i: usize| m[i].as_f64().unwrap();
    (f(0) * x + f(2) * y + f(4), f(1) * x + f(3) * y + f(5))
}

/// Equal within float rounding (1e-9 plus 1e-11 relative: the published matrices are formed from the mapping function).
fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    let near = |x: f64, y: f64| (x - y).abs() <= 1e-9 + 1e-11 * x.abs().max(y.abs());
    near(a.0, b.0) && near(a.1, b.1)
}

#[test]
fn v4_the_headless_drawing_view_is_1920_by_1080_at_100_percent() {
    let h = ready();
    let m = ok(&h, "draw.viewMapping.get", json!({ "view": "drawing" }));
    assert_eq!(m["drawingToView"], json!([1.0, 0.0, 0.0, -1.0, 960.0, 540.0]), "{m}");
    assert_eq!(m["viewToDrawing"], json!([1.0, 0.0, 0.0, -1.0, -960.0, 540.0]), "{m}");
    assert_eq!((m["zoom"].as_f64(), m["viewSize"].clone()), (Some(1.0), json!({ "width": 1920, "height": 1080 })));
    assert_eq!(m["drawingId"], D1);
}

#[test]
fn v1_one_mapping_round_trips_and_every_surface_agrees() {
    let h = ready();
    for (zoom, degrees, centre) in [(1.0, 0.0, (0.0, 0.0)), (2.0, 30.0, (120.0, -40.0)), (0.333, -60.0, (-500.0, 70.0))]
    {
        ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": zoom }));
        ok(&h, "view.rotation.set", json!({ "viewId": "drawingView", "degrees": degrees }));
        ok(&h, "view.pan.set", json!({ "viewId": "drawingView", "centre": { "x": centre.0, "y": centre.1 } }));
        let m = ok(&h, "draw.viewMapping.get", json!({ "view": "drawingView" }));
        for (x, y) in [(0.0, 0.0), (17.5, -3.25), (1919.0, 1079.0), (-200.0, 4000.0)] {
            let d = ok(
                &h,
                "view.coordinates.convert",
                json!({ "viewId": "drawingView", "point": { "x": x, "y": y }, "from": "view", "to": "drawing" }),
            )["point"]
                .clone();
            let dp = (d["x"].as_f64().unwrap(), d["y"].as_f64().unwrap());
            assert!(
                close(apply(&m["viewToDrawing"], x, y), dp),
                "viewMapping and view.coordinates.convert agree: {:?} {dp:?} {m}",
                apply(&m["viewToDrawing"], x, y)
            );
            let back = apply(&m["drawingToView"], dp.0, dp.1);
            assert!(close(back, (x, y)), "round trip {back:?} vs {x},{y} at zoom {zoom}");
            let i = ok(
                &h,
                "inspect.coordinates.convert",
                json!({ "from": "view", "to": "drawing", "points": [{ "x": x, "y": y }], "view": "drawingView" }),
            )["points"][0]
                .clone();
            assert!(close((i["x"].as_f64().unwrap(), i["y"].as_f64().unwrap()), dp), "inspect agrees");
        }
    }
}

#[test]
fn v2_view_state_is_not_undoable_and_is_kept_per_type() {
    let h = ready();
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "props": { "meta": { "n": 1 } } }));
    ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 4 }));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "view.transform.get", json!({ "viewId": "drawingView" }))["zoom"], 4.0, "undo left the view");
    ok(&h, "view.type.set", json!({ "viewId": "drawingView", "type": "camera" }));
    assert_eq!(ok(&h, "view.transform.get", json!({ "viewId": "drawingView" }))["zoom"], 1.0, "the camera type's own");
    ok(&h, "view.type.set", json!({ "viewId": "drawingView", "type": "drawing" }));
    assert_eq!(ok(&h, "view.transform.get", json!({ "viewId": "drawingView" }))["zoom"], 4.0, "restored per type");
}

#[test]
fn v3_an_agent_moves_views_only_with_let_agent_move_view() {
    let h = ready();
    let a = agent(&[Scope::Ui, Scope::Read]);
    let zoom = |o: Origin, z: f64| {
        h.invoke(tt_executor::Invocation {
            command: "view.zoom.set".into(),
            params: json!({ "viewId": "drawingView", "zoom": z }),
            options: Default::default(),
            origin: o,
        })
    };
    assert!(zoom(a.clone(), 2.0).is_ok(), "on by default");
    ok(&h, "agent.policy.defaults.set", json!({ "letAgentMoveView": false }));
    let e = zoom(a.clone(), 3.0).error.unwrap();
    assert_eq!((e.code.as_str(), &e.details["preference"]), ("permission.denied", &json!("letAgentMoveView")));
    assert_eq!(ok(&h, "view.transform.get", json!({ "viewId": "drawingView" }))["zoom"], 2.0, "nothing changed");
    assert!(zoom(Origin::User, 3.0).is_ok(), "the user's own change");
}

#[test]
fn v5_zoom_stays_within_its_limits_and_steps_follow_the_table() {
    let h = ready();
    let z = |h: &tt_executor::ExecutorHandle| {
        ok(h, "view.transform.get", json!({ "viewId": "drawingView" }))["zoom"].as_f64().unwrap()
    };
    let mut seen = vec![z(&h)];
    for _ in 0..20 {
        ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "steps": 1 }));
        seen.push(z(&h));
    }
    assert_eq!(*seen.last().unwrap(), 64.0, "6400 % at most");
    assert_eq!(&seen[..5], &[1.0, 1.5, 2.0, 3.0, 4.0], "the table");
    for _ in 0..40 {
        ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "steps": -1 }));
    }
    assert_eq!(z(&h), 0.0625, "6.25 % at least");
    assert_eq!(err(&h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 100 })).code, "arg.invalid");
    // The anchor stays fixed: the drawing point under view (1200, 300) is under it after the zoom.
    ok(&h, "view.transform.reset", json!({ "viewId": "drawingView" }));
    let before = ok(
        &h,
        "view.coordinates.convert",
        json!({ "viewId": "drawingView", "point": { "x": 1200, "y": 300 }, "from": "view", "to": "drawing" }),
    )["point"]
        .clone();
    ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "steps": 2, "anchor": { "x": 1200, "y": 300 } }));
    let after = ok(
        &h,
        "view.coordinates.convert",
        json!({ "viewId": "drawingView", "point": { "x": 1200, "y": 300 }, "from": "view", "to": "drawing" }),
    )["point"]
        .clone();
    let p = |v: &Value| (v["x"].as_f64().unwrap(), v["y"].as_f64().unwrap());
    assert!(close(p(&before), p(&after)), "{before} {after}");
}

#[test]
fn view_space_reaches_colour_pick_hit_test_and_the_active_view_render() {
    let h = ready();
    crate::equivalence::painted_rect(&h);
    ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 2 }));
    // Drawing (350, 330) at zoom 2 about the origin: view (960 + 700, 540 - 660).
    let (vx, vy) = (1660.0, -120.0);
    let pick_view =
        ok(&h, "colour.pick", json!({ "point": { "x": vx, "y": vy }, "space": "view", "view": "drawingView" }));
    let pick_drawing = ok(&h, "colour.pick", json!({ "point": { "x": 350, "y": 330 }, "space": "drawing" }));
    assert_eq!(pick_view, pick_drawing);
    assert!(pick_view["colourId"].is_string(), "{pick_view}");
    let hits =
        ok(&h, "inspect.hitTest", json!({ "space": "view", "point": { "x": vx, "y": vy }, "view": "drawingView" }));
    assert!(!hits["hits"].as_array().unwrap().is_empty(), "{hits}");
    assert_eq!(hits["hits"][0]["drawingId"], D1);
    // The `activeView` composition renders the Drawing view once it is active (SI-107).
    let camera = err(&h, "render.frame.run", json!({ "composition": "activeView" }));
    assert_eq!((camera.code.as_str(), &camera.details["owner"]), ("render.compositionUnavailable", &json!("M4 C4.3")));
    ok(&h, "view.active.set", json!({ "viewId": "drawingView" }));
    let r = ok(&h, "render.frame.run", json!({ "composition": "activeView", "scale": 0.25 }));
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(480), Some(270)), "{r}");
}

#[test]
fn view_changes_are_announced_to_the_host_as_shell_view() {
    let h = ready();
    let rx = h.subscribe();
    ok(&h, "view.rotation.set", json!({ "viewId": "drawingView", "steps": 1 }));
    let ev: Vec<Value> = rx.try_iter().filter(|e| e.name == "shell.view").map(|e| e.payload).collect();
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0]["viewId"], "drawingView");
    assert_eq!(ev[0]["transform"]["rotationDegrees"], 30.0, "{}", ev[0]);
    assert!(ev[0]["drawingToView"].is_array() && ev[0]["viewToDrawing"].is_array());
}

/// M3 merge note §8 item 3 (the Drawing-view half; the camera's roll and default FOV need the Camera view, M4 C4.3):
/// the refusals owned by C4.2 become implementations (`activeView`, view space), draft quality renders as final with
/// its warning, `exclude` wins over the current drawing, and the render's coordinates agree with the view mapping.
#[test]
fn m3_item3_refusals_become_implementations_and_agree_with_the_mapping() {
    let h = ready();
    crate::equivalence::painted_rect(&h);
    ok(&h, "view.active.set", json!({ "viewId": "drawingView" }));
    ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 1.5 }));
    ok(&h, "view.rotation.set", json!({ "viewId": "drawingView", "degrees": 30 }));
    let r = ok(
        &h,
        "render.frame.run",
        json!({ "composition": "activeView", "quality": "draft", "background": "#F0F0F0FF" }),
    );
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(1920), Some(1080)));
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w["code"] == "render.draftAsFinal"), "{r}");
    // Coordinates agree: the render's image-to-drawing matrix is the view's view-to-drawing matrix.
    let m = ok(&h, "draw.viewMapping.get", json!({ "view": "drawing" }));
    for i in 0..6 {
        let (a, b) = (r["imageToCamera"][i].as_f64().unwrap(), m["viewToDrawing"][i].as_f64().unwrap());
        assert!((a - b).abs() <= 1e-9 * (1.0 + b.abs()), "{i}: {a} vs {b}");
    }
    // The same pixels as the headless screenshot of the view (one rasteriser, one mapping).
    let shot = ok(&h, "inspect.screenshot", json!({}));
    let px = |b64: &str| tt_render::decode_png(&tt_persist::b64::decode(b64).unwrap(), 4096).unwrap().2;
    let data = r["data"].as_str().or_else(|| r["png"].as_str()).unwrap();
    assert!(px(data) == px(shot["png"].as_str().unwrap()), "activeView and screenshot agree");
    // `exclude` wins: the drawing's element excluded leaves the background only.
    let ex =
        ok(&h, "render.frame.run", json!({ "composition": "activeView", "background": "#F0F0F0FF", "exclude": [BOB] }));
    let e = px(ex["data"].as_str().or_else(|| ex["png"].as_str()).unwrap());
    assert!(e.chunks(4).all(|c| c == [0xF0, 0xF0, 0xF0, 0xFF]), "excluded: background only");
}
