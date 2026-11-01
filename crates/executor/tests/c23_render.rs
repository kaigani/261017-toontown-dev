//! M2 C2.3: `draw.drawing.render` (02 §13.5; ADR-0006): content order (fills, brush strokes, then lines; invisible
//! strokes undrawn; holes open), indirect colours through palette styles, gradients and textures through their
//! mapping (the tile axis, readiness item 14), determinism with committed pixel hashes (`goldens/c23_render.json`,
//! checked on every OS in CI), bounded inputs and file output.
//!
//! `TT_UPDATE_GOLDEN=1` rewrites the hash file and the PNGs in `reviews/evidence/M2-C2.3/goldens/`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::collections::BTreeMap;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

struct Img {
    w: usize,
    rgba: Vec<u8>,
    d2p: [f64; 6],
    sha: String,
    png: Vec<u8>,
}

impl Img {
    /// The straight-alpha pixel at drawing point (x, y).
    fn at(&self, x: f64, y: f64) -> [u8; 4] {
        let m = self.d2p;
        let (px, py) = (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let (i, j) = (px.floor() as usize, py.floor() as usize);
        let k = (j * self.w + i) * 4;
        [self.rgba[k], self.rgba[k + 1], self.rgba[k + 2], self.rgba[k + 3]]
    }
}

fn render(h: &ExecutorHandle, params: Value) -> Img {
    let r = ok(h, "draw.drawing.render", params);
    let png = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
    let (w, _, rgba) = tt_render::decode_png(&png, 8192).unwrap();
    let d2p: Vec<f64> = r["drawingToPixel"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    Img {
        w: usize::from(w),
        rgba,
        d2p: [d2p[0], d2p[1], d2p[2], d2p[3], d2p[4], d2p[5]],
        sha: r["pixelsSha256"].as_str().unwrap().to_owned(),
        png,
    }
}

fn rgba_of(hex: &str) -> [u8; 4] {
    tt_model::value::Colour::parse(hex).unwrap().channels()
}

/// The content scene: a painted ring (outer rectangle painted, inner rectangle a hole), a pencil line across it, an
/// invisible stroke and a brush stroke, away from the sample content.
fn content_scene(h: &ExecutorHandle) -> (String, String) {
    let c = colours(h);
    let outer = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 1000, "y": 1000}, "to": {"x": 1100, "y": 1080}, "layer": "new" }),
    );
    let lid = outer["layerId"].as_str().unwrap().to_owned();
    ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 1030, "y": 1020}, "to": {"x": 1070, "y": 1060}, "layer": lid }),
    );
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 1010, "y": 1010}], "colourId": c[0] }));
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "fit": false, "layer": lid, "thickness": 4, "colourId": c[4],
        "points": [{"x": 990, "y": 1040}, {"x": 1110, "y": 1040}] }),
    );
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "fit": false, "layer": lid,
        "points": [{"x": 1005, "y": 1070}, {"x": 1025, "y": 1070}] }),
    );
    ok(
        h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "kind": "brush",
        "points": [{"x": 1120, "y": 1000, "pressure": 1}, {"x": 1120, "y": 1080, "pressure": 1}],
        "pen": { "tipShape": "round", "minSize": 8, "maxSize": 8, "tipAngle": 0 } }),
    );
    (lid, c[0].clone())
}

const REGION: &str = r#"{"x": 980, "y": 990, "width": 160, "height": 100}"#;

fn region() -> Value {
    serde_json::from_str(REGION).unwrap()
}

