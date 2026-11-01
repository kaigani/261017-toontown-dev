//! Independent all-policy, mapped-texture flip and junction-cut controls.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
fn rect(h: &ExecutorHandle, x: f64, y: f64, c: &str) -> Value {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({"drawingId":D,"from":{"x":x,"y":y},"to":{"x":x+100.0,"y":y+60.0},"layer":"new","thickness":0}),
    );
    ok(h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[r["zoneIds"][0]],"colourId":c}));
    r
}
#[test]
fn every_flatten_policy_combination_and_junction_cut_keeps_winning_paint() {
    for precedence in ["upper", "lower"] {
        for position in ["top", "bottom"] {
            let h = fixture();
            let c: Vec<_> = h
                .snapshot()
                .unwrap()
                .unwrap()
                .palettes()
                .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string()))
                .collect();
            let a = rect(&h, 1000.0, 1000.0, &c[0]);
            let b = rect(&h, 1050.0, 1020.0, &c[1]);
            let r = ok(
                &h,
                "draw.flatten.apply",
                json!({"drawingId":D,"ids":[a["layerId"],b["layerId"]],"policy":{"precedence":precedence,"position":position}}),
            );
            assert_eq!(r["layerId"], if position == "top" { b["layerId"].clone() } else { a["layerId"].clone() });
            let hit = ok(&h, "draw.hitTest.get", json!({"drawingId":D,"point":{"x":1075,"y":1040}}));
            let zone = hit["hits"].as_array().unwrap().iter().find(|h| h["objectKind"] == "zone").unwrap();
            assert_eq!(zone["colourId"], if precedence == "upper" { c[1].clone() } else { c[0].clone() });
        }
    }
}
#[test]
fn a_cut_through_an_existing_junction_has_no_zero_length_piece_and_is_undoable() {
    let h = fixture();
    let c = "col_01K742SG059XGSNZ6M9MACZ2ZY";
    let r = rect(&h, 1000.0, 1000.0, c);
    ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"invisible","fit":false,"layer":r["layerId"],"points":[{"x":1050,"y":990},{"x":1050,"y":1070}]}),
    );
    let before = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
    ok(&h, "draw.cut.apply", json!({"drawingId":D,"segment":{"from":{"x":1040,"y":990},"to":{"x":1120,"y":1070}}}));
    let after = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
    let l = after["nodes"].as_array().unwrap().iter().find(|n| n["id"] == r["layerId"]).unwrap();
    for s in l["strokes"].as_array().unwrap() {
        let pts = s["bezier"].as_array().unwrap();
        assert!(pts.iter().any(|p| p != &pts[0]), "zero-length piece: {s}");
    }
    assert!(l["zones"].as_array().unwrap().iter().all(|z| z["colourId"] == c));
    ok(&h, "edit.undo", json!({}));
    let mut restored = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
    restored["rev"] = before["rev"].clone();
    assert_eq!(restored, before);
}
#[test]
fn asymmetric_texture_pixels_follow_a_horizontal_flip() {
    let h = fixture();
    let colour = "col_01K9ZZZZZZZZZZZZZZZZZZZZZ4";
    let style = "sty_01K9ZZZZZZZZZZZZZZZZZZZZZ3";
    let px = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 0, 255, 255, 255, 255, 0, 255, 255];
    let png = tt_render::encode_png(&px, 3, 2).unwrap();
    let palette = json!({"id":"pal_01K9ZZZZZZZZZZZZZZZZZZZZZ1","type":"palette","rev":1,"name":"Review axis","hidden":false,
        "styles":[{"id":style,"type":"paletteStyle","rev":1,"name":"Review"}],"swatches":[{"colourId":colour,"type":"swatch","rev":1,"name":"Review","kind":"texture","deformable":true,"values":{style:{"image":"textures/review.png","tile":false,"width":3,"height":2}}}]});
    ok(
        &h,
        "doc.import.json",
        json!({"source":{"format":"animjson","formatVersion":"1.0","scenes":[],"elements":[],"drawingBanks":[],"palettes":[palette],"pens":[],"assets":[],"blobs":{"textures/review.png":{"mediaType":"image/png","base64":tt_persist::b64::encode(&png)}}},"idPolicy":"preserve","onConflict":"fail"}),
    );
    let r = rect(&h, 1000.0, 1000.0, colour);
    let render = || {
        let r = ok(
            &h,
            "draw.drawing.render",
            json!({"drawingId":D,"width":100,"height":60,"region":{"x":1000,"y":1000,"width":100,"height":60}}),
        );
        let png = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
        tt_render::decode_png(&png, 100).unwrap().2
    };
    let before = render();
    ok(&h, "draw.flip.apply", json!({"drawingId":D,"ids":[r["layerId"]],"axis":"horizontal"}));
    let after = render();
    for (y, x) in [(15, 16), (15, 50), (45, 83)] {
        let a = (y * 100 + x) * 4;
        let b = (y * 100 + 99 - x) * 4;
        assert_eq!(&before[a..a + 4], &after[b..b + 4]);
    }
}
