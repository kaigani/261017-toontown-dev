#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression tests")]
mod common;
use common::*;
use serde_json::{Value, json};
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
fn narrow(h: &tt_executor::ExecutorHandle) -> Value {
    ok(
        h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"invisible","layer":"new","fit":false,"closed":true,
        "points":[{"x":1000.1,"y":1000.0},{"x":1000.2,"y":1000.0},{"x":1000.2,"y":1002.0},{"x":1000.1,"y":1002.0}]}),
    )
}
#[test]
fn crossing_path_paints_a_zone_between_sample_positions() {
    let h = fixture();
    let _r = narrow(&h);
    let result =
        run(&h, "colour.paint.apply", json!({"drawingId":D,"path":[{"x":999.0,"y":1001.0},{"x":1001.0,"y":1001.0}]}));
    assert!(result.is_ok(), "a continuous path crosses the 0.1 du wide zone: {:?}", result.error);
    assert!(result.result.unwrap()["painted"].as_array().unwrap().iter().any(|p| p["objectKind"] == "zone"));
}

mod c13_support;
const MISSING: &str = "col_01K9YYYYYYYYYYYYYYYYYYYYYY";
#[test]
fn save_has_a_recovery_entry_for_every_used_colour_even_without_prior_copy() {
    let dir = c13_support::tmp();
    let h = fixture();
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[format!("{D}/st_01K742SG0R61SYK9GG0HGYVNBT")],"ops":[{"op":"replace","path":"/colourId","value":MISSING}]}),
    );
    c13_support::job_ok(&h, "project.saveAs", json!({"path":dir.join("R.animpkg").to_string_lossy()}));
    let doc = h.snapshot().unwrap().unwrap();
    let d = doc.drawing(&tt_model::Id::parse(D).unwrap()).unwrap();
    let found = d.colour_recovery().iter().any(|e| e.colour_id.as_str() == MISSING);
    drop(doc);
    drop(h);
    cleanup(&dir);
    assert!(found, "03 1.6 / 10 4.11: every used ID must have a complete recovery copy on save");
}
#[test]
fn loading_a_drawing_recovers_its_missing_colour() {
    let dir = c13_support::tmp();
    let h = fixture();
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[format!("{D}/st_01K742SG0R61SYK9GG0HGYVNBT")],"ops":[{"op":"replace","path":"/colourId","value":MISSING}]}),
    );
    let package = dir.join("Load.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":package.to_string_lossy()}));
    drop(h);
    let h = c13_support::spawn(&dir.join("prefs"));
    c13_support::job_ok(&h, "project.open", json!({"path":package.to_string_lossy()}));
    let missing = ok(&h, "colour.missing.list", json!({}));
    drop(h);
    cleanup(&dir);
    assert!(
        !missing["missing"].as_array().unwrap().iter().any(|x| x["colourId"] == MISSING),
        "03 11.2: load should create a placeholder: {missing}"
    );
}
#[test]
fn loading_repairs_inconsistent_boundary_sides_and_reports_it() {
    let dir = c13_support::tmp();
    let h = c13_support::spawn(&dir.join("first"));
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let cols: Vec<_> =
        parts.palettes.iter().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect();
    let i = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
    let template = d["nodes"][0]["strokes"][1].clone();
    let pts = [(1000., 1000.), (1100., 1000.), (1100., 1010.), (1000., 1010.)];
    d["nodes"][0]["strokes"] = json!(
        (0..4)
            .map(|k| {
                let mut s = template.clone();
                s["id"] = json!(format!("st_review_load_{k}"));
                s["kind"] = json!("pencil");
                s["closed"] = json!(false);
                let (a, b) = (pts[k], pts[(k + 1) % 4]);
                s["points"] = json!([[a.0, a.1, 0, 0, 0, 0], [b.0, b.1, 0, 0, 0, 0]]);
                s["sides"] = json!({"left":{"colourId":cols[if k==3 {2}else{1}],"matrix":null},"right":null});
                s
            })
            .collect::<Vec<_>>()
    );
    parts.drawings[i] = tt_model::object::from_json(&d).unwrap();
    assert!(h.open_document(tt_model::document::Document::from_parts(parts).unwrap(), None, false).is_ok());
    let package = dir.join("Fills.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":package.to_string_lossy()}));
    drop(h);
    let h = c13_support::spawn(&dir.join("second"));
    c13_support::job_ok(&h, "project.open", json!({"path":package.to_string_lossy()}));
    let report = ok(&h, "doc.loadReport.get", json!({}));
    let doc = h.snapshot().unwrap().unwrap();
    let raw = doc.entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D).unwrap())).unwrap();
    let sides: Vec<_> =
        raw["nodes"][0]["strokes"].as_array().unwrap().iter().map(|s| s["sides"]["left"]["colourId"].clone()).collect();
    drop(doc);
    drop(h);
    cleanup(&dir);
    assert!(
        report["issues"].as_array().unwrap().iter().any(|x| x["code"] == "drawing.fillInconsistent"),
        "missing load repair report: {report}; sides: {sides:?}"
    );
    assert!(sides.iter().all(|c| *c == cols[1]));
}
#[test]
fn painting_closed_and_gap_closed_points_paints_both_targets() {
    let h = fixture();
    let first = narrow(&h);
    let lid = first["layerId"].as_str().unwrap();
    ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":lid,"fit":false,
      "points":[{"x":2000,"y":2100},{"x":2000,"y":2000},{"x":2100,"y":2000},{"x":2100,"y":2100},{"x":2051,"y":2100}]}),
    );
    ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":lid,"fit":false,
      "points":[{"x":2049,"y":2100},{"x":2000,"y":2100}]}),
    );
    let r = ok(
        &h,
        "colour.paint.apply",
        json!({"drawingId":D,"points":[{"x":1000.15,"y":1001},{"x":2050,"y":2050}],"gapClosing":"small"}),
    );
    assert_eq!(
        r["painted"].as_array().unwrap().len(),
        2,
        "gap closing must retain previously resolved closed targets: {r}"
    );
}

