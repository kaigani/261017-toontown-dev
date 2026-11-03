//! M2 C2.3: Flatten, Optimize and Arrange (02 §8.1, §8.2, §8.4): upper colour replaces lower, hidden lower content
//! is discarded, the merged layer sits at the top-most position (or per policy, SI-073); Optimize merges only where
//! appearance cannot change (alpha, gradient and texture overlap cases); Arrange keeps relative order, separates part
//! of a layer and keeps fills.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

/// Skin (solid, opaque), Sky (linear, opaque), Glow (radial, partly transparent), Fabric (texture).
struct C {
    skin: String,
    sky: String,
    glow: String,
    fabric: String,
}

fn c(h: &ExecutorHandle) -> C {
    let v = colours(h);
    C { skin: v[0].clone(), sky: v[1].clone(), glow: v[2].clone(), fabric: v[3].clone() }
}

/// A rectangle on a new layer, painted at its centre: (layer, stroke).
fn rect(h: &ExecutorHandle, x: f64, y: f64, w: f64, hg: f64, colour: &str) -> (String, String) {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": x, "y": y}, "to": {"x": x + w, "y": y + hg}, "layer": "new" }),
    );
    let (cx, cy) = (x + w / 2.0, y + hg / 2.0);
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": cx, "y": cy}], "colourId": colour }));
    (r["layerId"].as_str().unwrap().to_owned(), r["strokeIds"][0].as_str().unwrap().to_owned())
}

/// The colour of the top-most zone at a point.
fn zone_colour(h: &ExecutorHandle, x: f64, y: f64) -> Value {
    let r = ok(h, "draw.hitTest.get", json!({ "drawingId": D1, "point": {"x": x, "y": y} }));
    r["hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["objectKind"] == "zone")
        .map_or(Value::Null, |h| h["colourId"].clone())
}

fn layer_ids(h: &ExecutorHandle) -> Vec<String> {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    d["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["type"] == "layer")
        .map(|n| n["id"].as_str().unwrap().to_owned())
        .collect()
}

fn layer(h: &ExecutorHandle, id: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or(Value::Null)
}

#[test]
fn flatten_lets_the_upper_colour_replace_the_lower_and_discards_hidden_content() {
    let h = fixture();
    let c = c(&h);
    let (low, low_stroke) = rect(&h, 300.0, 300.0, 100.0, 60.0, &c.skin);
    // A short line wholly under the upper opaque zone: hidden, discarded.
    let hidden = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 380, "y": 340}, {"x": 395, "y": 345}], "snap": false, "layer": low }),
    );
    let hidden = hidden["strokeIds"][0].as_str().unwrap().to_owned();
    let (up, _) = rect(&h, 350.0, 320.0, 100.0, 60.0, &c.sky);
    assert_eq!(zone_colour(&h, 375.0, 340.0), json!(c.sky), "before: the upper layer is on top");
    let layers_before = layer_ids(&h).len();
    let start = rev(&h);
    let r = ok(&h, "draw.flatten.apply", json!({ "drawingId": D1, "ids": [low, up] }));
    assert_eq!(rev(&h), start + 1);
    assert_eq!(r["layerId"], json!(up), "the merged layer takes the top-most position");
    assert_eq!(layer_ids(&h).len(), layers_before - 1);
    let removed = r["removedIds"].as_array().unwrap();
    assert!(removed.contains(&json!(low)) && removed.contains(&json!(hidden)), "{r}");
    assert!(removed.contains(&json!(low_stroke)), "the lower outline was cut and its covered piece dropped: {r}");
    assert_eq!(zone_colour(&h, 320.0, 330.0), json!(c.skin));
    assert_eq!(zone_colour(&h, 375.0, 340.0), json!(c.sky), "upper replaces lower");
    assert_eq!(zone_colour(&h, 425.0, 370.0), json!(c.sky));
    // Nothing of the lower outline survives inside the upper zone.
    let merged = layer(&h, &up);
    for s in merged["strokes"].as_array().unwrap().iter().filter(|s| s["kind"] == "pencil") {
        for q in s["bezier"].as_array().unwrap() {
            let (x, y) = (q["x"].as_f64().unwrap(), q["y"].as_f64().unwrap());
            assert!(!(x > 350.5 && x < 449.5 && y > 320.5 && y < 379.5), "a covered point survived: {s}");
        }
    }
    ok(&h, "edit.undo", json!({}));
    assert_eq!(layer_ids(&h).len(), layers_before);
    // One layer: a no-op that writes nothing.
    let start = rev(&h);
    let r = ok(&h, "draw.flatten.apply", json!({ "drawingId": D1, "ids": [up] }));
    assert_eq!((r["layerId"].as_str(), rev(&h)), (Some(up.as_str()), start));
}

