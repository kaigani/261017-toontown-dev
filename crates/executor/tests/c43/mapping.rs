//! The Camera view's mapping (views note §6: V6-V8; SI-109, SI-125, SI-146, SI-149) and the re-owning of the Top and
//! Side views' refusals to M6 C6.1 (C4.3 plan answer 1).

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

use crate::camera::{BOB, apply, matrix, scene};
use crate::common::*;

fn pt(v: &Value) -> (f64, f64) {
    (v["x"].as_f64().unwrap(), v["y"].as_f64().unwrap())
}

fn near(a: (f64, f64), b: (f64, f64), tol: f64, what: &str) {
    assert!((a.0 - b.0).abs() <= tol && (a.1 - b.1).abs() <= tol, "{what}: {a:?} vs {b:?}");
}

fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    h
}

/// Bob's drawing units → Camera view pixels: the occurrence matrix, then the camera's output-to-view matrix (V7).
fn bob_to_view(h: &ExecutorHandle) -> [f64; 6] {
    let s = scene(h);
    let bob = crate::camera::id_of(h, BOB);
    let e = s["entries"].as_array().unwrap().iter().find(|e| e["elementId"] == json!(bob)).cloned().unwrap();
    let (m, v) = (matrix(&e["matrix"]), matrix(&s["camera"]["cameraToView"]));
    [
        v[0] * m[0] + v[2] * m[1],
        v[1] * m[0] + v[3] * m[1],
        v[0] * m[2] + v[2] * m[3],
        v[1] * m[2] + v[3] * m[3],
        v[0] * m[4] + v[2] * m[5] + v[4],
        v[1] * m[4] + v[3] * m[5] + v[5],
    ]
}

fn convert(h: &ExecutorHandle, p: (f64, f64), from: &str, to: &str) -> (f64, f64) {
    pt(&ok(
        h,
        "view.coordinates.convert",
        json!({ "viewId": "cameraView", "point": { "x": p.0, "y": p.1 }, "from": from, "to": to }),
    )["point"])
}

/// V6, V7: the Camera view's chain round-trips view, drawing and scene points for several view transforms and camera
/// rolls, and a drawing point lands in the view where the occurrence matrix and the camera's output-to-view matrix put
/// it (where the renderer draws it).
#[test]
fn v6_v7_the_camera_view_chain_round_trips_and_agrees_with_the_occurrences() {
    let h = ready();
    let cam = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
    let states = [
        json!([]),
        json!([["view.zoom.set", { "viewId": "cameraView", "zoom": 2.5 }]]),
        json!([["view.rotation.set", { "viewId": "cameraView", "degrees": 30 }], ["view.pan.set", { "viewId": "cameraView", "by": { "x": 40, "y": -25 } }]]),
        json!([["anim.camera.set", { "cameraId": cam, "roll": 12.0 }], ["view.pan.set", { "viewId": "cameraView", "centre": { "x": 1.0, "y": -0.5 } }]]),
    ];
    for (k, steps) in states.iter().enumerate() {
        for step in steps.as_array().unwrap() {
            ok(&h, step[0].as_str().unwrap(), step[1].clone());
        }
        let m = bob_to_view(&h);
        for d in [(0.0, 0.0), (37.5, -12.0), (-80.0, 64.0)] {
            let v = convert(&h, d, "drawing", "view");
            near(v, apply(&m, d.0, d.1), 1e-7, &format!("V7 state {k}"));
            near(convert(&h, v, "view", "drawing"), d, 1e-7, &format!("V6 view state {k}"));
            let s = convert(&h, d, "drawing", "scene");
            near(convert(&h, s, "scene", "drawing"), d, 1e-7, &format!("V6 scene state {k}"));
        }
    }
}