fn cleanup(dir: &std::path::Path) {
    for _ in 0..80 {
        if !dir.exists() || std::fs::remove_dir_all(dir).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn painting_a_pencil_with_a_gradient_stores_the_drag_mapping() {
    let h = fixture();
    let doc = h.snapshot().unwrap().unwrap();
    let gradient = doc
        .palettes()
        .flat_map(|p| p.swatches())
        .find(|s| *s.kind() == tt_model::entities::palette::SwatchKind::LinearGradient)
        .unwrap()
        .colour_id()
        .as_str()
        .to_owned();
    drop(doc);
    let add = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":"new","fit":false,"points":[{"x":1000,"y":1000},{"x":1100,"y":1000}]}),
    );
    let id = add["strokeIds"][0].as_str().unwrap();
    ok(
        &h,
        "colour.paint.apply",
        json!({"drawingId":D,"objectIds":[id],"colourId":gradient,"mapping":{"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1000}}}),
    );
    let doc = h.snapshot().unwrap().unwrap();
    let raw = doc.entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D).unwrap())).unwrap();
    let s = raw["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|n| n["strokes"].as_array().into_iter().flatten())
        .find(|s| s["id"] == id)
        .unwrap();
    assert_eq!(s["fillMatrix"], json!([100, 0, 0, 100, 1000, 950]), "03 7.2 also applies to lines");
}
#[test]
fn repainting_with_solid_preserves_the_existing_fill_matrix() {
    let h = fixture();
    let add = narrow(&h);
    let zone = add["zoneIds"][0].clone();
    let doc = h.snapshot().unwrap().unwrap();
    let colours: Vec<_> = doc.palettes().flat_map(|p| p.swatches()).collect();
    let gradient = colours
        .iter()
        .find(|s| *s.kind() == tt_model::entities::palette::SwatchKind::LinearGradient)
        .unwrap()
        .colour_id()
        .as_str()
        .to_owned();
    let solid = colours
        .iter()
        .find(|s| *s.kind() == tt_model::entities::palette::SwatchKind::Solid)
        .unwrap()
        .colour_id()
        .as_str()
        .to_owned();
    drop(doc);
    ok(
        &h,
        "colour.paint.apply",
        json!({"drawingId":D,"objectIds":[zone],"colourId":gradient,"mapping":{"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1000}}}),
    );
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[zone],"colourId":solid}));
    let d = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
    let l = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == add["layerId"]).unwrap();
    assert_eq!(
        l["zones"][0]["textureMatrix"],
        json!([100, 0, 0, 100, 1000, 950]),
        "03 1.6: solid mapping is ignored but preserved"
    );
}

#[test]
fn json_import_recovers_missing_colour_references_in_the_same_transaction() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let i = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
    d["nodes"][0]["strokes"][1]["colourId"] = json!(MISSING);
    parts.drawings[i] = tt_model::object::from_json(&d).unwrap();
    let source = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(source.open_document(tt_model::document::Document::from_parts(parts).unwrap(), None, false).is_ok());
    let bundle = ok(&source, "doc.export.json", json!({"scope":"project","binaries":"embed"}))["bundle"].clone();
    let dest = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&dest, "project.create", json!({"name":"Review import"}));
    let r = ok(&dest, "doc.import.json", json!({"source":bundle,"idPolicy":"preserve"}));
    let missing = ok(&dest, "colour.missing.list", json!({}));
    assert!(
        !missing["missing"].as_array().unwrap().iter().any(|c| c["colourId"] == MISSING),
        "10 4.11 explicitly requires recovery on doc.import.json: result {r}, missing {missing}"
    );
}
