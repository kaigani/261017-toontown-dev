//! Final round binary provenance and transaction variants.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
fn package(dir: &std::path::Path, seed: u64, c: u8) -> std::path::PathBuf {
    let mut parts = tt_model::fixture::sample_parts(seed).unwrap();
    if c == 199 {
        let other = tt_model::fixture::sample_parts(seed + 1).unwrap();
        let mut p = serde_json::to_value(&parts.project).unwrap();
        p["id"] = serde_json::to_value(other.project.id()).unwrap();
        parts.project = tt_model::object::from_json(&p).unwrap();
    }
    let e = parts.elements.iter().find(|e| e.name() == "Background").unwrap();
    let b = parts.banks.iter().find(|b| Some(b.id()) == e.drawing_bank_id().as_ref()).unwrap();
    let at = parts.drawings.iter().position(|d| d.id() == &b.drawings().ids()[0]).unwrap();
    let png = tt_render::encode_png(&[c, 93, 127, 255], 1, 1).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let entry = d["nodes"][0]["image"]["entry"].as_str().unwrap().to_owned();
    d["nodes"][0]["width"] = json!(1);
    d["nodes"][0]["height"] = json!(1);
    d["nodes"][0]["image"] =
        json!({"entry":entry,"mediaType":"image/png","size":png.len(),"sha256":tt_persist::fsx::sha256(&png)});
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
    let path = dir.join(format!("seed-{seed}-{c}.animpkg"));
    tt_persist::save::save_package(&path, &input, None).unwrap();
    path
}
fn image(h: &ExecutorHandle, d: &str) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":d}))["nodes"][0]["image"].clone()
}
fn copy(h: &ExecutorHandle) {
    ok(h, "cell.clipboard.set", json!({"elementIds":[BG],"from":1,"to":1}));
}

#[test]
fn same_ids_in_another_project_do_not_reuse_destination_artwork() {
    let dir = tmp();
    let a = package(&dir, 0x313931, 21);
    let b = package(&dir, 0x313931, 199);
    let h = fixture();
    job_ok(&h, "project.open", json!({"path":a,"onUnsaved":"discard"}));
    let did = get(&h, BG)["exposure"][0][0].as_str().unwrap().to_owned();
    let original = image(&h, &did);
    copy(&h);
    job_ok(&h, "project.open", json!({"path":b,"onUnsaved":"discard"}));
    ok(&h, "cell.clipboard.apply", json!({"elementId":BG,"frame":29}));
    let runs = get(&h, BG)["exposure"].as_array().unwrap().clone();
    let mut frame = 0;
    let mut pasted = String::new();
    for r in runs {
        frame += r[1].as_u64().unwrap();
        if frame >= 29 {
            pasted = r[0].as_str().unwrap().to_owned();
            break;
        }
    }
    assert_eq!(
        image(&h, &pasted)["sha256"],
        original["sha256"],
        "a captured copy must retain its source artwork when destination IDs collide"
    );
}

#[test]
fn three_distinct_blobs_and_dedupe_survive_save_open() {
    let dir = tmp();
    let h = fixture();
    let mut clips = vec![];
    let mut hashes = vec![];
    for c in [31, 97, 213] {
        let p = package(&dir, 0x313932, c);
        job_ok(&h, "project.open", json!({"path":p,"onUnsaved":"discard"}));
        hashes.push(image(&h, get(&h, BG)["exposure"][0][0].as_str().unwrap())["sha256"].clone());
        copy(&h);
        clips.push(ok(&h, "edit.clipboard.get", json!({"includeData":true})));
    }
    assert!(h.open_document(tt_model::fixture::sample_document(0x313933).unwrap(), None, false).is_ok());
    let target = ok(&h, "element.create", json!({"type":"image","name":"Three928"}))["elementIds"][0].clone();
    let mut refs = vec![];
    for (i, clip) in clips.iter().chain([&clips[0]]).enumerate() {
        ok(&h, "edit.clipboard.set", json!({"content":clip}));
        let r = ok(&h, "cell.clipboard.apply", json!({"elementId":target,"frame":1+i*3}));
        refs.push(image(&h, r["createdDrawingIds"][0].as_str().unwrap()));
    }
    assert_eq!(refs[0]["entry"], refs[3]["entry"]);
    let entries: std::collections::BTreeSet<_> = refs.iter().map(|v| v["entry"].as_str().unwrap()).collect();
    assert_eq!(entries.len(), 3);
    let out = dir.join("three.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":out}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    job_ok(&h, "project.open", json!({"path":out}));
    let loaded = tt_persist::load::load(&out, &Default::default()).unwrap();
    for (r, want) in refs.iter().zip(hashes.iter().chain([&hashes[0]])) {
        let bytes = loaded.blobs.read(r["entry"].as_str().unwrap(), 1 << 20).unwrap();
        assert_eq!(tt_persist::fsx::sha256(&bytes), *want);
    }
}

#[test]
fn fixture_for_official_sdk_lives_in_temp() {
    let dir = tmp();
    let path = package(&dir, 1, 111);
    println!("C31_R3_SDK_BASE={}", path.display());
}

#[test]
fn paste_then_copy_in_one_atomic_batch_captures_the_staged_binary_bytes() {
    let dir=tmp();let source=package(&dir,0x313934,73);let h=fixture();
    job_ok(&h,"project.open",json!({"path":source,"onUnsaved":"discard"}));copy(&h);
    let original=ok(&h,"edit.clipboard.get",json!({"includeData":true}));
    assert!(h.open_document(tt_model::fixture::sample_document(0x313935).unwrap(),None,false).is_ok());
    let target=ok(&h,"element.create",json!({"type":"image","name":"Staged928"}))["elementIds"][0].clone();
    let r=batch(&h,json!({"label":"Paste then copy928","atomic":true,"steps":[
        {"id":"cell.clipboard.apply","params":{"elementId":target,"frame":9}},
        {"id":"cell.clipboard.set","params":{"elementIds":[target],"from":9,"to":9}}
    ]}));assert_eq!(r.status,tt_executor::Status::Ok,"{r:?}");
    let recopy=ok(&h,"edit.clipboard.get",json!({"includeData":true}));
    assert_eq!(recopy["payload"]["blobs"],original["payload"]["blobs"],"a later step sees the staged drawing and must also capture its staged bytes");
}
