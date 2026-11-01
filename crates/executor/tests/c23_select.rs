//! M2 C2.3: drawing selection and point selection, bounds, the headless view mapping, the inspector preview, rename and
//! lock, and the shape tools (02 §7.1, §7.2.3–§7.2.6, §9, §13.5).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const BRUSH: &str = "st_01K742SG0QD1F2HFZSX8R4V7WP";

fn ids(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_owned()).collect()
}

#[test]
fn selection_by_ids_point_rect_lasso_all_and_modes_with_events() {
    let h = fixture();
    let rx = h.subscribe();
    let r = ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [PENCIL] }));
    assert_eq!(ids(&r["selectedIds"]), vec![PENCIL]);
    assert!(r["bounds"].is_object());
    let r = ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [BRUSH], "mode": "add" }));
    assert_eq!(ids(&r["selectedIds"]), vec![PENCIL, BRUSH]);
    let r = ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [PENCIL], "mode": "toggle" }));
    assert_eq!(ids(&r["selectedIds"]), vec![BRUSH]);
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "draw.selection.changed").collect();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2].payload["selectedIds"], json!([BRUSH]));
    // The same selection is the `drawing` context of edit.selection.get.
    let e = ok(&h, "edit.selection.get", json!({ "context": "drawing" }));
    assert_eq!(e["ids"], json!([format!("{D1}/{BRUSH}")]));
    // By point: the front-most object.
    let r = ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "point", "point": {"x": 50, "y": 50} }));
    assert_eq!(r["selectedIds"].as_array().unwrap().len(), 1);
    // By rect, inside and intersecting.
    let inside = ok(
        &h,
        "draw.selection.set",
        json!({ "drawingId": D1, "by": "rect", "rect": {"x": -1000, "y": -1000, "width": 2000, "height": 2000} }),
    );
    assert!(ids(&inside["selectedIds"]).contains(&PENCIL.to_owned()));
    let tiny = json!({ "drawingId": D1, "by": "rect", "rect": {"x": -1, "y": -1, "width": 2, "height": 2} });
    assert!(ok(&h, "draw.selection.set", tiny.clone())["selectedIds"].as_array().unwrap().is_empty());
    let mut meet = tiny;
    meet["rectRule"] = json!("intersect");
    assert!(!ok(&h, "draw.selection.set", meet)["selectedIds"].as_array().unwrap().is_empty());
    // By lasso (a large triangle) and all; none clears.
    let lasso = ok(
        &h,
        "draw.selection.set",
        json!({ "drawingId": D1, "by": "lasso", "polygon": [{"x": -5000, "y": -5000}, {"x": 5000, "y": -5000}, {"x": 0, "y": 5000}] }),
    );
    assert!(ids(&lasso["selectedIds"]).contains(&BRUSH.to_owned()));
    let all = ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "all" }));
    assert!(all["selectedIds"].as_array().unwrap().len() >= 3);
    let none = ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "none" }));
    assert!(none["selectedIds"].as_array().unwrap().is_empty() && none["bounds"].is_null());
    // Unknown IDs and missing inputs are refused with their pointer.
    let e = err(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": ["st_nope"] }));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("drawing.objectNotFound", Some("/ids/0")));
    let e = err(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "rect" }));
    assert_eq!(e.details["pointer"], "/rect");
    // Selection commands are not undoable edits: the document revision does not move.
    let before = rev(&h);
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [PENCIL] }));
    assert_eq!(rev(&h), before);
}

#[test]
fn point_selection_by_anchor_and_rect_is_read_back_and_pruned_with_the_stroke() {
    let h = fixture();
    let r =
        ok(&h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": PENCIL, "anchor": 0}] }));
    assert_eq!(r["selectedPoints"], json!([{"strokeId": PENCIL, "anchor": 0}]));
    let r = ok(
        &h,
        "draw.pointSelection.set",
        json!({ "drawingId": D1, "rect": {"x": 49, "y": 49, "width": 2, "height": 2}, "mode": "add" }),
    );
    assert_eq!(r["selectedPoints"].as_array().unwrap().len(), 2, "{r}");
    let g = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }));
    assert_eq!(g["selectedPoints"], r["selectedPoints"]);
    let e =
        err(&h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": PENCIL, "anchor": 99}] }));
    assert_eq!(e.details["pointer"], "/points/0");
    // Deleting the stroke drops its anchors from the selection.
    ok(&h, "draw.object.delete", json!({ "drawingId": D1, "ids": [PENCIL] }));
    let g = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }));
    assert!(g["selectedPoints"].as_array().unwrap().is_empty(), "{g}");
}

