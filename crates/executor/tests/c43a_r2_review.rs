//! Independent round-2 Camera kernel fixtures.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review fixtures")]
mod c33_support;
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
fn near(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            assert!((x.as_f64().unwrap() - y.as_f64().unwrap()).abs() <= 1e-9, "{a} != {b}")
        }
        (Value::Array(x), Value::Array(y)) => {
            assert_eq!(x.len(), y.len());
            for (a, b) in x.iter().zip(y) {
                near(a, b);
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            assert_eq!(x.len(), y.len());
            assert!(x.keys().all(|k| y.contains_key(k)));
            for (k, v) in x {
                near(v, &y[k]);
            }
        }
        _ => assert_eq!(a, b),
    }
}
fn apply(m: &Value, p: (f64, f64)) -> (f64, f64) {
    let n = |i: usize| m[i].as_f64().unwrap();
    (n(0) * p.0 + n(2) * p.1 + n(4), n(1) * p.0 + n(3) * p.1 + n(5))
}
fn setup() -> (ExecutorHandle, String, String) {
    let h = fixture();
    let colour = c33_support::solids(&h, 1)[0].0.clone();
    let c = c33_support::square(&h, "C", 50., &colour);
    let peg = ok(&h, "element.create", json!({"type":"peg","name":"ReviewPeg"}))["elementIds"][0].clone();
    ok(&h, "element.parent.set", json!({"elementIds":[c],"parentId":peg}));
    ok(
        &h,
        "anim.transform.set",
        json!({"nodeIds":[peg],"target":"static","position":{"x":2.,"y":-1.},"rotation":90.,"scale":{"x":2.,"y":0.5}}),
    );
    ok(&h, "anim.transform.set", json!({"nodeIds":[c],"target":"static","position":{"z":-4.}}));
    ok(&h, "app.context.set", json!({"elementId":c33_support::BOB,"frame":1}));
    (h, c, colour)
}
// 02:101-104: full drawing-plane map. Identity residual 1e-9 is conservative
// for this well-conditioned affine (scales 0.4..6 and translations < 2000).
#[test]
fn case3_nested_mapping() {
    let (h, c, _) = setup();
    ok(&h, "app.context.set", json!({"elementId":c,"frame":1}));
    let cam = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
    ok(&h, "anim.camera.set", json!({"cameraId":cam,"roll":23.,"position":{"x":0.3,"y":-0.4,"z":16.}}));
    ok(&h, "view.zoom.set", json!({"viewId":"cameraView","zoom":2.3}));
    ok(&h, "view.pan.set", json!({"viewId":"cameraView","by":{"x":31.,"y":-17.}}));
    let m = ok(&h, "draw.viewMapping.get", json!({"view":"cameraView"}));
    for p in [(0., 0.), (12., -37.), (-50., 50.)] {
        let v = apply(&m["drawingToView"], p);
        let q = ok(
            &h,
            "view.coordinates.convert",
            json!({"viewId":"cameraView","from":"drawing","to":"view","point":{"x":p.0,"y":p.1}}),
        );
        near(&json!([v.0, v.1]), &json!([q["point"]["x"], q["point"]["y"]]));
        let d = apply(&m["viewToDrawing"], v);
        near(&json!([d.0, d.1]), &json!([p.0, p.1]));
    }
}
// 02:61 and 13.1 items 4/5: scene inputs use explicitly addressed C, never Bob.
// Independent peg matrix: world=(2 - 0.5*y/100, -1 + 2*x/100), z=-4.
#[test]
fn case4_third_element_five_commands() {
    for command in
        ["draw.rectangle.add", "draw.pivot.set", "draw.selection.set", "draw.hitTest.get", "colour.paint.apply"]
    {
        let (h, c, colour) = setup();
        let d = c33_support::drawing_at(&h, &c, 1);
        let twin = ExecutorHandle::spawn(Default::default()).unwrap();
        assert!(twin.open_document(h.snapshot().unwrap().unwrap(), None, false).is_ok());
        ok(&twin, "app.context.set", json!({"elementId":c33_support::BOB,"frame":1}));
        let bob = ok(&h, "draw.drawing.get", json!({"elementId":c33_support::BOB,"frame":1}));
        let (mut a, mut b) = match command {
            "draw.rectangle.add" => (
                json!({"from":{"x":2.15,"y":1.},"to":{"x":2.05,"y":1.4}}),
                json!({"from":{"x":100.,"y":-30.},"to":{"x":120.,"y":-10.}}),
            ),
            "colour.paint.apply" => (
                json!({"points":[{"x":2.,"y":-1.}],"colourId":colour}),
                json!({"points":[{"x":0.,"y":0.}],"colourId":colour}),
            ),
            "draw.selection.set" => {
                (json!({"by":"point","point":{"x":2.,"y":0.}}), json!({"by":"point","point":{"x":50.,"y":0.}}))
            }
            _ => (json!({"point":{"x":2.,"y":0.}}), json!({"point":{"x":50.,"y":0.}})),
        };
        a["elementId"] = json!(c);
        a["frame"] = json!(1);
        a["space"] = json!("scene");
        b["drawingId"] = json!(d);
        let ra = ok(&h, command, a);
        let rb = ok(&twin, command, b);
        near(&ra, &rb);
        near(
            &ok(&h, "draw.drawing.get", json!({"drawingId":d})),
            &ok(&twin, "draw.drawing.get", json!({"drawingId":d})),
        );
        assert_eq!(
            bob,
            ok(&h, "draw.drawing.get", json!({"elementId":c33_support::BOB,"frame":1})),
            "{command}: Bob untouched"
        );
    }
}

