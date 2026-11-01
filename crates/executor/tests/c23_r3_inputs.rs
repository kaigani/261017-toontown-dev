//! Export the existing contract parameter corpus for the independent official-SDK driver.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const BRUSH: &str = "st_01K742SG0QD1F2HFZSX8R4V7WP";
const CAPTION: &str = "nd_01K742SG0WFAQJFKAZKV4CD56C";
const GROUP: &str = "nd_01K742SG0TX3WF1BSWWGMWM5WG";

fn cases() -> Vec<(&'static str, Value)> {
    let colours = {
        let h = fixture();
        let v: Vec<String> = h
            .snapshot()
            .unwrap()
            .unwrap()
            .palettes()
            .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
            .collect();
        v
    };
    let q = |p: Value| {
        let mut p = p;
        p["drawingId"] = json!(D1);
        p
    };
    vec![
        ("draw.transform.apply", q(json!({ "ids": [PENCIL], "translate": {"x": 7, "y": -3}, "rotate": 10 }))),
        ("draw.flip.apply", q(json!({ "ids": [BRUSH], "axis": "vertical" }))),
        ("draw.rotate.apply", q(json!({ "ids": [CAPTION], "by": "90cw" }))),
        (
            "draw.envelope.apply",
            q(
                json!({ "ids": [PENCIL], "corners": [{"x": -60, "y": -60}, {"x": 60, "y": -50}, {"x": 40, "y": 60}, {"x": -40, "y": 50}] }),
            ),
        ),
        ("draw.point.add", json!({ "strokeId": format!("{D1}/{PENCIL}"), "segment": 0, "t": 0.25 })),
        ("draw.point.move", q(json!({ "points": [{"strokeId": PENCIL, "anchor": 0}], "delta": {"x": -5, "y": 2} }))),
        (
            "draw.point.set",
            json!({ "strokeId": format!("{D1}/{PENCIL}"), "anchor": 1, "inHandle": {"x": -10, "y": 4}, "handleMode": "symmetric" }),
        ),
        ("draw.point.delete", q(json!({ "points": [{"strokeId": BRUSH, "anchor": 1}] }))),
        (
            "draw.segment.move",
            json!({ "strokeId": format!("{D1}/{PENCIL}"), "segment": 0, "delta": {"x": 0, "y": 12} }),
        ),
        ("draw.cut.apply", q(json!({ "rect": {"x": -20, "y": -20, "width": 40, "height": 40} }))),
        (
            "draw.cut.apply",
            q(json!({ "polygon": [{"x": -30, "y": -30}, {"x": 30, "y": -30}, {"x": 0, "y": 30}], "then": "delete" })),
        ),
        ("draw.cut.apply", q(json!({ "segment": {"from": {"x": -100, "y": 10}, "to": {"x": 100, "y": 10}} }))),
        (
            "draw.erase.apply",
            q(json!({ "path": [{"x": -60, "y": 5}, {"x": 60, "y": 5}], "pen": {"minSize": 6, "maxSize": 6} })),
        ),
        ("draw.arrange.apply", q(json!({ "ids": [CAPTION], "to": "back" }))),
        ("draw.arrange.apply", q(json!({ "ids": [PENCIL], "to": "front" }))),
        ("draw.flatten.apply", q(json!({ "ids": [GROUP, PENCIL] }))),
        ("draw.smooth.apply", q(json!({ "ids": [BRUSH], "tolerance": 8 }))),
        ("draw.linesToBrush.apply", q(json!({ "ids": [PENCIL] }))),
        ("draw.centreLine.apply", q(json!({ "ids": [BRUSH] }))),
        ("draw.rectangle.add", q(json!({ "from": {"x": 300, "y": 300}, "to": {"x": 360, "y": 340} }))),
        ("draw.ellipse.add", q(json!({ "from": {"x": 400, "y": 300}, "to": {"x": 460, "y": 340} }))),
        ("draw.line.add", q(json!({ "from": {"x": 300, "y": 400}, "to": {"x": 380, "y": 420} }))),
        (
            "draw.polyline.add",
            q(json!({ "points": [{"x": 300, "y": 500}, {"x": 340, "y": 540}, {"x": 380, "y": 500}] })),
        ),
        ("colour.selection.apply", q(json!({ "colourId": colours[1], "objectIds": [PENCIL, BRUSH] }))),
        (
            "colour.recolour.apply",
            json!({ "fromColourId": colours[0], "toColourId": colours[4], "scope": "drawing", "drawingIds": [D1] }),
        ),
        (
            "colour.fillMapping.set",
            q(json!({ "objectIds": [PENCIL], "mapping": {"matrix": [40, 0, 0, 20, -40, -10]} })),
        ),
        ("draw.breakApart.apply", q(json!({ "textIds": [CAPTION] }))),
        ("draw.drawing.rename", q(json!({ "name": "renamed" }))),
        ("draw.lock.set", q(json!({ "locked": true }))),
        ("draw.optimize.apply", q(json!({}))),
        (
            "draw.repositionAll.apply",
            json!({ "elementId": "scene:#1/element:\"Bob\"", "translate": {"x": 0, "y": 20} }),
        ),
        // The SI-061 override, explicit (the protection itself: c23_colour::protected_colours_are_skipped_unless_overridden).
        (
            "colour.selection.apply",
            q(json!({ "colourId": colours[4], "objectIds": [BRUSH], "respectProtected": false })),
        ),
        (
            "colour.recolour.apply",
            json!({ "fromColourId": colours[4], "toColourId": colours[0], "scope": "drawing", "drawingIds": [D1], "respectProtected": false }),
        ),
    ]
}

/// Commands a case needs set up first (static IDs only).
fn prelude(cmd: &str, colours: &[String]) -> Vec<(&'static str, Value)> {
    match cmd {
        // A gradient on the pencil line gives it a mapping to set.
        "colour.fillMapping.set" => {
            vec![("colour.paint.apply", json!({ "drawingId": D1, "objectIds": [PENCIL], "colourId": colours[1] }))]
        }
        // Two disjoint layers beside the sample's, all selected: Optimize merges them.
        "draw.optimize.apply" => vec![
            (
                "draw.rectangle.add",
                json!({ "drawingId": D1, "from": {"x": 1000, "y": 1000}, "to": {"x": 1050, "y": 1050}, "layer": "new" }),
            ),
            (
                "draw.rectangle.add",
                json!({ "drawingId": D1, "from": {"x": 1200, "y": 1000}, "to": {"x": 1250, "y": 1050}, "layer": "new" }),
            ),
            ("draw.selection.set", json!({ "drawingId": D1, "by": "all" })),
        ],
        _ => Vec::new(),
    }
}

#[test]
fn export_mcp_contract_inputs() {
    let Some(out) = std::env::var_os("TT_REVIEW_OUT") else { return };
    let h = fixture();
    let colours: Vec<_> = h
        .snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string()))
        .collect();
    let list:Vec<_>=cases().into_iter().map(|(id,p)|json!({"id":id,"params":p,"prelude":prelude(id,&colours).into_iter().map(|(id,params)|json!({"id":id,"params":params})).collect::<Vec<_>>()})).collect();
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        std::path::PathBuf::from(out).join("mcp-contract-inputs.json"),
        serde_json::to_vec_pretty(&list).unwrap(),
    )
    .unwrap();
}
