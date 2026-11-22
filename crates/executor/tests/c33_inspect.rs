//! M3 C3.3 (builder; readiness items 5–8): `inspect.hitTest`, `inspect.coordinates.convert`,
//! `inspect.contactSheet.render` and `inspect.frame.describe` against independent oracles (`c33_support`) and against
//! the rendered pixels.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c33_support;
mod common;

use c33_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const W: f64 = 1920.0;
const H: f64 = 1080.0;

fn hits(h: &ExecutorHandle, space: &str, x: f64, y: f64) -> Vec<String> {
    ok(h, "inspect.hitTest", json!({ "space": space, "point": { "x": x, "y": y }, "frame": 1 }))["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| hit["elementId"].as_str().unwrap().to_owned())
        .collect()
}

/// Two overlapping squares: `back` at Z = 0, `front` rotated, mirrored and skewed at Z = 1. Returns their IDs and the
/// oracle world transforms.
fn overlap(h: &ExecutorHandle) -> (String, String, Local, Local) {
    clear(h);
    let c = solids(h, 2);
    let back = square(h, "Back", 100.0, &c[0].0);
    let front = square(h, "Front", 80.0, &c[1].0);
    ok(
        h,
        "anim.transform.set",
        json!({ "nodeIds": [front], "target": "static", "position": { "x": 1, "y": 0.5, "z": 1 }, "rotation": 25, "scale": { "x": -1.2, "y": 0.9 }, "skew": 15 }),
    );
    (back, front, Local::of(0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0), Local::of(1.0, 0.5, 1.0, 25.0, -1.2, 0.9, 15.0))
}

#[test]
fn hit_test_follows_transformed_geometry_and_depth() {
    let h = fixture();
    let (back, front, wb, wf) = overlap(&h);
    // Inside both: front first (it is drawn last, Z = 1).
    let both = du_to_pixel(&wf, (0.0, 0.0), W, H);
    let (bx, _) = du_to_pixel(&wb, (0.0, 0.0), W, H);
    assert!((both.0 - bx).abs() < 400.0, "the squares overlap");
    let got = hits(&h, "image", both.0, both.1);
    assert_eq!(got.first(), Some(&front), "{got:?}");
    // Points just inside and just outside the front square's corner, from the oracle's inverse mapping: 1 du inside a
    // corner is inside, 1 du outside is outside (the corner's neighbourhood leaves the back square).
    let inside = du_to_pixel(&wf, (79.0, 79.0), W, H);
    let outside = du_to_pixel(&wf, (85.0, 85.0), W, H);
    assert!(hits(&h, "image", inside.0, inside.1).contains(&front));
    assert!(!hits(&h, "image", outside.0, outside.1).contains(&front));
    // The back square alone, away from the front one.
    let only_back = du_to_pixel(&wb, (-95.0, -95.0), W, H);
    assert_eq!(hits(&h, "image", only_back.0, only_back.1), vec![back.clone()]);
    // Scene space (table plane) and camera space reach the same object.
    assert_eq!(hits(&h, "scene", -0.95, -0.95), vec![back.clone()]);
    let cam = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "image", "to": "camera", "points": [{ "x": only_back.0, "y": only_back.1 }] }),
    );
    let p = &cam["points"][0];
    assert_eq!(hits(&h, "camera", p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap()), vec![back.clone()]);
    // Hidden elements are not hit (SI-100), and the view space waits for M4.
    ok(&h, "element.visibility.set", json!({ "elementIds": [front], "scope": "scene", "visible": false }));
    assert!(!hits(&h, "image", both.0, both.1).contains(&front), "the hidden element is not hit");
    assert_eq!(hits(&h, "image", only_back.0, only_back.1), vec![back]);
    // View space arrived with M4 C4.2 (SI-109): the Drawing view's current drawing under a view point.
    let v = ok(&h, "inspect.hitTest", json!({ "space": "view", "point": { "x": 0, "y": 0 } }));
    assert!(v["hits"].is_array(), "{v}");
}

#[test]
fn hits_name_the_drawing_layer_object_and_colour() {
    let h = fixture();
    let (back, _, wb, _) = overlap(&h);
    let p = du_to_pixel(&wb, (-95.0, -95.0), W, H);
    let r = ok(&h, "inspect.hitTest", json!({ "space": "image", "point": { "x": p.0, "y": p.1 }, "frame": 1 }));
    let hit = &r["hits"][0];
    assert_eq!(hit["elementId"], back.as_str());
    assert_eq!(hit["drawingId"].as_str(), Some(drawing_at(&h, &back, 1).as_str()));
    assert_eq!(hit["kind"], "zone");
    assert_eq!(hit["colourId"].as_str(), Some(solids(&h, 1)[0].0.as_str()));
    assert!(hit["layerId"].is_string() && hit["objectId"].is_string());
}

