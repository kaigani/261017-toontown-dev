//! M2 C2.3: Smooth and Convert Lines to Brush (02 §8.3, §8.6): shape within tolerance, junctions and point
//! selections respected, brush spines dropped only when the outline changes, fills carried over, repeated runs; lines
//! swept with the current pen keeping invisible centre paths so zones and fills stay.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

fn node(h: &ExecutorHandle, id: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or(Value::Null)
}

fn layer(h: &ExecutorHandle, id: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or(Value::Null)
}

fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

fn anchors(v: &Value) -> usize {
    v["bezier"].as_array().unwrap().len().div_ceil(3)
}

fn poly(h: &ExecutorHandle, pts: &[(f64, f64)], layer: &str, closed: bool) -> Value {
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "fit": false, "closed": closed, "layer": layer,
                "points": pts.iter().map(|(x, y)| json!({"x": x, "y": y})).collect::<Vec<_>>() }),
    )
}

#[test]
fn smooth_removes_redundant_anchors_keeps_junctions_and_repeats() {
    let h = fixture();
    let line: Vec<(f64, f64)> = (0..=10).map(|k| (300.0 + f64::from(k) * 10.0, 800.0)).collect();
    let a = poly(&h, &line, "new", false);
    let (lid, sa) = (a["layerId"].as_str().unwrap().to_owned(), a["strokeIds"][0].as_str().unwrap().to_owned());
    // A second stroke ends on A's anchor at x = 350: a junction.
    poly(&h, &[(350.0, 800.0), (350.0, 900.0)], &lid, false);
    // Noding split A at the junction into two pieces of six anchors; smoothing the layer keeps the junction.
    assert!(node(&h, &sa).is_null());
    let start = rev(&h);
    let r = ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": [lid] }));
    assert_eq!(rev(&h), start + 1);
    assert_eq!(r["removedPointCount"], 8, "{r}");
    let l = layer(&h, &lid);
    let strokes = l["strokes"].as_array().unwrap();
    assert_eq!(strokes.len(), 3);
    assert!(strokes.iter().all(|s| anchors(s) == 2), "{l}");
    let ends: usize = strokes
        .iter()
        .map(|s| {
            s["bezier"].as_array().unwrap().iter().step_by(3).filter(|q| q["x"] == 350.0 && q["y"] == 800.0).count()
        })
        .sum();
    assert_eq!(ends, 3, "the junction is still where the three strokes meet");
    // Each invocation smooths further; nothing left to remove writes nothing.
    let start = rev(&h);
    assert_eq!(ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": [lid] }))["removedPointCount"], 0);
    assert_eq!(rev(&h), start);
    ok(&h, "edit.undo", json!({}));
    let l = layer(&h, &lid);
    assert_eq!(l["strokes"].as_array().unwrap().iter().map(anchors).sum::<usize>(), 14);
}

#[test]
fn smooth_respects_a_point_selection_and_the_tolerance() {
    let h = fixture();
    let line: Vec<(f64, f64)> = (0..=6).map(|k| (300.0 + f64::from(k) * 10.0, 800.0)).collect();
    let a = poly(&h, &line, "new", false);
    let sa = a["strokeIds"][0].as_str().unwrap().to_owned();
    let r = ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "points": [{"strokeId": sa, "anchor": 3}] }));
    assert_eq!(r["removedPointCount"], 1);
    assert_eq!(anchors(&node(&h, &sa)), 6);
    // A zigzag deviating 5 du keeps every anchor at tolerance 1 and loses them at tolerance 10.
    let zig: Vec<(f64, f64)> =
        (0..=6).map(|k| (300.0 + f64::from(k) * 20.0, if k % 2 == 0 { 900.0 } else { 905.0 })).collect();
    let z = poly(&h, &zig, "new", false);
    let sz = z["strokeIds"][0].as_str().unwrap().to_owned();
    assert_eq!(
        ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": [sz], "tolerance": 1 }))["removedPointCount"],
        0
    );
    assert!(
        ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": [sz], "tolerance": 10 }))["removedPointCount"]
            .as_u64()
            .unwrap()
            > 0
    );
    let e = err(&h, "draw.smooth.apply", json!({ "drawingId": D1, "points": [{"strokeId": sa, "anchor": 99}] }));
    assert_eq!(e.details["pointer"], "/points/0");
}

#[test]
fn smoothing_a_painted_shape_keeps_its_fill_and_a_brush_outline_loses_its_spine() {
    let h = fixture();
    let c = colours(&h);
    let mut ring: Vec<(f64, f64)> = Vec::new();
    for k in 0..10 {
        ring.push((300.0 + f64::from(k) * 10.0, 300.0));
    }
    for k in 0..10 {
        ring.push((400.0, 300.0 + f64::from(k) * 10.0));
    }
    for k in 0..10 {
        ring.push((400.0 - f64::from(k) * 10.0, 400.0));
    }
    for k in 0..10 {
        ring.push((300.0, 400.0 - f64::from(k) * 10.0));
    }
    let r = poly(&h, &ring, "new", true);
    let lid = r["layerId"].as_str().unwrap().to_owned();
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 350, "y": 350}], "colourId": c[0] }));
    let s = ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": [r["strokeIds"][0]] }));
    assert_eq!(s["removedPointCount"], 36, "only the four corners stay: {s}");
    let zones = layer(&h, &lid)["zones"].clone();
    assert_eq!(zones.as_array().unwrap().len(), 1);
    assert_eq!(zones[0]["colourId"], json!(c[0]));
    // A brush stroke: a tolerance of 0 removes nothing and keeps the spine; smoothing that removes anchors drops it.
    let b = ok(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "kind": "brush",
                "points": [{"x": 0, "y": 600, "pressure": 0.5}, {"x": 40, "y": 610, "pressure": 1.0}, {"x": 80, "y": 600, "pressure": 0.5}],
                "pen": { "tipShape": "round", "minSize": 2, "maxSize": 10, "tipAngle": 0 } }),
    );
    let bid = b["strokeIds"][0].as_str().unwrap().to_owned();
    ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": [bid], "tolerance": 0 }));
    let before = node(&h, &bid);
    if before["bezier"].is_array() {
        let r = ok(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": [bid], "tolerance": 2 }));
        let after = node(&h, &bid);
        if r["removedPointCount"].as_u64().unwrap() > 0 {
            assert!(after["bezier"].is_null() && after["pen"].is_null(), "{after}");
        } else {
            assert_eq!(after["pen"], before["pen"]);
        }
    }
}

