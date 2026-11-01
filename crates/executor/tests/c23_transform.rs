//! M2 C2.3: transforms and contour editing (02 §7.1.1–§7.1.4, §8.13, §13.5): move/scale/rotate/skew/matrix and copies,
//! point-subset transforms, flips and quarter turns, envelopes, Reposition All Drawings, and the contour editor's point
//! and segment commands. Each is one revision and one undo step, keeps fills through the settle pass and refuses
//! locked drawings.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Invocation, InvokeOptions, Origin};

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

fn xy(v: &Value) -> (f64, f64) {
    (v["x"].as_f64().unwrap(), v["y"].as_f64().unwrap())
}

fn bounds(h: &ExecutorHandle, ids: &Value) -> (f64, f64, f64, f64) {
    let b = ok(h, "draw.bounds.get", json!({ "drawingId": D1, "ids": ids }))["bounds"].clone();
    (b["x"].as_f64().unwrap(), b["y"].as_f64().unwrap(), b["width"].as_f64().unwrap(), b["height"].as_f64().unwrap())
}

/// A rectangle on a new layer, away from the sample's content.
fn rect(h: &ExecutorHandle, x: f64, y: f64, w: f64, hg: f64) -> Value {
    ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": x, "y": y}, "to": {"x": x + w, "y": y + hg}, "layer": "new" }),
    )
}

fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

fn dry(h: &ExecutorHandle, id: &str, params: Value) {
    let before = rev(h);
    let r = h.invoke(Invocation {
        command: id.into(),
        params,
        options: InvokeOptions { dry_run: true, ..Default::default() },
        origin: Origin::User,
    });
    assert!(r.is_ok(), "{id} dry run");
    assert_eq!(rev(h), before, "{id} dry run writes nothing");
}

#[test]
fn translate_rotate_and_matrix_keep_ids_fills_and_undo_in_one_step() {
    let h = fixture();
    let r = rect(&h, 300.0, 300.0, 100.0, 60.0);
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    let c = colours(&h);
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 350, "y": 330}], "colourId": c[1] }));
    let before = node(&h, &sid);
    let params = json!({ "drawingId": D1, "ids": [sid], "translate": {"x": 10, "y": -5} });
    dry(&h, "draw.transform.apply", params.clone());
    let rx = h.subscribe();
    let start = rev(&h);
    let t = ok(&h, "draw.transform.apply", params);
    assert_eq!(t["ids"], json!([sid]), "an unsplit stroke keeps its ID");
    assert_eq!(rev(&h), start + 1);
    assert_eq!(rx.try_iter().filter(|e| e.name == "draw.drawing.changed").count(), 1);
    let after = node(&h, &sid);
    for (a, b) in before["bezier"].as_array().unwrap().iter().zip(after["bezier"].as_array().unwrap()) {
        let ((ax, ay), (bx, by)) = (xy(a), xy(b));
        assert_eq!((bx - ax, by - ay), (10.0, -5.0));
    }
    let zones = layer(&h, r["layerId"].as_str().unwrap())["zones"].clone();
    assert_eq!(zones.as_array().unwrap().len(), 1);
    assert_eq!(zones[0]["colourId"], c[1].as_str(), "the fill moves with its zone: {zones}");
    // A quarter turn about the bounds centre is exact and swaps width and height.
    let (x0, y0, w0, h0) = bounds(&h, &json!([sid]));
    ok(&h, "draw.rotate.apply", json!({ "drawingId": D1, "ids": [sid], "by": "90cw" }));
    let (x1, y1, w1, h1) = bounds(&h, &json!([sid]));
    assert_eq!((w1, h1), (h0, w0));
    assert_eq!((x1 + w1 / 2.0, y1 + h1 / 2.0), (x0 + w0 / 2.0, y0 + h0 / 2.0));
    assert_eq!(layer(&h, r["layerId"].as_str().unwrap())["zones"][0]["colourId"], c[1].as_str());
    // Undo restores the pre-rotation geometry; redo re-applies it.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(bounds(&h, &json!([sid])), (x0, y0, w0, h0));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(bounds(&h, &json!([sid])), (x1, y1, w1, h1));
    // A full matrix overrides the other parameters; a singular one is refused.
    ok(&h, "draw.transform.apply", json!({ "drawingId": D1, "ids": [sid], "matrix": [2, 0, 0, 2, 0, 0] }));
    let (_, _, w2, _) = bounds(&h, &json!([sid]));
    assert!((w2 - 2.0 * w1).abs() < 4.0, "{w2} vs {w1}");
    let e = err(&h, "draw.transform.apply", json!({ "drawingId": D1, "ids": [sid], "matrix": [1, 1, 1, 1, 0, 0] }));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("arg.invalid", Some("/matrix")));
}

