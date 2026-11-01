//! M2 C2.3: the Cutter and the Eraser (02 §7.2.2, §7.2.7, §13.5): crossing lines split on the boundary with reported
//! piece IDs, zones split along it keeping their fills, the inside selected or deleted (zones unpainted, cut edges
//! left as invisible strokes), segment cuts (SI-071), brush strokes never split (SI-059), one undo step each.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

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

/// A painted 100 x 60 rectangle at (300, 300) on a new layer: (layer, stroke, colour).
fn painted_rect(h: &ExecutorHandle) -> (String, String, String) {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}, "layer": "new" }),
    );
    let c = colours(h)[1].clone();
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 350, "y": 330}], "colourId": c }));
    (r["layerId"].as_str().unwrap().to_owned(), r["strokeIds"][0].as_str().unwrap().to_owned(), c)
}

fn strokes_of_kind(l: &Value, kind: &str) -> usize {
    l["strokes"].as_array().unwrap().iter().filter(|s| s["kind"] == kind).count()
}

fn painted(l: &Value) -> usize {
    l["zones"].as_array().unwrap().iter().filter(|z| !z["colourId"].is_null()).count()
}

const STRIP: [(f64, f64); 4] = [(340.0, 280.0), (360.0, 280.0), (360.0, 380.0), (340.0, 380.0)];

fn strip() -> Value {
    json!(STRIP.iter().map(|(x, y)| json!({"x": x, "y": y})).collect::<Vec<_>>())
}

#[test]
fn a_select_cut_splits_lines_and_zones_on_the_boundary_and_selects_the_inside() {
    let h = fixture();
    let (lid, sid, c) = painted_rect(&h);
    let start = rev(&h);
    let rx = h.subscribe();
    let r = ok(&h, "draw.cut.apply", json!({ "drawingId": D1, "polygon": strip() }));
    assert_eq!(rev(&h), start + 1, "one revision");
    assert_eq!(rx.try_iter().filter(|e| e.name == "draw.drawing.changed").count(), 1);
    let inside: Vec<String> =
        r["insideIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    assert_eq!(inside.len(), 2, "the top and bottom middles: {r}");
    assert!(r["removedIds"].as_array().unwrap().contains(&json!(sid)), "the split rectangle is replaced: {r}");
    let l = layer(&h, &lid);
    assert_eq!(l["zones"].as_array().unwrap().len(), 3);
    assert_eq!(painted(&l), 3, "each part keeps the fill: {}", l["zones"]);
    assert!(l["zones"].as_array().unwrap().iter().all(|z| z["colourId"] == c.as_str()));
    // The cut edges inside the fill stay as invisible strokes; the boundary outside it goes.
    assert_eq!(strokes_of_kind(&l, "invisible"), 2);
    let sel = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }));
    let mut selected: Vec<String> =
        sel["selectedIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    selected.sort();
    let mut want = inside.clone();
    want.sort();
    assert_eq!(selected, want);
    ok(&h, "edit.undo", json!({}));
    let l = layer(&h, &lid);
    assert_eq!((l["zones"].as_array().unwrap().len(), strokes_of_kind(&l, "invisible")), (1, 0));
}

#[test]
fn a_delete_cut_removes_the_inside_lines_and_unpaints_the_inside_zone() {
    let h = fixture();
    let (lid, _, _) = painted_rect(&h);
    let r = ok(&h, "draw.cut.apply", json!({ "drawingId": D1, "polygon": strip(), "then": "delete" }));
    assert!(r["insideIds"].as_array().unwrap().is_empty());
    let l = layer(&h, &lid);
    assert_eq!(strokes_of_kind(&l, "pencil"), 2, "the left and right parts of the outline: {l}");
    // The cut edges close the two remaining parts; the inside, open now, is no zone at all.
    assert_eq!(strokes_of_kind(&l, "invisible"), 2);
    assert_eq!((l["zones"].as_array().unwrap().len(), painted(&l)), (2, 2), "{}", l["zones"]);
    // Nothing reported as created was removed again, and every removed ID existed before.
    for id in r["createdIds"].as_array().unwrap() {
        assert!(!r["removedIds"].as_array().unwrap().contains(id));
    }
    let hit = ok(&h, "draw.hitTest.get", json!({ "drawingId": D1, "point": {"x": 350, "y": 330} }));
    assert!(hit["zones"].as_array().is_none_or(|z| z.iter().all(|z| z["colourId"].is_null())), "{hit}");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(painted(&layer(&h, &lid)), 1);
}

