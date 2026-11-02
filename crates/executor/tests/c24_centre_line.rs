//! M2 C2.4 (ticket C23-T1, 02 §8.5): Extract Centre Line of spineless brush outlines through the command, on the E2
//! line art vectorized (a frame split by a line — a junction and two holes —, a ring — a hole —, a triangle): pencil
//! lines with the source colour and the average width replace the shapes, painted zones keep their fills (carry-over),
//! undo/redo restore, and the result survives save/open; an explicit thickness overrides the width; an outline past the
//! work budget is refused atomically before the watchdog.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::path::PathBuf;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const BOB: &str = "scene:#1/element:\"Bob\"";

fn art() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("vectorize")
        .join("e2-lineart.png")
        .to_string_lossy()
        .into_owned()
}

fn job(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    let started = ok(h, id, params);
    let r = ok(h, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60_000 }));
    if r.get("record").is_some() { r["record"].clone() } else { r }
}

fn vectorized(h: &ExecutorHandle) -> (String, Vec<String>) {
    ok(
        h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{"op": "replace", "path": "/exposure", "value": [[null, 24]]}] }),
    );
    let rec = job(h, "draw.vectorize.run", json!({ "files": [art()], "targetElementId": BOB, "startFrame": 1 }));
    let d = rec["result"]["drawingIds"][0].as_str().unwrap().to_owned();
    let nodes = ok(h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"].clone();
    let ids = nodes[0]["strokes"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_owned()).collect();
    (d, ids)
}