/// `view.pan.set` with a scene centre in the Camera view puts that table-plane point at the view's centre (01 §12.1
/// item 13); `view.transform.reset` with `pan` recentres on the camera frame (01 §6.5).
#[test]
fn a_camera_view_pans_to_a_scene_point_and_recentres_on_the_frame() {
    let h = ready();
    ok(&h, "view.pan.set", json!({ "viewId": "cameraView", "centre": { "x": 1.25, "y": -0.75 } }));
    let size = ok(&h, "view.list", json!({}));
    let w = size.as_array().unwrap().iter().find(|v| v["viewId"] == "cameraView").cloned().unwrap();
    let (cw, ch) = (w["size"]["width"].as_f64().unwrap(), w["size"]["height"].as_f64().unwrap());
    let r = ok(
        &h,
        "anim.view.coords.get",
        json!({ "viewId": "cameraView", "points": [{ "x": 1.25, "y": -0.75 }], "from": "scene" }),
    );
    near(pt(&r["points"][0]), (cw / 2.0, ch / 2.0), 1e-7, "the scene point at the centre");
    ok(&h, "view.transform.reset", json!({ "viewId": "cameraView", "parts": ["pan"] }));
    let s = scene(&h);
    let v = matrix(&s["camera"]["cameraToView"]);
    let size = s["camera"]["size"].as_array().unwrap();
    let (fw, fh) = (size[0].as_f64().unwrap(), size[1].as_f64().unwrap());
    near(apply(&v, fw / 2.0, fh / 2.0), (cw / 2.0, ch / 2.0), 1e-7, "the frame's centre at the view's centre");
}

/// `anim.view.coords.get` (SI-149): scene points to view pixels and back on the plane Z = depth.
#[test]
fn anim_view_coords_get_maps_scene_points_and_view_pixels() {
    let h = ready();
    for depth in [0.0, -3.0] {
        let p = [(0.0, 0.0), (2.0, 1.5), (-4.5, 3.0)];
        let pts: Vec<Value> = p.iter().map(|q| json!({ "x": q.0, "y": q.1 })).collect();
        let v = ok(
            &h,
            "anim.view.coords.get",
            json!({ "viewId": "cameraView", "points": pts, "from": "scene", "depth": depth }),
        );
        let back = ok(
            &h,
            "anim.view.coords.get",
            json!({ "viewId": "cameraView", "points": v["points"], "from": "view", "depth": depth }),
        );
        for (i, q) in p.iter().enumerate() {
            near(pt(&back["points"][i]), *q, 1e-7, "round trip");
            assert!((back["points"][i]["z"].as_f64().unwrap() - depth).abs() < 1e-9);
        }
    }
    assert_eq!(
        err(
            &h,
            "anim.view.coords.get",
            json!({ "viewId": "drawingView", "points": [{ "x": 0, "y": 0 }], "from": "scene" })
        )
        .code,
        "arg.invalid"
    );
    let behind = err(
        &h,
        "anim.view.coords.get",
        json!({ "viewId": "cameraView", "points": [{ "x": 0, "y": 0, "z": 40 }], "from": "scene" }),
    );
    assert_eq!(behind.code, "inspect.behindCamera");
}