#[test]
fn flips_swap_side_fills_and_keep_the_painted_zone() {
    let h = fixture();
    let r = rect(&h, 300.0, 300.0, 100.0, 60.0);
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    let lid = r["layerId"].as_str().unwrap().to_owned();
    let c = colours(&h);
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 350, "y": 330}], "colourId": c[2] }));
    let zone_before = layer(&h, &lid)["zones"][0]["id"].clone();
    for axis in ["horizontal", "vertical"] {
        let f = ok(&h, "draw.flip.apply", json!({ "drawingId": D1, "ids": [sid], "axis": axis }));
        assert_eq!(f["ids"], json!([sid]));
        let zones = layer(&h, &lid)["zones"].clone();
        assert_eq!(zones.as_array().unwrap().len(), 1);
        assert_eq!(zones[0]["colourId"], c[2].as_str(), "{axis}: {zones}");
    }
    // A reflection reverses the boundary's sense, so the zone is re-identified (02 §3.12) and reported.
    let env = run(&h, "draw.flip.apply", json!({ "drawingId": D1, "ids": [sid], "axis": "vertical" }));
    assert_eq!(env.undo.as_ref().unwrap().label, "Flip Vertical");
    let created = env.result.as_ref().unwrap()["createdIds"].clone();
    let zone_now = layer(&h, &lid)["zones"][0]["id"].clone();
    assert!(zone_now == zone_before || created.as_array().unwrap().contains(&zone_now), "{created}");
}

#[test]
fn copies_get_fresh_ids_and_leave_the_originals() {
    let h = fixture();
    let r = rect(&h, 300.0, 300.0, 100.0, 60.0);
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    let before = node(&h, &sid);
    let t = ok(
        &h,
        "draw.transform.apply",
        json!({ "drawingId": D1, "ids": [sid], "translate": {"x": 0, "y": 200}, "copy": true }),
    );
    let copy = t["ids"][0].as_str().unwrap();
    assert_ne!(copy, sid);
    assert!(t["createdIds"].as_array().unwrap().iter().any(|v| v == copy));
    assert_eq!(node(&h, &sid)["bezier"], before["bezier"], "the original is unchanged");
    let moved = node(&h, copy);
    assert_eq!(xy(&moved["bezier"][0]).1, xy(&before["bezier"][0]).1 + 200.0);
    let e = err(
        &h,
        "draw.transform.apply",
        json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}], "translate": {"x": 1, "y": 0}, "copy": true }),
    );
    assert_eq!(e.details["pointer"], "/copy");
}

#[test]
fn point_subset_transforms_move_only_the_chosen_anchors_and_default_to_the_point_selection() {
    let h = fixture();
    let r = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 300, "y": 700}, {"x": 360, "y": 740}, {"x": 420, "y": 700}], "snap": false, "layer": "new" }),
    );
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    let before = node(&h, &sid)["bezier"].clone();
    ok(
        &h,
        "draw.transform.apply",
        json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 1}], "translate": {"x": 0, "y": 20} }),
    );
    let after = node(&h, &sid)["bezier"].clone();
    assert_eq!(after[0], before[0]);
    assert_eq!(xy(&after[3]).1, xy(&before[3]).1 + 20.0);
    assert_eq!(after[6], before[6]);
    // With no object selection, the Contour Editor's point selection is the subject; scale about its own centre.
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "none" }));
    ok(
        &h,
        "draw.pointSelection.set",
        json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}, {"strokeId": sid, "anchor": 2}] }),
    );
    let env = run(&h, "draw.transform.apply", json!({ "drawingId": D1, "scale": {"x": 0.5, "y": 1} }));
    assert_eq!(env.undo.as_ref().unwrap().label, "Contour Editing");
    let scaled = node(&h, &sid)["bezier"].clone();
    assert_eq!((xy(&scaled[0]).0, xy(&scaled[6]).0), (330.0, 390.0));
    assert_eq!(scaled[3], after[3], "the unselected anchor stays");
}