fn zone_at(h: &ExecutorHandle, d: &str, x: f64, y: f64) -> Option<(String, Option<String>)> {
    let r = ok(h, "draw.hitTest.get", json!({ "drawingId": d, "point": {"x": x, "y": y} }));
    r["hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["objectId"].as_str().is_some_and(|i| i.starts_with("zn_")))
        .map(|z| (z["objectId"].as_str().unwrap().to_owned(), z["colourId"].as_str().map(str::to_owned)))
}

#[test]
fn vectorized_line_art_becomes_pencil_lines_along_its_middle() {
    let h = fixture();
    let (d, shapes) = vectorized(&h);
    assert_eq!(shapes.len(), 3);
    let doc = h.snapshot().unwrap().unwrap();
    let black =
        doc.palettes().find(|p| p.name() == "Vectorization").unwrap().swatches()[0].colour_id().as_str().to_owned();
    drop(doc);
    // Paint the ring's inside (source pixel (70, 96) → drawing x = (70 − 128) × 1200 / 256, y = 0).
    let ring_x = (70.0 - 128.0) * 1200.0 / 256.0;
    let (ring_zone, _) = zone_at(&h, &d, ring_x, 0.0).expect("the ring's inside zone");
    let red = {
        let doc = h.snapshot().unwrap().unwrap();
        doc.palettes()
            .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
            .find(|c| *c != black)
            .unwrap()
    };
    ok(&h, "colour.paint.apply", json!({ "drawingId": d, "objectIds": [ring_zone], "colourId": red }));
    let before = ok(&h, "draw.drawing.get", json!({ "drawingId": d }));
    let v = rev(&h);
    let r = ok(&h, "draw.centreLine.apply", json!({ "drawingId": d, "ids": shapes }));
    assert!(r["skippedIds"].as_array().unwrap().is_empty(), "{r}");
    assert_eq!(rev(&h), v + 1, "one step");
    let after = ok(&h, "draw.drawing.get", json!({ "drawingId": d }));
    let strokes: Vec<Value> = after["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|n| n["strokes"].as_array().cloned().unwrap_or_default())
        .collect();
    assert!(
        strokes.iter().all(|s| s["kind"] == "pencil" && s["colourId"] == black.as_str()),
        "pencil lines in the source colour"
    );
    // The lines are 5 px wide in the source: 5 × 1200 / 256 ≈ 23 du.
    for s in &strokes {
        let w = s["lineStyle"]["thickness"].as_f64().unwrap();
        assert!((w - 23.4).abs() < 1.5, "width {w}");
    }
    // The frame (two holes, split by a line) gives three open lines between its two T junctions; the ring and the
    // triangle (one hole each) give one closed line each.
    let open: Vec<&Value> = strokes.iter().filter(|s| s["closed"] == false).collect();
    assert_eq!(open.len(), 3, "{open:?}");
    assert_eq!(strokes.iter().filter(|s| s["closed"] == true).count(), 2);
    let ends: Vec<(f64, f64)> = open
        .iter()
        .flat_map(|s| {
            let b = s["bezier"].as_array().unwrap();
            [&b[0], &b[b.len() - 1]].map(|p| (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap()))
        })
        .collect();
    // The split's centre is at source x = 128.5 → drawing x ≈ 2.3; the frame's bars at source y = 18.5 and 173.5.
    for (x, y) in &ends {
        assert!(
            (x - 2.3).abs() < 4.0 && (y.abs() - 358.0).abs() < 6.0,
            "every open line ends at a T junction: {x} {y}"
        );
    }
    // The ring's inside is still painted (carry-over into the zone the centre line now bounds).
    let (_, colour) = zone_at(&h, &d, ring_x, 0.0).expect("the ring's inside zone after");
    assert_eq!(colour.as_deref(), Some(red.as_str()));
    // Undo restores the shapes; redo the lines.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"], before["nodes"]);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"], after["nodes"]);
    // Save and open.
    let dir = std::env::temp_dir().join(format!("tt-c24-centre-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pkg = dir.join("C.animpkg");
    assert_eq!(
        job(&h, "project.saveAs", json!({ "path": pkg.to_string_lossy(), "onExists": "overwrite" }))["state"],
        "succeeded"
    );
    let h2 = fixture();
    assert_eq!(
        job(&h2, "project.open", json!({ "path": pkg.to_string_lossy(), "onUnsaved": "discard" }))["state"],
        "succeeded"
    );
    assert_eq!(ok(&h2, "draw.drawing.get", json!({ "drawingId": d }))["nodes"], after["nodes"]);
    drop(h2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_explicit_thickness_overrides_the_average_width() {
    let h = fixture();
    let (d, shapes) = vectorized(&h);
    let v = rev(&h);
    let r = ok(&h, "draw.centreLine.apply", json!({ "drawingId": d, "ids": [shapes[0]], "thickness": 5 }));
    assert_eq!(rev(&h), v + 1);
    let line = &ok(&h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"][0]["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "pencil")
        .cloned()
        .unwrap();
    assert_eq!(line["lineStyle"]["thickness"], 5.0, "{r}");
}

#[test]
fn an_outline_past_the_work_budget_is_refused_before_anything_is_staged() {
    // A closed zigzag of 12 000 corners around a circle of radius 2000: classifying its triangles alone needs more
    // than the centre line's work budget, so the command is refused with `limit.exceeded` (charged as it goes, so
    // well before the watchdog) and nothing changes.
    let h = fixture();
    let n = 12_000;
    let mut bezier = Vec::with_capacity(3 * n);
    for i in 0..n {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        let r = if i % 2 == 0 { 1990.0 } else { 2010.0 };
        let p = json!({ "x": r * a.cos(), "y": r * a.sin() });
        let a2 = std::f64::consts::TAU * ((i + 1) % n) as f64 / n as f64;
        let r2 = if (i + 1) % 2 == 0 { 1990.0 } else { 2010.0 };
        let q = json!({ "x": r2 * a2.cos(), "y": r2 * a2.sin() });
        bezier.extend([p.clone(), p, q]);
    }
    let colour = {
        let doc = h.snapshot().unwrap().unwrap();
        doc.palettes().next().unwrap().swatches()[0].colour_id().as_str().to_owned()
    };
    let added = ok(
        &h,
        "draw.content.add",
        json!({ "drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW", "select": false, "content": { "nodes": [{
        "type": "layer", "id": "l_z", "layerKind": "brush", "strokes": [{ "id": "s_z", "kind": "brush", "closed": true, "colourId": colour, "outline": [bezier] }] }] } }),
    );
    let id = added["idMap"]["s_z"].as_str().unwrap().to_owned();
    let v = rev(&h);
    let before = ok(&h, "draw.drawing.get", json!({ "drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW" }));
    let t = std::time::Instant::now();
    let e = err(&h, "draw.centreLine.apply", json!({ "drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW", "ids": [id] }));
    assert_eq!(e.code, "limit.exceeded", "{e:?}");
    assert!(t.elapsed().as_secs_f64() < 30.0, "refused before the watchdog: {:?}", t.elapsed());
    assert_eq!(rev(&h), v, "nothing staged");
    assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW" })), before);
}
