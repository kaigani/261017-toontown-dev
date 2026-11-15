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

mod scene_tools {
    use super::*;
    use tt_executor::Origin;
    fn ready() -> (ExecutorHandle, String) {
        let h = fixture();
        c33_support::clear(&h);
        let (colour, _) = c33_support::solids(&h, 1)[0].clone();
        let id = c33_support::square(&h, "Gesture", 50., &colour);
        ok(&h, "app.context.set", json!({"elementId":id,"frame":1}));
        ok(&h, "edit.selection.set", json!({"context":"staging","ids":[id]}));
        (h, id)
    }
    fn content(h: &ExecutorHandle) -> Vec<String> {
        let d = h.snapshot().unwrap().unwrap();
        d.entry_keys()
            .iter()
            .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
            .collect()
    }
    fn pointer(h: &ExecutorHandle, tool: &str, a: (f64, f64), b: (f64, f64), mods: &[&str]) -> Value {
        let e=h.pointer(Origin::User,json!({"view":"cameraView","tool":tool,"events":[{"type":"down","x":a.0,"y":a.1,"modifiers":mods},{"type":"up","x":b.0,"y":b.1,"modifiers":mods}]}));
        assert!(e.is_ok(), "{e:?}");
        e.result.unwrap()
    }
    #[test]
    fn select_and_transform_drag_have_spec_parameters_content_and_history() {
        // 05:347-350: X/Y delta at node depth; 397-398: static Select,
        // keyed Transform, only changed position channels; 399: one step.
        // 05:1135 operation + kind; SI-144 fixes wording to Move Element.
        for tool in ["select", "transform"] {
            let (a, id) = ready();
            let d = a.snapshot().unwrap().unwrap();
            let a = ExecutorHandle::spawn(Default::default()).unwrap();
            let b = ExecutorHandle::spawn(Default::default()).unwrap();
            assert!(a.open_document(d.clone(), None, false).is_ok());
            assert!(b.open_document(d, None, false).is_ok());
            for h in [&a, &b] {
                ok(h, "app.context.set", json!({"elementId":id,"frame":1}));
                ok(h, "edit.selection.set", json!({"context":"staging","ids":[id]}));
            }
            let before = ok(&a, "edit.history.get", json!({}))["position"].as_u64().unwrap();
            // Default camera focal=1920/(2*0.375), depth=16: 160px/field.
            // Off-centre press avoids the Transform pivot handle.
            let out = pointer(&a, tool, (490., 280.), (510., 260.), &[]);
            let cmd = out["commands"].as_array().unwrap().last().unwrap();
            let mut expected = json!({"nodeIds":[id],"target":if tool=="select" {"static"} else {"key"},"position":{"x":0.125,"y":0.125},"relative":true});
            if tool == "transform" {
                expected["frame"] = json!(1);
            }
            assert_eq!(cmd["commandId"], "anim.transform.set");
            // Numerical agreement is checked separately from structural parameters.
            // This probe does not establish D-4's value-for-value parameter gate.
            for axis in ["x", "y"] {
                assert!((cmd["params"]["position"][axis].as_f64().unwrap() - 0.125).abs() < 1e-12);
            }
            let mut actual = cmd["params"].clone();
            actual["position"] = expected["position"].clone();
            assert_eq!(actual, expected);
            // Replay checks command causality after the independent numeric
            // oracle above. Exact independently-derived parameters remain unverified.
            ok(&b, "anim.transform.set", cmd["params"].clone());
            assert!(content(&a) == content(&b), "canonical content differs");
            let mut hist = ok(&a, "edit.history.get", json!({}));
            let pos = hist["position"].as_u64().unwrap();
            assert_eq!(pos, before + 1);
            assert_eq!(hist["entries"][pos as usize - 1]["label"], "Move Element");
            let mut direct = ok(&b, "edit.history.get", json!({}));
            for history in [&mut hist, &mut direct] {
                for e in history["entries"].as_array_mut().unwrap() {
                    e.as_object_mut().unwrap().remove("time");
                }
            }
            assert_eq!(hist, direct);
        }
    }
    #[test]
    fn click_short_drag_and_empty_click_are_nonundoable_selection() {
        // 05:351: more than 3px starts drag. 05:395, ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¾Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¾ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â¦ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¦ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¾Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â¦ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â‚¬Å¾Ã‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â¦ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§17: selection,
        // not a document mutation or an undo step.
        let (h, id) = ready();
        let before = ok(&h, "edit.history.get", json!({}));
        let saved = content(&h);
        for (a, b, ids) in [((490., 280.), (492., 280.), json!([id])), ((1., 1.), (1., 1.), json!([]))] {
            let out = pointer(&h, "select", a, b, &[]);
            let c = out["commands"].as_array().unwrap().last().unwrap();
            assert_eq!(c["commandId"], "edit.selection.set");
            assert_eq!(c["params"], json!({"context":"staging","ids":ids,"mode":"replace"}));
            assert_eq!(content(&h), saved);
            assert_eq!(ok(&h, "edit.history.get", json!({})), before);
        }
    }
    #[test]
    fn tool_families_are_independent_and_nonundoable() {
        // spec/05_scene_planning.md:29-31,1217-1218: independent tool,
        // result {tool}, no undo. Non-C4.3 tools retain an M6 refusal.
        let (h, _) = ready();
        let hist = ok(&h, "edit.history.get", json!({}));
        let draw = ok(&h, "tool.active.get", json!({}));
        for tool in ["select", "transform"] {
            assert_eq!(ok(&h, "anim.tool.set", json!({"tool":tool})), json!({"tool":tool}));
            assert_eq!(ok(&h, "anim.tool.get", json!({})), json!({"tool":tool}));
            assert_eq!(ok(&h, "tool.active.get", json!({})), draw);
        }
        ok(&h, "tool.active.set", json!({"toolId":"brush"}));
        assert_eq!(ok(&h, "anim.tool.get", json!({})), json!({"tool":"transform"}));
        assert_eq!(ok(&h, "edit.history.get", json!({})), hist);
    }
}