#[test]
fn conversions_round_trip_and_match_the_oracle() {
    let h = fixture();
    let (_, front, _, wf) = overlap(&h);
    // Drawing → image matches the oracle; image → drawing returns the point.
    let du = [(10.0, -20.0), (79.0, 3.5), (-40.0, 55.25)];
    let pts: Vec<Value> = du.iter().map(|(x, y)| json!({ "x": x, "y": y })).collect();
    let img = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "drawing", "to": "image", "elementId": front, "points": pts.clone(), "frame": 1 }),
    );
    for (i, (x, y)) in du.iter().enumerate() {
        let want = du_to_pixel(&wf, (*x, *y), W, H);
        close(img["points"][i]["x"].as_f64().unwrap(), want.0, 1e-9, "x");
        close(img["points"][i]["y"].as_f64().unwrap(), want.1, 1e-9, "y");
    }
    // Back onto the element's plane (Z = 1): image → scene at z = 1 → drawing.
    let back: Vec<Value> =
        img["points"].as_array().unwrap().iter().map(|p| json!({ "x": p["x"], "y": p["y"], "z": 1.0 })).collect();
    let scene =
        ok(&h, "inspect.coordinates.convert", json!({ "from": "image", "to": "scene", "points": back, "frame": 1 }));
    let again = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "scene", "to": "drawing", "elementId": front, "points": scene["points"], "frame": 1 }),
    );
    for (i, (x, y)) in du.iter().enumerate() {
        close(again["points"][i]["x"].as_f64().unwrap(), *x, 1e-8, "round trip x");
        close(again["points"][i]["y"].as_f64().unwrap(), *y, 1e-8, "round trip y");
    }
    // Camera space: the frame is 12 fields wide on the table plane, so its corners are ±6 x ±3.375 fields (16:9).
    let corners = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "image", "to": "camera", "points": [{ "x": 0, "y": 0 }, { "x": W, "y": H }] }),
    );
    close(corners["points"][0]["x"].as_f64().unwrap(), -6.0, 1e-9, "left");
    close(corners["points"][0]["y"].as_f64().unwrap(), 3.375, 1e-9, "top");
    close(corners["points"][1]["x"].as_f64().unwrap(), 6.0, 1e-9, "right");
    close(corners["points"][1]["y"].as_f64().unwrap(), -3.375, 1e-9, "bottom");
    let norm = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "field", "to": "cameraNormalized", "points": [{ "x": 6, "y": -3.375 }, { "x": 0, "y": 0 }] }),
    );
    close(norm["points"][0]["x"].as_f64().unwrap(), 1.0, 1e-9, "u");
    close(norm["points"][0]["y"].as_f64().unwrap(), 1.0, 1e-9, "v");
    close(norm["points"][1]["x"].as_f64().unwrap(), 0.5, 1e-9, "centre u");
    // Local and parent spaces of an unparented node are its own matrix and the scene.
    let local = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "local", "to": "scene", "elementId": front, "points": [{ "x": 1, "y": 0 }] }),
    );
    let want = wf.apply(1.0, 0.0);
    close(local["points"][0]["x"].as_f64().unwrap(), want.0, 1e-12, "local x");
    let parent = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "parent", "to": "scene", "elementId": front, "points": [{ "x": 2, "y": 3, "z": 0.5 }] }),
    );
    assert_eq!(parent["points"][0], json!({ "x": 2.0, "y": 3.0, "z": 0.5 }));
}

#[test]
fn conversions_refuse_the_near_plane_views_and_missing_elements() {
    let h = fixture();
    let e = err(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "scene", "to": "image", "points": [{ "x": 0, "y": 0, "z": 0 }, { "x": 0, "y": 0, "z": 15.95 }] }),
    );
    assert_eq!((e.code.as_str(), &e.details["points"]), ("inspect.behindCamera", &json!([1])));
    assert_eq!(
        err(
            &h,
            "inspect.coordinates.convert",
            json!({ "from": "view", "to": "scene", "points": [{ "x": 0, "y": 0 }] })
        )
        .code,
        "arg.invalid",
        "view space arrived with M4 C4.2 (SI-109); through drawing space, scene space needs elementId"
    );
    assert_eq!(
        err(
            &h,
            "inspect.coordinates.convert",
            json!({ "from": "drawing", "to": "scene", "points": [{ "x": 0, "y": 0 }] })
        )
        .code,
        "arg.invalid"
    );
    assert_eq!(
        err(
            &h,
            "inspect.coordinates.convert",
            json!({ "from": "nowhere", "to": "scene", "points": [{ "x": 0, "y": 0 }] })
        )
        .code,
        "arg.invalid"
    );
}

