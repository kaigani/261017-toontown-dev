//! Independent native-pixel bitmap oracle with partial alpha and transparent picking.
mod c13_support;
mod c33_support;
mod common;

use serde_json::json;
use tt_model::{change::PatchOp, document::EntryKey};
use tt_persist::container::{BlobData, Blobs};

#[test]
fn native_checkerboard_alpha_bounds_and_picking_agree() -> Result<(), Box<dyn std::error::Error>> {
    let doc = tt_model::fixture::sample_document(1).map_err(|e| format!("{e:?}"))?;
    let element = doc.elements().find(|e| e.name() == "Background").ok_or("missing background")?;
    let element_id = element.id().clone();
    let drawing =
        element.exposure().as_ref().and_then(|e| e.first()).and_then(|e| e.0.clone()).ok_or("missing exposure")?;
    let node = doc.drawing(&drawing).and_then(|d| d.nodes().first()).ok_or("missing bitmap node")?;
    let entry = format!(
        "banks/{}/{}/{}.png",
        doc.bank_of(&drawing).ok_or("missing bank")?.as_str(),
        drawing.as_str(),
        node.id().as_str()
    );
    // Hand-authored premultiplied image: opaque red, half-alpha green, transparent, quarter-alpha white.
    let png = tt_render::encode_png(&[255, 0, 0, 255, 0, 128, 0, 128, 0, 0, 0, 0, 64, 64, 64, 64], 2, 2)?;
    use sha2::Digest;
    let sha = sha2::Sha256::digest(&png).iter().map(|b| format!("{b:02x}")).collect::<String>();
    let native = 1200.0 / 1920.0;
    let updates = [
        ("/nodes/0/width", json!(2)),
        ("/nodes/0/height", json!(2)),
        ("/nodes/0/transform", json!([native, 0, 0, -native, -600.0 + 100.0 * native, 337.5 - 50.0 * native])),
        ("/nodes/0/image", json!({"entry":entry,"mediaType":"image/png","size":png.len(),"sha256":sha})),
    ];
    let patches = updates
        .into_iter()
        .map(|(path, value)| PatchOp {
            op: "replace",
            from: None,
            path: path.into(),
            range: None,
            value: Some(value),
            old_value: None,
            elided: false,
        })
        .collect();
    let (doc, _) =
        doc.with_patched_entries(&[(EntryKey::Drawing(drawing.clone()), patches)]).map_err(|e| format!("{e:?}"))?;
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
        saved_at: "2026-11-10T00:00:00Z".into(),
    };
    let path = dir.join("alpha.animpkg");
    tt_persist::save::save_package(&path, &input, None)?;
    let h = c13_support::spawn(&dir.join("user"));
    c13_support::job_ok(&h, "project.open", json!({"path":path}));
    common::ok(&h, "element.visibility.set", json!({"elementIds":[c33_support::BOB],"scope":"scene","visible":false}));
    let image = c33_support::render(&h, json!({"frame":1,"background":"transparent"}));
    for (x, y, want) in [
        (100, 50, [255, 0, 0, 255]),
        (101, 50, [0, 255, 0, 128]),
        (100, 51, [0, 0, 0, 0]),
        (101, 51, [255, 255, 255, 64]),
    ] {
        assert_eq!(image.at(x, y), want, "native pixel {x},{y}");
        let hits = common::ok(
            &h,
            "inspect.hitTest",
            json!({"frame":1,"space":"image","point":{"x":f64::from(x)+0.5,"y":f64::from(y)+0.5}}),
        );
        let picked =
            hits["hits"].as_array().ok_or("missing hits")?.iter().any(|hit| hit["elementId"] == element_id.as_str());
        assert_eq!(picked, want[3] > 0, "alpha picking {x},{y}");
    }
    let description = common::ok(&h, "inspect.frame.describe", json!({"frame":1,"detail":"full"}));
    let described = description["elements"]
        .as_array()
        .ok_or("missing elements")?
        .iter()
        .find(|e| e["elementId"] == element_id.as_str())
        .ok_or("missing bitmap description")?;
    assert_eq!(described["cameraBounds"]["pixels"], json!({"x":100.0,"y":50.0,"width":2.0,"height":2.0}));
    for format in ["bmp", "tga"] {
        let output = dir.join(format);
        let manifest = c13_support::job_ok(
            &h,
            "render.range.run",
            json!({"from":1,"to":1,"format":format,"directory":output,"createDirs":true,"background":"transparent"}),
        );
        let file = manifest["files"][0]["path"].as_str().ok_or("missing sequence path")?;
        let bytes = std::fs::read(file)?;
        let decoded =
            tt_io::image::decode(&bytes, Some(format), &tt_io::image::LIMITS).map_err(|e| format!("{e:?}"))?;
        assert_eq!((decoded.width, decoded.height), (1920, 1080));
        assert_eq!(decoded.pixels.as_slice(), image.rgba.as_slice(), "lossless {format} pixels including alpha");
        let sha = sha2::Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(manifest["files"][0]["sha256"], sha);
        assert_eq!(manifest["files"][0]["bytes"], bytes.len());
    }
    Ok(())
}
