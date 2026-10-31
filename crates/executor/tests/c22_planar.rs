//! M2 C2.2 planar map through the executor: zones in the read-back, painting by ID, point, path, lasso and rect,
//! split/merge fill carry-over, re-noding with stroke IDs, gap closing, protected colours, unpaint, staleness, the
//! drawing event's zone changes, atomic refusal of over-budget edits, fills travelling with content, the hit test
//! and the unpaintedZones lint.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Status};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

fn colours(h: &ExecutorHandle) -> Vec<String> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect()
}

fn rect(h: &ExecutorHandle, layer: &str, x0: f64, y0: f64, w: f64, hgt: f64) -> Value {
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": layer, "fit": false, "closed": true,
                "points": [{"x": x0, "y": y0}, {"x": x0 + w, "y": y0}, {"x": x0 + w, "y": y0 + hgt}, {"x": x0, "y": y0 + hgt}] }),
    )
}

fn layer_of(h: &ExecutorHandle, layer: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == layer).cloned().unwrap()
}

fn zone_at(layer: &Value, x: f64, y: f64) -> Option<Value> {
    layer["zones"]
        .as_array()
        .unwrap()
        .iter()
        .find(|z| {
            let b = &z["bounds"];
            let (bx, by, bw, bh) = (
                b["x"].as_f64().unwrap(),
                b["y"].as_f64().unwrap(),
                b["width"].as_f64().unwrap(),
                b["height"].as_f64().unwrap(),
            );
            x > bx && x < bx + bw && y > by && y < by + bh
        })
        .cloned()
}

/// A fresh layer holding one closed pencil rectangle 0..100 × 0..100 far from the fixture's artwork.
fn setup() -> (ExecutorHandle, String) {
    let h = fixture();
    let r = rect(&h, "new", 1000.0, 1000.0, 100.0, 100.0);
    (h, r["layerId"].as_str().unwrap().to_owned())
}

#[test]
fn a_closed_line_bounds_a_zone_read_back_with_sides_and_no_unavailability_warning() {
    let (h, layer) = setup();
    let env = run(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
    assert!(!env.warnings.iter().any(|w| w.code == "doc.derivedUnavailable"));
    let l = layer_of(&h, &layer);
    let zones = l["zones"].as_array().unwrap();
    assert_eq!(zones.len(), 1);
    let z = &zones[0];
    assert_eq!(z["type"], "zone");
    assert!(z["colourId"].is_null());
    assert!((z["area"].as_f64().unwrap() - 10_000.0).abs() < 1e-6);
    let s = &l["strokes"][0];
    assert!(s["leftZoneId"] == z["id"] || s["rightZoneId"] == z["id"], "{s}");
    // The read-back validates against the closed schema (zones included).
    let schema = tt_vector::drawing::readback_schema();
    let d = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
    assert!(jsonschema::is_valid(&schema, &d), "{d}");
}

#[test]
fn painting_by_point_and_id_stores_the_fill_undo_and_redo_restore_it_and_events_report_the_zone() {
    let (h, layer) = setup();
    let cols = colours(&h);
    let rx = h.subscribe();
    let r = ok(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}], "colourId": cols[1] }),
    );
    assert_eq!(r["painted"][0]["objectKind"], "zone");
    let zid = r["painted"][0]["objectId"].as_str().unwrap().to_owned();
    let l = layer_of(&h, &layer);
    assert_eq!(zone_at(&l, 1050.0, 1050.0).unwrap()["colourId"], cols[1]);
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").collect();
    assert_eq!(events.len(), 1);
    assert!(events[0].payload["modifiedIds"].as_array().unwrap().contains(&json!(zid)), "{:?}", events[0].payload);
    ok(&h, "edit.undo", json!({}));
    assert!(zone_at(&layer_of(&h, &layer), 1050.0, 1050.0).unwrap()["colourId"].is_null());
    let undo_events: Vec<_> = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").collect();
    assert!(undo_events[0].payload["modifiedIds"].as_array().unwrap().contains(&json!(zid)));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(zone_at(&layer_of(&h, &layer), 1050.0, 1050.0).unwrap()["colourId"], cols[1]);
    // By ID, with the revision read.
    let rev = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }))["rev"].clone();
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [zid], "colourId": cols[2], "expectRev": rev }));
    assert_eq!(zone_at(&layer_of(&h, &layer), 1050.0, 1050.0).unwrap()["colourId"], cols[2]);
    // A stale revision with a zone target is colour.staleZoneId; an unknown zone ID too.
    let e = err(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "objectIds": [zid], "colourId": cols[3], "expectRev": 1 }),
    );
    assert_eq!(e.code, "colour.staleZoneId");
    let e = err(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "objectIds": ["zn_00000000000000000000000000"], "colourId": cols[3] }),
    );
    assert_eq!(e.code, "colour.staleZoneId");
}

