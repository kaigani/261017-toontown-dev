//! Asymmetric texture samples follow rotation, flip and affine envelope for tile-on/off/drag.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
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

#[test]
fn asymmetric_texture_pixels_follow_all_three_transforms_and_mapping_modes() {
    for mode in ["tile", "off", "drag"] {
        for operation in ["rotate", "flip", "envelope"] {
            let h = fixture();
            let colour = if mode == "off" { texture_project_first_stretching(&h) } else { texture_project(&h).0 };
            let r = ok(
                &h,
                "draw.rectangle.add",
                json!({"drawingId":D1,"from":{"x":1000,"y":1000},"to":{"x":1024,"y":1016},"layer":"new"}),
            );
            let mut paint = json!({"drawingId":D1,"objectIds":[r["zoneIds"][0]],"colourId":colour});
            if mode == "drag" {
                paint["mapping"] = json!({"from":{"x":1000,"y":1016},"to":{"x":1003,"y":1016}});
            }
            ok(&h, "colour.paint.apply", paint);
            let before = render(
                &h,
                json!({"drawingId":D1,"width":480,"height":320,"region":{"x":1000,"y":1000,"width":24,"height":16}}),
            );
            let samples: Vec<_> = (0..6)
                .map(|k| {
                    let (x, y) = if mode == "off" {
                        (1004. + 8. * f64::from(k % 3), 1012. - 8. * f64::from(k / 3))
                    } else if mode == "drag" {
                        (1006.5 + f64::from(k % 3), 1009.25 - 1.5 * f64::from(k / 3))
                    } else {
                        (1006.5 + f64::from(k % 3), 1011.5 - f64::from(k / 3))
                    };
                    ((x, y), before.at(x, y))
                })
                .collect();
            let (id, params) = match operation {
                "rotate" => ("draw.rotate.apply", json!({"drawingId":D1,"ids":[r["layerId"]],"by":"90ccw"})),
                "flip" => ("draw.flip.apply", json!({"drawingId":D1,"ids":[r["layerId"]],"axis":"horizontal"})),
                _ => (
                    "draw.envelope.apply",
                    json!({"drawingId":D1,"ids":[r["layerId"]],"corners":[{"x":2000,"y":1500},{"x":2024,"y":1500},{"x":2032,"y":1516},{"x":2008,"y":1516}]}),
                ),
            };
            println!(
                "{mode}/{operation} source bounds={}",
                ok(&h, "draw.bounds.get", json!({"drawingId":D1,"ids":[r["layerId"]]}))
            );
            println!(
                "mapping before={}",
                ok(&h, "colour.fillMapping.get", json!({"drawingId":D1,"objectId":r["zoneIds"][0]}))
            );
            ok(&h, id, params);
            println!(
                "drawing after={}",
                ok(&h, "draw.drawing.get", json!({"drawingId":D1}))["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|n| n["id"] == r["layerId"])
                    .unwrap()
            );
            let region = if operation == "envelope" {
                json!({"x":1998,"y":1498,"width":36,"height":20})
            } else {
                json!({"x":998,"y":994,"width":28,"height":28})
            };
            let after = render(&h, json!({"drawingId":D1,"width":720,"height":720,"region":region}));
            for ((x, y), want) in samples {
                let (tx, ty) = match operation {
                    "rotate" => (2020. - y, x - 4.),
                    "flip" => (2024. - x, y),
                    _ => (2000. + (x - 999.) * 24. / 26. + (y - 999.) * 8. / 18., 1500. + (y - 999.) * 16. / 18.),
                };
                let got = after.at(tx, ty);
                assert!(
                    got.iter().zip(want).all(|(a, b)| (i16::from(*a) - i16::from(b)).abs() <= 32),
                    "{mode}/{operation} at {x},{y}: {got:?} vs {want:?}"
                );
            }
        }
    }
}