// 02:61,101-104; SI-145/146: projected spaces refuse without edits;
// scene space remains a world-plane mapping even behind the camera.
#[test]
fn case5_refusal_recovery() {
    let (h, c, _) = setup();
    ok(&h, "app.context.set", json!({"elementId":c,"frame":1}));
    for z in [15.95, 16.] {
        ok(&h, "anim.transform.set", json!({"nodeIds":[c],"target":"static","position":{"z":z}}));
        let before = ok(&h, "draw.drawing.get", json!({}));
        for space in ["view", "camera"] {
            let p = json!({"space":space,"view":"cameraView","point":{"x":0.,"y":0.}});
            let a = h.availability(vec![("draw.pivot.set".into(), p.as_object().unwrap().clone())]).unwrap();
            assert!(!a[0].enabled);
            assert_eq!(
                err(&h, "draw.pivot.set", p).code,
                if space == "camera" { "arg.invalid" } else { "inspect.behindCamera" }
            );
            let e=h.pointer(tt_executor::Origin::User,json!({"view":"cameraView","space":space,"tool":"brush","events":[{"type":"down","x":480.,"y":270.},{"type":"up","x":500.,"y":270.}]}));
            assert_eq!(e.error.unwrap().code, "inspect.behindCamera");
        }
        assert_eq!(before, ok(&h, "draw.drawing.get", json!({})));
        assert_eq!(err(&h, "draw.viewMapping.get", json!({"view":"cameraView"})).code, "inspect.behindCamera");
        ok(&h, "draw.pivot.set", json!({"space":"scene","point":{"x":2.,"y":-1.}}));
    }
    ok(&h, "anim.transform.set", json!({"nodeIds":[c],"target":"static","position":{"z":-4.}}));
    let p = json!({"space":"view","view":"cameraView","point":{"x":480.,"y":270.}});
    assert!(h.availability(vec![("draw.pivot.set".into(), p.as_object().unwrap().clone())]).unwrap()[0].enabled);
    let e=h.pointer(tt_executor::Origin::User,json!({"view":"cameraView","space":"view","tool":"brush","events":[{"type":"down","x":480.,"y":270.},{"type":"up","x":500.,"y":270.}]}));
    assert!(e.is_ok(), "{e:?}");
}

// 05:352-355 precise visible-content picking, 4 view-pixel tolerance;
// CV1 hidden ancestors omit only their subtree. Bounds must follow new artwork.
#[test]
fn case7_transformed_edge_and_fresh_extent() {
    for (angle, sx, sy) in [(0., 1., 1.), (31., -1.5, 0.6), (90., 0.7, 1.8)] {
        let h = fixture();
        c33_support::clear(&h);
        let c = c33_support::solids(&h, 1)[0].0.clone();
        let id = c33_support::square(&h, "Edge", 20., &c);
        // Force an earlier extent computation, then grow artwork well past it.
        h.canvas_scene("cameraView").unwrap();
        ok(
            &h,
            "draw.rectangle.add",
            json!({"elementId":id,"frame":1,"from":{"x":80.,"y":-20.},"to":{"x":120.,"y":20.},"layer":"new"}),
        );
        ok(&h, "colour.paint.apply", json!({"elementId":id,"frame":1,"points":[{"x":100.,"y":0.}],"colourId":c}));
        ok(
            &h,
            "anim.transform.set",
            json!({"nodeIds":[id],"target":"static","rotation":angle,"scale":{"x":sx,"y":sy}}),
        );
        let s = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
        let entry = s["entries"].as_array().unwrap().iter().find(|e| e["elementId"] == id).unwrap();
        let p = apply(&entry["matrix"], (119., 0.));
        let p = apply(&s["camera"]["cameraToView"], p);
        let hit = ok(&h, "inspect.hitTest", json!({"space":"view","view":"cameraView","point":{"x":p.0,"y":p.1}}));
        assert!(hit["hits"].as_array().unwrap().iter().any(|v| v["elementId"] == id), "{hit}");
        let peg = ok(&h, "element.create", json!({"type":"peg"}))["elementIds"][0].clone();
        ok(&h, "element.parent.set", json!({"elementIds":[id],"parentId":peg}));
        let sibling = c33_support::square(&h, "Sibling", 10., &c);
        ok(&h, "element.visibility.set", json!({"elementIds":[peg],"scope":"scene","visible":false}));
        let s = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
        let es = s["entries"].as_array().unwrap();
        assert!(!es.iter().any(|v| v["elementId"] == id));
        assert!(es.iter().any(|v| v["elementId"] == sibling));
    }
}
