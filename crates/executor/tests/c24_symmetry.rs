//! M2 C2.4 readiness item 7 (ticket C21-R1-11): a cross-OS determinism corpus of command-created, non-identity
//! symmetry. For mirrors at several axis angles and kaleidoscopes of 2–16 mirrors at several angles, `tool.symmetry.set`
//! configures the mode and pencil lines, brush strokes, rectangles and lines are drawn with `symmetry: true`. Each copy
//! is checked against the exact map of the original (independent trigonometry), and the canonical drawing state of the
//! whole corpus is compared with the committed golden digest on every OS (CI: Windows and macOS `check`). Each
//! configuration also lays out a text object in the test-only fixture font (`tests/fixtures/fonts/B612-Regular.ttf`,
//! OFL; same-font layout controls, `c24_text_layout.rs`) and breaks it apart, so text bounds and glyph placements are
//! in the digest. `TT_UPDATE_GOLDEN=1` re-records it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};

const BOB: &str = "scene:#1/element:\"Bob\"";

/// Registers the fixture font (before the process first reads its font list).
fn register_fixture_font() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/fonts/B612-Regular.ttf");
        tt_vector::fonts::register_test_font(std::fs::read(path).unwrap());
    });
}

/// The configurations: mirrors at angles, kaleidoscopes with 2–16 mirrors.
fn configs() -> Vec<Value> {
    let mut v = Vec::new();
    for angle in [0.0, 30.0, 45.0, 90.0, 137.5] {
        v.push(json!({ "mode": "mirror", "angle": angle }));
    }
    for (mirrors, angle) in [(2, 0.0), (3, 15.0), (5, 22.5), (6, 0.0), (8, 15.0), (12, 7.5), (16, 3.0)] {
        v.push(json!({ "mode": "kaleidoscope", "angle": angle, "mirrors": mirrors }));
    }
    v
}

/// Copies a configuration makes of one stroke.
fn images(c: &Value) -> usize {
    if c["mode"] == "mirror" { 2 } else { 2 * c["mirrors"].as_u64().unwrap() as usize }
}

/// The exact images of a point (independent of the product's trigonometry): reflections about axes through the origin
/// at `angle + k·180°/n` from +y and rotations by `k·360°/n`.
fn mapped(c: &Value, p: (f64, f64)) -> Vec<(f64, f64)> {
    let a = c["angle"].as_f64().unwrap_or(0.0).to_radians();
    let reflect = |th: f64, (x, y): (f64, f64)| {
        // Axis direction at 90° + th from +x.
        let d = std::f64::consts::FRAC_PI_2 + th;
        let (c2, s2) = ((2.0 * d).cos(), (2.0 * d).sin());
        (c2 * x + s2 * y, s2 * x - c2 * y)
    };
    let rotate = |th: f64, (x, y): (f64, f64)| (th.cos() * x - th.sin() * y, th.sin() * x + th.cos() * y);
    if c["mode"] == "mirror" {
        return vec![p, reflect(a, p)];
    }
    let n = c["mirrors"].as_u64().unwrap() as f64;
    let mut out = Vec::new();
    for k in 0..n as u64 {
        out.push(rotate(2.0 * std::f64::consts::PI * k as f64 / n, p));
    }
    for k in 0..n as u64 {
        out.push(rotate(2.0 * std::f64::consts::PI * k as f64 / n, reflect(a, p)));
    }
    out
}

