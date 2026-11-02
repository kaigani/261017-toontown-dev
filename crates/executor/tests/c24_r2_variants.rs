//! Independent round-2 route variants beyond the straight isolated-line oracle corpus.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs,
    reason = "review tests"
)]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

fn drawing(h: &ExecutorHandle) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId": D, "include":["zones"]}))
}
fn layer(h: &ExecutorHandle, id: &Value) -> Value {
    drawing(h)["nodes"].as_array().unwrap().iter().find(|n| &n["id"] == id).unwrap().clone()
}
fn rect(h: &ExecutorHandle, x: i64, y: i64, w: i64, ht: i64, target: Value) -> Value {
    ok(
        h,
        "draw.rectangle.add",
        json!({"drawingId":D,"from":{"x":x,"y":y},
        "to":{"x":x+w,"y":y+ht},"layer":target}),
    )
}
fn palette(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

#[test]
fn optimize_declines_overlapping_layers_without_changing_geometry_or_fill() {
    let h = fixture();
    let a = rect(&h, 2000, 2000, 120, 90, json!("new"));
    let b = rect(&h, 2050, 2030, 120, 90, json!("new"));
    let cs = palette(&h);
    for (r, c) in [(&a, &cs[0]), (&b, &cs[1])] {
        let z = layer(&h, &r["layerId"])["zones"][0]["id"].clone();
        ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[z],"colourId":c}));
    }
    let before = drawing(&h)["nodes"].clone();
    let v = rev(&h);
    let r = ok(&h, "draw.optimize.apply", json!({"drawingId":D,"ids":[a["layerId"],b["layerId"]]}));
    assert_eq!(r["merged"], 0);
    assert_eq!(drawing(&h)["nodes"], before);
    assert_eq!(rev(&h), v);
}

#[test]
fn smooth_noop_and_shape_changing_tolerances_have_distinct_history() {
    let h = fixture();
    let a = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","fit":false,"layer":"new",
        "points":[{"x":2000,"y":2000},{"x":2020,"y":2005},{"x":2040,"y":2000},
                  {"x":2060,"y":2005},{"x":2080,"y":2000}]}),
    );
    let before = layer(&h, &a["layerId"]);
    let v = rev(&h);
    let low = ok(&h, "draw.smooth.apply", json!({"drawingId":D,"ids":a["strokeIds"],"tolerance":1}));
    assert_eq!(low["removedPointCount"], 0);
    assert_eq!(rev(&h), v);
    assert_eq!(layer(&h, &a["layerId"]), before);
    let high = ok(&h, "draw.smooth.apply", json!({"drawingId":D,"ids":a["strokeIds"],"tolerance":10}));
    assert!(high["removedPointCount"].as_u64().unwrap() > 0);
    let after = layer(&h, &a["layerId"]);
    assert_ne!(after["strokes"], before["strokes"]);
    // Independently bound all resulting controls by the original +/-5 du strip (convex hull of cubics).
    for p in after["strokes"][0]["bezier"].as_array().unwrap() {
        assert!((1995.0..=2010.0).contains(&p["y"].as_f64().unwrap()));
    }
    assert!(after["zones"].as_array().unwrap().is_empty());
    ok(&h, "edit.undo", json!({}));
    assert_eq!(layer(&h, &a["layerId"]), before);
}