/// V8: every declared use of view and scene points, as a table over the view windows: each Camera-view cell succeeds
/// with the chain's point, each Top and Side cell refuses `command.notImplemented` with owner M6 C6.1, and the Drawing
/// view keeps its C4.2 behaviour. A cell that ignored the space or the view would fail: the Camera cells assert the
/// converted point, which differs from the raw one.
#[test]
fn v8_scene_and_camera_view_points_in_every_declared_use() {
    let h = ready();
    let m = bob_to_view(&h);
    // A Bob point and its Camera-view pixel.
    let d = (12.0, -8.0);
    let v = apply(&m, d.0, d.1);
    let s = convert(&h, d, "drawing", "scene");
    assert!((v.0 - d.0).abs() > 1.0 && (s.0 - d.0).abs() > 1e-3, "the spaces differ, so a cell ignoring them fails");
    // 1. Addressing parameters of a draw.* command: the pivot lands at `d` from view and from scene points.
    for (space, p, view) in [("view", v, Some("cameraView")), ("scene", s, None)] {
        let mut params = json!({ "point": { "x": p.0, "y": p.1 }, "space": space });
        if let Some(view) = view {
            params["view"] = json!(view);
        }
        let got = ok(&h, "draw.pivot.set", params);
        near(pt(&got["pivot"]), d, 1e-6, &format!("draw.pivot.set {space}"));
    }
    // 2. A colour command's points: a view-space pick in the Camera view reads the drawing at `d`'s place.
    let picked =
        run(&h, "colour.pick", json!({ "point": { "x": v.0, "y": v.1 }, "space": "view", "view": "cameraView" }));
    assert!(picked.error.as_ref().is_none_or(|e| e.code != "command.notImplemented"), "{picked:?}");
    // 3. inspect.hitTest in the Camera view's pixels finds Bob at his centre.
    let c = apply(&m, 0.0, 0.0);
    let hits =
        ok(&h, "inspect.hitTest", json!({ "space": "view", "view": "cameraView", "point": { "x": c.0, "y": c.1 } }));
    let bob = crate::camera::id_of(&h, BOB);
    assert!(hits["hits"].as_array().unwrap().iter().any(|x| x["elementId"] == json!(bob)), "{hits}");
    // 4. inspect.coordinates.convert from the Camera view's pixels to Bob's drawing units.
    let r = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "view", "to": "drawing", "view": "cameraView", "elementId": BOB, "points": [{ "x": v.0, "y": v.1 }] }),
    );
    near(pt(&r["points"][0]), d, 1e-6, "inspect.coordinates.convert");
    // 5. pointer: a click in drawing space in the Camera view is the click at `v` (the final hover reports it).
    let env = h.pointer(
        Origin::User,
        json!({ "view": "cameraView", "space": "drawing", "events": [{ "type": "hover", "x": d.0, "y": d.1 }] }),
    );
    let hover = &env.result.unwrap()["hover"];
    near(pt(&hover["view"]), v, 1e-6, "pointer drawing space");
    near(pt(&hover["drawing"]), d, 1e-6, "pointer hover drawing point");
    // Top and Side: every use refuses with owner M6 C6.1.
    for view in ["topView", "sideView"] {
        let m6 = |e: tt_registry::CommandError, what: &str| {
            assert_eq!(e.code, "command.notImplemented", "{what} {view}: {e:?}");
            assert_eq!(e.details["milestone"], "M6 C6.1", "{what} {view}: {e:?}");
        };
        m6(
            err(&h, "draw.pivot.set", json!({ "point": { "x": 1, "y": 1 }, "space": "view", "view": view })),
            "draw.pivot.set",
        );
        m6(
            err(
                &h,
                "view.coordinates.convert",
                json!({ "viewId": view, "point": { "x": 1, "y": 1 }, "from": "view", "to": "drawing" }),
            ),
            "convert",
        );
        m6(
            err(&h, "inspect.hitTest", json!({ "space": "view", "view": view, "point": { "x": 1, "y": 1 } })),
            "hitTest",
        );
        m6(
            err(&h, "anim.view.coords.get", json!({ "viewId": view, "points": [{ "x": 0, "y": 0 }], "from": "scene" })),
            "coords",
        );
        m6(
            h.pointer(Origin::User, json!({ "view": view, "events": [{ "type": "hover", "x": 1, "y": 1 }] }))
                .error
                .unwrap(),
            "pointer",
        );
        m6(h.canvas_scene(view).unwrap_err(), "canvas_scene");
        let shot = run(&h, "inspect.screenshot", json!({ "target": view }));
        m6(shot.error.unwrap(), "screenshot");
    }
    // The Drawing view refuses scene and camera points in `pointer` (SI-125), as before.
    let e = h.pointer(
        Origin::User,
        json!({ "view": "drawingView", "space": "scene", "events": [{ "type": "hover", "x": 0, "y": 0 }] }),
    );
    assert_eq!(e.error.unwrap().code, "arg.invalid");
}
