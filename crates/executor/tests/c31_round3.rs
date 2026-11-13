//! M3 C3.1 round 3 (builder): the coverage each round-3 ruling asks for beyond the reviewer's reproductions — one
//! current staged view for the whole patch (C31-R3-01), live reuse only for copies made in the open project (-02),
//! one staged binary read view (-03) and recovery-texture reachability (-04).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
const D1: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";

// ----- C31-R3-01 ------------------------------------------------------------------------------------------------------

#[test]
fn r01_delegated_sources_see_every_earlier_operation() {
    let h = fixture();
    // `add` then `move` into a delegated property: the rename reads the value just added, the source goes.
    let env = run(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [
            { "op": "add", "path": "/meta/next", "value": "Robert" },
            { "op": "move", "from": "/meta/next", "path": "/name" }
        ] }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let bob = get(&h, "scene:\"Intro\"/element:\"Robert\"");
    assert!(bob["meta"].get("next").is_none(), "{bob}");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h, BOB)["name"], "Bob", "one undo step");
    // `props` then `ops`: a copy reads the merge's value; a later test sees the delegated write.
    let env = run(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "props": { "meta": { "n": "from-props" } }, "ops": [
            { "op": "replace", "path": "/meta/n", "value": "from-ops" },
            { "op": "copy", "from": "/meta/n", "path": "/note" },
            { "op": "test", "path": "/note", "value": "from-ops" }
        ] }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    assert_eq!(get(&h, BOB)["note"], "from-ops");
    // A source removed just before is gone: the whole patch refuses, nothing changes.
    let before = get(&h, BOB);
    let revision = rev(&h);
    let e = err(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [
            { "op": "replace", "path": "/markerColour", "value": "#00FF00" },
            { "op": "remove", "path": "/meta/n" },
            { "op": "copy", "from": "/meta/n", "path": "/note" }
        ] }),
    );
    assert_eq!(e.code, "arg.invalid");
    assert_eq!(get(&h, BOB), before);
    assert_eq!(rev(&h), revision);
}

// ----- C31-R3-02 ------------------------------------------------------------------------------------------------------

/// The fixture's document under another project's ID: every other object ID is the same.
fn twin_project(h: &ExecutorHandle, seed: u64) {
    let mut parts = tt_model::fixture::sample_parts(seed).unwrap();
    let other = tt_model::fixture::sample_parts(seed + 1).unwrap();
    let mut p = serde_json::to_value(&parts.project).unwrap();
    p["id"] = serde_json::to_value(other.project.id()).unwrap();
    parts.project = tt_model::object::from_json(&p).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let e = h.open_document_with(
        doc,
        tt_executor::OpenOptions { on_unsaved: tt_executor::UnsavedPolicy::Discard, ..Default::default() },
        tt_executor::Origin::User,
    );
    assert!(e.is_ok(), "{e:?}");
}

fn open_seed(h: &ExecutorHandle, seed: u64) {
    let e = h.open_document_with(
        tt_model::fixture::sample_document(seed).unwrap(),
        tt_executor::OpenOptions { on_unsaved: tt_executor::UnsavedPolicy::Discard, ..Default::default() },
        tt_executor::Origin::User,
    );
    assert!(e.is_ok(), "{e:?}");
}