#[test]
fn a_rect_cut_and_a_segment_cut() {
    let h = fixture();
    let (lid, _, _) = painted_rect(&h);
    let r = ok(
        &h,
        "draw.cut.apply",
        json!({ "drawingId": D1, "segment": {"from": {"x": 350, "y": 250}, "to": {"x": 350, "y": 410}} }),
    );
    assert_eq!(r["insideIds"].as_array().unwrap().len(), 2, "the two halves of the outline: {r}");
    let l = layer(&h, &lid);
    assert_eq!((l["zones"].as_array().unwrap().len(), painted(&l)), (2, 2));
    assert_eq!(strokes_of_kind(&l, "invisible"), 1, "only the part through the fill stays");
    let e = err(
        &h,
        "draw.cut.apply",
        json!({ "drawingId": D1, "segment": {"from": {"x": 0, "y": 0}, "to": {"x": 1, "y": 0}}, "then": "delete" }),
    );
    assert_eq!(e.details["pointer"], "/then");
    ok(&h, "edit.undo", json!({}));
    let r = ok(
        &h,
        "draw.cut.apply",
        json!({ "drawingId": D1, "rect": {"x": 280, "y": 280, "width": 200, "height": 200}, "then": "select" }),
    );
    assert_eq!(r["insideIds"].as_array().unwrap().len(), 1, "a wholly inside stroke is inside, unsplit: {r}");
    let e = err(&h, "draw.cut.apply", json!({ "drawingId": D1, "rect": {"x": 0, "y": 0, "width": 0, "height": 5} }));
    assert_eq!(e.code, "drawing.invalidGeometry");
    let e = err(
        &h,
        "draw.cut.apply",
        json!({ "drawingId": D1, "polygon": strip(), "rect": {"x": 0, "y": 0, "width": 1, "height": 1} }),
    );
    assert_eq!(e.code, "arg.invalid");
    let e =
        err(&h, "draw.cut.apply", json!({ "drawingId": D1, "rect": {"x": 5000, "y": 5000, "width": 1, "height": 1} }));
    assert_eq!(e.code, "drawing.nothingToDo");
}

#[test]
fn erasing_splits_lines_keeps_brush_strokes_whole_and_respects_layers_and_locks() {
    let h = fixture();
    let line = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 300, "y": 600}, {"x": 400, "y": 600}], "snap": false, "layer": "new" }),
    );
    let (lid, sid) = (line["layerId"].as_str().unwrap().to_owned(), line["strokeIds"][0].as_str().unwrap().to_owned());
    let brush = ok(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "kind": "brush",
                "points": [{"x": 330, "y": 650}, {"x": 370, "y": 650}],
                "pen": { "tipShape": "round", "minSize": 4, "maxSize": 4, "tipAngle": 0 } }),
    );
    let bid = brush["strokeIds"][0].as_str().unwrap().to_owned();
    let r = ok(
        &h,
        "draw.erase.apply",
        json!({ "drawingId": D1, "path": [{"x": 350, "y": 580}, {"x": 350, "y": 670}], "pen": {"minSize": 10, "maxSize": 10} }),
    );
    assert!(r["removedIds"].as_array().unwrap().contains(&json!(sid)), "{r}");
    assert_eq!(r["keptBrushIds"], json!([bid]));
    let l = layer(&h, &lid);
    let pencils: Vec<&Value> = l["strokes"].as_array().unwrap().iter().filter(|s| s["kind"] == "pencil").collect();
    assert_eq!(pencils.len(), 2, "a gap through the line: {l}");
    for p in &pencils {
        let xs: Vec<f64> = p["bezier"].as_array().unwrap().iter().map(|q| q["x"].as_f64().unwrap()).collect();
        assert!(xs.iter().all(|x| *x <= 345.0 + 1e-6) || xs.iter().all(|x| *x >= 355.0 - 1e-6), "{xs:?}");
    }
    assert_eq!(strokes_of_kind(&l, "invisible"), 0, "no cut edge outside a zone");
    // A layer list limits the erase; an unknown layer is refused with its pointer.
    let e = err(&h, "draw.erase.apply", json!({ "drawingId": D1, "region": strip(), "layers": ["nd_nope"] }));
    assert_eq!(e.details["pointer"], "/layers/0");
    let e = err(&h, "draw.erase.apply", json!({ "drawingId": D1, "region": strip(), "path": [{"x": 0, "y": 0}] }));
    assert_eq!(e.code, "arg.invalid");
    ok(&h, "draw.lock.set", json!({ "drawingId": D1, "locked": true }));
    let start = rev(&h);
    assert_eq!(err(&h, "draw.erase.apply", json!({ "drawingId": D1, "region": strip() })).code, "drawing.locked");
    assert_eq!(err(&h, "draw.cut.apply", json!({ "drawingId": D1, "polygon": strip() })).code, "drawing.locked");
    assert_eq!(rev(&h), start);
}