#[test]
fn envelopes_map_the_bounds_corners_and_reposition_all_moves_every_drawing() {
    let h = fixture();
    let r = rect(&h, 300.0, 300.0, 100.0, 100.0);
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    let (x, y, w, hg) = bounds(&h, &json!([sid]));
    // The bounds include half the line; target corners relative to them.
    let corners = json!([
        {"x": x, "y": y}, {"x": x + w, "y": y}, {"x": x + w - 20.0, "y": y + hg}, {"x": x + 20.0, "y": y + hg}
    ]);
    for mapping in ["projective", "bilinear"] {
        let t = ok(
            &h,
            "draw.envelope.apply",
            json!({ "drawingId": D1, "ids": [sid], "corners": corners, "mapping": mapping }),
        );
        assert_eq!(t["ids"], json!([sid]), "{mapping}");
        let b = node(&h, &sid)["bezier"].clone();
        let top: Vec<f64> =
            b.as_array().unwrap().iter().map(xy).filter(|p| (p.1 - 400.0).abs() < 2.0).map(|p| p.0).collect();
        assert!(top.iter().all(|x| *x > 300.0 && *x < 400.0), "{mapping}: the top edge narrows: {b}");
        ok(&h, "edit.undo", json!({}));
    }
    let e = err(&h, "draw.envelope.apply", json!({ "drawingId": D1, "ids": [sid], "corners": [{"x": 0, "y": 0}] }));
    assert_eq!(e.details["pointer"], "/corners");
    // Reposition All moves every unlocked drawing of the element, as one undo step.
    let docs = h.snapshot().unwrap().unwrap();
    let bank = docs.bank_of(&tt_model::Id::parse(D1).unwrap()).unwrap().clone();
    let all: Vec<String> = docs.bank(&bank).unwrap().drawings().ids().iter().map(|i| i.as_str().to_owned()).collect();
    drop(docs);
    let before: Vec<Value> =
        all.iter().map(|d| ok(&h, "draw.bounds.get", json!({ "drawingId": d }))["bounds"].clone()).collect();
    let start = rev(&h);
    let rp = ok(&h, "draw.repositionAll.apply", json!({ "elementId": BOB, "translate": {"x": 0, "y": 20} }));
    assert_eq!(rev(&h), start + 1);
    let moved: Vec<String> =
        rp["drawingIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    let locked = h.snapshot().unwrap().unwrap();
    for (d, b) in all.iter().zip(&before) {
        if b.is_null() || *locked.drawing(&tt_model::Id::parse(d).unwrap()).unwrap().locked() {
            continue;
        }
        assert!(moved.contains(d), "{d}");
        let a = ok(&h, "draw.bounds.get", json!({ "drawingId": d }))["bounds"].clone();
        assert_eq!(a["y"].as_f64().unwrap(), b["y"].as_f64().unwrap() + 20.0, "{d}");
    }
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "draw.bounds.get", json!({ "drawingId": all[0] }))["bounds"], before[0]);
}

