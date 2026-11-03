//! Round 2 binary clipboard and note persistence variants. All projects live in unique temporary directories.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use c13_support::{job_ok, tmp};
use common::*;
use serde_json::json;
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";

fn image_package(dir: &std::path::Path, seed: u64, colour: [u8; 4], entry: &str) -> std::path::PathBuf {
    let mut parts = tt_model::fixture::sample_parts(seed).unwrap();
    let element = parts.elements.iter().find(|e| e.name() == "Background").unwrap();
    let bank = parts.banks.iter().find(|b| Some(b.id()) == element.drawing_bank_id().as_ref()).unwrap();
    let id = &bank.drawings().ids()[0];
    let at = parts.drawings.iter().position(|d| d.id() == id).unwrap();
    let png = tt_render::encode_png(&colour, 1, 1).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    d["nodes"][0]["width"] = json!(1);
    d["nodes"][0]["height"] = json!(1);
    d["nodes"][0]["image"] =
        json!({"entry":entry,"mediaType":"image/png","size":png.len(),"sha256":tt_persist::fsx::sha256(&png)});
    parts.drawings[at] = tt_model::object::from_json(&d).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    blobs.insert(entry.to_owned(), tt_persist::container::BlobData::Bytes(std::sync::Arc::new(png)));
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-11-03T15:00:00Z".into(),
    };
    let path = dir.join(format!("Image-{seed}.animpkg"));
    tt_persist::save::save_package(&path, &input, None).unwrap();
    path
}

#[test]
fn r04_binary_copy_after_close_save_open_retains_pixels() {
    let dir = tmp();
    let source = image_package(&dir, 0x312031, [91, 37, 219, 255], "banks/review-r2-source.png");
    let h = fixture();
    job_ok(&h, "project.open", json!({"path":source,"onUnsaved":"discard"}));
    let drawing = get(&h, BG)["exposure"][0][0].as_str().unwrap().to_owned();
    let original = ok(&h, "draw.drawing.render", json!({"drawingId":drawing,"width":8,"height":8}));
    ok(&h, "cell.clipboard.set", json!({"elementIds":[BG],"from":1,"to":1}));
    ok(&h, "element.delete", json!({"elementIds":[BG]}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    assert!(h.open_document(tt_model::fixture::sample_document(0x312032).unwrap(), None, false).is_ok());
    let copied = ok(&h, "cell.clipboard.apply", json!({"elementId":BG,"frame":30}))["createdDrawingIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let output = dir.join("pasted.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":output}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    job_ok(&h, "project.open", json!({"path":output}));
    let rendered = ok(&h, "draw.drawing.render", json!({"drawingId":copied,"width":8,"height":8}));
    assert!(original["pixelsSha256"].is_string());
    assert_eq!(rendered["pixelsSha256"], original["pixelsSha256"]);
}

#[test]
fn r04_binary_entry_collision_must_not_replace_copied_art_with_destination_bytes() {
    let dir = tmp();
    let entry = "banks/review-r2-shared.png";
    let source = image_package(&dir, 0x312041, [255, 11, 23, 255], entry);
    let destination = image_package(&dir, 0x312042, [17, 255, 31, 255], entry);
    let h = fixture();
    job_ok(&h, "project.open", json!({"path":source,"onUnsaved":"discard"}));
    let original_id = get(&h, BG)["exposure"][0][0].as_str().unwrap().to_owned();
    let original = ok(&h, "draw.drawing.render", json!({"drawingId":original_id,"width":8,"height":8}));
    ok(&h, "cell.clipboard.set", json!({"elementIds":[BG],"from":1,"to":1}));
    job_ok(&h, "project.open", json!({"path":destination,"onUnsaved":"discard"}));
    let copied = ok(&h, "cell.clipboard.apply", json!({"elementId":BG,"frame":30}))["createdDrawingIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let rendered = ok(&h, "draw.drawing.render", json!({"drawingId":copied,"width":8,"height":8}));
    assert!(original["pixelsSha256"].is_string());
    assert_eq!(
        rendered["pixelsSha256"], original["pixelsSha256"],
        "self-contained copy must remap a colliding binary entry"
    );
}

#[test]
fn r05_shifted_notes_save_open_and_undo_redo() {
    let h = fixture();
    let dir = tmp();
    for (frame, text) in [(31, "left731"), (35, "right731")] {
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":frame,"text":text}));
    }
    let before = get(&h, B)["cellNotes"].clone();
    ok(&h, "cell.range.add", json!({"elementIds":[B],"frame":31,"position":"before","count":7}));
    let after = get(&h, B)["cellNotes"].clone();
    assert_eq!(after["38"], "left731");
    assert_eq!(after["42"], "right731");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h, B)["cellNotes"], before);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(get(&h, B)["cellNotes"], after);
    let output = dir.join("notes.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":output}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    job_ok(&h, "project.open", json!({"path":output}));
    assert_eq!(get(&h, B)["cellNotes"], after);
}
