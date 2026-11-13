//! Shared fixtures of the M3 C3.3 builder suites: painted squares on new drawing elements, an independent camera and
//! world-matrix oracle (05 §2.2, §2.6, §2.7, written from the spec, not from the product), and PNG read-back.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "test helpers")]

use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

use crate::common::*;

pub const INTRO: &str = "scene:\"Intro\"";
pub const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
pub const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
pub const CAMERA: &str = "scene:\"Intro\"/element:\"Camera\"";
pub const BACKGROUND: &str = "scene:\"Intro\"/element:\"Background\"";

/// Hides the fixture's content elements so a test sees only what it builds (the annotation is view-only anyway).
pub fn clear(h: &ExecutorHandle) {
    for e in [BOB, BACKGROUND] {
        ok(h, "element.visibility.set", json!({ "elementIds": [e], "scope": "scene", "visible": false }));
    }
}

/// Colour IDs of the document's swatches, in palette order.
pub fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

/// The solid value of a colour through the Intro scene's active style for its palette (03 §4).
pub fn solid(h: &ExecutorHandle, colour: &str) -> [u8; 4] {
    let doc = h.snapshot().unwrap().unwrap();
    let scene = doc.scenes().find(|s| s.name() == "Intro").unwrap();
    for pal in doc.palettes() {
        if let Some(sw) = pal.swatches().iter().find(|s| s.colour_id().as_str() == colour) {
            let style = scene.palette_styles().get(pal.id()).cloned().unwrap_or_else(|| pal.styles()[0].id().clone());
            if let Some(tt_model::entities::palette::SwatchValue::Solid(s)) = sw.values().get(&style) {
                return s.rgba.channels();
            }
        }
    }
    panic!("no solid colour {colour}");
}

/// The first solid colours of the document (opaque ones), at least `n`.
pub fn solids(h: &ExecutorHandle, n: usize) -> Vec<(String, [u8; 4])> {
    let out: Vec<(String, [u8; 4])> = colours(h)
        .into_iter()
        .filter_map(|c| {
            let doc = h.snapshot().unwrap().unwrap();
            let scene = doc.scenes().find(|s| s.name() == "Intro").unwrap();
            doc.palettes().find_map(|pal| {
                let sw = pal.swatches().iter().find(|s| s.colour_id().as_str() == c)?;
                let style = scene.palette_styles().get(pal.id()).cloned()?;
                match sw.values().get(&style)? {
                    tt_model::entities::palette::SwatchValue::Solid(s) if s.rgba.channels()[3] == 255 => {
                        Some((c.clone(), s.rgba.channels()))
                    }
                    _ => None,
                }
            })
        })
        .collect();
    assert!(out.len() >= n, "the fixture has {} opaque solid colours", out.len());
    out
}