#[test]
fn r02_same_project_reuses_live_drawings_other_projects_never_do() {
    const SEED: u64 = 0x0031_3021;
    let h = fixture();
    open_seed(&h, SEED);
    let d1 = get(&h, D1)["id"].clone();
    let bank = get(&h, BOB)["drawingBankId"].clone();
    // Same project: a paste within the element reuses the drawing (clone-group semantics), and a whole-element paste
    // shares the bank.
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 1 }));
    let r = ok(&h, "cell.clipboard.apply", json!({ "elementId": BOB, "frame": 40 }));
    assert_eq!(r["createdDrawingIds"], json!([]), "{r}");
    ok(&h, "edit.copy", json!({ "context": "staging", "objectIds": [BOB] }));
    let made = ok(&h, "edit.paste", json!({ "context": "staging" }))["createdIds"][0].as_str().unwrap().to_owned();
    assert_eq!(get(&h, &made)["drawingBankId"], bank);
    // Another project with the same IDs, whose drawing 1 differs: cells and whole elements come from the copy.
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 1 }));
    let cells = ok(&h, "edit.clipboard.get", json!({ "includeData": true }));
    ok(&h, "edit.copy", json!({ "context": "staging", "objectIds": [BOB] }));
    let elements = ok(&h, "edit.clipboard.get", json!({ "includeData": true }));
    twin_project(&h, SEED);
    assert_eq!(get(&h, D1)["id"], d1, "the twin really shares the IDs");
    ok(&h, "doc.object.patch", json!({ "targets": [D1], "props": { "meta": { "twin": true } } }));
    ok(&h, "edit.clipboard.set", json!({ "content": cells }));
    let r = ok(&h, "cell.clipboard.apply", json!({ "elementId": BOB, "frame": 40, "options": {} }));
    let pasted = r["createdDrawingIds"][0].as_str().unwrap().to_owned();
    assert_ne!(json!(pasted), d1, "a copy from another project is placed from its artwork");
    assert!(get(&h, &pasted)["meta"].get("twin").is_none());
    ok(&h, "edit.clipboard.set", json!({ "content": elements }));
    let made = ok(&h, "edit.paste", json!({ "context": "staging" }))["createdIds"].clone();
    let element = made
        .as_array()
        .unwrap()
        .iter()
        .map(|v| get(&h, v.as_str().unwrap()))
        .find(|e| e["elementType"] == "drawing")
        .unwrap();
    assert_ne!(element["drawingBankId"], bank, "a new bank, not the twin's");
    let first = element["exposure"][0][0].as_str().unwrap();
    assert_ne!(json!(first), d1);
    assert!(get(&h, first)["meta"].get("twin").is_none());
}

// ----- C31-R3-03 ------------------------------------------------------------------------------------------------------

fn image_package(dir: &std::path::Path, seed: u64, colour: [u8; 4]) -> std::path::PathBuf {
    let mut parts = tt_model::fixture::sample_parts(seed).unwrap();
    let element = parts.elements.iter().find(|e| e.name() == "Background").unwrap();
    let bank = parts.banks.iter().find(|b| Some(b.id()) == element.drawing_bank_id().as_ref()).unwrap();
    let did = bank.drawings().ids()[0].clone();
    let at = parts.drawings.iter().position(|d| *d.id() == did).unwrap();
    let png = tt_render::encode_png(&colour, 1, 1).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let entry = d["nodes"][0]["image"]["entry"].as_str().unwrap().to_owned();
    d["nodes"][0]["width"] = json!(1);
    d["nodes"][0]["height"] = json!(1);
    d["nodes"][0]["image"] =
        json!({ "entry": entry, "mediaType": "image/png", "size": png.len(), "sha256": tt_persist::fsx::sha256(&png) });
    parts.drawings[at] = tt_model::object::from_json(&d).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    blobs.insert(entry, tt_persist::container::BlobData::Bytes(std::sync::Arc::new(png)));
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-11-03T19:00:00Z".into(),
    };
    let path = dir.join(format!("Image-{seed}.animpkg"));
    tt_persist::save::save_package(&path, &input, None).unwrap();
    path
}

