//! M2 C2.3 round 2: explicit `createDrawing` on the shape commands (C23-R2-02) and zone selections that survive
//! command completion (C23-R2-03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

fn shape_params(cmd: &str, create: bool) -> Value {
    let mut p = if cmd == "draw.polyline.add" {
        json!({ "points": [{"x": 2000, "y": 2000}, {"x": 2100, "y": 2060}], "snap": true })
    } else {
        json!({ "from": {"x": 2000, "y": 2000}, "to": {"x": 2100, "y": 2060} })
    };
    p["elementId"] = json!(BOB);
    p["frame"] = json!(1);
    p["createDrawing"] = json!(create);
    p
}

/// The element's bank, exposure and the drawing count: what creating a drawing changes.
fn cell_state(h: &ExecutorHandle) -> Value {
    let doc = h.snapshot().unwrap().unwrap();
    let e = doc.elements().find(|e| e.name() == "Bob").unwrap();
    let bank = e.drawing_bank_id().clone().unwrap();
    json!({
        "exposure": serde_json::to_value(e.exposure()).unwrap(),
        "bank": doc.bank(&bank).unwrap().drawings().ids().iter().map(|i| i.as_str().to_owned()).collect::<Vec<_>>(),
        "drawings": doc.drawings().count(),
    })
}

#[test]
fn explicit_create_drawing_false_refuses_and_true_creates_on_every_shape() {
    for cmd in ["draw.rectangle.add", "draw.ellipse.add", "draw.line.add", "draw.polyline.add"] {
        let h = fixture();
        ok(
            &h,
            "doc.object.patch",
            json!({ "targets": [BOB], "ops": [{"op": "replace", "path": "/exposure", "value": [[null, 24]]}] }),
        );
        let (v, state) = (rev(&h), cell_state(&h));
        // False: refused; no drawing, bank, exposure or revision change.
        let e = run(&h, cmd, shape_params(cmd, false));
        assert!(!e.is_ok(), "{cmd}: {e:?}");
        assert_eq!((rev(&h), cell_state(&h)), (v, state.clone()), "{cmd}: refused without a trace");
        // Default (true): a dry run creates nothing; a failed batch rolls the creation back.
        let mut p = shape_params(cmd, true);
        p.as_object_mut().unwrap().remove("createDrawing");
        let dry = with(&h, cmd, p.clone(), InvokeOptions { dry_run: true, ..Default::default() });
        assert!(dry.is_ok(), "{cmd}: {dry:?}");
        assert_eq!((rev(&h), cell_state(&h)), (v, state.clone()));
        let failed = batch(
            &h,
            json!({ "label": "r2", "steps": [{"id": cmd, "params": p.clone()},
                {"id": "draw.transform.apply", "params": {"drawingId": D1, "ids": ["st_missing"], "translate": {"x": 1, "y": 0}}}] }),
        );
        assert!(!failed.is_ok());
        assert_eq!((rev(&h), cell_state(&h)), (v, state.clone()), "{cmd}: rolled back");
        // True: one revision creates the drawing; undo removes it again.
        let r = ok(&h, cmd, shape_params(cmd, true));
        assert!(r["drawingId"].as_str().is_some());
        assert_eq!(rev(&h), v + 1);
        assert_ne!(cell_state(&h), state);
        ok(&h, "edit.undo", json!({}));
        assert_eq!(cell_state(&h), state, "{cmd}: undo removes the created drawing");
    }
}

/// A painted (or unpainted) rectangle on a new layer of a drawing: its zone ID.
fn zone(h: &ExecutorHandle, drawing: &str, x: f64, colour: Option<&str>) -> String {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": drawing, "from": {"x": x, "y": 1000}, "to": {"x": x + 100.0, "y": 1060}, "layer": "new" }),
    );
    if let Some(c) = colour {
        ok(h, "colour.paint.apply", json!({ "drawingId": drawing, "objectIds": [r["zoneIds"][0]], "colourId": c }));
        let d = ok(h, "draw.drawing.get", json!({ "drawingId": drawing }));
        let l = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == r["layerId"]).unwrap().clone();
        return l["zones"][0]["id"].as_str().unwrap().to_owned();
    }
    r["zoneIds"][0].as_str().unwrap().to_owned()
}

fn selected(h: &ExecutorHandle, drawing: &str) -> Value {
    ok(h, "draw.selection.get", json!({ "drawingId": drawing }))["selectedIds"].clone()
}

#[test]
fn selected_zones_survive_command_completion_and_unrelated_edits() {
    let h = fixture();
    let c = colours(&h);
    let painted = zone(&h, D1, 1000.0, Some(&c[0]));
    let unpainted = zone(&h, D1, 1200.0, None);
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [painted, unpainted] }));
    assert_eq!(selected(&h, D1), json!([painted, unpainted]));
    // An unrelated edit elsewhere in the drawing keeps both.
    ok(
        &h,
        "draw.line.add",
        json!({ "drawingId": D1, "from": {"x": 3000, "y": 3000}, "to": {"x": 3100, "y": 3000}, "layer": "new" }),
    );
    assert_eq!(selected(&h, D1), json!([painted, unpainted]));
    // Undo and redo of that edit keep them too.
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(selected(&h, D1), json!([painted, unpainted]));
    // Set Colour on the default selection recolours the painted zone and leaves the unpainted one unpainted.
    let r = ok(&h, "colour.selection.apply", json!({ "drawingId": D1, "colourId": c[4] }));
    assert!(r["changed"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().starts_with("zn_")), "{r}");
    // Centre Line on a fill-only default selection gives the typed reason.
    let painted2 = zone(&h, D1, 1400.0, Some(&c[0]));
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [painted2] }));
    assert_eq!(err(&h, "draw.centreLine.apply", json!({ "drawingId": D1 })).code, "drawing.centreLineOnFill");
    // A topology change of the zone's layer re-identifies the zone: the stale ID is pruned.
    let before = selected(&h, D1);
    assert_eq!(before, json!([painted2]));
    ok(
        &h,
        "draw.cut.apply",
        json!({ "drawingId": D1, "segment": {"from": {"x": 1450, "y": 980}, "to": {"x": 1450, "y": 1080}} }),
    );
    let after = selected(&h, D1);
    assert!(!after.as_array().unwrap().contains(&json!(painted2)), "{after}");
}

#[test]
fn zone_selections_keep_their_drawing_identity() {
    let h = fixture();
    let c = colours(&h);
    let doc = h.snapshot().unwrap().unwrap();
    let other = doc
        .drawings()
        .map(|d| d.id().as_str().to_owned())
        .find(|d| d != D1 && !*doc.drawing(&tt_model::Id::parse(d).unwrap()).unwrap().locked())
        .unwrap();
    drop(doc);
    let z1 = zone(&h, D1, 1000.0, Some(&c[0]));
    let z2 = zone(&h, &other, 1000.0, Some(&c[0]));
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [z1] }));
    ok(&h, "draw.selection.set", json!({ "drawingId": other, "ids": [z2] }));
    assert_eq!(selected(&h, D1), json!([z1]));
    assert_eq!(selected(&h, &other), json!([z2]));
    // The same zone ID in the other drawing is not this drawing's selection (compound identity).
    let keys = ok(&h, "edit.selection.get", json!({ "context": "drawing" }))["ids"].clone();
    assert!(keys.as_array().unwrap().contains(&json!(format!("{D1}/{z1}"))), "{keys}");
    assert!(keys.as_array().unwrap().contains(&json!(format!("{other}/{z2}"))), "{keys}");
}
