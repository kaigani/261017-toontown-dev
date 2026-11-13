//! M3 C3.3 (builder; readiness item 3, SI-105): bitmap native size. A bitmap node's pixel space (origin top-left, y
//! down, 10 §4.9) maps to drawing space through its transform; at the native scale (1200 du across the camera width,
//! with the y flip that keeps it upright) one image pixel covers one output pixel of the default camera on the table
//! plane (05 §1.3), so a pixel-aligned image reads back exactly. Bounds, render and hit test agree, and fully transparent
//! pixels are not hit (06 §6.8).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod c33_support;
mod common;

use c33_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
use tt_model::change::PatchOp;
use tt_model::document::EntryKey;

/// The test image: 8 x 4 pixels, every pixel a different opaque colour except (5, 2), fully transparent.
fn image() -> (u32, u32, Vec<u8>) {
    let (w, h) = (8u32, 4u32);
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if (x, y) == (5, 2) {
                px.extend([0, 0, 0, 0]);
            } else {
                px.extend([u8::try_from(20 + x * 25).unwrap(), u8::try_from(30 + y * 50).unwrap(), 200, 255]);
            }
        }
    }
    (w, h, px)
}

fn replace(path: &str, value: Value) -> PatchOp {
    PatchOp {
        op: "replace",
        from: None,
        path: path.into(),
        range: None,
        value: Some(value),
        old_value: None,
        elided: false,
    }
}

/// Opens the fixture with its image element's bitmap replaced by the test image at native size, its top-left on output
/// pixel (`k`, `m`) of a render at the camera size.
fn open_with_bitmap(k: u32, m: u32) -> (ExecutorHandle, String, String) {
    use tt_persist::container::{BlobData, Blobs};
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let (cw, ch) = (f64::from(*doc.project().camera_width()), f64::from(*doc.project().camera_height()));
    let bg = doc.elements().find(|e| e.name() == "Background").unwrap();
    let element = bg.id().as_str().to_owned();
    let drawing = bg.exposure().as_ref().unwrap()[0].0.clone().unwrap();
    let node = doc.drawing(&drawing).unwrap().nodes()[0].id().as_str().to_owned();
    let (w, h, px) = image();
    let png = tt_render::encode_png(&px, w, h).unwrap();
    let entry = format!("banks/{}/{}/{node}.png", doc.bank_of(&drawing).unwrap().as_str(), drawing.as_str());
    let sha = {
        use sha2::Digest;
        sha2::Sha256::digest(&png).iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    // Native size: s = 1200 / camera width du per pixel; the image's top-left at drawing point (tx, ty).
    let s = 1200.0 / cw;
    let (tx, ty) = (-600.0 + s * f64::from(k), 600.0 * ch / cw - s * f64::from(m));
    let ops = vec![
        replace("/nodes/0/width", json!(w)),
        replace("/nodes/0/height", json!(h)),
        replace("/nodes/0/transform", json!([s, 0, 0, -s, tx, ty])),
        replace(
            "/nodes/0/image",
            json!({ "entry": entry, "mediaType": "image/png", "size": png.len(), "sha256": sha }),
        ),
    ];
    let (doc, _) = doc.with_patched_entries(&[(EntryKey::Drawing(drawing.clone()), ops)]).unwrap();
    let mut blobs = Blobs::default();
    blobs.insert(entry, BlobData::Bytes(std::sync::Arc::new(png)));
    blobs.insert(
        "media/as_01K742SG0G5HPQ0RBF5X4DSR0W.wav".into(),
        BlobData::Bytes(std::sync::Arc::new(vec![0; 352_844])),
    );
    let dir = c13_support::tmp();
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-10-01T09:00:00Z".into(),
    };
    let path = dir.join("Bitmap.animpkg");
    tt_persist::save::save_package(&path, &input, None).unwrap();
    let h = c13_support::spawn(&dir.join("user"));
    c13_support::job_ok(&h, "project.open", json!({ "path": path.to_string_lossy() }));
    ok(&h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": false }));
    (h, element, node)
}

#[test]
fn one_image_pixel_is_one_output_pixel_at_native_size() {
    let (k, m) = (100, 50);
    let (h, element, _) = open_with_bitmap(k, m);
    let r = render(&h, json!({ "frame": 1 }));
    let (w, hh, px) = image();
    assert_eq!((r.w, r.h), (1920, 1080), "the camera size");
    let white = [255, 255, 255, 255];
    for y in 0..hh {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let want = if px[i + 3] == 0 { white } else { [px[i], px[i + 1], px[i + 2], 255] };
            assert_eq!(r.at(k + x, m + y), want, "image pixel ({x}, {y})");
        }
    }
    // Nothing spills over the image's edges.
    for (x, y) in [(k - 1, m), (k + w, m), (k, m - 1), (k, m + hh)] {
        assert_eq!(r.at(x, y), white, "outside at ({x}, {y})");
    }
    // The description's pixel bounds are the image's pixels.
    let d = ok(&h, "inspect.frame.describe", json!({ "frame": 1, "detail": "full" }));
    let e = d["elements"].as_array().unwrap().iter().find(|e| e["elementId"] == element.as_str()).unwrap();
    let b = &e["cameraBounds"]["pixels"];
    for (key, want) in [("x", k), ("y", m), ("width", w), ("height", hh)] {
        close(b[key].as_f64().unwrap(), f64::from(want), 1e-9, key);
    }
}

#[test]
fn hit_test_agrees_with_the_pixels_and_skips_transparent_ones() {
    let (k, m) = (100, 50);
    let (h, element, node) = open_with_bitmap(k, m);
    let hit = |x: f64, y: f64| -> Vec<Value> {
        ok(&h, "inspect.hitTest", json!({ "space": "image", "point": { "x": x, "y": y }, "frame": 1 }))["hits"]
            .as_array()
            .unwrap()
            .clone()
    };
    let opaque = hit(f64::from(k) + 0.5, f64::from(m) + 0.5);
    assert_eq!(opaque.len(), 1, "{opaque:?}");
    assert_eq!(
        (opaque[0]["elementId"].as_str(), opaque[0]["objectId"].as_str()),
        (Some(element.as_str()), Some(node.as_str()))
    );
    assert_eq!(opaque[0]["kind"], "bitmap");
    assert!(hit(f64::from(k) + 5.5, f64::from(m) + 2.5).is_empty(), "the transparent pixel is not hit");
    assert!(hit(f64::from(k) + 4.5, f64::from(m) + 2.5).len() == 1, "its neighbour is");
    assert!(hit(f64::from(k) - 0.5, f64::from(m) + 0.5).is_empty(), "outside the image");
    // The same point in camera space (fields from the centre, y up) hits the same pixel.
    let cam = ok(
        &h,
        "inspect.coordinates.convert",
        json!({ "from": "image", "to": "camera", "points": [{ "x": f64::from(k) + 0.5, "y": f64::from(m) + 0.5 }] }),
    );
    let p = &cam["points"][0];
    let hits =
        ok(&h, "inspect.hitTest", json!({ "space": "camera", "point": { "x": p["x"], "y": p["y"] }, "frame": 1 }));
    assert_eq!(hits["hits"].as_array().unwrap().len(), 1, "{hits}");
}