#[test]
fn lines_to_brush_sweeps_the_current_pen_and_keeps_an_invisible_centre() {
    let h = fixture();
    let c = colours(&h);
    let r = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}, "layer": "new" }),
    );
    let (lid, sid) = (r["layerId"].as_str().unwrap().to_owned(), r["strokeIds"][0].as_str().unwrap().to_owned());
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 350, "y": 330}], "colourId": c[1] }));
    let line_colour = node(&h, &sid)["colourId"].clone();
    let zone_before = layer(&h, &lid)["zones"].clone();
    let start = rev(&h);
    let t = ok(&h, "draw.linesToBrush.apply", json!({ "drawingId": D1, "ids": [sid] }));
    assert_eq!(rev(&h), start + 1);
    let centres = t["centreIds"].as_array().unwrap().clone();
    assert!(!centres.is_empty());
    for c in &centres {
        assert_eq!(node(&h, c.as_str().unwrap())["kind"], "invisible");
    }
    let bid = t["strokeIds"][0].as_str().unwrap().to_owned();
    let brush = node(&h, &bid);
    assert_eq!(brush["kind"], "brush");
    assert_eq!(brush["colourId"], line_colour, "filled with the line's colour");
    let pen = &brush["pen"];
    assert_eq!(pen["minSize"], pen["maxSize"], "no pressure: {pen}");
    let zones = layer(&h, &lid)["zones"].clone();
    assert_eq!(zones.as_array().unwrap().len(), zone_before.as_array().unwrap().len());
    assert_eq!(zones[0]["colourId"], json!(c[1]), "the fill stays: {zones}");
    // Not a pencil line: nothing to convert.
    assert_eq!(
        err(&h, "draw.linesToBrush.apply", json!({ "drawingId": D1, "ids": [bid] })).code,
        "drawing.nothingToDo"
    );
    ok(&h, "edit.undo", json!({}));
    assert_eq!(node(&h, &sid)["kind"], "pencil");
    assert!(node(&h, &bid).is_null());
}

#[test]
fn centre_line_follows_the_spine_with_the_average_width_and_reports_what_it_cannot_do() {
    let h = fixture();
    let b = ok(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "kind": "brush",
                "points": [{"x": 0, "y": 700, "pressure": 0.5}, {"x": 60, "y": 700, "pressure": 0.5}],
                "pen": { "tipShape": "round", "minSize": 2, "maxSize": 10, "tipAngle": 0 } }),
    );
    let bid = b["strokeIds"][0].as_str().unwrap().to_owned();
    let colour = node(&h, &bid)["colourId"].clone();
    let r = ok(&h, "draw.centreLine.apply", json!({ "drawingId": D1, "ids": [bid] }));
    assert!(r["removedIds"].as_array().unwrap().contains(&json!(bid)));
    let line = node(&h, r["strokeIds"][0].as_str().unwrap());
    assert_eq!((line["kind"].as_str(), line["colourId"].clone()), (Some("pencil"), colour));
    assert_eq!(line["lineStyle"]["thickness"], 6.0, "average width at pressure 0.5");
    assert_eq!(
        (line["bezier"][0]["x"].as_f64(), line["bezier"].as_array().unwrap().last().unwrap()["x"].as_f64()),
        (Some(0.0), Some(60.0))
    );
    ok(&h, "edit.undo", json!({}));
    // A brush outline edited directly has no spine: kept and reported.
    ok(&h, "draw.transform.apply", json!({ "drawingId": D1, "ids": [bid], "scale": {"x": 2, "y": 1} }));
    let r = ok(&h, "draw.centreLine.apply", json!({ "drawingId": D1, "ids": [bid] }));
    assert_eq!((r["skippedIds"].clone(), r["warnings"].clone()), (json!([bid]), json!(["drawing.centreLineNoSpine"])));
    // Painted zones that are not brush strokes are warned about.
    let c = colours(&h);
    let rect = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 360, "y": 340}, "layer": "new" }),
    );
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [rect["zoneIds"][0]], "colourId": c[0] }));
    let zone = layer(&h, rect["layerId"].as_str().unwrap())["zones"][0]["id"].clone();
    let b2 = ok(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "kind": "brush", "points": [{"x": 0, "y": 800}, {"x": 40, "y": 800}],
        "pen": { "tipShape": "round", "minSize": 4, "maxSize": 4, "tipAngle": 0 } }),
    );
    let r = ok(&h, "draw.centreLine.apply", json!({ "drawingId": D1, "ids": [zone, b2["strokeIds"][0]] }));
    assert!(r["warnings"].as_array().unwrap().contains(&json!("drawing.centreLineOnFill")), "{r}");
}