#[test]
fn contour_points_add_set_delete_and_segment_move() {
    let h = fixture();
    let r = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 300, "y": 700}, {"x": 420, "y": 700}], "snap": false, "layer": "new" }),
    );
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    let qualified = format!("{D1}/{sid}");
    // Insert at a location: the curve is unchanged (de Casteljau) and the new anchor sits on it.
    let a = ok(&h, "draw.point.add", json!({ "strokeId": qualified, "at": {"x": 330, "y": 705} }));
    assert_eq!(a["anchor"], 1);
    let b = node(&h, &sid)["bezier"].clone();
    assert_eq!(b.as_array().unwrap().len(), 7);
    let p = xy(&b[3]);
    assert!((p.0 - 330.0).abs() < 1e-6 && p.1 == 700.0, "{b}");
    assert_eq!(
        err(&h, "draw.point.add", json!({ "strokeId": qualified, "segment": 9, "t": 0.5 })).details["pointer"],
        "/segment"
    );
    // Symmetric handle pull-out.
    ok(
        &h,
        "draw.point.set",
        json!({ "strokeId": qualified, "anchor": 1, "outHandle": {"x": 10, "y": 15}, "handleMode": "symmetric" }),
    );
    let b = node(&h, &sid)["bezier"].clone();
    let (at, o, i) = (xy(&b[3]), xy(&b[4]), xy(&b[2]));
    assert_eq!((o.0 - at.0, o.1 - at.1), (10.0, 15.0));
    assert_eq!((i.0 - at.0, i.1 - at.1), (-10.0, -15.0));
    ok(&h, "draw.point.set", json!({ "strokeId": qualified, "anchor": 1, "handleMode": "reset" }));
    let b = node(&h, &sid)["bezier"].clone();
    assert_eq!((xy(&b[2]), xy(&b[4])), (xy(&b[3]), xy(&b[3])));
    // Segment drag: the point at t moves by exactly delta.
    ok(&h, "draw.segment.move", json!({ "strokeId": qualified, "segment": 1, "t": 0.5, "delta": {"x": 0, "y": 30} }));
    let b = node(&h, &sid)["bezier"].clone();
    let (p0, p1, p2, p3) = (xy(&b[3]), xy(&b[4]), xy(&b[5]), xy(&b[6]));
    let mid_y = 0.125 * p0.1 + 0.375 * p1.1 + 0.375 * p2.1 + 0.125 * p3.1;
    assert!((mid_y - 730.0).abs() < 1e-9, "{mid_y}");
    assert_eq!(
        err(&h, "draw.segment.move", json!({ "strokeId": qualified, "segment": 0, "t": 0, "delta": {"x": 0, "y": 1} }))
            .details["pointer"],
        "/t"
    );
    // Delete: an anchor goes; too few left removes the stroke.
    let d = ok(&h, "draw.point.delete", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 1}] }));
    assert!(d["modifiedIds"].as_array().unwrap().iter().any(|v| v == &json!(sid)));
    assert_eq!(node(&h, &sid)["bezier"].as_array().unwrap().len(), 4);
    let d = ok(&h, "draw.point.delete", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}] }));
    assert_eq!(d["removedIds"], json!([sid]));
    assert!(node(&h, &sid).is_null());
    ok(&h, "edit.undo", json!({}));
    assert!(!node(&h, &sid).is_null());
}

#[test]
fn moving_a_shared_junction_anchor_moves_every_stroke_end_there() {
    let h = fixture();
    let a = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 300, "y": 900}, {"x": 400, "y": 900}], "snap": false, "layer": "new" }),
    );
    let lid = a["layerId"].as_str().unwrap().to_owned();
    let b = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 400, "y": 900}, {"x": 400, "y": 1000}], "snap": false, "layer": lid }),
    );
    let (sa, sb) = (a["strokeIds"][0].as_str().unwrap().to_owned(), b["strokeIds"][0].as_str().unwrap().to_owned());
    let m = ok(
        &h,
        "draw.point.move",
        json!({ "drawingId": D1, "points": [{"strokeId": sa, "anchor": 1}], "delta": {"x": 10, "y": 10} }),
    );
    let modified = m["modifiedIds"].as_array().unwrap().clone();
    assert!(modified.contains(&json!(sa)) && modified.contains(&json!(sb)), "{m}");
    assert_eq!(xy(&node(&h, &sa)["bezier"][3]), (410.0, 910.0));
    assert_eq!(xy(&node(&h, &sb)["bezier"][0]), (410.0, 910.0));
    // Snap to Contour pulls a moved end onto a nearby anchor of another stroke.
    ok(
        &h,
        "draw.point.move",
        json!({ "drawingId": D1, "points": [{"strokeId": sb, "anchor": 1}], "delta": {"x": -4, "y": 0}, "snap": false }),
    );
    let c = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 600, "y": 1000}, {"x": 700, "y": 1000}], "snap": false, "layer": lid }),
    );
    let sc = c["strokeIds"][0].as_str().unwrap().to_owned();
    ok(
        &h,
        "draw.point.move",
        json!({ "drawingId": D1, "points": [{"strokeId": sc, "anchor": 0}], "delta": {"x": -197, "y": 3}, "snap": true }),
    );
    assert_eq!(xy(&node(&h, &sc)["bezier"][0]), (396.0, 1000.0), "403,1003 snaps to the anchor at 396,1000");
}

