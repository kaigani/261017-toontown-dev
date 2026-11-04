//! M3 C3.2 (builder): the readiness items a reviewer asked for by name beyond `c32_semantics`: vectorization's 1200-du
//! extent through the scene (item 2), skeleton and drawing pivots (item 3), independent position axes (item 4).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use c13_support::job_ok;
use common::*;
use serde_json::{Value, json};

fn eval(h: &tt_executor::ExecutorHandle, node: &str, frame: f64) -> Value {
    ok(h, "anim.evaluate", json!({ "nodeIds": [node], "frames": [frame] }))["frames"][0]["nodes"][0].clone()
}

fn near(v: &Value, want: f64, what: &str) {
    let got = v.as_f64().unwrap_or(f64::NAN);
    assert!((got - want).abs() <= 1e-6, "{what}: {got} vs {want}");
}

fn sans_rev(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("rev");
    v
}

#[test]
fn vectorized_art_keeps_its_1200_du_frame_through_the_scene() {
    let h = fixture();
    let art = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("tests/fixtures/vectorize/e2-lineart.png");
    let element = ok(&h, "element.create", json!({ "type": "drawing", "name": "Lines" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let done = job_ok(&h, "draw.vectorize.run", json!({ "files": [art], "targetElementId": element, "startFrame": 1 }));
    let drawing = done["drawingIds"][0].as_str().unwrap().to_owned();
    let before = ok(&h, "draw.drawing.get", json!({ "drawingId": drawing }));
    let rev_before = get(&h, &drawing)["rev"].clone();
    // Identity element: du ÷ 100 = fields. The image's width spans 1200 du centred on the origin (ADR-0018), so the art
    // stays inside ±6 fields and reaches past half of them.
    let e = eval(&h, &element, 1.0);
    let b = &e["bounds"];
    let (min_x, max_x) = (b["minX"].as_f64().unwrap(), b["maxX"].as_f64().unwrap());
    let drawn = &before["bounds"];
    near(&json!(min_x), drawn["x"].as_f64().unwrap() / 100.0, "the drawing's own bounds, in fields");
    near(&json!(max_x), (drawn["x"].as_f64().unwrap() + drawn["width"].as_f64().unwrap()) / 100.0, "right edge");
    assert!(min_x >= -6.05 && max_x <= 6.05 && max_x - min_x > 6.0, "{b}");
    // Through the default camera it maps by the 12-field width.
    let w = ok(&h, "anim.camera.get", json!({ "frame": 1 }))["resolution"]["w"].as_f64().unwrap();
    let box_ = &e["screen"]["bbox"];
    let width_px = box_["width"].as_f64().unwrap();
    assert!((width_px - (max_x - min_x) / 12.0 * w).abs() < 0.01, "{box_}");
    // A transform moves the whole frame, mirrored too, without touching the drawing.
    ok(
        &h,
        "anim.transform.set",
        json!({ "nodeIds": [element], "target": "static", "scale": { "x": -2, "y": 1 }, "position": { "x": 1, "y": 0, "z": 0 } }),
    );
    let b = eval(&h, &element, 1.0)["bounds"].clone();
    near(&b["minX"], 1.0 - 2.0 * max_x, "mirrored left edge");
    near(&b["maxX"], 1.0 - 2.0 * min_x, "mirrored right edge");
    assert_eq!(sans_rev(ok(&h, "draw.drawing.get", json!({ "drawingId": drawing }))), sans_rev(before));
    assert_eq!(get(&h, &drawing)["rev"], rev_before, "geometry, IDs and revision unchanged");
}

#[test]
fn skeletons_follow_the_peg_pivot_rule_and_the_drawing_pivot_stays_out() {
    let h = fixture();
    let s = ok(&h, "element.create", json!({ "type": "skeleton", "name": "RigA" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(
        &h,
        "anim.transform.set",
        json!({ "nodeIds": [s], "target": "static", "rotation": 90, "pivot": { "x": 2, "y": 0 } }),
    );
    // Default for skeletons: rotation (05 §2.3). (2, 0) stays put under a rotation about itself.
    let e = eval(&h, &s, 1.0);
    near(&e["local"]["rotationPivot"]["x"], 2.0, "rotation pivot");
    near(&e["local"]["scalePivot"]["x"], 0.0, "scale pivot");
    let m: Vec<f64> = e["world"]["matrix"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!((m[0] * 2.0 + m[3] - 2.0).abs() < 1e-9, "{m:?}");
    assert!((m[4] * 2.0 + m[7]).abs() < 1e-9, "{m:?}");
    ok(&h, "doc.object.patch", json!({ "targets": [s], "props": { "transform": { "applyPivotTo": "scaling" } } }));
    let e = eval(&h, &s, 1.0);
    near(&e["local"]["rotationPivot"]["x"], 0.0, "rotation pivot off");
    near(&e["local"]["scalePivot"]["x"], 2.0, "scale pivot on");
    // A drawing's own pivot (02) is a drawing property: it does not enter the element's matrix.
    let d1 = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    ok(&h, "anim.transform.set", json!({ "nodeIds": [bob], "target": "static", "rotation": 30 }));
    let before = eval(&h, bob, 1.0)["local"]["matrix"].clone();
    let pivot = ok(&h, "draw.drawing.get", json!({ "drawingId": d1 }))["pivot"].clone();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [d1], "props": { "pivot": { "x": pivot["x"].as_f64().unwrap() + 55.0, "y": 7 } } }),
    );
    assert_eq!(eval(&h, bob, 1.0)["local"]["matrix"], before);
}

#[test]
fn position_axes_are_keyed_independently_and_read_as_projections() {
    let h = fixture();
    let p = ok(&h, "element.create", json!({ "type": "peg", "name": "Axes" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 30 }));
    // Only X is named: Y and Z take the node's current position there, here the defaults.
    ok(&h, "anim.transform.set", json!({ "nodeIds": [p], "frame": 1, "position": { "x": 4 } }));
    let at = eval(&h, &p, 1.0)["animated"]["position"].clone();
    assert_eq!(at, json!({ "x": 4.0, "y": 0.0, "z": 0.0 }));
    // Only Y at a second frame: X holds what the single-point path gives.
    ok(&h, "anim.transform.set", json!({ "nodeIds": [p], "frame": 11, "position": { "y": 6 } }));
    let at = eval(&h, &p, 11.0)["animated"]["position"].clone();
    assert_eq!(at, json!({ "x": 4.0, "y": 6.0, "z": 0.0 }));
    // Overwriting Z at an existing keyframe keeps the point and its ID, and the other axes.
    let id = ok(&h, "anim.path.get", json!({ "nodeId": p }))["points"][1]["id"].clone();
    ok(&h, "anim.transform.set", json!({ "nodeIds": [p], "frame": 11, "position": { "z": -3 } }));
    let got = ok(&h, "anim.path.get", json!({ "nodeId": p }));
    assert_eq!(got["points"][1]["id"], id);
    assert_eq!(
        (got["points"][1]["x"].as_f64(), got["points"][1]["y"].as_f64(), got["points"][1]["z"].as_f64()),
        (Some(4.0), Some(6.0), Some(-3.0))
    );
    // Each projection reads its axis; a value edit moves only that coordinate.
    let z = ok(&h, "anim.function.get", json!({ "nodeId": p, "channel": "position.z" }));
    assert_eq!(z["keys"][1]["value"], -3.0);
    let r = ok(&h, "anim.function.key.set", json!({ "nodeId": p, "channel": "position.y", "frame": 11, "value": 8 }));
    assert_eq!(r["value"], 8.0);
    let at = eval(&h, &p, 11.0)["animated"]["position"].clone();
    assert_eq!(at, json!({ "x": 4.0, "y": 8.0, "z": -3.0 }));
    let e = err(
        &h,
        "anim.function.key.set",
        json!({ "nodeId": p, "channel": "position.y", "frame": 11, "right": { "dt": 1, "dv": 1 } }),
    );
    assert_eq!(e.code, "anim.channelUnsupported", "TCB, not handles");
    // Axes mid-segment are a straight projection of the same path position.
    let s = ok(&h, "anim.function.sample", json!({ "nodeId": p, "channel": "position.z", "from": 6, "to": 6 }));
    let mid = eval(&h, &p, 6.0)["animated"]["position"]["z"].clone();
    assert_eq!(s["samples"][0]["value"], mid);
}
