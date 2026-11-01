//! M2 C2.3: fill mappings, Set Colour and Replace Colour (03 §8.7, §9.1, §9.3): explicit, default and clipboard
//! mappings with typed refusals and per-object skips; recolouring a selection (painted zones inside it, unpainted
//! zones left alone, text runs); replacing colours by scope with locked drawings skipped, pens, counts and the
//! keep/default matrix rule. Each change is one revision and one undo step.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const CAPTION: &str = "nd_01K742SG0WFAQJFKAZKV4CD56C";

fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

fn layer(h: &ExecutorHandle, id: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or(Value::Null)
}

fn node(h: &ExecutorHandle, id: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or(Value::Null)
}

/// A rectangle on a new layer painted with `colour`: (layer, stroke, zone).
fn painted(h: &ExecutorHandle, x: f64, colour: &str) -> (String, String, String) {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": x, "y": 300}, "to": {"x": x + 100.0, "y": 360}, "layer": "new" }),
    );
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [r["zoneIds"][0]], "colourId": colour }));
    let lid = r["layerId"].as_str().unwrap().to_owned();
    let zone = layer(h, &lid)["zones"][0]["id"].as_str().unwrap().to_owned();
    (lid, r["strokeIds"][0].as_str().unwrap().to_owned(), zone)
}

#[test]
fn fill_mapping_get_reads_handles_and_refuses_unmappable_objects() {
    let h = fixture();
    let c = colours(&h);
    let (_, _, sky_zone) = painted(&h, 300.0, &c[1]);
    let g = ok(&h, "colour.fillMapping.get", json!({ "drawingId": D1, "objectId": sky_zone }));
    assert_eq!((g["kind"].as_str(), g["colourId"].as_str()), (Some("linearGradient"), Some(c[1].as_str())));
    assert_eq!(g["matrix"], json!([100.0, 0.0, 0.0, 60.0, 300.0, 300.0]));
    assert_eq!(g["linear"], json!({ "start": {"x": 300.0, "y": 330.0}, "end": {"x": 400.0, "y": 330.0} }));
    let (_, _, glow_zone) = painted(&h, 500.0, &c[2]);
    let r = ok(&h, "colour.fillMapping.get", json!({ "drawingId": D1, "objectId": glow_zone }));
    assert!(r["radial"]["radius"].as_f64().unwrap() > 0.0, "{r}");
    let (_, _, skin_zone) = painted(&h, 700.0, &c[0]);
    let unpainted = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 900, "y": 300}, "to": {"x": 950, "y": 360}, "layer": "new" }),
    )["zoneIds"][0]
        .clone();
    let inv = ok(&h, "draw.stroke.add", json!({ "drawingId": D1, "kind": "invisible", "fit": false, "layer": "new", "points": [{"x": 0, "y": 900}, {"x": 50, "y": 900}] }))["strokeIds"][0].clone();
    // A solid colour reports the matrix it keeps (none stored: the box mapping); nothing to map otherwise (SI-075).
    let s = ok(&h, "colour.fillMapping.get", json!({ "drawingId": D1, "objectId": skin_zone }));
    assert_eq!(
        (s["kind"].as_str(), s["matrix"].clone()),
        (Some("solid"), json!([100.0, 0.0, 0.0, 60.0, 700.0, 300.0]))
    );
    for (id, reason) in [(unpainted, "unpainted"), (inv, "invisible"), (json!(CAPTION), "text")] {
        let e = err(&h, "colour.fillMapping.get", json!({ "drawingId": D1, "objectId": id }));
        assert_eq!((e.code.as_str(), e.details["reason"].as_str()), ("colour.notMappable", Some(reason)), "{id}");
    }
    let e = err(&h, "colour.fillMapping.get", json!({ "drawingId": D1, "objectId": "zn_0000000000000000000000000Z" }));
    assert_eq!(e.code, "drawing.staleZoneId");
}

