//! M2 C2.4 (02 §11.2–§11.3): `draw.vectorize.preview` and `draw.vectorize.run` on the fixture project — imported
//! geometry, holes as paintable zones, the generated palette, IDs and revisions, malformed input, conflicts, undo,
//! dry runs, a new element, the texture path, and save/open.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::path::PathBuf;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions};

const BOB: &str = "scene:#1/element:\"Bob\"";

/// A corpus file by a normalised absolute path (the file-system policy refuses `..` segments).
fn corpus(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = root.parent().and_then(|p| p.parent()).unwrap();
    repo.join("tests").join("fixtures").join("vectorize").join(name).to_string_lossy().into_owned()
}

fn art() -> String {
    corpus("e2-lineart.png")
}

/// Runs a job command and waits for its final record.
fn job(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    let started = ok(h, id, params);
    let r = ok(h, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60_000 }));
    if r.get("record").is_some() { r["record"].clone() } else { r }
}

fn empty_bob(h: &ExecutorHandle) {
    ok(
        h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{"op": "replace", "path": "/exposure", "value": [[null, 24]]}] }),
    );
}

/// The drawing an element's cell at `frame` shows.
fn shown(doc: &tt_model::document::Document, element: &str, frame: u32) -> Option<String> {
    let e = doc.elements().find(|e| e.name() == element)?;
    let mut f = 0u32;
    for (d, n) in e.exposure().iter().flatten() {
        f += *n;
        if frame <= f {
            return d.as_ref().map(|x| x.as_str().to_owned());
        }
    }
    None
}

fn layers(h: &ExecutorHandle, drawing: &str) -> Vec<Value> {
    ok(h, "draw.drawing.get", json!({ "drawingId": drawing }))["nodes"].as_array().unwrap().clone()
}

#[test]
fn the_e2_line_art_becomes_brush_shapes_with_paintable_zones_in_one_step() {
    let h = fixture();
    empty_bob(&h);
    let v = rev(&h);
    let rec = job(&h, "draw.vectorize.run", json!({ "files": [art()], "targetElementId": BOB, "startFrame": 3 }));
    assert_eq!(rec["state"], "succeeded", "{rec}");
    let r = &rec["result"];
    assert_eq!((r["completed"].as_u64(), r["cancelled"].as_bool()), (Some(1), Some(false)), "{r}");
    assert_eq!(rev(&h), v + 1, "one image, one revision");
    let drawing = r["drawingIds"][0].as_str().unwrap().to_owned();
    // The cell at frame 3 shows it.
    let state = ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 3 }));
    let _ = state;
    let nodes = layers(&h, &drawing);
    assert_eq!(nodes.len(), 1, "one layer");
    let strokes = nodes[0]["strokes"].as_array().unwrap();
    // Frame (split in two), ring and triangle: three ink regions.
    assert_eq!(strokes.len(), 3, "{}", strokes.len());
    assert!(strokes.iter().all(|s| s["kind"] == "brush" && s["closed"] == true));
    let black = strokes[0]["colourId"].as_str().unwrap().to_owned();
    assert!(strokes.iter().all(|s| s["colourId"] == black));
    // The colour is the generated palette's black.
    let doc = h.snapshot().unwrap().unwrap();
    let pal = doc.palettes().find(|p| p.name() == "Vectorization").expect("the generated palette");
    assert!(pal.swatches().iter().any(|s| s.colour_id().as_str() == black && s.name() == "Vectorization Black"));
    drop(doc);
    // Holes are unpainted zones: two halves of the frame, the ring's inside and the triangle's inside (and the
    // outside), which `doc.validate` reports for E2.
    let issues = ok(&h, "doc.validate", json!({ "checks": ["unpaintedZones"] }))["issues"].as_array().unwrap().clone();
    let mine: Vec<&Value> = issues
        .iter()
        .filter(|i| i["details"]["drawingId"] == drawing.as_str() || i["drawingId"] == drawing.as_str())
        .collect();
    assert!(mine.len() >= 4, "{issues:#?}");
    // Undo removes the drawing, the palette and the exposure in one step.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(rev(&h), v + 2);
    let doc = h.snapshot().unwrap().unwrap();
    assert!(doc.palettes().all(|p| p.name() != "Vectorization"));
    assert!(doc.drawing(&tt_model::Id::parse(&drawing).unwrap()).is_none());
}

