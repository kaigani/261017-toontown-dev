//! Independent C2.3 round-one adversarial cases. Product code is unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{json, Value};
use tt_executor::ExecutorHandle;

const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";
fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot().unwrap().unwrap().palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string())).collect()
}
fn drawing(h: &ExecutorHandle) -> Value { ok(h, "draw.drawing.get", json!({"drawingId":D})) }
fn layer(h: &ExecutorHandle, id: &Value) -> Value {
    drawing(h)["nodes"].as_array().unwrap().iter().find(|n| n["id"] == *id).unwrap().clone()
}
fn rect(h: &ExecutorHandle, colour: &str) -> Value {
    let r = ok(h,"draw.rectangle.add",json!({"drawingId":D,"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1060},"thickness":0,"layer":"new"}));
    ok(h,"colour.paint.apply",json!({"drawingId":D,"objectIds":[r["zoneIds"][0]],"colourId":colour})); r
}
fn pixel(h: &ExecutorHandle) -> Vec<u8> {
    let r = ok(h,"draw.drawing.render",json!({"drawingId":D,"width":160,"height":60,"region":{"x":980,"y":980,"width":160,"height":60}}));
    let bytes = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
    tt_render::decode_png(&bytes,8192).unwrap().2
}

#[test]
fn point_subset_transform_moves_every_incident_anchor() {
    let h=fixture();
    let a=ok(&h,"draw.line.add",json!({"drawingId":D,"from":{"x":1000,"y":1000},"to":{"x":1050,"y":1000},"layer":"new"}));
    let b=ok(&h,"draw.line.add",json!({"drawingId":D,"from":{"x":1050,"y":1000},"to":{"x":1050,"y":1050},"layer":a["layerId"]}));
    ok(&h,"draw.transform.apply",json!({"drawingId":D,"points":[{"strokeId":a["strokeIds"][0],"anchor":1}],"translate":{"x":10,"y":10}}));
    let l=layer(&h,&a["layerId"]);
    let s=l["strokes"].as_array().unwrap().iter().find(|s| s["id"]==b["strokeIds"][0]).unwrap();
    println!("incident second stroke after transform: {s}");
    assert_eq!(s["bezier"][0],json!({"x":1060.0,"y":1010.0}),"the shared anchor must move on both strokes");
}

#[test]
fn envelope_translation_keeps_zone_colour_and_maps_its_matrix() {
    let h=fixture(); let c=colours(&h); let r=rect(&h,&c[1]);
    ok(&h,"draw.envelope.apply",json!({"drawingId":D,"ids":[r["layerId"]],"corners":[{"x":1300,"y":1000},{"x":1400,"y":1000},{"x":1400,"y":1060},{"x":1300,"y":1060}]}));
    let l=layer(&h,&r["layerId"]); println!("envelope zones: {}",l["zones"]);
    assert_eq!(l["zones"][0]["colourId"],c[1]);
    assert_eq!(l["zones"][0]["textureMatrix"],json!([100,0,0,60,1300,1000]));
}

#[test]
fn reposition_all_maps_zone_matrices_with_geometry() {
    let h=fixture(); let c=colours(&h); let r=rect(&h,&c[1]);
    ok(&h,"draw.repositionAll.apply",json!({"elementId":BOB,"translate":{"x":300,"y":0}}));
    let l=layer(&h,&r["layerId"]); println!("reposition zones: {}",l["zones"]);
    assert_eq!(l["zones"][0]["colourId"],c[1]);
    assert_eq!(l["zones"][0]["textureMatrix"],json!([100,0,0,60,1300,1000]));
}

#[test]
fn optimize_preserves_opaque_brush_over_thick_lower_pencil() {
    let h=fixture(); let c=colours(&h);
    let a=ok(&h,"draw.line.add",json!({"drawingId":D,"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1000},"thickness":20,"colourId":c[0],"layer":"new"}));
    let b=ok(&h,"draw.stroke.add",json!({"drawingId":D,"kind":"brush","fit":false,"points":[{"x":1010,"y":1008},{"x":1090,"y":1008}],"colourId":c[4],"pen":{"minSize":10,"maxSize":10,"tipShape":"round","tipAngle":0},"layer":"new"}));
    let before=pixel(&h);
    let r=ok(&h,"draw.optimize.apply",json!({"drawingId":D,"ids":[a["layerId"],b["layerId"]]}));
    let after=pixel(&h); let changed=before.iter().zip(&after).filter(|(a,b)|a!=b).count();
    println!("optimize {r}; changed channels {changed}");
    assert_eq!(changed,0,"Optimize must preserve composited appearance");
}

#[test]
fn lines_to_brush_accepts_a_gradient_pencil_and_keeps_brush_mapping() {
    let h=fixture(); let c=colours(&h);
    let a=ok(&h,"draw.line.add",json!({"drawingId":D,"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1000},"colourId":c[1],"layer":"new"}));
    let sid=&a["strokeIds"][0];
    ok(&h,"colour.fillMapping.set",json!({"drawingId":D,"objectIds":[sid],"mapping":{"matrix":[80,0,0,10,1010,990]}}));
    let e=run(&h,"draw.linesToBrush.apply",json!({"drawingId":D,"ids":[sid]}));
    println!("gradient conversion: {e:?}"); assert!(e.is_ok(),"gradient pencils must convert");
    let r=e.result.unwrap();
    let m=ok(&h,"colour.fillMapping.get",json!({"drawingId":D,"objectId":r["strokeIds"][0]}));
    assert_eq!(m["matrix"],json!([80.0,0.0,0.0,10.0,1010.0,990.0]));
}

#[test]
fn centre_line_fill_only_selection_reports_the_typed_reason() {
    let h=fixture(); let c=colours(&h); let r=rect(&h,&c[0]);
    let e=run(&h,"draw.centreLine.apply",json!({"drawingId":D,"ids":[r["zoneIds"][0]]}));
    println!("fill-only centre line: {e:?}");
    assert!(e.error.as_ref().is_some_and(|e| e.code=="drawing.centreLineOnFill") || e.result.as_ref().is_some_and(|r|r["warnings"].as_array().is_some_and(|w|w.contains(&json!("drawing.centreLineOnFill")))));
}
