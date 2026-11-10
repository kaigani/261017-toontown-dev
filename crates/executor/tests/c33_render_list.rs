//! M3 C3.3 (builder; readiness items 2, 4): the render list — order, visibility verdicts and the drawing → pixel map of
//! every element — read back through `inspect.frame.describe` (full detail) and compared with an independent oracle
//! (`c33_support`: 05 §2.2 local matrices, §2.6 projection, §2.7 default camera), never with product output.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c33_support;
mod common;

use c33_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const W: f64 = 1920.0;
const H: f64 = 1080.0;

fn describe(h: &ExecutorHandle, frame: u32) -> Value {
    ok(h, "inspect.frame.describe", json!({ "frame": frame, "detail": "full" }))
}

fn entry<'a>(d: &'a Value, id: &str) -> &'a Value {
    d["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["elementId"] == id)
        .unwrap_or_else(|| panic!("{id} not drawn: {d}"))
}

fn order(d: &Value) -> Vec<String> {
    d["elements"].as_array().unwrap().iter().map(|e| e["elementId"].as_str().unwrap().to_owned()).collect()
}

fn reason(d: &Value, id: &str) -> String {
    d["omitted"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["elementId"] == id)
        .map_or_else(|| "drawn".to_owned(), |e| e["reason"].as_str().unwrap().to_owned())
}

