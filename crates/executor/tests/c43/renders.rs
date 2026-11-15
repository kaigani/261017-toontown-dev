//! Renders and captures of the Camera view (plan §3.5; SI-107, SI-142): `activeView` with the Camera view active is the
//! camera composition; a range refuses the active view; the headless capture draws the camera composition through the
//! view transform.

use serde_json::json;

use crate::camera::{BOB, apply, id_of, matrix, scene};
use crate::common::*;

#[test]
fn an_active_view_render_of_the_camera_view_is_the_camera_composition() {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    ok(&h, "view.active.set", json!({ "viewId": "cameraView" }));
    let active = ok(&h, "render.frame.run", json!({ "composition": "activeView", "scale": 0.25 }));
    let camera = ok(&h, "render.frame.run", json!({ "composition": "camera", "scale": 0.25 }));
    assert_eq!(active["sha256"], camera["sha256"]);
    assert_eq!(
        (active["width"].clone(), active["height"].clone()),
        (camera["width"].clone(), camera["height"].clone())
    );
    // A range is not an active-view composition (09 line 1013).
    let e = err(
        &h,
        "render.range.run",
        json!({ "composition": "activeView", "from": 1, "to": 2, "directory": std::env::temp_dir().join("tt-c43-range").to_string_lossy(), "createDirs": true, "onConflict": "overwrite" }),
    );
    assert_eq!((e.code.as_str(), e.details["reason"].as_str()), ("render.compositionUnavailable", Some("range")));
    // Top and Side views: M6 C6.1 (precheck equal to the handler).
    ok(&h, "view.active.set", json!({ "viewId": "topView" }));
    let e = err(&h, "render.frame.run", json!({ "composition": "activeView" }));
    assert_eq!((e.code.as_str(), e.details["owner"].as_str()), ("render.compositionUnavailable", Some("M6 C6.1")));
}

#[test]
fn the_headless_capture_of_the_camera_view_draws_the_camera_composition_through_the_view() {
    let h = fixture();
    // A painted square at the scene origin, in the middle of the frame (the headless view's 100 % window).
    let fill = crate::camera::square(&h, "Fill", (0.0, 0.0, 0.0), 0.0);
    ok(&h, "app.context.set", json!({ "elementId": fill, "frame": 1 }));
    ok(&h, "colour.paint.apply", json!({ "points": [{ "x": 0, "y": 0 }] }));
    let env = run(&h, "inspect.screenshot", json!({ "target": "cameraView" }));
    assert!(env.is_ok(), "{env:?}");
    let r = env.result.unwrap();
    assert_eq!(r["viewId"], "cameraView");
    assert_eq!(r["source"], "virtual");
    let png = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
    let (w, hgt, px) = tt_render::decode_png(&png, 16_384).unwrap();
    assert_eq!(
        (u32::from(w), u32::from(hgt)),
        (r["width"].as_u64().unwrap() as u32, r["height"].as_u64().unwrap() as u32)
    );
    // The capture is the camera render seen through the view transform: at 100 % the headless Camera view shows the
    // middle of the frame, so each capture pixel is the render's pixel at the view-to-output position (ruling O-5's
    // bound: under 1 % of pixels differ by more than 32).
    let s = scene(&h);
    let v = matrix(&s["camera"]["cameraToView"]);
    assert!((v[0] - 1.0).abs() < 1e-9 && v[1].abs() < 1e-12 && v[2].abs() < 1e-12, "100 %, unrotated: {v:?}");
    let full = ok(&h, "render.frame.run", json!({ "composition": "camera" }));
    let rpng = tt_persist::b64::decode(full["data"].as_str().unwrap()).unwrap();
    let (rw, rh, rpx) = tt_render::decode_png(&rpng, 16_384).unwrap();
    let (ox, oy) = (-v[4], -v[5]);
    let (mut differ, mut total, mut content) = (0u64, 0u64, 0u64);
    let bg: Vec<u8> = s["background"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u8).collect();
    for y in (0..usize::from(hgt)).step_by(2) {
        for x in (0..usize::from(w)).step_by(2) {
            let (rx, ry) = ((x as f64 + ox).round() as usize, (y as f64 + oy).round() as usize);
            if rx >= usize::from(rw) || ry >= usize::from(rh) {
                continue;
            }
            let a = &px[(y * usize::from(w) + x) * 4..][..4];
            let b = &rpx[(ry * usize::from(rw) + rx) * 4..][..4];
            total += 1;
            if a.iter().zip(b).any(|(p, q)| p.abs_diff(*q) > 32) {
                differ += 1;
            }
            if a[..3] != bg[..3] {
                content += 1;
            }
        }
    }
    assert!(content > 0, "the capture shows something besides the background");
    assert!(differ * 100 < total, "{differ} of {total} sampled pixels differ");
    let _ = (apply, id_of);
}