#[test]
fn a_second_image_reuses_the_palette_and_fills_consecutive_cells() {
    let h = fixture();
    empty_bob(&h);
    let rec = job(
        &h,
        "draw.vectorize.run",
        json!({ "files": [art(), corpus("card-rgb.png")], "targetElementId": BOB, "startFrame": 5 }),
    );
    assert_eq!(rec["state"], "succeeded", "{rec}");
    assert_eq!(rec["result"]["completed"], 2);
    let doc = h.snapshot().unwrap().unwrap();
    assert_eq!(doc.palettes().filter(|p| p.name() == "Vectorization").count(), 1);
    let ids: Vec<String> =
        rec["result"]["drawingIds"].as_array().unwrap().iter().map(|d| d.as_str().unwrap().to_owned()).collect();
    for (i, d) in ids.iter().enumerate() {
        assert_eq!(shown(&doc, "Bob", 5 + i as u32).as_deref(), Some(d.as_str()), "frame {}", 5 + i);
    }
}

#[test]
fn occupied_cells_ask_replace_or_skip() {
    let h = fixture();
    empty_bob(&h);
    let first = job(&h, "draw.vectorize.run", json!({ "files": [art()], "targetElementId": BOB, "startFrame": 1 }));
    let old = first["result"]["drawingIds"][0].clone();
    // Skip leaves the occupied cell.
    let v = rev(&h);
    let skipped = job(
        &h,
        "draw.vectorize.run",
        json!({ "files": [art()], "targetElementId": BOB, "startFrame": 1, "onConflict": "skip" }),
    );
    assert_eq!(skipped["result"]["completed"], 0, "{skipped}");
    assert_eq!(skipped["result"]["skipped"], json!([{ "index": 0, "source": art(), "reason": "occupied" }]));
    assert_eq!(rev(&h), v);
    // Replace puts a new drawing in the cell; the old one stays in the bank.
    let replaced = job(
        &h,
        "draw.vectorize.run",
        json!({ "files": [art()], "targetElementId": BOB, "startFrame": 1, "onConflict": "replace" }),
    );
    let new = replaced["result"]["drawingIds"][0].clone();
    assert_ne!(new, old);
    let doc = h.snapshot().unwrap().unwrap();
    assert_eq!(shown(&doc, "Bob", 1), new.as_str().map(str::to_owned));
    assert!(doc.drawing(&tt_model::Id::parse(old.as_str().unwrap()).unwrap()).is_some());
}

#[test]
fn malformed_input_and_ranges_are_refused_before_anything_starts() {
    let h = fixture();
    empty_bob(&h);
    let v = rev(&h);
    assert_eq!(
        err(&h, "draw.vectorize.run", json!({ "files": [art()], "targetElementId": BOB, "threshold": 101 })).code,
        "vectorize.thresholdOutOfRange"
    );
    assert_eq!(
        err(&h, "draw.vectorize.preview", json!({ "file": art(), "threshold": -1 })).code,
        "vectorize.thresholdOutOfRange"
    );
    assert_eq!(
        err(&h, "draw.vectorize.preview", json!({ "file": corpus("garbage.png") })).code,
        "vectorize.unsupportedFile"
    );
    assert_eq!(
        err(&h, "draw.vectorize.preview", json!({ "file": corpus("huge-declared.psd") })).code,
        "limit.exceeded"
    );
    assert_eq!(
        err(&h, "draw.vectorize.run", json!({ "files": [corpus("missing.png")], "targetElementId": BOB })).code,
        "vectorize.unsupportedFile"
    );
    assert_eq!(err(&h, "draw.vectorize.run", json!({})).code, "arg.invalid");
    // A camera is not a vector drawing element.
    let camera = h
        .snapshot()
        .unwrap()
        .unwrap()
        .elements()
        .find(|e| *e.element_type() == tt_model::entities::element::ElementType::Camera)
        .unwrap()
        .id()
        .as_str()
        .to_owned();
    assert_eq!(
        err(&h, "draw.vectorize.run", json!({ "files": [art()], "targetElementId": camera })).code,
        "drawing.notVectorElement"
    );
    // A damaged file fails the job, and nothing is committed.
    let rec = job(&h, "draw.vectorize.run", json!({ "files": [corpus("truncated.png")], "targetElementId": BOB }));
    assert_eq!(rec["state"], "failed", "{rec}");
    assert_eq!(rec["error"]["code"], "vectorize.unsupportedFile");
    assert_eq!(rev(&h), v);
    // A dry run starts nothing.
    let dry = with(
        &h,
        "draw.vectorize.run",
        json!({ "files": [art()], "targetElementId": BOB }),
        InvokeOptions { dry_run: true, ..Default::default() },
    );
    assert_eq!(dry.result.unwrap()["jobId"], "dryRun");
    assert_eq!(rev(&h), v);
}