/// The product's drawing → image map of an element compared with the oracle's at three points.
fn assert_map(d: &Value, id: &str, world: &Local, tol: f64) {
    let m: Vec<f64> = entry(d, id)["drawingToImage"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    for du in [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (37.5, -61.25)] {
        let got = (m[0] * du.0 + m[2] * du.1 + m[4], m[1] * du.0 + m[3] * du.1 + m[5]);
        let want = du_to_pixel(world, du, W, H);
        close(got.0, want.0, tol, &format!("{id} x at {du:?}"));
        close(got.1, want.1, tol, &format!("{id} y at {du:?}"));
    }
}

fn set_static(h: &ExecutorHandle, id: &str, params: Value) {
    let mut p = json!({ "nodeIds": [id], "target": "static" });
    p.as_object_mut().unwrap().extend(params.as_object().unwrap().clone());
    ok(h, "anim.transform.set", p);
}

#[test]
fn static_nested_mirrored_and_skewed_transforms_map_exactly() {
    let h = fixture();
    clear(&h);
    let c = solids(&h, 1);
    let child = square(&h, "Child", 50.0, &c[0].0);
    let peg = ok(&h, "element.create", json!({ "type": "peg", "name": "C33 Rig", "sceneId": INTRO }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "element.parent.set", json!({ "elementIds": [child], "parentId": peg }));
    set_static(
        &h,
        &peg,
        json!({ "position": { "x": 1.5, "y": -0.5, "z": 2 }, "rotation": 30, "scale": { "x": -1.5, "y": 0.75 }, "skew": 20 }),
    );
    set_static(&h, &child, json!({ "position": { "x": 2, "y": 1 }, "rotation": 15 }));
    let d = describe(&h, 1);
    let world = Local::of(1.5, -0.5, 2.0, 30.0, -1.5, 0.75, 20.0).then(&Local::of(2.0, 1.0, 0.0, 15.0, 1.0, 1.0, 0.0));
    assert_map(&d, &child, &world, 1e-9);
    // The mirror reads back as a negative sy in the description (05 §2.5).
    assert!(entry(&d, &child)["worldTransform"]["scale"][1].as_f64().unwrap() < 0.0);
    // Depth 2 is in front of the table plane: the element is larger than at Z = 0 (perspective, 05 §2.6).
    assert_eq!(entry(&d, &child)["depth"], 2.0);
}

#[test]
fn keyed_transforms_and_fractional_frames_follow_the_keys() {
    let h = fixture();
    clear(&h);
    let c = solids(&h, 1);
    let a = square(&h, "Keyed", 40.0, &c[0].0);
    // Linear keys (SI-103: new keys are eased linear): rotation 0 → 90 and x 0 → 3 over frames 1 → 13.
    for (frame, rot, x) in [(1, 0.0, 0.0), (13, 90.0, 3.0)] {
        ok(
            &h,
            "anim.transform.set",
            json!({ "nodeIds": [a], "frame": frame, "rotation": rot, "position": { "x": x } }),
        );
    }
    let d = describe(&h, 7);
    let world = Local::of(1.5, 0.0, 0.0, 45.0, 1.0, 1.0, 0.0);
    assert_map(&d, &a, &world, 1e-6);
}

#[test]
fn depth_then_timeline_order_and_roles_decide_the_draw_order() {
    let h = fixture();
    clear(&h);
    let c = solids(&h, 1);
    let back = square(&h, "Back", 30.0, &c[0].0);
    let mid = square(&h, "Mid", 30.0, &c[0].0);
    let front = square(&h, "Front", 30.0, &c[0].0);
    set_static(&h, &front, json!({ "position": { "z": 1 } }));
    set_static(&h, &back, json!({ "position": { "z": -1 } }));
    let d = describe(&h, 1);
    assert_eq!(order(&d), vec![back.clone(), mid.clone(), front.clone()], "world Z ascending (05 §2.8)");
    // Equal Z: the Timeline order decides (index 0 of a level is the front, drawn last).
    set_static(&h, &front, json!({ "position": { "z": 0 } }));
    set_static(&h, &back, json!({ "position": { "z": 0 } }));
    let doc = h.snapshot().unwrap().unwrap();
    let scene = doc.scenes().find(|s| s.name() == "Intro").unwrap();
    let mut displayed: Vec<String> = scene.tree().iter().map(|n| n.id.as_str().to_owned()).collect();
    displayed.retain(|i| [&back, &mid, &front].contains(&i));
    displayed.reverse();
    assert_eq!(order(&describe(&h, 1)), displayed, "back to front = displayed rows bottom to top");
    // Roles: a background element is drawn first and a foreground one last, whatever their depth (04 §6.1).
    ok(&h, "element.properties.set", json!({ "elementId": front, "role": "background" }));
    ok(&h, "element.properties.set", json!({ "elementId": back, "role": "foreground" }));
    set_static(&h, &front, json!({ "position": { "z": 5 } }));
    let got = order(&describe(&h, 1));
    assert_eq!(got.first(), Some(&front), "background first: {got:?}");
    assert_eq!(got.last(), Some(&back), "foreground last: {got:?}");
}

#[test]
fn visibility_verdicts_match_anim_evaluate() {
    let h = fixture();
    clear(&h);
    let c = solids(&h, 1);
    let shown = square(&h, "Shown", 30.0, &c[0].0);
    let child = square(&h, "UnderHidden", 30.0, &c[0].0);
    let peg = ok(&h, "element.create", json!({ "type": "peg", "name": "Hidden peg", "sceneId": INTRO }))["elementIds"]
        [0]
    .as_str()
    .unwrap()
    .to_owned();
    ok(&h, "element.parent.set", json!({ "elementIds": [child], "parentId": peg }));
    ok(&h, "element.visibility.set", json!({ "elementIds": [peg], "scope": "scene", "visible": false }));
    let near = square(&h, "Near", 30.0, &c[0].0);
    set_static(&h, &near, json!({ "position": { "z": 15.95 } }));
    let far = square(&h, "Off", 30.0, &c[0].0);
    set_static(&h, &far, json!({ "position": { "x": 40 } }));
    let d = describe(&h, 1);
    assert_eq!(reason(&d, &shown), "drawn");
    assert_eq!(reason(&d, &child), "hidden", "a hidden ancestor hides it (SI-100)");
    assert_eq!(reason(&d, &near), "behindCamera", "inside the near plane (05 §2.6)");
    assert_eq!(reason(&d, &far), "outOfFrame");
    assert_eq!(reason(&d, BOB_ID), "hidden");
    // The annotation element is view-only.
    let notes = h
        .snapshot()
        .unwrap()
        .unwrap()
        .elements()
        .find(|e| e.name() == "Notes")
        .map(|e| e.id().as_str().to_owned())
        .unwrap();
    assert_eq!(reason(&d, &notes), "annotation");
    // anim.evaluate agrees on `screen.visible` for the drawn and hidden ones.
    let e = ok(&h, "anim.evaluate", json!({ "nodeIds": [shown, child], "frames": [1], "include": ["screen"] }));
    assert_eq!(e["frames"][0]["nodes"][0]["screen"]["visible"], true);
    assert_eq!(e["frames"][0]["nodes"][1]["screen"]["visible"], false);
    // After the exposure: no drawing; a background element extends its last exposure to the scene length (04 §6.1).
    let short = square(&h, "Short", 30.0, &c[0].0);
    let name = get(&h, &drawing_at(&h, &short, 1))["name"].as_str().unwrap().to_owned();
    ok(
        &h,
        "xsheet.exposures.set",
        json!({ "columns": [{ "elementId": short, "from": 1, "to": 48, "exposures": [{ "drawing": name, "from": 1, "to": 10 }] }] }),
    );
    assert_eq!(reason(&describe(&h, 20), &short), "noDrawing");
    ok(&h, "element.properties.set", json!({ "elementId": short, "role": "background" }));
    assert_eq!(reason(&describe(&h, 20), &short), "drawn");
}

#[test]
fn arbitrary_roll_and_dynamic_fov_project_as_the_spec_says() {
    let h = fixture();
    clear(&h);
    let c = solids(&h, 1);
    let a = square(&h, "Rolled", 40.0, &c[0].0);
    set_static(&h, &a, json!({ "position": { "x": 1, "y": 0.5 } }));
    ok(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "roll": 30, "fovMode": "dynamic", "fov": 30, "frame": 1 }));
    ok(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 60, "frame": 13 }));
    let d = describe(&h, 7);
    let m: Vec<f64> = entry(&d, &a)["drawingToImage"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    // Oracle: the camera at (0, 0, 16) rolled 30° sees the world rotated −30°; FOV 45 at frame 7 (linear keys).
    let (cr, sr) = ((-30f64).to_radians().cos(), (-30f64).to_radians().sin());
    let t = (22.5f64).to_radians().tan();
    for du in [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0)] {
        let (x, y) = (1.0 + du.0 / 100.0, 0.5 + du.1 / 100.0);
        let (vx, vy) = (cr * x - sr * y, sr * x + cr * y);
        let want = ((vx / (16.0 * t) + 1.0) / 2.0 * W, (1.0 - vy / (16.0 * t * H / W)) / 2.0 * H);
        let got = (m[0] * du.0 + m[2] * du.1 + m[4], m[1] * du.0 + m[3] * du.1 + m[5]);
        close(got.0, want.0, 1e-6, "x");
        close(got.1, want.1, 1e-6, "y");
    }
}

const BOB_ID: &str = "el_01K742SG11W3SGB8Q1H50W74VB";