#[test]
fn fill_mapping_set_explicit_default_and_clipboard_with_skips_and_undo() {
    let h = fixture();
    let c = colours(&h);
    let (la, _, za) = painted(&h, 300.0, &c[1]);
    let (lb, sb, zb) = painted(&h, 500.0, &c[1]);
    let (_, _, solid) = painted(&h, 700.0, &c[0]);
    let start = rev(&h);
    let r = ok(
        &h,
        "colour.fillMapping.set",
        json!({ "drawingId": D1, "objectIds": [za, solid], "mapping": {"matrix": [50, 0, 0, 30, 320, 310]} }),
    );
    assert_eq!(rev(&h), start + 1);
    assert_eq!(r["changed"], json!([za]));
    assert_eq!(r["skipped"], json!([{ "objectId": solid, "reason": "solid" }]));
    assert_eq!(layer(&h, &la)["zones"][0]["textureMatrix"], json!([50, 0, 0, 30, 320, 310]));
    // Copy A's mapping and paste it on B (zone) and on B's outline (a pencil line painted with a gradient too).
    ok(&h, "colour.fillMapping.get", json!({ "drawingId": D1, "objectId": za, "toClipboard": true }));
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [sb], "colourId": c[1] }));
    let r = ok(&h, "colour.fillMapping.set", json!({ "drawingId": D1, "objectIds": [zb, sb], "mapping": "clipboard" }));
    assert_eq!(r["changed"].as_array().unwrap().len(), 2, "{r}");
    assert_eq!(layer(&h, &lb)["zones"][0]["textureMatrix"], json!([50, 0, 0, 30, 320, 310]));
    assert_eq!(
        ok(&h, "colour.fillMapping.get", json!({ "drawingId": D1, "objectId": sb }))["matrix"],
        json!([50.0, 0.0, 0.0, 30.0, 320.0, 310.0])
    );
    // Default refits to the bounds.
    ok(&h, "colour.fillMapping.set", json!({ "drawingId": D1, "objectIds": [za], "mapping": "default" }));
    assert_eq!(layer(&h, &la)["zones"][0]["textureMatrix"], json!([100, 0, 0, 60, 300, 300]));
    // Handles of another kind are skipped; malformed mappings refused.
    let r = ok(
        &h,
        "colour.fillMapping.set",
        json!({ "drawingId": D1, "objectIds": [za], "mapping": {"radial": {"centre": {"x": 0, "y": 0}, "radius": 5}} }),
    );
    assert_eq!(r["skipped"][0]["reason"], "kindMismatch");
    let e = err(
        &h,
        "colour.fillMapping.set",
        json!({ "drawingId": D1, "objectIds": [za], "mapping": {"matrix": [1, 0, 0, 1, 0, 0], "linear": {"start": {"x": 0, "y": 0}, "end": {"x": 1, "y": 0}}} }),
    );
    assert_eq!(e.details["pointer"], "/mapping");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(layer(&h, &la)["zones"][0]["textureMatrix"], json!([50, 0, 0, 30, 320, 310]));
    let h2 = fixture();
    assert_eq!(
        err(&h2, "colour.fillMapping.set", json!({ "drawingId": D1, "objectIds": [za], "mapping": "clipboard" })).code,
        "colour.clipboardEmpty"
    );
}

