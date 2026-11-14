//! The headless screenshot (design note 5: S1, S2, S4; S3's wire half for the virtual view).

use serde_json::{Value, json};

use crate::common::*;
use crate::equivalence::{D1, painted_rect, ready};

fn decode(v: &Value) -> (u32, u32, Vec<u8>) {
    let png = tt_persist::b64::decode(v["png"].as_str().unwrap()).unwrap();
    let sha = tt_persist::fsx::sha256(&png);
    assert_eq!(v["sha256"], sha, "S1: the sha256 is the PNG's");
    let (w, h, px) = tt_render::decode_png(&png, 4096).unwrap();
    (u32::from(w), u32::from(h), px)
}

#[test]
fn s1_s4_the_png_decodes_to_the_stated_size_and_equals_the_drawing_render() {
    let h = ready();
    painted_rect(&h);
    let shot = ok(&h, "inspect.screenshot", json!({}));
    let (w, hh, px) = decode(&shot);
    assert_eq!((w, hh), (1920, 1080));
    assert_eq!((shot["width"].as_u64(), shot["height"].as_u64()), (Some(1920), Some(1080)));
    assert_eq!((shot["viewId"].as_str(), shot["drawingId"].as_str()), (Some("drawingView"), Some(D1)));
    // The same pixels `draw.drawing.render` gives for the view's region, size and background (one rasteriser).
    let r = ok(
        &h,
        "draw.drawing.render",
        json!({ "drawingId": D1, "width": 1920, "height": 1080,
                "region": { "x": -960, "y": -540, "width": 1920, "height": 1080 }, "background": "#F0F0F0FF" }),
    );
    let render = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
    let (_, _, rpx) = tt_render::decode_png(&render, 4096).unwrap();
    assert!(px == rpx, "S4: headless pixels equal draw.drawing.render's");
    // A region crops before encoding; the painted zone is inside it.
    let crop = ok(
        &h,
        "inspect.screenshot",
        json!({ "target": "drawingView", "region": { "x": 1250, "y": 170, "w": 120, "h": 80 } }),
    );
    let (cw, ch, cpx) = decode(&crop);
    assert_eq!((cw, ch), (120, 80));
    for row in 0..80usize {
        let full = &px[((170 + row) * 1920 + 1250) * 4..((170 + row) * 1920 + 1250 + 120) * 4];
        assert!(full == &cpx[row * 120 * 4..(row + 1) * 120 * 4], "the crop is that part of the view (row {row})");
    }
    assert!(cpx.chunks(4).any(|c| c != [0xF0, 0xF0, 0xF0, 0xFF]), "the painted rectangle is in the crop");
}

#[test]
fn s2_the_caps_are_checked_before_rendering() {
    let h = ready();
    let e = err(&h, "inspect.screenshot", json!({ "region": { "x": 0, "y": 0, "w": 2049, "h": 10 } }));
    assert_eq!((e.code.as_str(), e.details["maxInlinePixels"].as_u64()), ("render.inlineTooLarge", Some(2048)));
    assert_eq!(err(&h, "inspect.screenshot", json!({ "target": "dialogs" })).code, "view.notFound");
    let e = err(&h, "inspect.screenshot", json!({ "target": "cameraView" }));
    assert_eq!((e.code.as_str(), &e.details["milestone"]), ("command.notImplemented", &json!("M4 C4.3")));
}