#[test]
fn a_lower_precedence_bottom_policy_merges_behind() {
    let h = fixture();
    let c = c(&h);
    let (low, _) = rect(&h, 300.0, 300.0, 100.0, 60.0, &c.skin);
    let (up, _) = rect(&h, 350.0, 320.0, 100.0, 60.0, &c.sky);
    let r = ok(
        &h,
        "draw.flatten.apply",
        json!({ "drawingId": D1, "ids": [low, up], "policy": {"precedence": "lower", "position": "bottom"} }),
    );
    assert_eq!(r["layerId"], json!(low));
    assert_eq!(zone_colour(&h, 375.0, 340.0), json!(c.skin), "the lower colour wins");
    assert_eq!(zone_colour(&h, 425.0, 370.0), json!(c.sky));
}

#[test]
fn flatten_keeps_gradient_matrices() {
    let h = fixture();
    let c = c(&h);
    let (low, _) = rect(&h, 300.0, 300.0, 100.0, 60.0, &c.sky);
    let (up, _) = rect(&h, 600.0, 300.0, 50.0, 50.0, &c.skin);
    let zone = |h: &ExecutorHandle, l: &str| -> Value {
        layer(h, l)["zones"].as_array().unwrap().iter().find(|z| z["colourId"] == json!(c.sky)).cloned().unwrap()
    };
    let before = zone(&h, &low)["textureMatrix"].clone();
    assert!(before.is_array(), "{before}");
    ok(&h, "draw.flatten.apply", json!({ "drawingId": D1, "ids": [low, up] }));
    assert_eq!(zone(&h, &up)["textureMatrix"], before);
}

#[test]
fn optimize_merges_only_where_appearance_cannot_change() {
    // Only content that does not meet merges (C23-R1-04); every accepted merge renders identically.
    let h = fixture();
    let c = c(&h);
    let pixels = |h: &ExecutorHandle, x: f64, y: f64, w: f64, hg: f64| -> String {
        ok(h, "draw.drawing.render", json!({ "drawingId": D1, "width": 400, "region": {"x": x, "y": y, "width": w, "height": hg}, "background": "#FFFFFFFF" }))
            ["pixelsSha256"].as_str().unwrap().to_owned()
    };
    // An opaque fill inside a painted zone: the zone would split along a seam, so it stays separate.
    let (a, _) = rect(&h, 300.0, 300.0, 200.0, 200.0, &c.skin);
    rect(&h, 350.0, 350.0, 50.0, 50.0, &c.sky);
    // A disjoint opaque layer merges with it; the transparent gradient and the texture over painted zones stay.
    let (e, _) = rect(&h, 800.0, 300.0, 200.0, 200.0, &c.skin);
    let (f, _) = rect(&h, 850.0, 350.0, 50.0, 50.0, &c.glow);
    let (g, _) = rect(&h, 860.0, 360.0, 20.0, 20.0, &c.fabric);
    let all = layer_ids(&h);
    let considered = &all[all.len() - 5..];
    let before = pixels(&h, 280.0, 280.0, 760.0, 240.0);
    let r = ok(&h, "draw.optimize.apply", json!({ "drawingId": D1, "ids": considered }));
    assert_eq!(r["merged"], 1, "{r}");
    assert_eq!(r["layerIds"], json!([a, e, f, g]));
    assert_eq!(pixels(&h, 280.0, 280.0, 760.0, 240.0), before, "the accepted merge renders identically");
    assert_eq!(zone_colour(&h, 375.0, 375.0), json!(c.sky));
    assert_eq!(zone_colour(&h, 320.0, 320.0), json!(c.skin));
    // Overlapping shapes stay separate.
    let (x, _) = rect(&h, 300.0, 700.0, 100.0, 60.0, &c.skin);
    let (y, _) = rect(&h, 350.0, 720.0, 100.0, 60.0, &c.sky);
    let r = ok(&h, "draw.optimize.apply", json!({ "drawingId": D1, "ids": [x, y] }));
    assert_eq!(r["merged"], 0);
    // Disjoint layers merge and render identically.
    let (p, _) = rect(&h, 300.0, 900.0, 50.0, 50.0, &c.skin);
    let (q, _) = rect(&h, 500.0, 900.0, 50.0, 50.0, &c.glow);
    let before = pixels(&h, 280.0, 880.0, 300.0, 90.0);
    let r = ok(&h, "draw.optimize.apply", json!({ "drawingId": D1, "ids": [p, q] }));
    assert_eq!((r["merged"].as_u64(), r["layerIds"].clone()), (Some(1), json!([q])));
    assert_eq!(pixels(&h, 280.0, 880.0, 300.0, 90.0), before, "the accepted merge renders identically");
    assert_eq!(zone_colour(&h, 325.0, 925.0), json!(c.skin));
    assert_eq!(zone_colour(&h, 525.0, 925.0), json!(c.glow));
}

