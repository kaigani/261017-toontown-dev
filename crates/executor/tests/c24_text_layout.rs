//! M2 C2.4 readiness item 7 (ticket C21-R1-11): same-font text-layout controls. Layout reads the machine's fonts, so
//! cross-OS equality is only promised for the same font file: the test-only fixture font
//! `tests/fixtures/fonts/B612-Regular.ttf` (SIL OFL 1.1, `OFL.txt` beside it; ADR-0003 rule 7) is registered through
//! the dev-only `tt-vector/test-hooks` hook before any text is laid out. With it, text bounds (runs, sizes, bold
//! fallback, kerning, alignment, wrapping) and Break Text Apart placements are compared against a golden digest on
//! every OS (CI: Windows and macOS `check`), and checked against properties that do not depend on the recording: an
//! advance-proportional width, glyphs inside the text box, one group per visible character. `TT_UPDATE_GOLDEN=1`
//! re-records the golden.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const FONT: &str = "B612";

/// Registers the fixture font (before the process first reads its font list).
fn register_fixture_font() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/fonts/B612-Regular.ttf");
        tt_vector::fonts::register_test_font(std::fs::read(path).unwrap());
    });
}

fn add(h: &tt_executor::ExecutorHandle, text: &str, at: (f64, f64), format: Value) -> Value {
    let mut f = format;
    f["font"] = json!(FONT);
    ok(h, "draw.text.add", json!({ "drawingId": D1, "position": {"x": at.0, "y": at.1}, "text": text, "format": f }))
}

#[test]
fn the_fixture_font_is_installed_and_lays_out_by_its_own_advances() {
    register_fixture_font();
    assert!(tt_vector::fonts::is_installed(FONT), "the fixture font is registered");
    let r = tt_vector::fonts::resolve(FONT, false, false);
    assert!(r.installed);
    assert_eq!(r.face.family, FONT);
    let h = fixture();
    // Width is additive in the font's advances (no kerning between identical letters in this face).
    let one = add(&h, "M", (0.0, 0.0), json!({ "size": 50 }))["bounds"]["width"].as_f64().unwrap();
    let four = add(&h, "MMMM", (0.0, -200.0), json!({ "size": 50 }))["bounds"]["width"].as_f64().unwrap();
    let (adv, _) = r.face.advances(&['M']);
    assert!((one - adv[0] * 50.0).abs() < 1e-9, "{one} = advance {} × 50", adv[0]);
    assert!((four - 4.0 * one).abs() < 1e-9, "{four} = 4 × {one}");
    // No missing-font report for the registered family.
    let lint = ok(&h, "doc.validate", json!({ "checks": ["missingFonts"] }));
    assert!(!lint["issues"].as_array().unwrap().iter().any(|i| i["details"]["font"] == FONT), "{lint}");
}

#[test]
fn same_font_layout_and_break_apart_are_identical_across_platforms() {
    register_fixture_font();
    let h = fixture();
    let mut state = Vec::new();
    let cases = [
        ("Toontown", json!({ "size": 40 })),
        ("Kerning AV To\nwrapped lines", json!({ "size": 30, "kerning": 12, "align": "centre" })),
        ("Bold fallback é ö ß", json!({ "size": 24, "bold": true, "italic": true })),
        ("Right aligned\ntwo", json!({ "size": 36, "align": "right" })),
    ];
    let mut ids = Vec::new();
    for (k, (text, format)) in cases.iter().enumerate() {
        let r = add(&h, text, (-300.0, 300.0 - 120.0 * k as f64), format.clone());
        state.push(json!({ "text": text, "bounds": r["bounds"] }));
        ids.push(r["textId"].clone());
    }
    let texts = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }))["nodes"].clone();
    let before: Vec<Value> = ids
        .iter()
        .map(|id| ok(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": [id] }))["bounds"].clone())
        .collect();
    let r = ok(&h, "draw.breakApart.apply", json!({ "drawingId": D1, "textIds": ids }));
    let visible: usize = cases.iter().map(|(t, _)| t.chars().filter(|c| !c.is_whitespace()).count()).sum();
    assert_eq!(r["groupIds"].as_array().unwrap().len(), visible, "one group per visible character: {r}");
    let after = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
    // Every glyph lies inside its text box (the box adds line height above and below the outlines).
    let glyphs = ok(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": r["groupIds"] }))["bounds"].clone();
    let (x0, x1) = before.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), bb| {
        let x = bb["x"].as_f64().unwrap();
        (a.min(x), b.max(x + bb["width"].as_f64().unwrap()))
    });
    let gx = glyphs["x"].as_f64().unwrap();
    assert!(gx >= x0 - 2.0 && gx + glyphs["width"].as_f64().unwrap() <= x1 + 2.0, "{glyphs} within {x0}..{x1}");
    state.push(json!({ "texts": texts, "brokenApart": after["nodes"] }));
    let text = tt_model::canonical::to_canonical_string(&Value::Array(state)).unwrap();
    let digest = tt_persist::fsx::sha256(text.as_bytes());
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/c24-text-layout.json");
    if std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(
            &golden,
            serde_json::to_string_pretty(
                &json!({ "sha256": digest, "bytes": text.len(), "font": "tests/fixtures/fonts/B612-Regular.ttf" }),
            )
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
        "same-font layout differs from the golden recording ({} bytes)",
        text.len()
    );
}