#[test]
fn content_order_holes_invisible_strokes_and_determinism() {
    let h = fixture();
    let (_, skin) = content_scene(&h);
    let p = json!({ "drawingId": D1, "width": 320, "region": region(), "background": "#FFFFFFFF" });
    let img = render(&h, p.clone());
    let skin_px = {
        let doc = h.snapshot().unwrap().unwrap();
        let pal = doc.palettes().find(|p| p.swatches().iter().any(|s| s.colour_id().as_str() == skin)).unwrap();
        let sw = pal.swatches().iter().find(|s| s.colour_id().as_str() == skin).unwrap();
        match sw.values().get(pal.styles()[0].id()).unwrap() {
            tt_model::entities::palette::SwatchValue::Solid(s) => s.rgba.channels(),
            _ => panic!("skin is solid"),
        }
    };
    assert_eq!(img.at(1010.0, 1010.0), skin_px, "the painted ring");
    assert_eq!(img.at(1050.0, 1050.0), [255, 255, 255, 255], "the hole stays open");
    assert_eq!(img.at(985.0, 1070.0), [255, 255, 255, 255], "outside");
    let line = img.at(1015.0, 1040.0);
    assert_ne!(line, skin_px, "the line is drawn over the fill: {line:?}");
    assert_eq!(img.at(1015.0, 1070.0), skin_px, "the invisible stroke is not drawn");
    let brush = img.at(1120.0, 1040.0);
    assert_ne!(brush, [255, 255, 255, 255], "the brush stroke is filled");
    // Deterministic: the same call gives the same pixels and file.
    let again = render(&h, p);
    assert_eq!((again.sha.clone(), again.png.clone()), (img.sha.clone(), img.png.clone()));
    // Strokes shown: the invisible stroke appears.
    let shown = render(
        &h,
        json!({ "drawingId": D1, "width": 320, "region": region(), "background": "#FFFFFFFF", "showStrokes": true }),
    );
    let m = shown.at(1015.0, 1070.0);
    assert!(m[0] > 200 && m[1] < 160 && m[2] > 200, "magenta stroke aid: {m:?}");
}