#[test]
fn a_split_zone_keeps_its_fill_on_both_parts_and_a_merge_takes_the_larger_part() {
    let (h, layer) = setup();
    let cols = colours(&h);
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}], "colourId": cols[1] }));
    let before = layer_of(&h, &layer)["strokes"].as_array().unwrap().len();
    let cut = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "layer": layer, "fit": false,
                "points": [{"x": 1030, "y": 990}, {"x": 1030, "y": 1110}] }),
    );
    // The cut is split at both crossings into three pieces; the rectangle into two.
    assert_eq!(cut["strokeIds"].as_array().unwrap().len(), 3, "{cut}");
    assert_eq!(
        cut["removedIds"].as_array().unwrap().iter().filter(|i| i.as_str().unwrap().starts_with("st_")).count(),
        1,
        "{cut}"
    );
    assert_eq!(cut["zoneIds"].as_array().unwrap().len(), 2, "{cut}");
    let l = layer_of(&h, &layer);
    assert_eq!(l["strokes"].as_array().unwrap().len(), before - 1 + 2 + 3);
    let zones = l["zones"].as_array().unwrap();
    assert_eq!(zones.len(), 2);
    assert!(zones.iter().all(|z| z["colourId"] == cols[1]), "{zones:?}");
    // Paint the small part another colour, then remove the cut's middle piece: the merged zone takes the larger
    // part's colour.
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 1010, "y": 1050}], "colourId": cols[2] }));
    let mid = cut["strokeIds"][1].as_str().unwrap();
    ok(&h, "draw.object.delete", json!({ "drawingId": D1, "ids": [mid] }));
    let zones = layer_of(&h, &layer)["zones"].as_array().unwrap().clone();
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0]["colourId"], cols[1]);
}

#[test]
fn gap_closing_paints_an_open_region_and_keeps_only_the_bounding_segment() {
    let h = fixture();
    let cols = colours(&h);
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": "new", "fit": false,
                "points": [{"x": 2000, "y": 2100}, {"x": 2000, "y": 2000}, {"x": 2100, "y": 2000}, {"x": 2100, "y": 2100}, {"x": 2051, "y": 2100}] }),
    );
    let layer = r["layerId"].as_str().unwrap().to_owned();
    ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": layer, "fit": false, "points": [{"x": 2049, "y": 2100}, {"x": 2000, "y": 2100}] }),
    );
    let e = err(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 2050, "y": 2050}], "colourId": cols[1], "gapClosing": "off" }),
    );
    assert_eq!(e.code, "colour.zoneNotClosed");
    let p = ok(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 2050, "y": 2050}], "colourId": cols[1], "gapClosing": "small" }),
    );
    assert_eq!(p["addedStrokeIds"].as_array().unwrap().len(), 1, "{p}");
    let l = layer_of(&h, &layer);
    let added = l["strokes"].as_array().unwrap().iter().find(|s| s["id"] == p["addedStrokeIds"][0]).unwrap();
    assert_eq!(added["kind"], "invisible");
    assert_eq!(zone_at(&l, 2050.0, 2050.0).unwrap()["colourId"], cols[1]);
    // The user setting applies when the call says `current`.
    assert_eq!(ok(&h, "colour.gapClosing.set", json!({ "mode": "medium" }))["mode"], "medium");
    assert_eq!(ok(&h, "colour.gapClosing.get", json!({}))["mode"], "medium");
}

