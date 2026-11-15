//! Independent Camera kernel review, round 1 part A.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c33_support;
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
fn scene(h: &ExecutorHandle) -> Value {
    serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap()
}
mod composition {
    use super::*;
    // 05_scene_planning.md:240-250: focal = W/(2 tan(FOV/2));
    // inverse rigid camera roll, divide by depth; output Y is down.
    #[test]
    fn independent_projection_and_final_pixels() {
        for (case, roll, fov, z, hidden, frame) in [
            (1, 0.0_f64, 2. * 0.375_f64.atan().to_degrees(), -4., false, 1),
            (2, 0., 2. * 0.375_f64.atan().to_degrees(), 0., false, 1),
            (3, 30., 2. * 0.375_f64.atan().to_degrees(), 0., false, 1),
            (4, 0., 2. * 0.375_f64.atan().to_degrees(), 0., false, 1),
            (5, 0., 60., 0., false, 1),
            (6, 0., 60., 0., true, 1),
            (7, 0., 60., 16., false, 1),
            (9, 0., 60., 0., false, 1),
            (10, 0., 60., 0., false, 49),
        ] {
            let h = fixture();
            c33_support::clear(&h);
            let (colour, rgba) = c33_support::solids(&h, 1)[0].clone();
            let id = c33_support::square(&h, "Marker", 50., &colour);
            let x = if case == 4 { 1.2 } else { 0.6 };
            ok(&h, "anim.transform.set", json!({"nodeIds":[id],"target":"static","position":{"x":x,"y":0.3,"z":z}}));
            let cam = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
            ok(&h, "anim.camera.set", json!({"cameraId":cam,"roll":roll,"fov":fov}));
            if hidden {
                ok(&h, "element.visibility.set", json!({"elementIds":[id],"scope":"scene","visible":false}));
            }
            ok(&h, "app.context.set", json!({"frame":frame}));
            let s = scene(&h);
            let entry = s["entries"].as_array().unwrap().iter().find(|e| e["elementId"] == id);
            if hidden || z >= 15.9 || frame == 49 {
                assert!(entry.is_none(), "case {case}: {s}");
                continue;
            }
            let e = entry.unwrap();
            let m = e["matrix"].as_array().unwrap();
            let w = s["camera"]["size"][0].as_f64().unwrap();
            let height = s["camera"]["size"][1].as_f64().unwrap();
            let focal = w / (2. * (fov.to_radians() / 2.).tan());
            let (sn, cs) = roll.to_radians().sin_cos();
            let expected = (
                w / 2. + focal * (cs * x + sn * 0.3) / (16. - z),
                height / 2. - focal * (-sn * x + cs * 0.3) / (16. - z),
            );
            assert!((m[4].as_f64().unwrap() - expected.0).abs() < 1e-6, "case {case}");
            assert!((m[5].as_f64().unwrap() - expected.1).abs() < 1e-6, "case {case}");
            let img = c33_support::render(&h, json!({"composition":"camera","scale":0.25}));
            let (px, py) = ((expected.0 * 0.25).round() as u32, (expected.1 * 0.25).round() as u32);
            assert!(
                (px.saturating_sub(2)..=px + 2)
                    .any(|xx| (py.saturating_sub(2)..=py + 2).any(|yy| img.at(xx, yy) == rgba)),
                "case {case}: marker missing"
            );
            assert_eq!(e["drawingId"], c33_support::drawing_at(&h, &id, frame));
            assert_eq!(e["rev"], ok(&h, "draw.drawing.get", json!({"drawingId":e["drawingId"]}))["rev"]);
        }
    }
}

mod mapping {
    use super::*;
    #[test]
    fn explicit_element_scene_point_uses_addressed_element() {
        let h = fixture();
        let (colour, _) = c33_support::solids(&h, 1)[0].clone();
        let target = c33_support::square(&h, "Addressed", 50., &colour);
        ok(&h, "anim.transform.set", json!({"nodeIds":[target],"target":"static","position":{"x":3.,"y":2.}}));
        ok(&h, "app.context.set", json!({"elementId":c33_support::BOB,"frame":1}));
        // spec/02_drawing.md:61, 1151-1155: explicit element addressing and
        // scene points refer to the addressed drawing's full scene transform.
        let r = ok(&h, "draw.pivot.set", json!({"elementId":target,"frame":1,"space":"scene","point":{"x":3.,"y":2.}}));
        assert!(r["pivot"]["x"].as_f64().unwrap().abs() < 1e-6, "{r}");
        assert!(r["pivot"]["y"].as_f64().unwrap().abs() < 1e-6, "{r}");
    }
    #[test]
    fn camera_drawing_mapping_is_queryable() {
        let h = fixture();
        ok(&h, "app.context.set", json!({"elementId":c33_support::BOB,"frame":1}));
        // spec/02_drawing.md:101-104 explicitly requires querying the Camera
        // view's drawing-plane inverse with draw.viewMapping.get.
        let result = ok(&h, "draw.viewMapping.get", json!({"view":"cameraView"}));
        assert!(result["drawingToView"].is_array());
        assert!(result["viewToDrawing"].is_array());
    }

    #[test]
    fn scene_depth_projection_round_trips_and_refusals() {
        let h = fixture();
        ok(&h, "view.zoom.set", json!({"viewId":"cameraView","zoom":2.5}));
        ok(&h, "view.pan.set", json!({"viewId":"cameraView","by":{"x":23,"y":-17}}));
        let cam = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
        ok(&h, "anim.camera.set", json!({"cameraId":cam,"roll":30.,"position":{"x":1.,"y":-2.,"z":16.}}));
        // SI-149: depth is scene Z. Negative Z and zero are valid here.
        // V6: round trips; 05:240-249: inverse rigid camera and similar triangles.
        for depth in [-4., 0., 10.] {
            let points = json!([{"x":1.2,"y":-0.7}]);
            let forward = ok(
                &h,
                "anim.view.coords.get",
                json!({"viewId":"cameraView","points":points,"from":"scene","depth":depth}),
            );
            let back = ok(
                &h,
                "anim.view.coords.get",
                json!({"viewId":"cameraView","points":forward["points"],"from":"view","depth":depth}),
            );
            for (axis, want) in [("x", 1.2), ("y", -0.7), ("z", depth)] {
                assert!((back["points"][0][axis].as_f64().unwrap() - want).abs() < 1e-6);
            }
        }
        for depth in [16., 17.] {
            assert_eq!(
                err(
                    &h,
                    "anim.view.coords.get",
                    json!({"viewId":"cameraView","points":[{"x":0.,"y":0.}],"from":"view","depth":depth})
                )
                .code,
                "inspect.behindCamera"
            );
        }
        for view in ["topView", "sideView"] {
            let e = err(&h, "anim.view.coords.get", json!({"viewId":view,"points":[{"x":0,"y":0}],"from":"view"}));
            assert_eq!(e.code, "command.notImplemented");
            assert!(e.message.contains("M6 C6.1"));
        }
    }
}