#[test]
fn the_contact_sheet_is_the_frames_laid_out_with_labels() {
    let h = fixture();
    clear(&h);
    let (c, _) = solids(&h, 1)[0].clone();
    let a = square(&h, "Mover", 60.0, &c);
    ok(&h, "anim.transform.set", json!({ "nodeIds": [a], "frame": 1, "position": { "x": -3 } }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [a], "frame": 24, "position": { "x": 3 } }));
    let s =
        ok(&h, "inspect.contactSheet.render", json!({ "frames": { "from": 1, "to": 24, "step": 8 }, "cellWidth": 96 }));
    // 3 frames, 2 columns (ceil sqrt 3), cells 96 x 54 with a 14-pixel label band and 4-pixel gaps.
    assert_eq!((s["width"].as_u64(), s["height"].as_u64()), (Some(2 * 96 + 3 * 4), Some(2 * (54 + 14) + 3 * 4)));
    let cells = s["cells"].as_array().unwrap();
    assert_eq!(cells.iter().map(|c| c["frame"].as_u64().unwrap()).collect::<Vec<_>>(), vec![1, 9, 17]);
    assert_eq!((&cells[1]["x"], &cells[1]["y"]), (&json!(104), &json!(4)));
    assert_eq!((&cells[2]["x"], &cells[2]["y"]), (&json!(4), &json!(76)));
    // Each cell is exactly the frame rendered at the cell size.
    let sheet = decode(s.clone());
    for cell in cells {
        let f = cell["frame"].as_u64().unwrap();
        let one = render(&h, json!({ "frame": f, "width": 96, "height": 54 }));
        let (cx, cy) = (cell["x"].as_u64().unwrap() as u32, cell["y"].as_u64().unwrap() as u32);
        for y in 0..54 {
            for x in 0..96 {
                assert_eq!(sheet.at(cx + x, cy + y), one.at(x, y), "frame {f} at ({x}, {y})");
            }
        }
        // The label band holds dark digit pixels.
        let dark = (0..30)
            .flat_map(|x| (0..12).map(move |y| (x, y)))
            .filter(|(x, y)| sheet.at(cx + 2 + x, cy + 54 + 2 + y) == [0, 0, 0, 255])
            .count();
        assert!(dark > 0, "frame {f} has a label");
    }
    // Repeated bytes; no labels gives a shorter sheet.
    assert_eq!(
        ok(&h, "inspect.contactSheet.render", json!({ "frames": { "from": 1, "to": 24, "step": 8 }, "cellWidth": 96 }))
            ["sha256"],
        s["sha256"]
    );
    let plain = ok(
        &h,
        "inspect.contactSheet.render",
        json!({ "frames": [1, 9, 17], "cellWidth": 96, "labels": false, "columns": 3 }),
    );
    assert_eq!((plain["width"].as_u64(), plain["height"].as_u64()), (Some(3 * 96 + 4 * 4), Some(54 + 2 * 4)));
    assert_eq!(err(&h, "inspect.contactSheet.render", json!({ "frames": { "from": 5, "to": 1 } })).code, "arg.invalid");
}

#[test]
fn the_description_agrees_with_the_rendered_pixels() {
    let h = fixture();
    let (back, front, _, _) = overlap(&h);
    let d = ok(&h, "inspect.frame.describe", json!({ "frame": 1 }));
    let ids: Vec<&str> = d["elements"].as_array().unwrap().iter().map(|e| e["elementId"].as_str().unwrap()).collect();
    assert_eq!(ids, vec![back.as_str(), front.as_str()], "draw order");
    let img = render(&h, json!({ "frame": 1 }));
    let colours = solids(&h, 2);
    for (e, rgba) in d["elements"].as_array().unwrap().iter().zip([colours[0].1, colours[1].1]) {
        let b = &e["cameraBounds"]["pixels"];
        let (x, y, w, hh) = (
            b["x"].as_f64().unwrap(),
            b["y"].as_f64().unwrap(),
            b["width"].as_f64().unwrap(),
            b["height"].as_f64().unwrap(),
        );
        // Every drawn pixel of the element lies inside its bounds: the bounds' outside ring has no pixel of its colour
        // unless another element covers it.
        let ring =
            [(x - 2.0, y + hh / 2.0), (x + w + 2.0, y + hh / 2.0), (x + w / 2.0, y - 2.0), (x + w / 2.0, y + hh + 2.0)];
        for (px, py) in ring {
            if px >= 0.0 && py >= 0.0 && px < W && py < H {
                assert_ne!(img.at(px as u32, py as u32), rgba, "outside {}'s bounds at ({px}, {py})", e["name"]);
            }
        }
        // The normalized bounds are the pixel bounds over the size.
        close(e["cameraBounds"]["normalized"]["x"].as_f64().unwrap(), x / W, 1e-12, "normalized x");
    }
    // The front element's centre pixel shows its colour (it is on top).
    let fb = &d["elements"][1]["cameraBounds"]["pixels"];
    let centre = (
        fb["x"].as_f64().unwrap() + fb["width"].as_f64().unwrap() / 2.0,
        fb["y"].as_f64().unwrap() + fb["height"].as_f64().unwrap() / 2.0,
    );
    assert_eq!(img.at(centre.0 as u32, centre.1 as u32), colours[1].1);
    // Omitted elements: the hidden fixture content and the annotation.
    let omitted: Vec<&str> = d["omitted"].as_array().unwrap().iter().map(|e| e["reason"].as_str().unwrap()).collect();
    assert!(omitted.contains(&"hidden") && omitted.contains(&"annotation"), "{omitted:?}");
}
