//! M2 C2.2 readiness item 8 (02 §13.1 item 9, 11 P7): a fixed script of planar-map commands — crossing strokes in one
//! layer (persistent noding), a split and a merge of painted zones (fill carry-over and its tie rule), a brush ring
//! whose hole is painted (brush exterior records, SI-060), automatic gap closing while painting, unpaint, colour
//! recovery of pasted content and painted content pasted again — produces the same canonical entries and the same
//! derived planar graph (zone IDs, areas, interior points, boundaries, holes, parents, side zone IDs, fills and point
//! membership on a lattice) in two executors of one run and the committed golden digest on every OS (CI: Windows x86_64
//! and macOS aarch64). Recording: `TT_UPDATE_GOLDEN=1 cargo test -p tt-executor --test c22_determinism`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

fn colours(h: &ExecutorHandle) -> Vec<String> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect()
}

fn script(h: &ExecutorHandle) {
    let c = colours(h);
    // A layer of crossing pencil and invisible strokes: noded into pieces.
    let r = ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": "new", "fit": false, "closed": true,
                "points": [{"x": 300, "y": 300}, {"x": 420, "y": 300}, {"x": 420, "y": 400}, {"x": 300, "y": 400}] }),
    );
    let layer = r["layerId"].as_str().unwrap().to_owned();
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 360, "y": 350}], "colourId": c[1] }));
    let cut = ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "layer": layer, "fit": false,
                "points": [{"x": 340, "y": 280}, {"x": 340, "y": 420}] }),
    );
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": layer, "fit": true, "smoothness": 1,
                "points": [{"x": 290, "y": 330}, {"x": 330, "y": 345}, {"x": 380, "y": 360}, {"x": 430, "y": 372}] }),
    );
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 320, "y": 320}], "colourId": c[2] }));
    // Merge: removing the cut's middle piece joins zones of different colours (the larger part's colour wins).
    let _ = cut;
    // The piece of the cut under (340, 380), found by hit test (later strokes split the cut again).
    let hit = ok(
        h,
        "draw.hitTest.get",
        json!({ "drawingId": D1, "point": {"x": 340, "y": 380}, "tolerance": 0.5, "includeInvisible": true }),
    );
    let mid = hit["hits"].as_array().unwrap().iter().find(|x| x["objectKind"] == "invisible").unwrap()["objectId"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(h, "draw.object.delete", json!({ "drawingId": D1, "ids": [mid] }));
    // A brush ring whose hole is painted (exterior records).
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "brush", "layer": "new", "closed": true, "fit": false,
                "pen": {"tipShape": "round", "minSize": 6, "maxSize": 6},
                "points": [{"x": 500, "y": 300}, {"x": 560, "y": 300}, {"x": 560, "y": 360}, {"x": 500, "y": 360}] }),
    );
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 530, "y": 330}], "colourId": c[3] }));
    // An open square closed by gap closing while painting.
    let g = ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": "new", "fit": false,
                "points": [{"x": 650, "y": 360}, {"x": 650, "y": 300}, {"x": 710, "y": 300}, {"x": 710, "y": 360}, {"x": 681, "y": 360}] }),
    );
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": g["layerId"], "fit": false, "points": [{"x": 679, "y": 360}, {"x": 650, "y": 360}] }),
    );
    ok(
        h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 680, "y": 330}], "colourId": c[4], "gapClosing": "small" }),
    );
    // Unpaint a zone and paint unpainted zones in a lasso.
    ok(h, "colour.paint.remove", json!({ "drawingId": D1, "points": [{"x": 680, "y": 330}] }));
    ok(
        h,
        "colour.paintUnpainted.apply",
        json!({ "drawingId": D1, "lasso": [{"x": 280, "y": 280}, {"x": 720, "y": 280}, {"x": 720, "y": 420}, {"x": 280, "y": 420}], "colourId": c[5] }),
    );
    // Painted content pasted again, with a colour recovered from its copy.
    let copy = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "ids": [layer.clone()] }));
    let mut content = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    content["nodes"] =
        json!(content["nodes"].as_array().unwrap().iter().filter(|n| n["id"] == layer).cloned().collect::<Vec<_>>());
    let _ = copy;
    ok(h, "draw.content.add", json!({ "drawingId": D1, "content": content, "offset": {"x": 0, "y": -200} }));
}

/// The canonical entries and the derived planar graph of every drawing, normalised as in `c21_determinism`.
fn state(h: &ExecutorHandle) -> String {
    let doc = h.snapshot().unwrap().unwrap();
    let mut out = String::new();
    for key in doc.entry_keys() {
        let mut v = doc.entry_raw_json(&key).unwrap();
        if let Some(m) = v.as_object_mut() {
            m.remove("modifiedAt");
            if m.get("type").and_then(Value::as_str) == Some("drawing")
                && m.get("rev").and_then(Value::as_u64) != Some(1)
            {
                m.insert("createdAt".into(), json!("<created by the script>"));
            }
        }
        out.push_str(&format!("{key:?}\n"));
        out.push_str(&tt_model::canonical::to_canonical_string(&v).unwrap());
    }
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "include": ["zones"] }));
    out.push_str("\nzones\n");
    out.push_str(&tt_model::canonical::to_canonical_string(&d["nodes"]).unwrap());
    // Membership: the zone under every lattice point of the painted area.
    out.push_str("\nmembership\n");
    for y in (90..=430).step_by(10) {
        for x in (280..=730).step_by(10) {
            let hit = ok(h, "draw.hitTest.get", json!({ "drawingId": D1, "point": {"x": x, "y": y}, "tolerance": 0 }));
            out.push_str(hit["zoneId"].as_str().unwrap_or("-"));
            out.push(' ');
        }
        out.push('\n');
    }
    out
}

#[test]
fn planar_graph_side_assignments_zone_ids_fills_and_membership_are_identical_across_runs_and_platforms() {
    let a = fixture();
    script(&a);
    let b = fixture();
    script(&b);
    let sa = state(&a);
    assert_eq!(sa, state(&b), "two executors of one run");
    let digest = tt_persist::fsx::sha256(sa.as_bytes());
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/c22-determinism.json");
    let zones = ok(&a, "draw.drawing.get", json!({ "drawingId": D1, "include": ["zones"] }));
    if std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        let text =
            serde_json::to_string_pretty(&json!({ "sha256": digest, "bytes": sa.len(), "nodes": zones["nodes"] }))
                .unwrap()
                + "\n";
        std::fs::write(&golden, text).unwrap();
        return;
    }
    let g: Value = serde_json::from_str(&std::fs::read_to_string(&golden).unwrap()).unwrap();
    if g["sha256"] != digest.as_str() {
        assert_eq!(zones["nodes"], g["nodes"], "the planar graph differs from the golden recording");
        panic!("canonical state digest {digest} differs from the golden {}", g["sha256"]);
    }
}
