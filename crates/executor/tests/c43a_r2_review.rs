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

// 05:394 temporary Skew; 397-399 keyed channels/current frame/one step;
// 403-418 Select static, Transform keyed. SI-144/151 label Skew Element.
#[test]
fn case6_channels_geometry_and_exact_replay() {
    for tool in ["transform", "select"] {
        for kind in ["skew", "horizontal", "vertical", "corner"] {
            let h = fixture();
            let colour = c33_support::solids(&h, 1)[0].0.clone();
            let id = c33_support::square(&h, "Handles", 50., &colour);
            ok(&h, "app.context.set", json!({"elementId":id,"frame":7}));
            ok(&h, "anim.tool.set", json!({"tool":tool}));
            ok(&h, "edit.selection.set", json!({"context":"staging","ids":[id]}));
            let s = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
            let u = &s["overlays"]["selectionUnion"];
            let a = apply(&s["camera"]["cameraToView"], (u[0].as_f64().unwrap(), u[1].as_f64().unwrap()));
            let b = apply(&s["camera"]["cameraToView"], (u[2].as_f64().unwrap(), u[3].as_f64().unwrap()));
            let (l, t, r, b) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
            let (start, end, fields, channels, mods) = match kind {
                "skew" => (
                    ((l + r) / 2., t),
                    ((l + r) / 2. + 20., t),
                    json!({"skew":(20./((b-t)/2.)).atan().to_degrees()}),
                    vec!["skew"],
                    vec!["alt"],
                ),
                "horizontal" => (
                    (r, (t + b) / 2.),
                    (r + 20., (t + b) / 2.),
                    json!({"scale":{"x":(r+20.-l)/(r-l)}}),
                    vec!["scale.x"],
                    vec![],
                ),
                "vertical" => (
                    ((l + r) / 2., t),
                    ((l + r) / 2., t - 20.),
                    json!({"scale":{"y":(t-20.-b)/(t-b)}}),
                    vec!["scale.y"],
                    vec![],
                ),
                _ => (
                    (r, t),
                    (r + 20., t - 20.),
                    json!({"scale":{"x":(r+20.-l)/(r-l),"y":(t-20.-b)/(t-b)}}),
                    vec!["scale.x", "scale.y"],
                    vec![],
                ),
            };
            // Select's alternate modifier does not activate Skew; use its skew handle.
            let (start, end) = if kind == "skew" && tool == "select" {
                ((start.0, start.1 - 12.), (end.0, end.1 - 12.))
            } else {
                (start, end)
            };
            let twin = ExecutorHandle::spawn(Default::default()).unwrap();
            assert!(twin.open_document(h.snapshot().unwrap().unwrap(), None, false).is_ok());
            let before = ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap();
            let out=h.pointer(tt_executor::Origin::User,json!({"view":"cameraView","tool":tool,"events":[{"type":"down","x":start.0,"y":start.1,"modifiers":mods},{"type":"up","x":end.0,"y":end.1,"modifiers":mods}]}));
            assert!(out.is_ok(), "{out:?}");
            let out = out.result.unwrap();
            let cmd =
                out["commands"].as_array().unwrap().iter().find(|c| c["commandId"] == "anim.transform.set").unwrap();
            let mut expected = fields;
            expected["nodeIds"] = json!([id]);
            expected["relative"] = json!(true);
            expected["target"] = json!(if tool == "transform" { "key" } else { "static" });
            if tool == "transform" {
                expected["frame"] = json!(7);
            }
            near(&cmd["params"], &expected);
            ok(&twin, "anim.transform.set", cmd["params"].clone());
            let snap = h.snapshot().unwrap().unwrap();
            let other = twin.snapshot().unwrap().unwrap();
            for k in snap.entry_keys() {
                assert_eq!(snap.entry_raw_json(&k), other.entry_raw_json(&k), "D-4 {k:?}");
            }
            let keys = ok(&h, "anim.key.list", json!({"nodeId":id}));
            let got: Vec<&str> = keys["keys"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|v| v["channels"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()))
                .collect();
            assert_eq!(got, if tool == "transform" { channels } else { vec![] });
            eprintln!("{tool} {kind}: keys {keys}");
            let hist = ok(&h, "edit.history.get", json!({}));
            let pos = hist["position"].as_u64().unwrap();
            assert_eq!(pos, before + 1);
            assert_eq!(
                hist["entries"][pos as usize - 1]["label"],
                if kind == "skew" { "Skew Element" } else { "Scale Element" }
            );
            assert_eq!(ok(&h, "anim.tool.get", json!({}))["tool"], tool);
        }
    }
}