#[test]
fn sizes_regions_padding_and_bounded_inputs() {
    let h = fixture();
    let r = ok(&h, "draw.drawing.render", json!({ "drawingId": D1, "width": 120, "region": "field12" }));
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(120), Some(90)), "4:3");
    assert_eq!(r["drawingToPixel"], json!([0.1, 0.0, 0.0, -0.1, 60.0, 45.0]));
    let r = ok(
        &h,
        "draw.drawing.render",
        json!({ "drawingId": D1, "width": 100, "height": 100, "region": "field16", "padding": 0 }),
    );
    assert_eq!(r["height"], 100, "fitted and centred");
    let cam = ok(&h, "draw.drawing.render", json!({ "drawingId": D1, "width": 64, "region": "camera" }));
    assert!(cam["height"].as_u64().unwrap() >= 1);
    for (p, ptr) in [
        (json!({ "drawingId": D1, "width": 0 }), "/width"),
        (json!({ "drawingId": D1, "width": 9000 }), "/width"),
        (json!({ "drawingId": D1, "width": 10, "height": 0 }), "/height"),
        (json!({ "drawingId": D1, "width": 10, "background": "red" }), "/background"),
        (json!({ "drawingId": D1, "width": 10, "region": {"x": 0, "y": 0, "width": 0, "height": 1} }), "/region"),
        (json!({ "drawingId": D1, "width": 10, "styleId": "nope" }), "/styleId"),
    ] {
        let e = run(&h, "draw.drawing.render", p.clone());
        let e = e.error.unwrap_or_else(|| panic!("{p} should fail"));
        assert!(e.details["pointer"] == ptr || e.code == "arg.invalid" || e.code == "arg.schema", "{p}: {e:?}");
    }
    // A file output.
    let dir = std::env::temp_dir().join(format!("tt-c23-render-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("d1.png");
    let r = ok(&h, "draw.drawing.render", json!({ "drawingId": D1, "width": 32, "path": path }));
    assert!(r["png"].is_null() && r["path"].is_string());
    let bytes = std::fs::read(&path).unwrap();
    assert!(tt_render::decode_png(&bytes, 64).is_some());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Imports a 3 x 2 texture whose six pixels all differ into the fabric swatch: the first style (by order) tiles at
/// native size, the second stretches; the style order is reversed against ID order.
fn texture_project(h: &ExecutorHandle) -> (String, [[u8; 4]; 6], String, String) {
    let px: [[u8; 4]; 6] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [0, 255, 255, 255],
        [255, 0, 255, 255],
    ];
    let png = tt_render::encode_png(&px.concat(), 3, 2).unwrap();
    // A new palette whose first style (by order) has the larger ID.
    let (first, second) = ("sty_01K9ZZZZZZZZZZZZZZZZZZZZZ3".to_owned(), "sty_01K9ZZZZZZZZZZZZZZZZZZZZZ2".to_owned());
    let colour = "col_01K9ZZZZZZZZZZZZZZZZZZZZZ4".to_owned();
    let palette = json!({
        "id": "pal_01K9ZZZZZZZZZZZZZZZZZZZZZ1", "type": "palette", "rev": 1, "name": "Axis", "hidden": false,
        "styles": [{"id": first, "type": "paletteStyle", "rev": 1, "name": "Tiled"},
                   {"id": second, "type": "paletteStyle", "rev": 1, "name": "Stretched"}],
        "swatches": [{"colourId": colour, "type": "swatch", "rev": 1, "name": "Axis", "kind": "texture", "deformable": true,
            "values": { first.clone(): {"image": "textures/axis.png", "tile": true, "width": 3, "height": 2},
                        second.clone(): {"image": "textures/axis.png", "tile": false, "width": 3, "height": 2} }}]
    });
    let bundle = json!({ "format": "animjson", "formatVersion": "1.0", "scenes": [], "elements": [], "drawingBanks": [],
        "palettes": [palette], "pens": [], "assets": [],
        "blobs": { "textures/axis.png": { "mediaType": "image/png", "base64": tt_persist::b64::encode(&png) } } });
    let imp = ok(h, "doc.import.json", json!({ "source": bundle, "idPolicy": "preserve", "onConflict": "fail" }));
    assert!(imp["issues"].as_array().unwrap().is_empty(), "{imp}");
    (colour, px, first, second)
}

#[test]
fn the_tile_axis_style_order_drag_and_transforms() {
    let h = fixture();
    let (fabric, px, _first, second) = texture_project(&h);
    let r = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 1000, "y": 1000}, "to": {"x": 1024, "y": 1016}, "layer": "new" }),
    );
    let zone = r["zoneIds"][0].clone();
    ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [zone], "colourId": fabric }));
    let reg = json!({"x": 1000, "y": 1000, "width": 24, "height": 16});
    let img = render(&h, json!({ "drawingId": D1, "width": 240, "region": reg }));
    // Tile on, native size: the image's top-left pixel sits at the zone's top-left corner, one image pixel per du,
    // repeating every 3 x 2 du (the stored default [3, 0, 0, -2, minX, maxY] agrees with what is drawn).
    let near = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).all(|(x, y)| (i16::from(*x) - i16::from(y)).abs() <= 32);
    for (k, want) in px.iter().enumerate() {
        let (i, j) = ((k % 3) as f64, (k / 3) as f64);
        // The tile two repeats right and two down from the top-left corner, clear of the outline.
        let got = img.at(1006.5 + i, 1011.5 - j);
        assert!(near(got, *want), "image pixel ({i}, {j}): got {got:?} want {want:?}");
        let tiled = img.at(1006.5 + i + 3.0, 1011.5 - j - 2.0);
        assert!(near(tiled, *want), "repeat of ({i}, {j}): {tiled:?}");
    }
    // The second style (later in order, smaller ID) stretches the image over the zone bounds.
    let stretched = render(&h, json!({ "drawingId": D1, "width": 240, "region": reg, "styleId": second }));
    assert_ne!(stretched.sha, img.sha, "styles resolve by order, the chosen one wins");
    // The matrix travels with the zone: a quarter turn of the painted rectangle turns the texture with it.
    ok(&h, "draw.rotate.apply", json!({ "drawingId": D1, "ids": [r["strokeIds"][0]], "by": "90ccw" }));
    // The box turns about its centre (1012, 1008): it now spans x 1004..1020, y 996..1020.
    let turned = render(
        &h,
        json!({ "drawingId": D1, "width": 160, "region": {"x": 1004, "y": 996, "width": 16, "height": 24} }),
    );
    let before: Vec<[u8; 4]> = (0..3).map(|k| img.at(1006.5 + f64::from(k), 1011.5)).collect();
    // A quarter turn counter-clockwise maps (x, y) to (2012 - y + 1008 - 1008, ...): (1006.5 + k, 1011.5) goes to
    // (1012 - (1011.5 - 1008), 1008 + (1006.5 + k - 1012)).
    let samples: Vec<[u8; 4]> =
        (0..3).map(|k| turned.at(1012.0 - 3.5, 1008.0 + (1006.5 + f64::from(k) - 1012.0))).collect();
    assert_eq!(samples.len(), before.len());
    for (a, b) in samples.iter().zip(&before) {
        assert!(near(*a, *b), "the texture turns with the zone: {samples:?} vs {before:?}");
    }
    assert!(samples.iter().all(|s| px.iter().any(|p| near(*s, *p))), "texture pixels after the turn: {samples:?}");
    // A drag along +x of 3 du: unit space is 3 x 3 du from the drag start, the image upright (first row at the top),
    // each image pixel 1 du wide and 1.5 du tall, repeated (SI-074).
    ok(&h, "edit.undo", json!({}));
    ok(
        &h,
        "colour.paint.apply",
        json!({ "drawingId": D1, "objectIds": [zone], "colourId": fabric,
        "mapping": {"from": {"x": 1000, "y": 1016}, "to": {"x": 1003, "y": 1016}} }),
    );
    let dragged = render(&h, json!({ "drawingId": D1, "width": 240, "region": reg }));
    for (k, want) in px.iter().enumerate() {
        let (i, j) = ((k % 3) as f64, (k / 3) as f64);
        let got = dragged.at(1006.5 + i, 1010.0 - 1.5 * j - 0.75);
        assert!(near(got, *want), "dragged image pixel ({i}, {j}): got {got:?} want {want:?}");
    }
    // Tile off: a swatch whose first style stretches; the default box mapping [24, 0, 0, -16, minX, maxY] shows the
    // image upright over the zone, each image pixel 8 x 8 du.
    let h2 = fixture();
    let stretch = texture_project_first_stretching(&h2);
    let r2 = ok(
        &h2,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 1000, "y": 1000}, "to": {"x": 1024, "y": 1016}, "layer": "new" }),
    );
    ok(&h2, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [r2["zoneIds"][0]], "colourId": stretch }));
    let off = render(&h2, json!({ "drawingId": D1, "width": 240, "region": reg }));
    for (k, want) in px.iter().enumerate() {
        let (i, j) = ((k % 3) as f64, (k / 3) as f64);
        let got = off.at(1004.0 + 8.0 * i, 1012.0 - 8.0 * j);
        assert!(near(got, *want), "stretched image pixel ({i}, {j}): got {got:?} want {want:?}");
    }
}