#[test]
fn closed_concentric_loops_keep_two_painted_regions_through_lines_to_brush() {
    let h = fixture();
    let a = rect(&h, 2000, 2000, 120, 120, json!("new"));
    rect(&h, 2040, 2040, 40, 40, a["layerId"].clone());
    let cs = palette(&h);
    let before = layer(&h, &a["layerId"]);
    assert_eq!(before["zones"].as_array().unwrap().len(), 2);
    for (i, z) in before["zones"].as_array().unwrap().iter().enumerate() {
        ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[z["id"]],"colourId":cs[i]}));
    }
    let strokes =
        layer(&h, &a["layerId"])["strokes"].as_array().unwrap().iter().map(|s| s["id"].clone()).collect::<Vec<_>>();
    let r = ok(&h, "draw.linesToBrush.apply", json!({"drawingId":D,"ids":strokes}));
    assert_eq!(r["strokeIds"].as_array().unwrap().len(), 2);
    let after = layer(&h, &a["layerId"]);
    assert_eq!(after["zones"].as_array().unwrap().len(), 2);
    let mut fills = after["zones"]
        .as_array()
        .unwrap()
        .iter()
        .map(|z| z["colourId"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    fills.sort();
    let mut want = cs[..2].to_vec();
    want.sort();
    assert_eq!(fills, want);
    assert_eq!(after["strokes"].as_array().unwrap().iter().filter(|s| s["kind"] == "invisible").count(), 2);
}

#[test]
fn multiglyph_two_counter_text_and_spineless_centre_lines() {
    tt_vector::fonts::register_test_font(
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/fonts/B612-Regular.ttf"),
        )
        .unwrap(),
    );
    let h = fixture();
    let t = ok(
        &h,
        "draw.text.add",
        json!({"drawingId":D,"position":{"x":2000,"y":2000},"text":"BO8",
        "format":{"font":"B612","size":80}}),
    );
    let r = ok(&h, "draw.breakApart.apply", json!({"drawingId":D,"textIds":[t["textId"]]}));
    assert_eq!(r["groupIds"].as_array().unwrap().len(), 3);
    let all = drawing(&h);
    for (g, holes) in r["groupIds"].as_array().unwrap().iter().zip([2, 1, 2]) {
        let group = all["nodes"].as_array().unwrap().iter().find(|n| &n["id"] == g).unwrap();
        let glyph = &group["children"][0];
        assert_eq!(glyph["zones"].as_array().unwrap().len(), holes, "{glyph}");
        for z in glyph["zones"].as_array().unwrap() {
            assert_eq!(z["boundaryStrokeIds"], json!([glyph["strokes"][0]["id"]]));
            assert!(z["colourId"].is_null());
        }
    }
    let c = ok(&h, "draw.centreLine.apply", json!({"drawingId":D,"ids":r["groupIds"]}));
    assert!(c["skippedIds"].as_array().unwrap().is_empty(), "{c}");
    assert!(c["strokeIds"].as_array().unwrap().len() >= 3);
    println!("spineless BO8: {c}");
}

#[test]
fn snap_to_a_curved_contour_reaches_its_analytic_midpoint() {
    let h = fixture();
    // Cubic symmetric about x=2050, with midpoint (2050,2075); incoming tangent there is horizontal.
    let curve = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"layer":"new","kind":"pencil","fit":false,
        "bezier":[{"x":2000,"y":2000},{"x":2000,"y":2100},{"x":2100,"y":2100},{"x":2100,"y":2000}]}),
    );
    let line = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"layer":curve["layerId"],"kind":"pencil","fit":false,
        "points":[{"x":2050,"y":2095},{"x":2050,"y":2130}]}),
    );
    ok(
        &h,
        "draw.point.move",
        json!({"drawingId":D,"points":[{"strokeId":line["strokeIds"][0],"anchor":0}],
        "delta":{"x":0,"y":-15},"snap":true}),
    );
    let l = layer(&h, &curve["layerId"]);
    let anchors = l["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["bezier"].as_array().unwrap().iter().step_by(3))
        .collect::<Vec<_>>();
    assert!(
        anchors
            .iter()
            .filter(|p| (p["x"].as_f64().unwrap() - 2050.).abs() < 1. / 32.
                && (p["y"].as_f64().unwrap() - 2075.).abs() < 1. / 32.)
            .count()
            >= 2,
        "{l}"
    );
}

#[test]
fn non_parallelogram_envelope_has_independently_predicted_corners_and_area() {
    let h = fixture();
    let a = rect(&h, 2000, 2000, 100, 100, json!("new"));
    let b = ok(&h, "draw.bounds.get", json!({"drawingId":D,"ids":a["strokeIds"]}))["bounds"].clone();
    let (x0, y0, w, ht) = (
        b["x"].as_f64().unwrap(),
        b["y"].as_f64().unwrap(),
        b["width"].as_f64().unwrap(),
        b["height"].as_f64().unwrap(),
    );
    // Homography (u,v) -> (100u/(1+v),100v/(1+v)); three source corners plus bottom right form a trapezoid.
    let before = layer(&h, &a["layerId"]);
    let expected = before["strokes"][0]["bezier"]
        .as_array()
        .unwrap()
        .iter()
        .step_by(3)
        .map(|p| {
            let (u, v) = ((p["x"].as_f64().unwrap() - x0) / w, (p["y"].as_f64().unwrap() - y0) / ht);
            (((100. * u / (1. + v) * 64.).round() / 64.), ((100. * v / (1. + v) * 64.).round() / 64.))
        })
        .collect::<Vec<_>>();
    ok(
        &h,
        "draw.envelope.apply",
        json!({"drawingId":D,"ids":a["strokeIds"],
        "corners":[{"x":0,"y":0},{"x":100,"y":0},{"x":50,"y":50},{"x":0,"y":50}]}),
    );
    let after = layer(&h, &a["layerId"]);
    let area = (0..expected.len())
        .map(|i| {
            let p = expected[i];
            let q = expected[(i + 1) % expected.len()];
            p.0 * q.1 - p.1 * q.0
        })
        .sum::<f64>()
        .abs()
        / 2.;
    assert_eq!(after["zones"][0]["area"].as_f64().unwrap(), area, "{after}");
    assert_eq!(after["zones"].as_array().unwrap().len(), 1);
}