/// A new drawing element `name` showing, from frame 1 to 48, a square of side `2 * half` du centred on its origin,
/// filled with `colour`. Returns the element ID.
pub fn square(h: &ExecutorHandle, name: &str, half: f64, colour: &str) -> String {
    let id = ok(h, "element.create", json!({ "type": "drawing", "name": name, "sceneId": INTRO }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(
        h,
        "draw.rectangle.add",
        json!({ "elementId": id, "frame": 1, "from": {"x": -half, "y": -half}, "to": {"x": half, "y": half}, "layer": "new" }),
    );
    let d = drawing_at(h, &id, 1);
    ok(h, "colour.paint.apply", json!({ "drawingId": d, "points": [{"x": 0, "y": 0}], "colourId": colour }));
    let name = get(h, &d)["name"].as_str().unwrap().to_owned();
    ok(
        h,
        "xsheet.exposures.set",
        json!({ "columns": [{ "elementId": id, "from": 1, "to": 48, "exposures": [{ "drawing": name, "from": 1, "to": 48 }] }] }),
    );
    id
}

/// The drawing an element shows at a frame.
pub fn drawing_at(h: &ExecutorHandle, element: &str, frame: u32) -> String {
    let doc = h.snapshot().unwrap().unwrap();
    let id = tt_model::Id::parse(element).unwrap();
    let e = doc.element(&id).unwrap();
    let mut at = 1u32;
    for (d, n) in e.exposure().as_ref().unwrap() {
        if frame < at + n {
            return d.as_ref().unwrap().as_str().to_owned();
        }
        at += n;
    }
    panic!("no drawing at frame {frame}");
}

/// A decoded render.
pub struct Img {
    pub w: u32,
    pub h: u32,
    /// Straight RGBA8.
    pub rgba: Vec<u8>,
    pub png: Vec<u8>,
    pub result: Value,
}

impl Img {
    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let k = ((y * self.w + x) * 4) as usize;
        [self.rgba[k], self.rgba[k + 1], self.rgba[k + 2], self.rgba[k + 3]]
    }
}

pub fn decode(result: Value) -> Img {
    let png = tt_persist::b64::decode(result["data"].as_str().or(result["png"].as_str()).unwrap()).unwrap();
    let (w, h, rgba) = tt_render::decode_png(&png, 8192).unwrap();
    Img { w: u32::from(w), h: u32::from(h), rgba, png, result }
}

pub fn render(h: &ExecutorHandle, params: Value) -> Img {
    decode(ok(h, "render.frame.run", params))
}

// ----- independent oracle -------------------------------------------------------------------------------------------

/// 05 §2.7: the exact default FOV, written from the expression.
pub fn default_fov() -> f64 {
    2.0 * (0.375f64).atan().to_degrees()
}

/// A 2-D affine `[a, b, c, d, e, f]`: (x, y) ↦ (a x + c y + e, b x + d y + f), with a Z translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Local {
    pub m: [f64; 6],
    pub z: f64,
}

impl Local {
    /// 05 §2.2 with zero pivots: T(position) · R(rotation) · K(skew) · S(sx, sy).
    pub fn of(x: f64, y: f64, z: f64, rotation: f64, sx: f64, sy: f64, skew: f64) -> Self {
        let (c, s) = (rotation.to_radians().cos(), rotation.to_radians().sin());
        let k = skew.to_radians().tan();
        // R · K = [[c, c k - s], [s, s k + c]]; then · S.
        let (a, b) = (c * sx, s * sx);
        let (cc, d) = ((c * k - s) * sy, (s * k + c) * sy);
        Self { m: [a, b, cc, d, x, y], z }
    }

    /// `self · o` (o applied first).
    pub fn then(&self, o: &Local) -> Local {
        let [a, b, c, d, e, f] = self.m;
        let [a2, b2, c2, d2, e2, f2] = o.m;
        Local {
            m: [
                a * a2 + c * b2,
                b * a2 + d * b2,
                a * c2 + c * d2,
                b * c2 + d * d2,
                a * e2 + c * f2 + e,
                b * e2 + d * f2 + f,
            ],
            z: self.z + o.z,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.m;
        (a * x + c * y + e, b * x + d * y + f)
    }
}

/// 05 §2.6: a world point to output pixels through the default camera at (0, 0, 16), roll 0, the exact default FOV.
pub fn project(x: f64, y: f64, z: f64, w: f64, h: f64) -> Option<(f64, f64)> {
    let depth = 16.0 - z;
    if depth <= 0.1 {
        return None;
    }
    let t = (default_fov() / 2.0).to_radians().tan();
    let xn = x / (depth * t);
    let yn = y / (depth * t * h / w);
    Some(((xn + 1.0) / 2.0 * w, (1.0 - yn) / 2.0 * h))
}

/// Drawing units → pixels of an element with world transform `world` through the default camera.
pub fn du_to_pixel(world: &Local, du: (f64, f64), w: f64, h: f64) -> (f64, f64) {
    let (x, y) = world.apply(du.0 / 100.0, du.1 / 100.0);
    project(x, y, world.z, w, h).unwrap()
}

pub fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}
