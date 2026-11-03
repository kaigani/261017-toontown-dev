//! Final independent import, recovery and mapping variants. No product mutations outside commands.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
fn drawing(h: &ExecutorHandle, id: &str) -> Value {
    h.snapshot()
        .unwrap()
        .unwrap()
        .entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(id).unwrap()))
        .unwrap()
}
fn box_zone(h: &ExecutorHandle) -> Value {
    ok(
        h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"invisible","layer":"new","fit":false,"closed":true,"points":[{"x":1000,"y":1000},{"x":1100,"y":1000},{"x":1100,"y":1050},{"x":1000,"y":1050}]}),
    )
}
fn layer(h: &ExecutorHandle, id: &Value) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":D}))["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == *id)
        .unwrap()
        .clone()
}

#[test]
fn schema_children_stamp_but_model_shaped_opaque_values_survive() {
    for policy in ["preserve", "new"] {
        let src = fixture();
        let mut fake = drawing(&src, D)["nodes"][0]["strokes"][1].clone();
        fake["rev"] = json!(5);
        let ext = json!({"x.y":fake});
        let meta = json!({"nested":{"type":"Palette","rev":9}});
        ok(&src, "doc.object.patch", json!({"targets":[D],"props":{"ext":ext,"meta":meta}}));
        let expected = drawing(&src, D);
        assert_eq!(expected["ext"]["x.y"]["rev"], 5);
        assert_eq!(expected["meta"]["nested"]["rev"], 9);
        let bundle = ok(&src, "doc.export.json", json!({"scope":"project","binaries":"embed"}))["bundle"].clone();
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        ok(&h, "project.create", json!({"name":"R4"}));
        let result = ok(&h, "doc.import.json", json!({"source":bundle,"idPolicy":policy}));
        let id = result["created"]["drawings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .find(|id| drawing(&h, id)["ext"]["x.y"].is_object())
            .unwrap();
        let got = drawing(&h, id);
        assert_eq!(got["ext"], expected["ext"], "{policy}");
        assert_eq!(got["meta"], expected["meta"], "{policy}");
        let stamp = json!(rev(&h));
        assert_eq!(got["rev"], stamp);
        assert_eq!(got["nodes"][0]["rev"], stamp);
        assert_eq!(got["nodes"][0]["strokes"][1]["rev"], stamp, "inline stroke {policy}");
    }
}
#[test]
fn three_reverse_order_styles_share_pick_and_native_click_selection() {
    for texture in [false, true] {
        let mut p = tt_model::fixture::sample_parts(1).unwrap();
        let mut pal = serde_json::to_value(&p.palettes[0]).unwrap();
        let c = pal["swatches"][0]["colourId"].as_str().unwrap().to_owned();
        let mut style = pal["styles"][0].clone();
        let mut styles = Vec::new();
        let mut values = serde_json::Map::new();
        let mut ids =
            pal["styles"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        ids.push("sty_r4_z".into());
        ids.sort_by(|a, b| b.cmp(a));
        for (i, id) in ids.iter().enumerate() {
            style["id"] = json!(id);
            style["name"] = json!(format!("R4 {i}"));
            styles.push(style.clone());
            values.insert(
                id.clone(),
                if texture {
                    json!({"image":"textures/r4.png","tile":true,"width":17+i*10,"height":9+i*7})
                } else {
                    json!({"rgba":(["#123456FF","#ABCDEF88","#FF0000FF"][i])})
                },
            );
        }
        pal["styles"] = json!(styles);
        // Every swatch must have values for the new style IDs.
        for sw in pal["swatches"].as_array_mut().unwrap() {
            let v = sw["values"].as_object().unwrap().values().next().unwrap().clone();
            sw["values"] =
                json!(ids.iter().map(|id| (id.clone(), v.clone())).collect::<serde_json::Map<String, Value>>());
        }
        pal["swatches"][0]["kind"] = json!(if texture { "texture" } else { "solid" });
        pal["swatches"][0]["values"] = json!(values);
        p.palettes[0] = tt_model::object::from_json(&pal).unwrap();
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        assert!(h.open_document(tt_model::document::Document::from_parts(p).unwrap(), None, false).is_ok());
        let zone = box_zone(&h);
        ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[zone["zoneIds"][0]],"colourId":c}));
        let pick = ok(&h, "colour.pick", json!({"drawingId":D,"space":"drawing","point":{"x":1050,"y":1025}}));
        assert_eq!(pick["colourId"], c);
        assert_eq!(pick["rgba"], if texture { "#00000000" } else { "#123456FF" });
        if texture {
            let picked: [f64; 6] = serde_json::from_value(pick["matrix"].clone()).unwrap();
            let stored: [f64; 6] =
                serde_json::from_value(layer(&h, &zone["layerId"])["zones"][0]["textureMatrix"].clone()).unwrap();
            assert_eq!(picked, [17., 0., 0., -9., 1000., 1050.]);
            assert_eq!(stored, picked);
        }
    }
}
