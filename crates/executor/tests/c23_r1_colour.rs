//! Independent colour mapping and protection variants.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
#[test]
fn a_bulk_map_with_only_protected_effective_targets_refuses() {
    let h = fixture();
    let doc = h.snapshot().unwrap().unwrap();
    let c: Vec<_> = doc.palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string())).collect();
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[c[4]],"ops":[{"op":"add","path":"/ext","value":{"dev.toontown.paint":{"protected":true}}}]}),
    );
    let start = rev(&h);
    // The caption uses protected Ink; no text run uses Sky. The second pair cannot change anything in this scope.
    let e = run(
        &h,
        "colour.recolour.apply",
        json!({"map":[{"from":c[4],"to":c[0]},{"from":c[1],"to":c[0]}],"scope":"drawing","drawingIds":[D],"targets":["text"]}),
    );
    println!("protected-only effective map: {e:?}");
    assert_eq!(rev(&h), start);
    assert_eq!(
        e.error.map(|e| e.code),
        Some("colour.protected".into()),
        "SI-061 requires refusal whenever nothing else changes"
    );
}
#[test]
fn linear_radial_and_texture_forms_map_strokes_with_kind_skips() {
    let h = fixture();
    let doc = h.snapshot().unwrap().unwrap();
    let c: Vec<_> = doc.palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string())).collect();
    for (index, form, want) in [
        (
            1,
            json!({"linear":{"start":{"x":1000,"y":1000},"end":{"x":1020,"y":1040}}}),
            json!([20.0, 40.0, -40.0, 20.0, 1000.0, 1000.0]),
        ),
        (2, json!({"radial":{"centre":{"x":1030,"y":1010},"radius":20}}), json!([40.0, 0.0, 0.0, 40.0, 1010.0, 990.0])),
        (
            3,
            json!({"texture":{"origin":{"x":1000,"y":1040},"xAxis":{"x":20,"y":0},"yAxis":{"x":0,"y":-40}}}),
            json!([20.0, 0.0, 0.0, -40.0, 1000.0, 1040.0]),
        ),
    ] {
        let r = ok(
            &h,
            "draw.line.add",
            json!({"drawingId":D,"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1060},"layer":"new","colourId":c[index]}),
        );
        let id = &r["strokeIds"][0];
        let changed = ok(&h, "colour.fillMapping.set", json!({"drawingId":D,"objectIds":[id],"mapping":form}));
        assert_eq!(changed["changed"], json!([id]));
        let got = ok(&h, "colour.fillMapping.get", json!({"drawingId":D,"objectId":id,"toClipboard":true}));
        assert_eq!(got["matrix"], want);
        ok(&h, "colour.fillMapping.set", json!({"drawingId":D,"objectIds":[id],"mapping":"default"}));
        ok(&h, "colour.fillMapping.set", json!({"drawingId":D,"objectIds":[id],"mapping":"clipboard"}));
        assert_eq!(ok(&h, "colour.fillMapping.get", json!({"drawingId":D,"objectId":id}))["matrix"], want);
    }
}