#[test]
fn erase_brush_outline_preserves_the_other_half_and_has_no_spurious_hole() {
    let h = fixture();
    let a = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"brush","layer":"new","fit":false,
        "points":[{"x":2000,"y":2000,"pressure":1},{"x":2100,"y":2000,"pressure":1}],
        "pen":{"tipShape":"round","minSize":10,"maxSize":10,"tipAngle":0}}),
    );
    let before = layer(&h, &a["layerId"]);
    ok(
        &h,
        "draw.erase.apply",
        json!({"drawingId":D,"layers":[a["layerId"]],
        "region":[{"x":2040,"y":1980},{"x":2060,"y":1980},{"x":2060,"y":2020},{"x":2040,"y":2020}]}),
    );
    let after = layer(&h, &a["layerId"]);
    assert_ne!(after["strokes"], before["strokes"]);
    assert_eq!(after["strokes"].as_array().unwrap().len(), 2);
    assert!(after["strokes"].as_array().unwrap().iter().all(|s| s["kind"] == "brush" && s["closed"] == true));
    assert!(after["zones"].as_array().unwrap().is_empty(), "{after}");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(layer(&h, &a["layerId"]), before);
}

#[test]
fn flatten_mixed_overlapping_pencil_and_brush_layers_has_two_exact_regions() {
    let h = fixture();
    let a = rect(&h, 2020, 2020, 60, 60, json!("new"));
    let c = palette(&h)[0].clone();
    let zid = layer(&h, &a["layerId"])["zones"][0]["id"].clone();
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[zid],"colourId":c}));
    let b = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"brush","layer":"new","fit":false,
        "points":[{"x":2000,"y":2050,"pressure":1},{"x":2100,"y":2050,"pressure":1}],
        "pen":{"tipShape":"round","minSize":10,"maxSize":10,"tipAngle":0}}),
    );
    let r = ok(&h, "draw.flatten.apply", json!({"drawingId":D,"ids":[a["layerId"],b["layerId"]]}));
    let l = layer(&h, &r["layerId"]);
    let zones = l["zones"].as_array().unwrap();
    assert_eq!(zones.len(), 2, "{l}");
    for z in zones {
        assert_eq!(z["area"].as_f64().unwrap(), 1500.0, "60 x 25 du independent expected area: {l}");
        assert_eq!(z["colourId"], c);
        assert!(z["boundaryStrokeIds"].as_array().unwrap().len() >= 2);
    }
}

#[test]
fn tiny_vectorized_ring_uses_generated_black_and_reuses_the_palette_after_current_colour_changes() {
    let h = fixture();
    let cs = palette(&h);
    ok(&h, "colour.current.set", json!({"colourId":cs[1]}));
    let dir = std::env::temp_dir().join(format!("tt-c24-r2-tiny-{}", tt_persist::fsx::random_token(8)));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("tiny-ring.png");
    let rgba: Vec<u8> = (0..9).flat_map(|i| if i == 4 { [255, 255, 255, 255] } else { [0, 0, 0, 255] }).collect();
    std::fs::write(&file, tt_render::encode_png(&rgba, 3, 3).unwrap()).unwrap();
    let mut black = None;
    for frame in [1, 2] {
        let start = ok(
            &h,
            "draw.vectorize.run",
            json!({"files":[file.to_string_lossy()],"targetElementId":"scene:#1/element:\"Bob\"",
            "startFrame":frame,"onConflict":"replace"}),
        );
        let rec = ok(&h, "job.wait", json!({"jobId":start["jobId"],"timeoutMs":60000}));
        let rec = rec.get("record").unwrap_or(&rec);
        assert_eq!(rec["state"], "succeeded", "{rec}");
        let doc = h.snapshot().unwrap().unwrap();
        let palettes = doc.palettes().filter(|p| p.name() == "Vectorization").collect::<Vec<_>>();
        assert_eq!(palettes.len(), 1);
        let id = palettes[0].swatches()[0].colour_id().as_str().to_owned();
        if let Some(want) = &black {
            assert_eq!(&id, want)
        } else {
            black = Some(id.clone())
        }
        assert_ne!(id, cs[1]);
        drop(doc);
        let d = &rec["result"]["drawingIds"][0];
        let art = ok(&h, "draw.drawing.get", json!({"drawingId":d}));
        assert_eq!(art["nodes"][0]["zones"].as_array().unwrap().len(), 1, "single enclosed paper cell: {art}");
        assert_eq!(art["nodes"][0]["strokes"][0]["colourId"], id);
        assert!(art["nodes"][0]["zones"][0]["colourId"].is_null());
        ok(&h, "colour.current.set", json!({"colourId":cs[0]}));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
