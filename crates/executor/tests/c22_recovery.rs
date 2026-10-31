//! M2 C2.2 colour recovery (03 §11.2; 10 §4.11): pasted content with recovery copies or without, the missing-colour
//! list, on-demand recovery with undo, and recovery lists refreshed on save.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use common::*;
use serde_json::{Value, json};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const MISSING: &str = "col_01K9ZZZZZZZZZZZZZZZZZZZZZZ";
const ORPHAN: &str = "col_01K9YYYYYYYYYYYYYYYYYYYYYY";

fn content(colour: &str, recovery: Option<Value>) -> Value {
    let mut c = json!({
        "nodes": [{ "type": "layer", "id": "l", "layerKind": "pencil", "strokes": [
            { "id": "s", "kind": "pencil", "closed": false, "colourId": colour,
              "lineStyle": {"thickness": 1, "lineType": "solid"},
              "bezier": [{"x": 0, "y": 0}, {"x": 3, "y": 0}, {"x": 7, "y": 0}, {"x": 10, "y": 0}] }
        ]}]
    });
    if let Some(r) = recovery {
        c["colourRecovery"] = r;
    }
    c
}

fn swatch_palette(h: &tt_executor::ExecutorHandle, colour: &str) -> Option<(String, Value)> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.palettes().find_map(|p| {
        p.swatches()
            .iter()
            .find(|s| s.colour_id().as_str() == colour)
            .map(|s| (p.name().clone(), serde_json::to_value(s).unwrap()))
    })
}

#[test]
fn pasted_content_recovers_its_colours_into_the_recorded_palette_in_the_same_step() {
    let h = fixture();
    let rx = h.subscribe();
    let copy = json!([{
        "colourId": MISSING, "paletteId": "pal_01K9XXXXXXXXXXXXXXXXXXXXXX", "paletteName": "Main",
        "styles": [{"styleId": "sty_01K9XXXXXXXXXXXXXXXXXXXXXX", "styleName": "Night"}],
        "swatch": {"colourId": MISSING, "type": "swatch", "rev": 3, "name": "Shadow", "kind": "solid", "deformable": false,
                   "values": {"sty_01K9XXXXXXXXXXXXXXXXXXXXXX": {"rgba": "#102030FF"}}}
    }]);
    let r = run(&h, "draw.content.add", json!({ "drawingId": D1, "content": content(MISSING, Some(copy)) }));
    assert!(r.is_ok(), "{:?}", r.error);
    assert!(!r.warnings.iter().any(|w| w.code == "colour.unresolved"), "recovered, not reported unresolved");
    let (palette, swatch) = swatch_palette(&h, MISSING).unwrap();
    assert_eq!(palette, "Main", "the palette with the recorded name receives it");
    assert_eq!(swatch["name"], "Shadow");
    // Main has styles Day and Night: Night takes the copy's value, Day the copy's first value.
    assert!(swatch["values"].as_object().unwrap().values().all(|v| v["rgba"] == "#102030FF"), "{swatch}");
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "colour.recovery.performed").collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["colourIds"], json!([MISSING]));
    // One undo step undoes the paste and the recovery together.
    ok(&h, "edit.undo", json!({}));
    assert!(swatch_palette(&h, MISSING).is_none());
}

#[test]
fn a_colour_without_a_copy_gets_a_placeholder_in_recovered() {
    let h = fixture();
    ok(&h, "draw.content.add", json!({ "drawingId": D1, "content": content(ORPHAN, None) }));
    let (palette, swatch) = swatch_palette(&h, ORPHAN).unwrap();
    assert_eq!(palette, "Recovered");
    assert_eq!(swatch["name"], "Deleted colour");
    assert!(swatch["values"].as_object().unwrap().values().all(|v| v["rgba"] == "#808080FF"));
}

#[test]
fn missing_colours_are_listed_and_recovered_on_demand_as_one_undoable_step() {
    let h = fixture();
    // A line referencing an unknown colour, through a generic patch (nothing recovers it implicitly).
    let line = "st_01K742SG0R61SYK9GG0HGYVNBT";
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [format!("{D1}/{line}")], "ops": [{"op": "replace", "path": "/colourId", "value": ORPHAN}] }),
    );
    let m = ok(&h, "colour.missing.list", json!({}));
    let entry = m["missing"].as_array().unwrap().iter().find(|x| x["colourId"] == ORPHAN).unwrap();
    assert_eq!(entry["drawingIds"], json!([D1]));
    assert_eq!(entry["recoverable"], false);
    let r = ok(&h, "colour.recovery.run", json!({ "all": true }));
    assert_eq!(r["placeholders"], json!([ORPHAN]));
    assert_eq!(r["createdPaletteIds"].as_array().unwrap().len(), 1);
    assert!(
        ok(&h, "colour.missing.list", json!({}))["missing"].as_array().unwrap().iter().all(|x| x["colourId"] != ORPHAN)
    );
    ok(&h, "edit.undo", json!({}));
    assert!(swatch_palette(&h, ORPHAN).is_none());
}

#[test]
fn saving_refreshes_each_drawings_recovery_list_to_one_entry_per_used_colour() {
    let dir = c13_support::tmp();
    let h = fixture();
    let cols: Vec<String> = {
        let doc = h.snapshot().unwrap().unwrap();
        doc.palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect()
    };
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": "new", "fit": false, "closed": true, "colourId": cols[2],
                "points": [{"x": 900, "y": 900}, {"x": 950, "y": 900}, {"x": 950, "y": 950}, {"x": 900, "y": 950}] }),
    );
    let _ = r;
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 925, "y": 925}], "colourId": cols[3] }));
    c13_support::job_ok(&h, "project.saveAs", json!({ "path": dir.join("R.animpkg").to_string_lossy() }));
    let doc = h.snapshot().unwrap().unwrap();
    let d = doc.drawing(&tt_model::Id::parse(D1).unwrap()).unwrap();
    let ids: Vec<&str> = d.colour_recovery().iter().map(|e| e.colour_id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "exactly once: {ids:?}");
    for c in [&cols[2], &cols[3]] {
        assert!(ids.contains(&c.as_str()), "{c} used by a line or a zone fill: {ids:?}");
    }
    let _ = std::fs::remove_dir_all(dir);
}
