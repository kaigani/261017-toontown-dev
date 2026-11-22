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

/// C43A-R1-01 (02 lines 101-104: the Camera view's mapping "includes the camera projection at the current frame" and
/// "MUST be queryable (`draw.viewMapping.get`)"): the published matrices map the current drawing's points as
/// `view.coordinates.convert` does, for several view transforms and camera rolls and at another frame, are each
/// other's inverse, equal the occurrence matrix through the view transform, and name the current drawing.
#[test]
fn c43a_r1_01_draw_view_mapping_get_publishes_the_camera_views_mapping() {
    let h = ready();
    let cam = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
    let states = [
        json!([]),
        json!([["view.zoom.set", { "viewId": "cameraView", "zoom": 2.5 }], ["view.rotation.set", { "viewId": "cameraView", "degrees": 30 }]]),
        json!([["anim.camera.set", { "cameraId": cam, "roll": 12.0 }], ["view.pan.set", { "viewId": "cameraView", "by": { "x": 40, "y": -25 } }]]),
        json!([["anim.transform.set", { "nodeIds": [BOB], "target": "static", "position": { "x": 1.5, "y": -0.5, "z": -3.0 }, "rotation": 20.0 }]]),
    ];
    // At a later frame where Bob exposes another drawing (the fixture's second exposure).
    let later = (2..=48u64)
        .find(|f| {
            run(&h, "draw.drawing.get", json!({ "elementId": BOB, "frame": f })).result.is_some_and(|v| {
                v["drawingId"] != ok(&h, "draw.drawing.get", json!({ "elementId": BOB, "frame": 1 }))["drawingId"]
            })
        })
        .expect("Bob exposes another drawing later");
    let states: Vec<Value> =
        states.into_iter().chain([json!([["app.context.set", { "elementId": BOB, "frame": later }]])]).collect();
    for (i, state) in states.iter().enumerate() {
        for step in state.as_array().unwrap() {
            ok(&h, step[0].as_str().unwrap(), step[1].clone());
        }
        let r = ok(&h, "draw.viewMapping.get", json!({ "view": "cameraView" }));
        let to_view = matrix(&r["drawingToView"]);
        let to_drawing = matrix(&r["viewToDrawing"]);
        for p in [(0.0, 0.0), (37.0, -12.0), (-80.0, 64.0)] {
            let v = apply(&to_view, p.0, p.1);
            let c = convert(&h, p, "drawing", "view");
            assert!((v.0 - c.0).abs() < 1e-6 && (v.1 - c.1).abs() < 1e-6, "state {i}: {v:?} vs {c:?}");
            let back = apply(&to_drawing, v.0, v.1);
            assert!((back.0 - p.0).abs() < 1e-6 && (back.1 - p.1).abs() < 1e-6, "state {i}: inverse");
        }
        let occ = bob_to_view(&h);
        for k in 0..6 {
            assert!((occ[k] - to_view[k]).abs() <= 1e-9 * (1.0 + occ[k].abs()), "state {i}: {occ:?} vs {to_view:?}");
        }
        let current = ok(&h, "draw.drawing.get", json!({}))["drawingId"].clone();
        assert_eq!(r["drawingId"], current, "state {i}: the current drawing");
        let t = ok(&h, "view.transform.get", json!({ "viewId": "cameraView" }));
        assert_eq!(r["zoom"], t["zoom"], "state {i}");
    }
    // Top and Side views are M6 C6.1's.
    let e = err(&h, "draw.viewMapping.get", json!({ "view": "topView" }));
    assert_eq!((e.code.as_str(), &e.details["milestone"]), ("command.notImplemented", &json!("M6 C6.1")));
}

/// A deterministic generator (SplitMix64) for the seeded table below.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

/// V6 over a seeded table (the round-1 observation that the round trips used a small fixed set): 40 states, each with
/// its own camera (position, roll, field of view), element (position with depth, rotation, scale, skew) and view
/// transform (zoom, rotation, pan). In each, drawing points go to the view and back through `view.coordinates.convert`
/// within 1e-6 drawing units, land where `draw.viewMapping.get`'s matrix puts them, and scene points at a seeded depth
/// round-trip through `anim.view.coords.get` within 1e-6 field.
#[test]
fn v6_round_trips_over_a_seeded_table_of_cameras_elements_and_views() {
    let h = ready();
    let cam = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
    let mut r = Seeded(0x00C4_3A01);
    for i in 0..40 {
        let steps = [
            (
                "anim.camera.set",
                json!({ "cameraId": cam, "roll": r.range(-180.0, 180.0), "fov": r.range(20.0, 90.0),
                "position": { "x": r.range(-2.0, 2.0), "y": r.range(-2.0, 2.0), "z": r.range(12.0, 24.0) } }),
            ),
            (
                "anim.transform.set",
                json!({ "nodeIds": [BOB], "target": "static",
                "position": { "x": r.range(-3.0, 3.0), "y": r.range(-2.0, 2.0), "z": r.range(-6.0, 6.0) },
                "rotation": r.range(-180.0, 180.0), "scale": { "x": r.range(0.3, 2.5), "y": r.range(0.3, 2.5) },
                "skew": r.range(-40.0, 40.0) }),
            ),
            ("view.zoom.set", json!({ "viewId": "cameraView", "zoom": r.range(0.3, 6.0) })),
            ("view.rotation.set", json!({ "viewId": "cameraView", "degrees": r.range(-180.0, 180.0) })),
            (
                "view.pan.set",
                json!({ "viewId": "cameraView", "by": { "x": r.range(-200.0, 200.0), "y": r.range(-200.0, 200.0) } }),
            ),
        ];
        for (id, p) in steps {
            ok(&h, id, p);
        }
        let to_view = matrix(&ok(&h, "draw.viewMapping.get", json!({ "view": "cameraView" }))["drawingToView"]);
        for _ in 0..3 {
            let p = (r.range(-300.0, 300.0), r.range(-300.0, 300.0));
            let v = convert(&h, p, "drawing", "view");
            let m = apply(&to_view, p.0, p.1);
            assert!(
                (v.0 - m.0).abs() < 1e-6 * (1.0 + v.0.abs()) && (v.1 - m.1).abs() < 1e-6 * (1.0 + v.1.abs()),
                "state {i}"
            );
            let back = convert(&h, v, "view", "drawing");
            assert!(
                (back.0 - p.0).abs() < 1e-6 && (back.1 - p.1).abs() < 1e-6,
                "state {i}: {p:?} -> {v:?} -> {back:?}"
            );
        }
        let depth = r.range(-6.0, 6.0);
        let q = (r.range(-3.0, 3.0), r.range(-2.0, 2.0));
        let f = ok(
            &h,
            "anim.view.coords.get",
            json!({ "viewId": "cameraView", "points": [{ "x": q.0, "y": q.1 }], "from": "scene", "depth": depth }),
        );
        let b = ok(
            &h,
            "anim.view.coords.get",
            json!({ "viewId": "cameraView", "points": f["points"], "from": "view", "depth": depth }),
        );
        let back = pt(&b["points"][0]);
        assert!((back.0 - q.0).abs() < 1e-6 && (back.1 - q.1).abs() < 1e-6, "state {i}: scene {q:?} at depth {depth}");
    }
}