#[test]
fn arrange_reorders_back_to_front_and_separates_part_of_a_layer() {
    let h = fixture();
    let c = c(&h);
    let (a, _) = rect(&h, 300.0, 300.0, 50.0, 50.0, &c.skin);
    let (b, _) = rect(&h, 400.0, 300.0, 50.0, 50.0, &c.skin);
    let (cc, _) = rect(&h, 500.0, 300.0, 50.0, 50.0, &c.skin);
    let order = |h: &ExecutorHandle| -> Vec<String> {
        let all = layer_ids(h);
        all.into_iter().filter(|l| [&a, &b, &cc].contains(&l)).collect()
    };
    assert_eq!(order(&h), vec![a.clone(), b.clone(), cc.clone()]);
    let env = run(&h, "draw.arrange.apply", json!({ "drawingId": D1, "ids": [b, a], "to": "front" }));
    assert_eq!(env.undo.as_ref().unwrap().label, "Bring to Front");
    assert_eq!(order(&h), vec![cc.clone(), a.clone(), b.clone()], "the moved pair keeps its relative order");
    let r = ok(&h, "draw.arrange.apply", json!({ "drawingId": D1, "ids": [cc], "to": "forward" }));
    assert_eq!(order(&h), vec![a.clone(), cc.clone(), b.clone()]);
    assert_eq!(r["order"].as_array().unwrap().last(), Some(&json!(b)));
    ok(&h, "draw.arrange.apply", json!({ "drawingId": D1, "ids": [b], "to": "back" }));
    let all = layer_ids(&h);
    assert_eq!(all[0], b, "behind everything in the drawing");
    ok(&h, "draw.arrange.apply", json!({ "drawingId": D1, "ids": [cc], "to": "backward" }));
    assert_eq!(order(&h), vec![b.clone(), cc.clone(), a.clone()]);
    // Part of a layer: the selected stroke is separated into its own layer and moved; the rectangle's fill stays.
    let line = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 300, "y": 400}, {"x": 350, "y": 400}], "snap": false, "layer": a }),
    );
    let lsid = line["strokeIds"][0].as_str().unwrap().to_owned();
    let r = ok(&h, "draw.arrange.apply", json!({ "drawingId": D1, "ids": [lsid], "to": "front" }));
    let fresh = r["createdLayerIds"][0].as_str().unwrap().to_owned();
    assert_eq!(layer_ids(&h).last(), Some(&fresh));
    let moved = layer(&h, &fresh);
    assert_eq!(moved["strokes"].as_array().unwrap().len(), 1);
    assert_eq!(moved["strokes"][0]["id"], json!(lsid));
    assert_eq!(zone_colour(&h, 325.0, 325.0), json!(c.skin));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(layer(&h, &a)["strokes"].as_array().unwrap().len(), 2);
    assert_eq!(
        err(&h, "draw.arrange.apply", json!({ "drawingId": D1, "ids": ["st_nope"], "to": "front" })).details["pointer"],
        "/ids/0"
    );
}
