//! Independent round-two closure variants. No product or predecessor test edits.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const UNUSED: &str = "dr_01K742SG0NG5HTFZRD7238FZXJ";
const BOB: &str = "scene:#1/element:\"Bob\"";
fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string()))
        .collect()
}
fn drawing(h: &ExecutorHandle, d: &str) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":d}))
}
fn layer(h: &ExecutorHandle, d: &str, id: &Value) -> Value {
    drawing(h, d)["nodes"].as_array().unwrap().iter().find(|n| n["id"] == *id).unwrap().clone()
}
fn rect(h: &ExecutorHandle, d: &str, colour: &str) -> Value {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({"drawingId":d,"from":{"x":2000,"y":2000},"to":{"x":2100,"y":2060},"thickness":0,"layer":"new"}),
    );
    ok(h, "colour.paint.apply", json!({"drawingId":d,"objectIds":[r["zoneIds"][0]],"colourId":colour}));
    r
}
fn line(h: &ExecutorHandle, from: [i32; 2], to: [i32; 2], l: Value) -> Value {
    ok(h, "draw.line.add", json!({"drawingId":D,"from":{"x":from[0],"y":from[1]},"to":{"x":to[0],"y":to[1]},"layer":l}))
}
fn anchors(l: &Value) -> Vec<(f64, f64)> {
    l["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["bezier"].as_array().unwrap().iter().step_by(3))
        .map(|p| (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap()))
        .collect()
}
fn pixels(h: &ExecutorHandle) -> Value {
    ok(
        h,
        "draw.drawing.render",
        json!({"drawingId":D,"width":400,"height":200,"region":{"x":1950,"y":1950,"width":400,"height":200}}),
    )["pixelsSha256"]
        .clone()
}
#[test]
fn r01_reposition_two_drawings_reflection_and_undo_redo() {
    let h = fixture();
    let c = colours(&h);
    let a = rect(&h, D, &c[1]);
    let b = rect(&h, UNUSED, &c[2]);
    let before_a = layer(&h, D, &a["layerId"])["zones"][0].clone();
    let before_b = layer(&h, UNUSED, &b["layerId"])["zones"][0].clone();
    ok(&h, "draw.repositionAll.apply", json!({"elementId":BOB,"matrix":[-1,0,0,1,5000,0]}));
    for (d, r, want, old) in [(D, &a, &c[1], &before_a), (UNUSED, &b, &c[2], &before_b)] {
        let z = layer(&h, d, &r["layerId"])["zones"][0].clone();
        assert_eq!(z["colourId"], *want);
        let m: Vec<f64> = old["textureMatrix"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let actual: Vec<f64> = z["textureMatrix"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert_eq!(actual, vec![-m[0], m[1], -m[2], m[3], 5000.0 - m[4], m[5]]);
    }
    ok(&h, "edit.undo", json!({}));
    assert_eq!(layer(&h, D, &a["layerId"])["zones"][0], before_a);
    assert_eq!(layer(&h, UNUSED, &b["layerId"])["zones"][0], before_b);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(layer(&h, D, &a["layerId"])["zones"][0]["colourId"], c[1]);
}
#[test]
fn r01_non_parallelogram_envelope_uses_three_corner_fill_fit() {
    let h = fixture();
    let c = colours(&h);
    let r = rect(&h, D, &c[1]);
    ok(
        &h,
        "draw.envelope.apply",
        json!({"drawingId":D,"ids":[r["layerId"]],"corners":[{"x":2500,"y":2000},{"x":2620,"y":2000},{"x":2600,"y":2080},{"x":2500,"y":2060}]}),
    );
    let z = layer(&h, D, &r["layerId"])["zones"][0].clone();
    assert_eq!(z["colourId"], c[1]);
    println!("three-corner matrix: {}", z["textureMatrix"]);
    assert_eq!(z["textureMatrix"], json!([120, 0, 0, 60, 2500, 2000]));
}
#[test]
fn r02_rotation_pivot_is_requested_subset_and_other_layer_stays() {
    let h = fixture();
    let a = line(&h, [2000, 2000], [2050, 2000], json!("new"));
    line(&h, [2050, 2000], [2050, 2060], a["layerId"].clone());
    let other = line(&h, [2050, 2000], [2100, 2000], json!("new"));
    let outside = layer(&h, D, &other["layerId"]);
    let l = layer(&h, D, &a["layerId"]);
    let sid =
        l["strokes"].as_array().unwrap().iter().find(|s| s["bezier"][0]["x"].as_f64() == Some(2000.0)).unwrap()["id"]
            .clone();
    // Two requested endpoints give pivot (2025,2000). The incident vertical endpoint must follow to (2025,2025).
    ok(
        &h,
        "draw.transform.apply",
        json!({"drawingId":D,"points":[{"strokeId":sid,"anchor":0},{"strokeId":sid,"anchor":1}],"rotate":90}),
    );
    let pts = anchors(&layer(&h, D, &a["layerId"]));
    assert!(pts.iter().filter(|p| **p == (2025.0, 2025.0)).count() >= 2, "{pts:?}");
    assert!(!pts.contains(&(2050.0, 2000.0)));
    assert_eq!(layer(&h, D, &other["layerId"]), outside);
}
#[test]
fn r03_snap_radius_boundary_and_cross_layer_exclusion() {
    for (distance, same, want) in [(8, true, 2000.0), (9, true, 2009.0), (5, false, 2005.0)] {
        let h = fixture();
        let a = line(&h, [2000, 2000], [2100, 2000], json!("new"));
        let b = line(&h, [2050, 2020], [2050, 2080], if same { a["layerId"].clone() } else { json!("new") });
        ok(
            &h,
            "draw.point.move",
            json!({"drawingId":D,"points":[{"strokeId":b["strokeIds"][0],"anchor":0}],"delta":{"x":0,"y":distance-20},"snap":true}),
        );
        assert!(anchors(&layer(&h, D, &b["layerId"])).contains(&(2050.0, want)), "distance {distance}, same {same}");
    }
}
#[test]
fn r03_equal_distance_contours_choose_lower_stroke_id() {
    let h = fixture();
    let a = line(&h, [2000, 2000], [2100, 2000], json!("new"));
    let b = line(&h, [2000, 2016], [2100, 2016], a["layerId"].clone());
    let moving = line(&h, [2050, 2040], [2050, 2080], a["layerId"].clone());
    let want = if a["strokeIds"][0].as_str().unwrap() < b["strokeIds"][0].as_str().unwrap() { 2000.0 } else { 2016.0 };
    ok(
        &h,
        "draw.point.move",
        json!({"drawingId":D,"points":[{"strokeId":moving["strokeIds"][0],"anchor":0}],"delta":{"x":0,"y":-32},"snap":true}),
    );
    assert!(anchors(&layer(&h, D, &a["layerId"])).contains(&(2050.0, want)));
}
#[test]
fn r04_disjoint_thick_caps_merge_with_identical_pixels() {
    let h = fixture();
    let c = colours(&h);
    let a = ok(
        &h,
        "draw.line.add",
        json!({"drawingId":D,"from":{"x":2000,"y":2000},"to":{"x":2050,"y":2000},"thickness":20,"colourId":c[0],"layer":"new"}),
    );
    let b = ok(
        &h,
        "draw.line.add",
        json!({"drawingId":D,"from":{"x":2071,"y":2000},"to":{"x":2120,"y":2000},"thickness":20,"colourId":c[4],"layer":"new"}),
    );
    let before = pixels(&h);
    let r = ok(&h, "draw.optimize.apply", json!({"drawingId":D,"ids":[a["layerId"],b["layerId"]]}));
    assert_eq!(r["merged"], 1, "{r}");
    assert_eq!(pixels(&h), before);
}
#[test]
fn r05_closed_texture_pencil_conversion_clears_centre_and_keeps_mapping() {
    let h = fixture();
    let c = colours(&h);
    let r = rect(&h, D, &c[0]);
    ok(&h, "colour.selection.apply", json!({"drawingId":D,"objectIds":r["strokeIds"],"colourId":c[3]}));
    let m = json!([100.0, 10.0, 20.0, -60.0, 2000.0, 2060.0]);
    ok(&h, "colour.fillMapping.set", json!({"drawingId":D,"objectIds":r["strokeIds"],"mapping":{"matrix":m}}));
    let out = ok(&h, "draw.linesToBrush.apply", json!({"drawingId":D,"ids":r["strokeIds"]}));
    let l = layer(&h, D, &r["layerId"]);
    for s in l["strokes"].as_array().unwrap() {
        if s["kind"] == "invisible" {
            assert!(s["fillMatrix"].is_null());
            assert!(s["colourId"].is_null());
        }
    }
    for s in out["strokeIds"].as_array().unwrap() {
        assert_eq!(ok(&h, "colour.fillMapping.get", json!({"drawingId":D,"objectId":s}))["matrix"], m);
    }
}
#[test]
fn r09_explicit_stale_and_unknown_targets_are_atomic_across_routes() {
    let h = fixture();
    let c = colours(&h);
    let r = rect(&h, D, &c[1]);
    for bad in ["zn_0000000000000000000000000Z", "st_0000000000000000000000000Z"] {
        for mixed in [false, true] {
            let ids = if mixed { json!([r["strokeIds"][0], bad]) } else { json!([bad]) };
            for (cmd, mut p) in [
                ("draw.transform.apply", json!({"ids":ids,"translate":{"x":2,"y":0}})),
                ("draw.smooth.apply", json!({"ids":ids})),
                ("draw.arrange.apply", json!({"ids":ids,"to":"front"})),
                ("colour.fillMapping.set", json!({"objectIds":ids,"mapping":{"matrix":[10,0,0,10,2000,2000]}})),
                ("colour.selection.apply", json!({"objectIds":ids,"colourId":c[0]})),
            ] {
                p["drawingId"] = json!(D);
                let before = drawing(&h, D);
                let v = rev(&h);
                let e = run(&h, cmd, p);
                let code = if bad.starts_with("zn_") { "colour.staleZoneId" } else { "drawing.objectNotFound" };
                assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some(code), "{cmd} {bad}: {e:?}");
                assert_eq!(rev(&h), v);
                assert_eq!(drawing(&h, D), before);
            }
        }
    }
}
#[test]
fn r10_all_shapes_empty_cell_create_or_refuse_atomically() {
    for cmd in ["draw.rectangle.add", "draw.ellipse.add", "draw.line.add", "draw.polyline.add"] {
        let h = fixture();
        ok(
            &h,
            "doc.object.patch",
            json!({"targets":[BOB],"ops":[{"op":"replace","path":"/exposure","value":[[null,24]]}]}),
        );
        let mut p = if cmd == "draw.polyline.add" {
            json!({"points":[{"x":2000,"y":2000},{"x":2100,"y":2060}],"snap":true})
        } else {
            json!({"from":{"x":2000,"y":2000},"to":{"x":2100,"y":2060}})
        };
        p["elementId"] = json!(BOB);
        p["frame"] = json!(1);
        p["createDrawing"] = json!(false);
        let v = rev(&h);
        let e = run(&h, cmd, p.clone());
        assert!(!e.is_ok(), "{cmd}: {e:?}");
        assert_eq!(rev(&h), v);
        p["createDrawing"] = json!(true);
        let r = ok(&h, cmd, p);
        assert!(r["drawingId"].as_str().is_some());
        assert_eq!(rev(&h), v + 1);
    }
}
#[test]
fn r11_partial_text_is_reported_and_eraser_leaves_text() {
    let h = fixture();
    let text = ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":2000,"y":2000},"text":"ABCDEFGHIJ"}));
    let before = get(&h, text["textId"].as_str().unwrap());
    let r = ok(
        &h,
        "draw.cut.apply",
        json!({"drawingId":D,"rect":{"x":1999,"y":1999,"width":5,"height":100},"then":"delete"}),
    );
    assert!(
        r["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["objectId"] == text["textId"] && s["reason"] == "notClippable"),
        "{r}"
    );
    assert_eq!(get(&h, text["textId"].as_str().unwrap()), before);
    let _ = run(
        &h,
        "draw.erase.apply",
        json!({"drawingId":D,"region":[{"x":1900,"y":1900},{"x":2300,"y":1900},{"x":2300,"y":2200},{"x":1900,"y":2200}]}),
    );
    assert_eq!(get(&h, text["textId"].as_str().unwrap()), before);
}
#[test]
fn r12_painted_zone_warning_survives_drawing_selection() {
    let h = fixture();
    let c = colours(&h);
    let r = rect(&h, D, &c[0]);
    let selected = ok(&h, "draw.selection.set", json!({"drawingId":D,"ids":[r["zoneIds"][0]]}));
    println!("selection result {selected}; subsequent read {}", ok(&h, "draw.selection.get", json!({"drawingId":D})));
    let v = rev(&h);
    assert_eq!(err(&h, "draw.centreLine.apply", json!({"drawingId":D})).code, "drawing.centreLineOnFill");
    assert_eq!(rev(&h), v);
}
#[test]
fn r13_protected_colour_outside_filter_does_not_cause_false_refusal() {
    let h = fixture();
    let c = colours(&h);
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[c[4]],"ops":[{"op":"add","path":"/ext","value":{"dev.toontown.paint":{"protected":true}}}]}),
    );
    // Ink is present in text; choose only zones, and map an unused radial colour too.
    let v = rev(&h);
    let r = ok(
        &h,
        "colour.recolour.apply",
        json!({"map":[{"from":c[4],"to":c[0]},{"from":c[2],"to":c[0]}],"scope":"drawing","drawingIds":[UNUSED],"targets":["zones"]}),
    );
    assert_eq!(r["skippedProtected"], json!([]));
    assert_eq!(rev(&h), v);
}
#[test]
fn r16_locked_exposed_is_skipped_but_element_reaches_unexposed() {
    let h = fixture();
    let c = colours(&h);
    for d in [D, UNUSED] {
        ok(
            &h,
            "draw.line.add",
            json!({"drawingId":d,"from":{"x":2000,"y":2000},"to":{"x":2100,"y":2000},"colourId":c[0],"layer":"new"}),
        );
    }
    ok(&h, "draw.lock.set", json!({"drawingId":D,"locked":true}));
    let before = drawing(&h, D);
    let unexposed = drawing(&h, UNUSED);
    let r = ok(
        &h,
        "colour.recolour.apply",
        json!({"fromColourId":c[0],"toColourId":c[1],"scope":"scene","targets":["lines"]}),
    );
    assert!(r["skippedLocked"].as_array().unwrap().contains(&json!(D)));
    assert_eq!(drawing(&h, D), before);
    assert_eq!(drawing(&h, UNUSED), unexposed);
    let r = ok(
        &h,
        "colour.recolour.apply",
        json!({"fromColourId":c[0],"toColourId":c[1],"scope":"element","elementIds":[BOB],"targets":["lines"]}),
    );
    assert!(r["changed"].as_array().unwrap().iter().any(|x| x["drawingId"] == UNUSED));
    assert_eq!(drawing(&h, D), before);
    assert_ne!(drawing(&h, UNUSED), unexposed);
}