mod part_b_composition {
    use super::*;
    fn ready() -> ExecutorHandle {
        let h = fixture();
        c33_support::clear(&h);
        h
    }
    fn put(h: &ExecutorHandle, id: &str, x: f64, z: f64) {
        ok(h, "anim.transform.set", json!({"nodeIds":[id],"target":"static","position":{"x":x,"y":0.,"z":z}}));
    }
    fn visible(h: &ExecutorHandle, id: &str, v: bool) {
        ok(h, "element.visibility.set", json!({"elementIds":[id],"scope":"scene","visible":v}));
    }
    fn ids(s: &Value) -> Vec<String> {
        s["entries"].as_array().unwrap().iter().map(|e| e["elementId"].as_str().unwrap().to_owned()).collect()
    }
    fn point(e: &Value, x: f64, y: f64) -> (f64, f64) {
        let m: Vec<f64> = e["matrix"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        ((m[0] * x + m[2] * y + m[4]) * 0.25, (m[1] * x + m[3] * y + m[5]) * 0.25)
    }
    fn colour_at(img: &c33_support::Img, p: (f64, f64), rgba: [u8; 4]) {
        let (x, y) = (p.0.round() as i32, p.1.round() as i32);
        assert!(
            (-2..=2).any(|dx| (-2..=2).any(|dy| x + dx >= 0
                && y + dy >= 0
                && x + dx < (img.w as i32)
                && y + dy < (img.h as i32)
                && img.at((x + dx) as u32, (y + dy) as u32) == rgba)),
            "missing {rgba:?} at {p:?}"
        );
    }
    fn check(h: &ExecutorHandle, id: &str, rgba: [u8; 4]) -> Value {
        let s = scene(h);
        let e = s["entries"].as_array().unwrap().iter().find(|e| e["elementId"] == id).unwrap().clone();
        assert_eq!(e["drawingId"], c33_support::drawing_at(h, id, 1));
        assert_eq!(e["rev"], ok(h, "draw.drawing.get", json!({"drawingId":e["drawingId"]}))["rev"]);
        let img = c33_support::render(h, json!({"composition":"camera","scale":0.25}));
        colour_at(&img, point(&e, 0., 0.), rgba);
        e
    }
    #[test]
    fn case1_three_depths() {
        // 05:263-265: smaller world Z first, front-most last; CV1 matrices.
        let h = ready();
        let cs = c33_support::solids(&h, 3);
        let es: Vec<String> = (0..3).map(|i| c33_support::square(&h, &format!("Depth{i}"), 50., &cs[i].0)).collect();
        for (i, e) in es.iter().enumerate() {
            put(&h, e, 0., (i as f64 - 1.) * 4.);
        }
        assert_eq!(ids(&scene(&h)), es);
        for (i, e) in es.iter().enumerate() {
            for other in &es {
                visible(&h, other, other == e);
            }
            check(&h, e, cs[i].1);
        }
        for e in &es {
            visible(&h, e, true);
        }
        check(&h, &es[2], cs[2].1);
    }
    #[test]
    fn case2_timeline_tie_reorder() {
        // 05:263-267 ties follow Timeline; 04:467 above = front.
        let h = ready();
        let cs = c33_support::solids(&h, 2);
        let a = c33_support::square(&h, "TieA", 50., &cs[0].0);
        let b = c33_support::square(&h, "TieB", 50., &cs[1].0);
        let root = ok(&h, "element.create", json!({"type":"peg","name":"TieRoot"}))["elementIds"][0].clone();
        ok(&h, "element.parent.set", json!({"elementIds":[a,b],"parentId":root}));
        for order in [[a.clone(), b.clone()], [b.clone(), a.clone()]] {
            ok(&h, "element.order.set", json!({"order":"timeline","parentId":root,"elementIds":order}));
            assert_eq!(ids(&scene(&h)), vec![order[1].clone(), order[0].clone()]);
            check(&h, &order[0], if order[0] == a { cs[0].1 } else { cs[1].1 });
            visible(&h, &order[0], false);
            check(&h, &order[1], if order[1] == a { cs[0].1 } else { cs[1].1 });
            visible(&h, &order[0], true);
        }
    }

    #[test]
    fn case3_two_cameras() {
        // 05:236-247 inverse rigid view; 313 current camera; 901-902 outline only.
        let h = ready();
        let cs = c33_support::solids(&h, 1);
        let a = c33_support::square(&h, "CameraMarker", 30., &cs[0].0);
        put(&h, &a, 1., 0.);
        let first = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
        let second = ok(
            &h,
            "anim.camera.create",
            json!({"name":"Other","position":{"x":-1.,"y":0.5,"z":16.},"makeCurrent":false}),
        )["cameraId"]
            .clone();
        ok(&h, "anim.camera.set", json!({"cameraId":second,"roll":30.}));
        let mut matrices = Vec::new();
        let mut images = Vec::new();
        for (cam, x, y, roll) in [(first, 0., 0., 0_f64), (second, -1., 0.5, 30.)] {
            ok(&h, "anim.camera.current.set", json!({"cameraId":cam}));
            assert_eq!(ids(&scene(&h)), vec![a.clone()]);
            let e = check(&h, &a, cs[0].1);
            let p = point(&e, 0., 0.);
            let (sn, co) = roll.to_radians().sin_cos();
            // Default focal 2560, depth 16; output 1920x1080, render scale 1/4.
            assert!((p.0 - (960. + 160. * (co * (1. - x) + sn * (-y))) * 0.25).abs() < 1e-6);
            assert!((p.1 - (540. - 160. * (-sn * (1. - x) + co * (-y))) * 0.25).abs() < 1e-6);
            matrices.push(e["matrix"].clone());
            images.push(c33_support::render(&h, json!({"composition":"camera","scale":0.25})).rgba);
        }
        assert_ne!(matrices[0], matrices[1]);
        assert_ne!(images[0], images[1]);
    }
    #[test]
    fn case4_hidden_ancestor() {
        // CV1 hidden ancestors absent in both; 05:314 all visible hierarchy content.
        let h = ready();
        let cs = c33_support::solids(&h, 1);
        let a = c33_support::square(&h, "Child", 30., &cs[0].0);
        let peg = ok(&h, "element.create", json!({"type":"peg","name":"Parent"}))["elementIds"][0].clone();
        ok(&h, "element.parent.set", json!({"elementIds":[a],"parentId":peg}));
        visible(&h, &a, true);
        check(&h, &a, cs[0].1);
        visible(&h, peg.as_str().unwrap(), false);
        assert!(ids(&scene(&h)).is_empty());
        let img = c33_support::render(&h, json!({"composition":"camera","scale":0.25}));
        assert!(!img.rgba.as_chunks::<4>().0.iter().any(|c| *c == cs[0].1));
        visible(&h, peg.as_str().unwrap(), true);
        assert_eq!(ids(&scene(&h)), vec![a.clone()]);
        check(&h, &a, cs[0].1);
    }

    #[test]
    fn case5_near_plane() {
        // 05:240-241 d <=0.1 omitted; SI-149 depth argument is scene Z.
        // Camera Z=16 looking -Z: Z=15.901 gives d=.099, Z=15.899 gives d=.101.
        let h = ready();
        let cs = {
            let c = c33_support::solids(&h, 3);
            vec![c[2].clone(), c[1].clone()]
        };
        let behind = c33_support::square(&h, "NearClipped", 5., &cs[0].0);
        let front = c33_support::square(&h, "NearVisible", 5., &cs[1].0);
        put(&h, &behind, -0.01, 15.901);
        put(&h, &front, 0.01, 15.899);
        assert_eq!(ids(&scene(&h)), vec![front.clone()]);
        check(&h, &front, cs[1].1);
        let img = c33_support::render(&h, json!({"composition":"camera","scale":0.25}));
        assert!(!img.rgba.as_chunks::<4>().0.iter().any(|c| *c == cs[0].1));
    }
    #[test]
    fn case6_nested_pegs_three_corners() {
        // 05:314 hierarchy transforms; 02:61 full chain; CV1 placement agrees.
        let h = ready();
        let cs = c33_support::solids(&h, 1);
        let a = c33_support::square(&h, "Chain", 50., &cs[0].0);
        let outer = ok(&h, "element.create", json!({"type":"peg","name":"Outer"}))["elementIds"][0].clone();
        let inner = ok(&h, "element.create", json!({"type":"peg","name":"Inner"}))["elementIds"][0].clone();
        ok(&h, "element.parent.set", json!({"elementIds":[inner],"parentId":outer}));
        ok(&h, "element.parent.set", json!({"elementIds":[a],"parentId":inner}));
        for (id, x, y, r, sx, sy) in [(outer, 0.4, 0.2, 23., 1.4, 0.8), (inner, 0.2, -0.3, -11., 0.7, 1.3)] {
            ok(
                &h,
                "anim.transform.set",
                json!({"nodeIds":[id],"target":"static","position":{"x":x,"y":y},"rotation":r,"scale":{"x":sx,"y":sy}}),
            );
        }
        assert_eq!(ids(&scene(&h)), vec![a.clone()]);
        let e = check(&h, &a, cs[0].1);
        let img = c33_support::render(&h, json!({"composition":"camera","scale":0.25}));
        let world = c33_support::Local::of(0.4, 0.2, 0., 23., 1.4, 0.8, 0.)
            .then(&c33_support::Local::of(0.2, -0.3, 0., -11., 0.7, 1.3, 0.));
        for (x, y) in [(-50., -50.), (50., -50.), (-50., 50.)] {
            let p = point(&e, x, y);
            let oracle = c33_support::du_to_pixel(&world, (x, y), 1920., 1080.);
            assert!((p.0 - oracle.0 * 0.25).abs() < 1e-6 && (p.1 - oracle.1 * 0.25).abs() < 1e-6);
            colour_at(&img, p, cs[0].1);
            // An edge pixel of this colour must occur within two pixels of
            // the predicted corner; interior-only proximity is insufficient.
            let mut edge = false;
            for dx in -2..=2 {
                for dy in -2..=2 {
                    let xx = (p.0.round() as i32 + dx) as u32;
                    let yy = (p.1.round() as i32 + dy) as u32;
                    if img.at(xx, yy) == cs[0].1
                        && [(xx - 1, yy), (xx + 1, yy), (xx, yy - 1), (xx, yy + 1)]
                            .iter()
                            .any(|&(nx, ny)| img.at(nx, ny) != cs[0].1)
                    {
                        edge = true;
                    }
                }
            }
            assert!(edge, "no colour edge near {p:?}");
        }
    }

    #[test]
    fn case7_shared_drawing_clone() {
        // 04:429-431 shared drawing bank, independent timing/transformation;
        // CV2 one geometry per drawing, CV1 one occurrence per visible placement.
        let h = ready();
        let cs = c33_support::solids(&h, 3);
        let rgba = cs[2].1;
        let a = c33_support::square(&h, "Shared", 50., &cs[2].0);
        let b = ok(&h, "element.clone", json!({"elementId":a,"name":"SharedClone"}))["elementId"]
            .as_str()
            .unwrap()
            .to_owned();
        put(&h, &a, -1., 0.);
        put(&h, &b, 1., 0.);
        let s = scene(&h);
        assert_eq!(ids(&s).len(), 2);
        assert!(ids(&s).contains(&a) && ids(&s).contains(&b));
        let ea = check(&h, &a, rgba);
        let eb = check(&h, &b, rgba);
        assert_eq!(ea["drawingId"], eb["drawingId"]);
        assert_eq!(ea["rev"], eb["rev"]);
        assert_ne!(ea["matrix"], eb["matrix"]);
        // Then overlap at unequal depth, so same-colour geometry still has
        // independently specified membership/order and shared drawing identity.
        put(&h, &a, 0., -4.);
        put(&h, &b, 0., 0.);
        assert_eq!(ids(&scene(&h)), vec![a, b.clone()]);
        check(&h, &b, rgba);
    }
}

mod part_c_mapping {
    use super::*;
    fn ready() -> (ExecutorHandle, String) {
        let h = fixture();
        c33_support::clear(&h);
        let col = c33_support::solids(&h, 1)[0].0.clone();
        let id = c33_support::square(&h, "Mapping", 50., &col);
        ok(&h, "app.context.set", json!({"elementId":id,"frame":1}));
        (h,id)
    }
    fn near(a: f64,b:f64) { assert!((a-b).abs()<1e-6,"{a} != {b}"); }
    fn get(h:&ExecutorHandle,id:&str)->Value { ok(h,"draw.drawing.get",json!({"elementId":id,"frame":1})) }
    fn geometry(v:&Value)->Value {
        match v {
            Value::Object(o)=> Value::Object(o.iter().filter(|(k,_)| !matches!(k.as_str(),"id"|"rev"|"drawingId"|"name") && !k.ends_with("Ids") && !k.ends_with("Id")).map(|(k,v)|(k.clone(),geometry(v))).collect()),
            Value::Array(a)=>Value::Array(a.iter().map(geometry).collect()),
            _=>v.clone()
        }
    }
    fn same(a:&Value,b:&Value) {
        match (a,b) {
            (Value::Number(x),Value::Number(y))=>near(x.as_f64().unwrap(),y.as_f64().unwrap()),
            (Value::Array(x),Value::Array(y))=>{assert_eq!(x.len(),y.len());for (a,b) in x.iter().zip(y) {same(a,b);}},
            (Value::Object(x),Value::Object(y))=>{assert_eq!(x.len(),y.len(),"{a} vs {b}");for (k,a) in x {same(a,&y[k]);}},
            _=>assert_eq!(a,b)
        }
    }
    #[test]
    fn case1_scene_rectangle_nested_pegs() {
        // 02:61, 1151-1155; SI-146: invert T_outer R_outer S_outer T_inner R_inner S_inner,
        // then fields*100 = drawing units (02:86-89). Camera depth does not affect scene XY.
        let (h,id)=ready();
        let outer=ok(&h,"element.create",json!({"type":"peg","name":"Outer"}))["elementIds"][0].clone();
        let inner=ok(&h,"element.create",json!({"type":"peg","name":"Inner"}))["elementIds"][0].clone();
        ok(&h,"element.parent.set",json!({"elementIds":[inner],"parentId":outer}));
        ok(&h,"element.parent.set",json!({"elementIds":[id],"parentId":inner}));
        for (node,x,y,r,sx,sy) in [(outer,0.4,0.2,23.,1.4,0.8),(inner,0.2,-0.3,-11.,0.7,1.3)] {
            ok(&h,"anim.transform.set",json!({"nodeIds":[node],"target":"static","position":{"x":x,"y":y},"rotation":r,"scale":{"x":sx,"y":sy}}));
        }
        ok(&h,"anim.transform.set",json!({"nodeIds":[id],"target":"static","position":{"z":2.}}));
        let inverse=|mut x:f64,mut y:f64| {
            for (tx,ty,r,sx,sy) in [(0.4,0.2,23_f64,1.4,0.8),(0.2,-0.3,-11.,0.7,1.3)] {
                x-=tx; y-=ty; let (s,c)=r.to_radians().sin_cos();
                (x,y)=((c*x+s*y)/sx,(-s*x+c*y)/sy);
            }
            json!({"x":100.*x,"y":100.*y})
        };
        ok(&h,"app.context.set",json!({"elementId":id,"frame":1}));
        let before=get(&h,&id);
        ok(&h,"draw.rectangle.add",json!({"space":"scene","from":{"x":1.,"y":1.},"to":{"x":2.,"y":3.},"layer":"new"}));
        let got=geometry(&get(&h,&id));
        ok(&h,"edit.undo",json!({}));
        same(&geometry(&get(&h,&id)),&geometry(&before));
        ok(&h,"draw.rectangle.add",json!({"from":inverse(1.,1.),"to":inverse(2.,3.),"layer":"new"}));
        same(&got,&geometry(&get(&h,&id)));
    }
    #[test]
    fn case2_view_four_shapes() {
        // 02:61,103,1151-1155; 05:240-249. focal=1920/(2*.375)=2560;
        // output=(960+f*R^-1(scene-camera).x/d,540-f*R^-1(...).y/d).
        // At zoom 2, pan by (23,-17): view = (480,270)+2*(output-outputCentre)+(23,-17).
        let (h,id)=ready();
        let cam=ok(&h,"anim.camera.current.get",json!({}))["cameraId"].clone();
        ok(&h,"anim.camera.set",json!({"cameraId":cam,"roll":30.,"position":{"x":1.,"y":-2.,"z":16.}}));
        ok(&h,"view.zoom.set",json!({"viewId":"cameraView","zoom":2.}));
        ok(&h,"view.pan.set",json!({"viewId":"cameraView","by":{"x":23.,"y":-17.}}));
        let view=|x:f64,y:f64| { let (s,c)=30_f64.to_radians().sin_cos();
            json!({"x":503.+320.*(c*(x/100.-1.)+s*(y/100.+2.)),"y":253.-320.*(-s*(x/100.-1.)+c*(y/100.+2.))}) };
        for command in ["draw.stroke.add","draw.selection.set","draw.hitTest.get","colour.paint.apply"] {
            let mut args=match command {
                "draw.stroke.add"=>json!({"points":[{"x":10.,"y":10.},{"x":20.,"y":20.}],"fit":false}),
                "draw.selection.set"=>json!({"by":"point","point":{"x":0.,"y":0.}}),
                "draw.hitTest.get"=>json!({"point":{"x":0.,"y":0.}}),
                _=>json!({"points":[{"x":0.,"y":0.}],"colourId":c33_support::solids(&h,2)[1].0})
            };
            let direct=args.clone();
            if let Some(points)=args.get_mut("points") { for p in points.as_array_mut().unwrap() { *p=view(p["x"].as_f64().unwrap(),p["y"].as_f64().unwrap()); } }
            if args.get("point").is_some() {args["point"]=view(0.,0.);}
            args["space"]=json!("view");args["view"]=json!("cameraView");
            let before=get(&h,&id);
            let a=ok(&h,command,args);let ga=geometry(&get(&h,&id));
            if get(&h,&id)!=before {ok(&h,"edit.undo",json!({}));}
            let b=ok(&h,command,direct);
            same(&geometry(&a),&geometry(&b));
            same(&ga,&geometry(&get(&h,&id)));
        }
    }
}