/// [`texture_project`] with a second palette whose only style stretches the same image (tile off): its colour.
fn texture_project_first_stretching(h: &ExecutorHandle) -> String {
    let (_, px, _, _) = texture_project(h);
    let png = tt_render::encode_png(&px.concat(), 3, 2).unwrap();
    let colour = "col_01K9ZZZZZZZZZZZZZZZZZZZZZ7".to_owned();
    let palette = json!({
        "id": "pal_01K9ZZZZZZZZZZZZZZZZZZZZZ5", "type": "palette", "rev": 1, "name": "Axis stretched", "hidden": false,
        "styles": [{"id": "sty_01K9ZZZZZZZZZZZZZZZZZZZZZ6", "type": "paletteStyle", "rev": 1, "name": "Stretched"}],
        "swatches": [{"colourId": colour, "type": "swatch", "rev": 1, "name": "Axis stretched", "kind": "texture", "deformable": true,
            "values": { "sty_01K9ZZZZZZZZZZZZZZZZZZZZZ6": {"image": "textures/axis.png", "tile": false, "width": 3, "height": 2} }}]
    });
    let bundle = json!({ "format": "animjson", "formatVersion": "1.0", "scenes": [], "elements": [], "drawingBanks": [],
        "palettes": [palette], "pens": [], "assets": [],
        "blobs": { "textures/axis.png": { "mediaType": "image/png", "base64": tt_persist::b64::encode(&png) } } });
    let imp = ok(h, "doc.import.json", json!({ "source": bundle, "idPolicy": "preserve", "onConflict": "fail" }));
    assert!(imp["issues"].as_array().unwrap().is_empty(), "{imp}");
    colour
}