#[test]
fn r10_each_shape_preserves_explicit_false_and_true_creates() {
    let mut failures = Vec::new();
    for cmd in ["draw.rectangle.add", "draw.ellipse.add", "draw.line.add", "draw.polyline.add"] {
        for create in [true, false] {
            let h = fixture();
            ok(
                &h,
                "doc.object.patch",
                json!({"targets":[BOB],"ops":[{"op":"replace","path":"/exposure","value":[[null,24]]}]}),
            );
            let mut p = if cmd == "draw.polyline.add" {
                json!({"points":[{"x":2000,"y":2000},{"x":2100,"y":2060}],"snap":true})
            } else {
                json!({"from":{"x":2000,"y":2000},"to":{"x":2100,"y":2060}})
            };
            p["elementId"] = json!(BOB);
            p["frame"] = json!(1);
            p["createDrawing"] = json!(create);
            let v = rev(&h);
            let r = run(&h, cmd, p);
            println!("{cmd} createDrawing={create}: status {:?}, revision {v} -> {}", r.status, rev(&h));
            if create {
                assert!(r.is_ok(), "{cmd}: {r:?}");
                assert_eq!(rev(&h), v + 1);
            } else if r.is_ok() || rev(&h) != v {
                failures.push(cmd);
            }
        }
    }
    assert!(failures.is_empty(), "commands that created despite explicit false: {failures:?}");
}

#[test]
fn r12_zone_selection_readback_must_keep_a_valid_derived_id() {
    let h = fixture();
    let c = colours(&h);
    let r = rect(&h, D, &c[0]);
    let requested = json!([r["zoneIds"][0]]);
    let set = ok(&h, "draw.selection.set", json!({"drawingId":D,"ids":requested}));
    assert_eq!(set["selectedIds"], requested);
    let read = ok(&h, "draw.selection.get", json!({"drawingId":D}));
    assert_eq!(read["selectedIds"], requested, "the selection must survive the call that sets it");
}