fn first_points(h: &tt_executor::ExecutorHandle, drawing: &str, ids: &[Value]) -> Vec<(f64, f64)> {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": drawing }));
    let mut out = Vec::new();
    for id in ids {
        let s = d["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|n| n["strokes"].as_array().cloned().unwrap_or_default())
            .find(|s| s["id"] == *id)
            .unwrap_or_else(|| panic!("stroke {id}"));
        let p = &s["bezier"][0];
        out.push((p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap()));
    }
    out
}

#[test]
fn command_created_mirrors_and_kaleidoscopes_are_exact_and_identical_across_platforms() {
    register_fixture_font();
    assert!(tt_vector::fonts::is_installed("B612"));
    let mut state = Vec::new();
    for c in configs() {
        let h = fixture();
        ok(
            &h,
            "doc.object.patch",
            json!({ "targets": [BOB], "ops": [{"op": "replace", "path": "/exposure", "value": [[null, 24]]}] }),
        );
        let r = ok(&h, "tool.symmetry.set", c.clone());
        assert_eq!(r["mode"], c["mode"]);
        // A pencil line (unfitted, so its first anchor is the input point) creates the drawing; then a brush stroke, a
        // rectangle and a line in the same drawing.
        let line = ok(
            &h,
            "draw.stroke.add",
            json!({ "elementId": BOB, "frame": 1, "kind": "pencil", "fit": false, "layer": "new",
            "points": [{"x": 130, "y": 40}, {"x": 190, "y": 70}, {"x": 210, "y": 140}] }),
        );
        let d = line["drawingId"].as_str().unwrap().to_owned();
        let ids = line["strokeIds"].as_array().unwrap().clone();
        // One stroke per image, more when copies cross one another (noding splits them into pieces).
        assert!(ids.len() >= images(&c), "{c}: {} strokes for {} images", ids.len(), images(&c));
        // Each copy starts at the exact image of the original's first point (as the product created them, in order).
        let got = first_points(&h, &d, &ids);
        let want = mapped(&c, (130.0, 40.0));
        for w in &want {
            assert!(
                got.iter().any(|g| (g.0 - w.0).abs() < 1e-6 && (g.1 - w.1).abs() < 1e-6),
                "{c}: image {w:?} in {got:?}"
            );
        }
        let brush = ok(
            &h,
            "draw.stroke.add",
            json!({ "drawingId": d, "kind": "brush", "layer": "new",
            "points": [{"x": 60, "y": 200, "pressure": 0.3}, {"x": 90, "y": 230, "pressure": 1.0}, {"x": 140, "y": 240, "pressure": 0.6}] }),
        );
        assert!(brush["strokeIds"].as_array().unwrap().len() >= images(&c), "{c}: brush images");
        let rect = ok(
            &h,
            "draw.rectangle.add",
            json!({ "drawingId": d, "from": {"x": 300, "y": 20}, "to": {"x": 360, "y": 60}, "layer": "new" }),
        );
        assert!(rect["strokeIds"].as_array().unwrap().len() >= images(&c), "{c}: rectangle images");
        ok(
            &h,
            "draw.line.add",
            json!({ "drawingId": d, "from": {"x": 20, "y": 300}, "to": {"x": 80, "y": 360}, "layer": "new" }),
        );
        let text = ok(
            &h,
            "draw.text.add",
            json!({ "drawingId": d, "position": {"x": -200, "y": -150}, "text": "Sym AV",
            "format": {"font": "B612", "size": 32, "kerning": 8} }),
        );
        ok(&h, "draw.breakApart.apply", json!({ "drawingId": d, "textIds": [text["textId"]] }));
        let drawing = ok(&h, "draw.drawing.get", json!({ "drawingId": d, "include": ["zones"] }));
        state.push(json!({ "config": c, "drawing": drawing["nodes"] }));
    }
    let text = tt_model::canonical::to_canonical_string(&Value::Array(state)).unwrap();
    // The corpus itself, for reviewing a golden change (C24-R2-05): `TT_DUMP_SYMMETRY=<file>`.
    if let Ok(path) = std::env::var("TT_DUMP_SYMMETRY") {
        std::fs::write(path, &text).unwrap();
    }
    let digest = tt_persist::fsx::sha256(text.as_bytes());
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/c24-symmetry.json");
    if std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(
            &golden,
            serde_json::to_string_pretty(&json!({ "sha256": digest, "bytes": text.len(), "configs": configs() }))
                .unwrap()
                + "\n",
        )
        .unwrap();
        return;
    }
    let g: Value = serde_json::from_str(&std::fs::read_to_string(&golden).unwrap()).unwrap();
    assert_eq!(
        g["sha256"],
        digest.as_str(),
        "the symmetry corpus differs from the golden recording ({} bytes)",
        text.len()
    );
}

#[test]
fn symmetry_values_out_of_range_are_refused_and_none_draws_once() {
    // Tests of this binary share one font list: register before any executor reads it.
    register_fixture_font();
    let h = fixture();
    assert_eq!(
        err(&h, "tool.symmetry.set", json!({ "mode": "kaleidoscope", "mirrors": 17 })).code,
        "tool.valueOutOfRange"
    );
    assert_eq!(err(&h, "tool.symmetry.set", json!({ "mode": "mirror", "angle": 361 })).code, "tool.valueOutOfRange");
    ok(&h, "tool.symmetry.set", json!({ "mode": "mirror", "angle": 0 }));
    // `symmetry: false` on the command draws one stroke even with a mode set.
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW", "kind": "pencil", "fit": false, "layer": "new", "symmetry": false,
        "points": [{"x": 10, "y": 10}, {"x": 40, "y": 30}] }),
    );
    assert_eq!(r["strokeIds"].as_array().unwrap().len(), 1);
    ok(&h, "tool.symmetry.set", json!({ "mode": "none" }));
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW", "kind": "pencil", "fit": false, "layer": "new",
        "points": [{"x": 10, "y": 50}, {"x": 40, "y": 70}] }),
    );
    assert_eq!(r["strokeIds"].as_array().unwrap().len(), 1);
}