#[test]
fn the_preview_renders_what_the_run_creates() {
    let h = fixture();
    let r = ok(&h, "draw.vectorize.preview", json!({ "file": art(), "width": 256 }));
    assert_eq!(r["strokeCount"], 3);
    let png = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
    let (w, hgt, px) = tt_render::decode_png(&png, 4096).unwrap();
    assert_eq!((w, hgt), (256, 192));
    let at = |x: usize, y: usize| &px[(y * 256 + x) * 4..(y * 256 + x) * 4 + 3];
    assert_eq!(at(18, 96), &[0, 0, 0], "the frame line is ink");
    assert_eq!(at(70, 96), &[255, 255, 255], "the ring's inside is paper");
    // Deterministic.
    assert_eq!(ok(&h, "draw.vectorize.preview", json!({ "file": art(), "width": 256 }))["png"], r["png"]);
}

/// The fixture with its second scene emptied of elements (no element can be selected there).
fn empty_second_scene() -> (ExecutorHandle, String) {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let mut scene = serde_json::to_value(&parts.scenes[1]).unwrap();
    let gone: Vec<String> =
        scene["elements"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().to_owned()).collect();
    scene["tree"] = json!([]);
    scene["columnOrder"] = json!([]);
    scene["elements"] = json!([]);
    scene["currentCameraId"] = Value::Null;
    let id = scene["id"].as_str().unwrap().to_owned();
    parts.scenes[1] = tt_model::object::from_json(&scene).unwrap();
    parts.elements.retain(|e| !gone.contains(&e.id().as_str().to_owned()));
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let h = ExecutorHandle::spawn(tt_executor::ExecutorConfig::default()).unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    (h, id)
}

#[test]
fn a_selected_element_that_is_not_a_drawing_element_is_refused() {
    let h = fixture();
    let camera = h
        .snapshot()
        .unwrap()
        .unwrap()
        .elements()
        .find(|e| *e.element_type() == tt_model::entities::element::ElementType::Camera)
        .unwrap()
        .id()
        .as_str()
        .to_owned();
    ok(&h, "app.context.set", json!({ "elementId": camera }));
    assert_eq!(err(&h, "draw.vectorize.run", json!({ "files": [art()] })).code, "drawing.notVectorElement");
}

#[test]
fn without_a_selected_element_a_new_drawing_element_is_created() {
    let (h, scene) = empty_second_scene();
    ok(&h, "app.context.set", json!({ "sceneId": scene }));
    assert!(ok(&h, "app.state.get", json!({}))["element"].is_null(), "no element in that scene");
    let before = h.snapshot().unwrap().unwrap().elements().count();
    let v = rev(&h);
    let rec = job(&h, "draw.vectorize.run", json!({ "files": [art()], "startFrame": 1 }));
    assert_eq!(rec["state"], "succeeded", "{rec}");
    assert_eq!(rev(&h), v + 1, "the element, its bank and the drawing in one revision");
    let doc = h.snapshot().unwrap().unwrap();
    assert_eq!(doc.elements().count(), before + 1);
    let e = doc.elements().find(|e| e.name() == "e2-lineart").expect("named after the file");
    assert_eq!(e.id().as_str(), rec["result"]["elementId"].as_str().unwrap());
    assert_eq!(shown(&doc, "e2-lineart", 1).as_deref(), rec["result"]["drawingIds"][0].as_str());
    let sc = doc.scenes().find(|s| s.id().as_str() == scene).unwrap();
    assert!(sc.column_order().iter().any(|c| c == e.id()), "a column of the current scene");
    drop(doc);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(h.snapshot().unwrap().unwrap().elements().count(), before);
}