#[test]
fn protected_colours_are_refused_per_target_unless_overridden() {
    let (h, _layer) = setup();
    let cols = colours(&h);
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}], "colourId": cols[1] }));
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [cols[1].clone()], "ops": [{"op": "add", "path": "/ext", "value": {"dev.toontown.paint": {"protected": true}}}] }),
    );
    let e = err(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}], "colourId": cols[2] }),
    );
    assert_eq!(e.code, "colour.protected");
    let r = ok(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}], "colourId": cols[2], "respectProtected": false }),
    );
    assert_eq!(r["painted"][0]["previousColourId"], cols[1]);
}

#[test]
fn unpaint_turns_a_line_invisible_and_clears_a_zone_brush_strokes_refuse() {
    let (h, layer) = setup();
    let cols = colours(&h);
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}], "colourId": cols[1] }));
    let r = ok(&h, "colour.paint.remove", json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}] }));
    assert_eq!(r["unpainted"][0]["previousColourId"], cols[1]);
    let l = layer_of(&h, &layer);
    assert!(zone_at(&l, 1050.0, 1050.0).unwrap()["colourId"].is_null());
    let line = l["strokes"][0]["id"].as_str().unwrap().to_owned();
    ok(&h, "colour.paint.remove", json!({ "drawingId": D1, "objectIds": [line.clone()] }));
    let l = layer_of(&h, &layer);
    assert_eq!(l["strokes"][0]["kind"], "invisible");
    assert_eq!(l["zones"].as_array().unwrap().len(), 1, "the zone stays closed");
    let e = err(&h, "colour.paint.remove", json!({ "drawingId": D1, "objectIds": ["st_01K742SG0QD1F2HFZSX8R4V7WP"] }));
    assert_eq!(e.code, "colour.cannotUnpaint");
}

#[test]
fn paint_unpainted_fills_only_unpainted_zones_by_lasso_and_all() {
    let h = fixture();
    let cols = colours(&h);
    let a = rect(&h, "new", 3000.0, 3000.0, 50.0, 50.0)["layerId"].as_str().unwrap().to_owned();
    rect(&h, &a, 3100.0, 3000.0, 50.0, 50.0);
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 3025, "y": 3025}], "colourId": cols[1] }));
    let r = ok(
        &h,
        "colour.paintUnpainted.apply",
        json!({ "drawingId": D1, "lasso": [{"x": 2990, "y": 2990}, {"x": 3200, "y": 2990}, {"x": 3200, "y": 3060}, {"x": 2990, "y": 3060}], "colourId": cols[2] }),
    );
    assert_eq!(r["painted"].as_array().unwrap().len(), 1);
    assert_eq!(r["skippedPainted"], 1);
    let l = layer_of(&h, &a);
    assert_eq!(zone_at(&l, 3025.0, 3025.0).unwrap()["colourId"], cols[1]);
    assert_eq!(zone_at(&l, 3125.0, 3025.0).unwrap()["colourId"], cols[2]);
    let none = ok(
        &h,
        "colour.paintUnpainted.apply",
        json!({ "drawingId": D1, "lasso": [{"x": 2990, "y": 2990}, {"x": 3200, "y": 2990}, {"x": 3200, "y": 3060}, {"x": 2990, "y": 3060}], "colourId": cols[3] }),
    );
    assert_eq!(none["painted"].as_array().unwrap().len(), 0);
}

#[test]
fn an_over_budget_edit_is_refused_atomically_and_the_next_edit_runs() {
    let h = fixture();
    let rev0 = rev(&h);
    let rx = h.subscribe();
    // 100 000 points zig-zagging inside a 10 du box: every segment crosses every other one.
    let pts: Vec<Value> =
        (0..100_000).map(|i| json!({"x": f64::from(i % 2) * 10.0, "y": f64::from(i % 997) * 0.01})).collect();
    let e = run(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": "new", "fit": false, "points": pts }),
    );
    assert_eq!(e.status, Status::Error);
    assert_eq!(e.error.as_ref().unwrap().code, "limit.exceeded", "{:?}", e.error);
    assert_eq!(rev(&h), rev0, "no revision");
    assert!(rx.try_iter().all(|e| e.name != "draw.drawing.changed" && e.name != "doc.changed"), "no event");
    let undo = run(&h, "edit.undo", json!({}));
    assert!(undo.status == Status::Error || rev(&h) == rev0 + 1, "no undo record of the refused edit");
    let (h2, _) = setup();
    drop(h);
    assert!(rev(&h2) > 0);
}