fn image(h: &ExecutorHandle, d: &str) -> Value {
    ok(h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"][0]["image"].clone()
}

#[test]
fn r03_paste_copy_replace_paste_save_reads_back_the_copied_bytes() {
    let dir = tmp();
    let h = fixture();
    job_ok(
        &h,
        "project.open",
        json!({ "path": image_package(&dir, 0x0031_3031, [12, 200, 40, 255]), "onUnsaved": "discard" }),
    );
    let want = image(&h, get(&h, BG)["exposure"][0][0].as_str().unwrap())["sha256"].clone();
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BG], "from": 1, "to": 1 }));
    open_seed(&h, 0x0031_3032);
    let target = ok(&h, "element.create", json!({ "type": "image", "name": "Staged" }))["elementIds"][0].clone();
    let steps = json!([
        { "id": "cell.clipboard.apply", "params": { "elementId": target, "frame": 9 } },
        { "id": "cell.clipboard.set", "params": { "elementIds": [target], "from": 9, "to": 9 } }
    ]);
    // A dry run of the batch changes neither the project nor the clipboard.
    let before = ok(&h, "edit.clipboard.get", json!({ "includeData": true }));
    let dry = batch(&h, json!({ "label": "dry", "atomic": true, "dryRun": true, "steps": steps }));
    assert_eq!(dry.status, Status::Ok, "{dry:?}");
    assert_eq!(ok(&h, "edit.clipboard.get", json!({ "includeData": true })), before);
    // Accepted: the copy made after the paste carries the bytes the paste staged.
    let r = batch(&h, json!({ "label": "paste then copy", "atomic": true, "steps": steps }));
    assert_eq!(r.status, Status::Ok, "{r:?}");
    // Replace the project; the second-generation copy still pastes its own bytes, which survive save and open.
    open_seed(&h, 0x0031_3033);
    let target = ok(&h, "element.create", json!({ "type": "image", "name": "Again" }))["elementIds"][0].clone();
    let pasted = ok(&h, "cell.clipboard.apply", json!({ "elementId": target, "frame": 1 }))["createdDrawingIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let entry = image(&h, &pasted)["entry"].as_str().unwrap().to_owned();
    let out = dir.join("generations.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": out }));
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    job_ok(&h, "project.open", json!({ "path": out }));
    let loaded = tt_persist::load::load(&out, &Default::default()).unwrap();
    assert_eq!(json!(tt_persist::fsx::sha256(&loaded.blobs.read(&entry, 1 << 20).unwrap())), want);
    // A failed batch leaves the clipboard as it was.
    let clip = ok(&h, "edit.clipboard.get", json!({ "includeData": true }));
    let failed = batch(
        &h,
        json!({ "label": "fails", "atomic": true, "steps": [
            { "id": "cell.clipboard.set", "params": { "elementIds": [BOB], "from": 1, "to": 1 } },
            { "id": "element.rename", "params": { "elementId": BOB, "name": "bad.name" } }
        ] }),
    );
    assert_ne!(failed.status, Status::Ok);
    assert_eq!(ok(&h, "edit.clipboard.get", json!({ "includeData": true })), clip);
}

// ----- C31-R3-04 ------------------------------------------------------------------------------------------------------

/// A project whose drawings' recovery copy of the first palette swatch is a texture, while the live swatch stays a
/// plain colour: only recovery data references the texture.
fn recovery_only_doc(seed: u64, entry: &str) -> tt_model::document::Document {
    let mut parts = tt_model::fixture::sample_parts(seed).unwrap();
    let palette = serde_json::to_value(&parts.palettes[0]).unwrap();
    let mut swatch = palette["swatches"][0].clone();
    swatch["kind"] = json!("texture");
    for value in swatch["values"].as_object_mut().unwrap().values_mut() {
        *value = json!({ "image": entry, "tile": true, "width": 1, "height": 1 });
    }
    let mut used = 0;
    for drawing in &mut parts.drawings {
        let mut value = serde_json::to_value(&*drawing).unwrap();
        if let Some(recs) = value["colourRecovery"].as_array_mut() {
            for rec in recs.iter_mut().filter(|r| r["colourId"] == swatch["colourId"]) {
                rec["swatch"] = swatch.clone();
                used += 1;
            }
        }
        // An opaque namespace naming the entry is not a reference.
        value["meta"] = json!({ "image": entry });
        *drawing = tt_model::object::from_json(&value).unwrap();
    }
    assert!(used > 0, "the fixture's drawings use the first swatch");
    // The live swatch stays a plain colour: no live palette references the texture.
    tt_model::document::Document::from_parts(parts).unwrap()
}

#[test]
fn r04_recovery_textures_are_reachable_without_a_live_palette() {
    let entry = "textures/recovery-only.png";
    let doc = recovery_only_doc(0x0031_3041, entry);
    let saved: Vec<String> = tt_persist::load::saved_blob_refs(&doc).into_iter().map(|r| r.0).collect();
    assert!(saved.iter().any(|e| e == entry), "{saved:?}");
    assert_eq!(saved.iter().filter(|e| *e == entry).count(), 1, "one reference per entry");
    // Saved, reopened, saved again: the recovery-only texture survives both generations with its bytes.
    let dir = tmp();
    let png = tt_render::encode_png(&[3, 140, 250, 255], 1, 1).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    blobs.insert(entry.into(), tt_persist::container::BlobData::Bytes(std::sync::Arc::new(png.clone())));
    let preserved = tt_persist::load::Preserved::default();
    let first = dir.join("recovery.animpkg");
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-11-03T19:00:00Z".into(),
    };
    tt_persist::save::save_package(&first, &input, None).unwrap();
    let h = fixture();
    job_ok(&h, "project.open", json!({ "path": first, "onUnsaved": "discard" }));
    let second = dir.join("recovery-2.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": second }));
    let loaded = tt_persist::load::load(&second, &Default::default()).unwrap();
    assert_eq!(loaded.blobs.read(entry, 1 << 20).unwrap(), png);
}