#[test]
fn with_texture_the_shapes_carry_the_source_colours() {
    let h = fixture();
    empty_bob(&h);
    // A 40 × 20 bar: left half red, right half blue, on white.
    let (w, hgt) = (40u32, 20u32);
    let mut rgba = vec![255u8; (w * hgt * 4) as usize];
    for y in 5..15 {
        for x in 4..36 {
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 3].copy_from_slice(if x < 20 { &[220, 0, 0] } else { &[0, 0, 220] });
        }
    }
    let dir = std::env::temp_dir().join(format!("tt-c24-tex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bar.png");
    std::fs::write(&file, tt_render::encode_png(&rgba, w, hgt).unwrap()).unwrap();
    let rec = job(
        &h,
        "draw.vectorize.run",
        json!({ "files": [file.to_string_lossy()], "targetElementId": BOB, "startFrame": 2, "type": "withTexture" }),
    );
    assert_eq!(rec["state"], "succeeded", "{rec}");
    let drawing = rec["result"]["drawingIds"][0].as_str().unwrap().to_owned();
    let r = ok(
        &h,
        "draw.drawing.render",
        json!({ "drawingId": drawing, "width": 400, "region": {"x": -600, "y": -300, "width": 1200, "height": 600} }),
    );
    let png = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
    let (rw, _, px) = tt_render::decode_png(&png, 4096).unwrap();
    let at = |x: usize, y: usize| px[(y * rw as usize + x) * 4..(y * rw as usize + x) * 4 + 3].to_vec();
    // The bar spans x 4..36 of 40 → pixels 40..360 of 400; its rows 5..15 of 20 → 50..150 of 200.
    let left = at(100, 100);
    let right = at(300, 100);
    assert!(left[0] > 150 && left[2] < 60, "left is red: {left:?}");
    assert!(right[2] > 150 && right[0] < 60, "right is blue: {right:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn vectorized_drawings_survive_save_and_open() {
    let h = fixture();
    empty_bob(&h);
    let rec = job(&h, "draw.vectorize.run", json!({ "files": [art()], "targetElementId": BOB, "startFrame": 1 }));
    let drawing = rec["result"]["drawingIds"][0].as_str().unwrap().to_owned();
    let before = ok(&h, "draw.drawing.get", json!({ "drawingId": drawing }));
    let dir = std::env::temp_dir().join(format!("tt-c24-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("V.animpkg");
    let saved = job(&h, "project.saveAs", json!({ "path": path.to_string_lossy(), "onExists": "overwrite" }));
    assert_eq!(saved["state"], "succeeded", "{saved}");
    let h2 = fixture();
    let opened = job(&h2, "project.open", json!({ "path": path.to_string_lossy(), "onUnsaved": "discard" }));
    assert_eq!(opened["state"], "succeeded", "{opened}");
    let after = ok(&h2, "draw.drawing.get", json!({ "drawingId": drawing }));
    assert_eq!(before["nodes"], after["nodes"]);
    drop(h2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cancelling_keeps_the_images_already_completed() {
    let h = fixture();
    empty_bob(&h);
    // Twenty noisy 768 × 768 images: each takes long enough that the cancellation lands mid-run.
    let dir = std::env::temp_dir().join(format!("tt-c24-cancel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let side = 768u32;
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut rgba = vec![255u8; (side * side * 4) as usize];
    for px in rgba.chunks_mut(4) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let v = if state.is_multiple_of(3) { 0 } else { 255 };
        px[..3].copy_from_slice(&[v, v, v]);
    }
    let png = tt_render::encode_png(&rgba, side, side).unwrap();
    let files: Vec<String> = (0..20)
        .map(|i| {
            let f = dir.join(format!("noise-{i}.png"));
            std::fs::write(&f, &png).unwrap();
            f.to_string_lossy().into_owned()
        })
        .collect();
    let v = rev(&h);
    let started = ok(&h, "draw.vectorize.run", json!({ "files": files, "targetElementId": BOB, "startFrame": 1 }));
    ok(&h, "job.cancel", json!({ "jobId": started["jobId"] }));
    let r = ok(&h, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 120_000 }));
    let rec = if r.get("record").is_some() { r["record"].clone() } else { r };
    assert_eq!(rec["state"], "cancelled", "{rec}");
    let result = &rec["result"];
    assert_eq!(result["cancelled"], true, "{rec}");
    let done = result["completed"].as_u64().unwrap();
    assert!(done < 20);
    assert_eq!(result["drawingIds"].as_array().unwrap().len() as u64, done);
    // The images not committed are reported, in order, as cancelled.
    let skipped = result["skipped"].as_array().unwrap();
    assert_eq!(skipped.len() as u64, 20 - done, "{rec}");
    assert!(skipped.iter().all(|s| s["reason"] == "cancelled"));
    assert_eq!(skipped[0]["index"].as_u64(), Some(done));
    assert_eq!(rev(&h), v + done, "each completed image is its own revision and stays");
    let _ = std::fs::remove_dir_all(dir);
}

/// The fixture with its image element's bitmap made real (the E2 line art), saved as a package with that blob and
/// opened through `project.open`: the image element, its bank drawing and the bitmap node's entry, as imported.
fn with_image_element() -> (ExecutorHandle, String, std::path::PathBuf) {
    use tt_persist::container::{BlobData, Blobs};
    let png = std::fs::read(art()).unwrap();
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let image = parts.elements.iter().find(|e| e.name() == "Background").unwrap();
    let bank_id = image.drawing_bank_id().clone().unwrap();
    let element_id = image.id().as_str().to_owned();
    let bank = parts.banks.iter().find(|b| *b.id() == bank_id).unwrap();
    let drawing_id = bank.drawings().ids()[0].clone();
    let at = parts.drawings.iter().position(|d| *d.id() == drawing_id).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let node = &mut d["nodes"][0];
    let entry = node["image"]["entry"].as_str().unwrap().to_owned();
    node["image"]["size"] = json!(png.len());
    node["image"]["sha256"] = json!(tt_persist::fsx::sha256(&png));
    node["width"] = json!(256);
    node["height"] = json!(192);
    parts.drawings[at] = tt_model::object::from_json(&d).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let mut blobs = Blobs::default();
    blobs.insert(entry, BlobData::Bytes(std::sync::Arc::new(png)));
    let dir =
        std::env::temp_dir().join(format!("tt-c24-image-{}-{}", std::process::id(), tt_persist::fsx::random_token(4)));
    std::fs::create_dir_all(&dir).unwrap();
    let package = dir.join("Image.animpkg");
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-11-02T09:00:00Z".into(),
    };
    tt_persist::save::save_package(&package, &input, None).unwrap();
    let h = fixture();
    let opened = job(&h, "project.open", json!({ "path": package.to_string_lossy(), "onUnsaved": "discard" }));
    assert_eq!(opened["state"], "succeeded", "{opened}");
    (h, element_id, dir)
}

#[test]
fn an_image_element_becomes_a_new_drawing_element_with_matching_exposure() {
    let (h, image, dir) = with_image_element();
    // The preview reads the bitmap the image element shows at a frame.
    let r = ok(&h, "draw.vectorize.preview", json!({ "imageElementId": image, "frame": 1, "width": 256 }));
    assert_eq!(r["strokeCount"], 3);
    assert_eq!(
        err(&h, "draw.vectorize.run", json!({ "imageElementId": image, "targetElementId": BOB })).code,
        "arg.invalid"
    );
    let doc = h.snapshot().unwrap().unwrap();
    let before = doc.elements().count();
    let source = doc.elements().find(|e| e.id().as_str() == image).unwrap();
    let source_runs: Vec<(Option<String>, u32)> =
        source.exposure().iter().flatten().map(|(d, n)| (d.as_ref().map(|x| x.as_str().to_owned()), *n)).collect();
    let name = format!("{} Vector", source.name());
    drop(doc);
    let v = rev(&h);
    let rec = job(&h, "draw.vectorize.run", json!({ "imageElementId": image }));
    assert_eq!(rec["state"], "succeeded", "{rec}");
    assert_eq!(rec["result"]["completed"], 1);
    assert_eq!(rev(&h), v + 1, "one image, one step");
    let doc = h.snapshot().unwrap().unwrap();
    assert_eq!(doc.elements().count(), before + 1);
    let new = doc.elements().find(|e| *e.name() == name).expect("the new drawing element");
    assert_eq!(new.id().as_str(), rec["result"]["elementId"].as_str().unwrap());
    let drawing = rec["result"]["drawingIds"][0].as_str().unwrap().to_owned();
    // The exposure mirrors the image element's, drawing for drawing.
    let new_runs: Vec<(Option<String>, u32)> =
        new.exposure().iter().flatten().map(|(d, n)| (d.as_ref().map(|x| x.as_str().to_owned()), *n)).collect();
    assert_eq!(new_runs.len(), source_runs.len());
    for ((a, n), (b, m)) in source_runs.iter().zip(&new_runs) {
        assert_eq!(n, m);
        assert_eq!(a.is_some(), b.is_some());
        if a.is_some() {
            assert_eq!(b.as_deref(), Some(drawing.as_str()));
        }
    }
    // The image element is kept unchanged.
    assert!(doc.elements().any(|e| e.id().as_str() == image));
    drop(doc);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(h.snapshot().unwrap().unwrap().elements().count(), before);
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