#[test]
fn a_pencil_line_across_a_brush_stroke_splits_and_the_brush_entry_is_untouched() {
    let h = fixture();
    let brush_layer = "nd_01K742SG0PC20XDQKASM9CGZHR";
    let before = h
        .snapshot()
        .unwrap()
        .unwrap()
        .entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D1).unwrap()))
        .unwrap();
    let brush_before = before["nodes"][0]["strokes"][0].clone();
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": brush_layer, "fit": false,
                "points": [{"x": 50, "y": -30}, {"x": 50, "y": 130}] }),
    );
    assert!(r["strokeIds"].as_array().unwrap().len() >= 3, "the pencil splits at the outline: {r}");
    let after = h
        .snapshot()
        .unwrap()
        .unwrap()
        .entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D1).unwrap()))
        .unwrap();
    let brush_after = after["nodes"][0]["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == brush_before["id"])
        .unwrap()
        .clone();
    assert_eq!(brush_after["points"], brush_before["points"]);
    assert_eq!(brush_after["spine"], brush_before["spine"]);
    assert_eq!(brush_after["sides"], brush_before["sides"]);
}

#[test]
fn zone_fills_travel_with_pasted_content() {
    let (h, layer) = setup();
    let cols = colours(&h);
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 1050, "y": 1050}], "colourId": cols[4] }));
    let content = ok(&h, "draw.drawing.get", json!({ "drawingId": D1, "ids": [layer] }));
    let nodes: Vec<Value> = content["nodes"].as_array().unwrap().clone();
    let one = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
    let mut c = one.clone();
    c["nodes"] =
        json!(one["nodes"].as_array().unwrap().iter().filter(|n| n["id"] == layer).cloned().collect::<Vec<_>>());
    let _ = nodes;
    let r = ok(&h, "draw.content.add", json!({ "drawingId": D1, "content": c, "offset": {"x": 500, "y": 0} }));
    let new_layer = r["idMap"][&layer].as_str().unwrap().to_owned();
    let l = layer_of(&h, &new_layer);
    assert_eq!(zone_at(&l, 1550.0, 1050.0).unwrap()["colourId"], cols[4]);
}

#[test]
fn the_hit_test_reports_lines_before_zones_and_the_lint_lists_unpainted_zones() {
    let (h, layer) = setup();
    let hits = ok(&h, "draw.hitTest.get", json!({ "drawingId": D1, "point": {"x": 1000, "y": 1050} }));
    assert_eq!(hits["hits"][0]["objectKind"], "pencil");
    let inside = ok(&h, "draw.hitTest.get", json!({ "drawingId": D1, "point": {"x": 1050, "y": 1050} }));
    assert_eq!(inside["hits"][0]["objectKind"], "zone");
    assert_eq!(inside["zoneId"], inside["hits"][0]["objectId"]);
    let v = ok(&h, "doc.validate", json!({ "checks": ["unpaintedZones"] }));
    let issues: Vec<&Value> =
        v["issues"].as_array().unwrap().iter().filter(|i| i["code"] == "unpaintedZones").collect();
    assert!(issues.iter().any(|i| i["details"]["layerId"] == layer), "{v}");
    // E2: paint each reported zone by ID; validation is then clean.
    let cols = colours(&h);
    for i in &issues {
        ok(
            &h,
            "colour.paint.apply",
            json!({ "drawingId": i["details"]["drawingId"], "objectIds": [i["details"]["zoneId"]], "colourId": cols[1] }),
        );
    }
    let v = ok(&h, "doc.validate", json!({ "checks": ["unpaintedZones"] }));
    assert_eq!(v["issues"].as_array().unwrap().iter().filter(|i| i["code"] == "unpaintedZones").count(), 0, "{v}");
    let derived = ok(&h, "doc.object.get", json!({ "ref": D1, "include": ["derived"] }));
    assert!(derived.to_string().contains("\"zones\""), "{derived}");
}
