//! Reviewer lifetime controls: persisted stretch records, history, font outlines and session symmetry.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review test")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_vector::planar::{
    build,
    fill::{self, Fill},
    layer::{inputs, settle, stored},
};
use tt_vector::{
    geom::P,
    path::{Anchor, Path},
    work::Work,
};
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

fn wait(h: &tt_executor::ExecutorHandle, id: &str, p: Value) -> Value {
    let r = ok(h, id, p);
    let w = ok(h, "job.wait", json!({"jobId":r["jobId"],"timeoutMs":60000}));
    if w.get("record").is_some() { w["record"].clone() } else { w }
}
fn nodes(h: &tt_executor::ExecutorHandle) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":D}))["nodes"].clone()
}

#[test]
fn preexisting_stretch_records_survive_owner_deletion_undo_redo_and_package_open() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let at = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut drawing = serde_json::to_value(&parts.drawings[at]).unwrap();
    let template = drawing["nodes"][0]["strokes"][1].clone();
    let specs = vec![
        (true, vec![(0., 0.), (100., 0.), (100., 100.), (0., 100.)]),
        (true, vec![(0., 0.), (100., 0.), (100., 100.), (0., 100.)]),
        (true, vec![(0., 0.), (100., 0.), (100., 100.), (0., 100.)]),
        (false, vec![(0., 50.), (100., 50.), (0., 50.)]),
    ];
    let strokes: Vec<Value> = specs
        .iter()
        .enumerate()
        .map(|(i, (closed, ps))| {
            let path =
                Path { anchors: ps.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed: *closed };
            let mut s = template.clone();
            s["id"] = json!(format!("st_review_{i}"));
            s["kind"] = json!("pencil");
            s["closed"] = json!(closed);
            s["points"] = path.points_json();
            s["sides"] = json!({"left":null,"right":null});
            s["ext"] = json!({"org.review.opaque":{"array":[1,false,"kept"]}});
            s
        })
        .collect();
    let mut id = 0;
    let mut fresh = || {
        id += 1;
        Ok::<_, ()>(format!("st_review_piece_{id}"))
    };
    let base =
        settle(None, &strokes, None, &Default::default(), &mut fresh, &mut Work::unlimited("review")).unwrap().strokes;
    let map = build(&inputs(&base), &mut Work::unlimited("review")).unwrap();
    let colours: Vec<_> =
        parts.palettes.iter().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect();
    let paint = map
        .zones
        .iter()
        .enumerate()
        .map(|(i, z)| (z.id.clone(), Some(Fill { colour_id: Some(colours[i % colours.len()].clone()), matrix: None })))
        .collect();
    let painted = settle(Some(&base), &base, None, &paint, &mut fresh, &mut Work::unlimited("review")).unwrap().strokes;
    assert!(stored(&painted).exterior.iter().any(|r| !r.is_empty()), "fixture really has stretch records");
    let edge = map.edges.iter().find(|e| e.srcs.len() >= 3).unwrap();
    let owner = fill::owner(&map, &edge.srcs.iter().collect::<Vec<_>>()).unwrap();
    let owner_id = map.strokes[owner.stroke].0.clone();
    let mut layer = drawing["nodes"][0].clone();
    layer["strokes"] = json!(painted);
    drawing["nodes"] = json!([layer]);
    parts.drawings[at] = tt_model::object::from_json(&drawing).unwrap();
    let document = tt_model::document::Document::from_parts(parts).unwrap();
    let h = fixture();
    let opened = h.open_document(document, None, false);
    assert!(opened.is_ok(), "{opened:?}");
    let initial = nodes(&h);
    ok(&h, "draw.object.delete", json!({"drawingId":D,"ids":[owner_id]}));
    let deleted = nodes(&h);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(nodes(&h), initial);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(nodes(&h), deleted);
    let dir = std::env::temp_dir().join(format!("tt-c24-r1-lifetime-{}", tt_persist::fsx::random_token(8)));
    std::fs::create_dir_all(&dir).unwrap();
    let pkg = dir.join("records.animpkg");
    let saved = wait(&h, "project.saveAs", json!({"path":pkg.to_string_lossy(),"onExists":"overwrite"}));
    assert_eq!(saved["state"], "succeeded");
    let before = nodes(&h);
    let h2 = fixture();
    let opened = wait(&h2, "project.open", json!({"path":pkg.to_string_lossy(),"onUnsaved":"discard"}));
    assert_eq!(opened["state"], "succeeded", "{opened}");
    assert_eq!(nodes(&h2), before);
    drop(h2);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fixture_font_letters_break_apart_and_extract_with_source_colour() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/fonts/B612-Regular.ttf");
    tt_vector::fonts::register_test_font(std::fs::read(file).unwrap());
    let h = fixture();
    let text = ok(
        &h,
        "draw.text.add",
        json!({"drawingId":D,"position":{"x":0,"y":400},"text":"YTOX","format":{"font":"B612","size":96}}),
    );
    let apart = ok(&h, "draw.breakApart.apply", json!({"drawingId":D,"textIds":[text["textId"]]}));
    let r = ok(&h, "draw.centreLine.apply", json!({"drawingId":D,"ids":apart["groupIds"]}));
    println!("letters centre line: {r}");
    assert!(!r["strokeIds"].as_array().unwrap().is_empty());
    assert!(r["warnings"].as_array().unwrap().is_empty(), "simple non-crossing font outlines should extract: {r}");
}