#[test]
fn brush_spines_survive_rigid_motions_and_are_dropped_by_scaling() {
    let h = fixture();
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "kind": "brush",
                "points": [{"x": 0, "y": 600, "pressure": 0.5}, {"x": 40, "y": 610, "pressure": 1.0}, {"x": 80, "y": 600, "pressure": 0.5}],
                "pen": { "tipShape": "round", "minSize": 2, "maxSize": 10, "tipAngle": 0 } }),
    );
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    ok(&h, "draw.rotate.apply", json!({ "drawingId": D1, "ids": [sid], "by": "90ccw" }));
    let s = node(&h, &sid);
    assert!(s["bezier"].is_array() && s["pen"]["maxSize"] == 10.0, "a turned brush keeps its spine and pen: {s}");
    ok(&h, "draw.flip.apply", json!({ "drawingId": D1, "ids": [sid], "axis": "horizontal" }));
    assert!(node(&h, &sid)["bezier"].is_array());
    ok(&h, "draw.transform.apply", json!({ "drawingId": D1, "ids": [sid], "scale": {"x": 2, "y": 1} }));
    let s = node(&h, &sid);
    assert!(s["bezier"].is_null() && s["pen"].is_null(), "a stretched outline becomes authoritative: {s}");
    assert!(!s["outline"].is_null() || s["closed"] == true);
}

#[test]
fn locked_drawings_and_missing_objects_are_refused() {
    let h = fixture();
    let r = rect(&h, 300.0, 300.0, 100.0, 60.0);
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    let e =
        err(&h, "draw.transform.apply", json!({ "drawingId": D1, "ids": ["st_nope"], "translate": {"x": 1, "y": 0} }));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("drawing.objectNotFound", Some("/ids/0")));
    ok(&h, "draw.lock.set", json!({ "drawingId": D1, "locked": true }));
    let start = rev(&h);
    for (cmd, p) in [
        ("draw.transform.apply", json!({ "drawingId": D1, "ids": [sid], "translate": {"x": 1, "y": 0} })),
        ("draw.flip.apply", json!({ "drawingId": D1, "ids": [sid], "axis": "vertical" })),
        ("draw.rotate.apply", json!({ "drawingId": D1, "ids": [sid], "by": "180" })),
        (
            "draw.point.move",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}], "delta": {"x": 1, "y": 0} }),
        ),
        ("draw.point.set", json!({ "strokeId": format!("{D1}/{sid}"), "anchor": 0, "position": {"x": 0, "y": 0} })),
    ] {
        assert_eq!(err(&h, cmd, p).code, "drawing.locked", "{cmd}");
    }
    assert_eq!(rev(&h), start);
    // Reposition All skips locked drawings: with every drawing of Bob that has content locked, nothing moves.
    let e = err(&h, "draw.repositionAll.apply", json!({ "elementId": BOB, "translate": {"x": 1, "y": 0} }));
    assert_eq!(e.code, "drawing.nothingToDo");
    assert_eq!(rev(&h), start);
}