#[test]
fn set_colour_recolours_lines_brushes_text_and_painted_zones_inside_the_selection() {
    let h = fixture();
    let c = colours(&h);
    // Sky: none of the targets uses it yet (the caption is ink, the rectangle skin).
    let ink = c[1].clone();
    let (lid, stroke, zone) = painted(&h, 300.0, &c[0]);
    // An unpainted closed shape in the selection stays unpainted.
    let open = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 600, "y": 300}, "to": {"x": 650, "y": 360}, "layer": "new" }),
    );
    let start = rev(&h);
    let r = ok(
        &h,
        "colour.selection.apply",
        json!({ "drawingId": D1, "colourId": ink, "objectIds": [stroke, open["strokeIds"][0], CAPTION] }),
    );
    assert_eq!(rev(&h), start + 1);
    let changed: Vec<String> =
        r["changed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    assert!(changed.contains(&stroke) && changed.contains(&CAPTION.to_owned()) && changed.contains(&zone), "{r}");
    assert_eq!(node(&h, &stroke)["colourId"], json!(ink));
    assert_eq!(layer(&h, &lid)["zones"][0]["colourId"], json!(ink));
    let open_zone = layer(&h, open["layerId"].as_str().unwrap())["zones"][0].clone();
    assert!(open_zone["colourId"].is_null(), "{open_zone}");
    let cap = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }))["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == CAPTION)
        .cloned()
        .unwrap();
    assert!(cap.to_string().contains(&ink), "{cap}");
    // The current selection is the default.
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [stroke] }));
    ok(&h, "colour.selection.apply", json!({ "drawingId": D1, "colourId": c[0] }));
    assert_eq!(node(&h, &stroke)["colourId"], json!(c[0]));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(node(&h, &stroke)["colourId"], json!(ink));
}

#[test]
fn replace_colour_by_scope_with_locks_pens_counts_and_the_matrix_rule() {
    let h = fixture();
    let c = colours(&h);
    let (sky, glow, skin) = (c[1].clone(), c[2].clone(), c[0].clone());
    let (la, _, za) = painted(&h, 300.0, &sky);
    ok(
        &h,
        "colour.fillMapping.set",
        json!({ "drawingId": D1, "objectIds": [za], "mapping": {"matrix": [10, 0, 0, 10, 300, 300]} }),
    );
    let (lb, _, _) = painted(&h, 500.0, &skin);
    // Gradient to gradient keeps the mapping; solid to gradient computes the default.
    let start = rev(&h);
    let r = ok(
        &h,
        "colour.recolour.apply",
        json!({ "map": [{"from": sky, "to": glow}, {"from": skin, "to": sky}], "scope": "drawing", "drawingIds": [D1], "targets": ["zones"] }),
    );
    assert_eq!(rev(&h), start + 1);
    let n = r["changed"][0]["count"].as_u64().unwrap();
    assert!(n >= 2, "{r}");
    let za_now = layer(&h, &la)["zones"][0].clone();
    assert_eq!(
        (za_now["colourId"].clone(), za_now["textureMatrix"].clone()),
        (json!(glow), json!([10, 0, 0, 10, 300, 300]))
    );
    let zb_now = layer(&h, &lb)["zones"][0].clone();
    assert_eq!(
        (zb_now["colourId"].clone(), zb_now["textureMatrix"].clone()),
        (json!(sky), json!([100, 0, 0, 60, 500, 300]))
    );
    ok(&h, "edit.undo", json!({}));
    assert_eq!(layer(&h, &la)["zones"][0]["colourId"], json!(sky));
    // A locked drawing is skipped and reported; pens follow when asked.
    ok(&h, "draw.lock.set", json!({ "drawingId": D1, "locked": true }));
    let r = ok(&h, "colour.recolour.apply", json!({ "fromColourId": skin, "toColourId": c[4], "includePens": true }));
    assert!(r["skippedLocked"].as_array().unwrap().contains(&json!(D1)), "{r}");
    assert!(r["changed"].as_array().unwrap().iter().all(|d| d["drawingId"] != json!(D1)));
    let pens_using = h
        .snapshot()
        .unwrap()
        .unwrap()
        .project()
        .pens()
        .iter()
        .filter(|p| p.colour_id().as_ref().map(|x| x.as_str()) == Some(skin.as_str()))
        .count();
    assert_eq!(pens_using, 0, "pens recoloured: {r}");
    let e = err(&h, "colour.recolour.apply", json!({ "fromColourId": skin }));
    assert_eq!(e.code, "arg.invalid");
}
