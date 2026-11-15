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