#[test]
fn bounds_view_mapping_and_inspector_preview() {
    let h = fixture();
    let all = ok(&h, "draw.bounds.get", json!({ "drawingId": D1 }));
    let pencil = ok(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": [PENCIL] }));
    // Visible bounds include half the line thickness (1.5 du).
    assert_eq!(pencil["bounds"], json!({"x": -50.75, "y": -50.75, "width": 101.5, "height": 101.5}));
    assert!(all["bounds"]["width"].as_f64().unwrap() >= 100.0);
    let e = err(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": ["st_nope"] }));
    assert_eq!(e.details["pointer"], "/ids/0");
    let m = ok(&h, "draw.viewMapping.get", json!({}));
    assert_eq!(m["zoom"], 1.0);
    let (a, b) = (&m["drawingToView"], &m["viewToDrawing"]);
    // The two maps are inverse: the drawing origin is the view centre.
    assert_eq!((a[4].as_f64().unwrap(), a[5].as_f64().unwrap()), (960.0, 540.0));
    assert_eq!((b[4].as_f64().unwrap(), b[5].as_f64().unwrap()), (-960.0, 540.0));
    assert_eq!(err(&h, "draw.viewMapping.get", json!({ "view": "camera" })).code, "view.notFound");
    assert_eq!(ok(&h, "draw.inspectorPreview.set", json!({ "visible": true }))["visible"], true);
}

#[test]
fn rename_and_lock_validate_and_undo() {
    let h = fixture();
    let r = ok(&h, "draw.drawing.rename", json!({ "drawingId": D1, "name": "key-1" }));
    assert_eq!(r["name"], "key-1");
    assert_eq!(err(&h, "draw.drawing.rename", json!({ "drawingId": D1, "name": "a/b" })).code, "drawing.nameInvalid");
    assert_eq!(err(&h, "draw.drawing.rename", json!({ "drawingId": D1, "name": "x.y" })).code, "drawing.nameInvalid");
    let doc = h.snapshot().unwrap().unwrap();
    let bank = doc.bank_of(&tt_model::Id::parse(D1).unwrap()).unwrap().clone();
    let other = doc.bank(&bank).unwrap().drawings().ids().iter().find(|d| d.as_str() != D1).unwrap().clone();
    let other_name = doc.drawing(&other).unwrap().name().clone();
    drop(doc);
    assert_eq!(
        err(&h, "draw.drawing.rename", json!({ "drawingId": D1, "name": other_name })).code,
        "drawing.nameExists"
    );
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }))["name"], "1");
    let r = ok(&h, "draw.lock.set", json!({ "drawingId": D1, "locked": true }));
    assert_eq!(r["locked"], true);
    assert_eq!(err(&h, "draw.pivot.set", json!({ "drawingId": D1, "point": {"x": 0, "y": 0} })).code, "drawing.locked");
    ok(&h, "draw.lock.set", json!({ "drawingId": D1, "locked": false }));
    ok(&h, "draw.pivot.set", json!({ "drawingId": D1, "point": {"x": 0, "y": 0} }));
}

fn stroke(h: &tt_executor::ExecutorHandle, id: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or(Value::Null)
}

#[test]
fn shape_tools_add_closed_and_open_pencil_paths_with_zones_and_undo() {
    let h = fixture();
    let r = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}, "layer": "new" }),
    );
    assert_eq!(r["zoneIds"].as_array().unwrap().len(), 1, "a closed rectangle bounds one zone");
    let s = stroke(&h, r["strokeIds"][0].as_str().unwrap());
    assert_eq!((s["kind"].as_str(), s["closed"].as_bool()), (Some("pencil"), Some(true)));
    let sq = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 600, "y": 300}, "to": {"x": 650, "y": 380}, "square": true, "layer": "new" }),
    );
    let b = ok(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": sq["strokeIds"] }))["bounds"].clone();
    assert!((b["width"].as_f64().unwrap() - b["height"].as_f64().unwrap()).abs() < 1e-6, "{b}");
    let e = ok(
        &h,
        "draw.ellipse.add",
        json!({ "drawingId": D1, "from": {"x": 800, "y": 300}, "to": {"x": 850, "y": 350}, "circle": true, "fromCentre": true, "layer": "new" }),
    );
    assert_eq!(e["zoneIds"].as_array().unwrap().len(), 1);
    let eb = ok(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": e["strokeIds"] }))["bounds"].clone();
    let line = eb["width"].as_f64().unwrap() - 100.0;
    assert!(
        line > 0.0 && line <= 4.0 && (eb["height"].as_f64().unwrap() - eb["width"].as_f64().unwrap()).abs() < 1e-6,
        "{eb}"
    );
    let l = ok(
        &h,
        "draw.line.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 500}, "to": {"x": 400, "y": 510}, "constrainAngle": true, "layer": "new" }),
    );
    let ls = stroke(&h, l["strokeIds"][0].as_str().unwrap());
    assert_eq!(ls["bezier"][3]["y"], 500.0, "constrained to the horizontal: {ls}");
    let c = ok(
        &h,
        "draw.line.add",
        json!({ "drawingId": D1, "to": {"x": 450, "y": 560}, "continuePrevious": true, "layer": l["layerId"] }),
    );
    let cs = stroke(&h, c["strokeIds"][0].as_str().unwrap());
    assert!((cs["bezier"][0]["x"].as_f64().unwrap() - ls["bezier"][3]["x"].as_f64().unwrap()).abs() < 1e-6);
    let p = ok(
        &h,
        "draw.polyline.add",
        json!({ "drawingId": D1, "points": [{"x": 300, "y": 700}, {"x": 360, "y": 740, "inHandle": {"x": -20, "y": 0}, "outHandle": {"x": 20, "y": 0}}, {"x": 420, "y": 700}], "snap": false, "layer": "new" }),
    );
    let ps = stroke(&h, p["strokeIds"][0].as_str().unwrap());
    assert_eq!(ps["bezier"].as_array().unwrap().len(), 7, "{ps}");
    assert_eq!(
        err(&h, "draw.rectangle.add", json!({ "drawingId": D1, "from": {"x": 0, "y": 0}, "to": {"x": 0, "y": 10} }))
            .code,
        "drawing.invalidGeometry"
    );
    // Each shape is one undo step.
    let before = rev(&h);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(rev(&h), before + 1);
    assert!(stroke(&h, p["strokeIds"][0].as_str().unwrap()).is_null());
}