/// The committed pixel hashes: the same on every OS (ADR-0006 §3).
#[test]
fn golden_hashes_hold_on_every_platform() {
    let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/c23_render.json");
    let png_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reviews/evidence/M2-C2.3/goldens");
    let mut got: BTreeMap<String, (String, Vec<u8>)> = BTreeMap::new();
    {
        let h = fixture();
        content_scene(&h);
        let a = render(&h, json!({ "drawingId": D1, "width": 320, "region": region(), "background": "#FFFFFFFF" }));
        got.insert("content".into(), (a.sha, a.png));
        let s = render(&h, json!({ "drawingId": D1, "width": 320, "region": region(), "showStrokes": true }));
        got.insert("content-strokes".into(), (s.sha, s.png));
    }
    {
        let h = fixture();
        let c = colours(&h);
        for (k, colour) in [c[1].clone(), c[2].clone()].iter().enumerate() {
            let x = 1000.0 + 120.0 * k as f64;
            ok(
                &h,
                "draw.rectangle.add",
                json!({ "drawingId": D1, "from": {"x": x, "y": 1000}, "to": {"x": x + 100.0, "y": 1060}, "layer": "new" }),
            );
            ok(
                &h,
                "colour.paint.apply",
                json!({ "drawingId": D1, "points": [{"x": x + 50.0, "y": 1030}], "colourId": colour }),
            );
        }
        let g = render(
            &h,
            json!({ "drawingId": D1, "width": 300, "region": {"x": 990, "y": 990, "width": 240, "height": 80}, "background": "#202020FF" }),
        );
        got.insert("gradients".into(), (g.sha, g.png));
    }
    {
        let h = fixture();
        let (fabric, _, _, second) = texture_project(&h);
        let r = ok(
            &h,
            "draw.rectangle.add",
            json!({ "drawingId": D1, "from": {"x": 1000, "y": 1000}, "to": {"x": 1012, "y": 1008}, "layer": "new" }),
        );
        ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [r["zoneIds"][0]], "colourId": fabric }));
        let reg = json!({"x": 999, "y": 999, "width": 14, "height": 10});
        let t = render(&h, json!({ "drawingId": D1, "width": 140, "region": reg }));
        got.insert("texture-tile".into(), (t.sha, t.png));
        // A drag along +x of 3 du: the image upright, repeated (SI-074).
        ok(
            &h,
            "colour.paint.apply",
            json!({ "drawingId": D1, "objectIds": [r["zoneIds"][0]], "colourId": fabric,
            "mapping": {"from": {"x": 1000, "y": 1008}, "to": {"x": 1003, "y": 1008}} }),
        );
        let g = render(&h, json!({ "drawingId": D1, "width": 140, "region": reg }));
        got.insert("texture-drag".into(), (g.sha, g.png));
        let _ = second;
    }
    {
        // Tile off: the default box mapping stretches the image over the zone, upright (SI-074).
        let h = fixture();
        let stretch = texture_project_first_stretching(&h);
        let r = ok(
            &h,
            "draw.rectangle.add",
            json!({ "drawingId": D1, "from": {"x": 1000, "y": 1000}, "to": {"x": 1012, "y": 1008}, "layer": "new" }),
        );
        ok(&h, "colour.paint.apply", json!({ "drawingId": D1, "objectIds": [r["zoneIds"][0]], "colourId": stretch }));
        let reg = json!({"x": 999, "y": 999, "width": 14, "height": 10});
        let s = render(&h, json!({ "drawingId": D1, "width": 140, "region": reg }));
        got.insert("texture-stretch".into(), (s.sha, s.png));
        // Tile off with a drag mapping smaller than the zone: transparent outside the image (03 §7.4; C23-R1-06).
        ok(
            &h,
            "colour.paint.apply",
            json!({ "drawingId": D1, "objectIds": [r["zoneIds"][0]], "colourId": stretch,
            "mapping": {"from": {"x": 1003, "y": 1006}, "to": {"x": 1007, "y": 1006}} }),
        );
        let t = render(&h, json!({ "drawingId": D1, "width": 140, "region": reg }));
        got.insert("texture-stretch-inset".into(), (t.sha, t.png));
    }
    let hashes: BTreeMap<String, String> = got.iter().map(|(k, (s, _))| (k.clone(), s.clone())).collect();
    if std::env::var("TT_UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::create_dir_all(golden_path.parent().unwrap()).unwrap();
        std::fs::write(&golden_path, serde_json::to_string_pretty(&hashes).unwrap() + "\n").unwrap();
        std::fs::create_dir_all(&png_dir).unwrap();
        for (k, (_, png)) in &got {
            std::fs::write(png_dir.join(format!("{k}.png")), png).unwrap();
        }
        return;
    }
    let want: BTreeMap<String, String> = serde_json::from_slice(&std::fs::read(&golden_path).unwrap()).unwrap();
    assert_eq!(hashes, want, "render pixels changed: a golden-gated change (ADR-0006 §5)");
    for (k, (_, png)) in &got {
        let committed = std::fs::read(png_dir.join(format!("{k}.png"))).unwrap();
        assert_eq!(&committed, png, "{k}.png");
    }
    let _ = rgba_of("#000000");
}
