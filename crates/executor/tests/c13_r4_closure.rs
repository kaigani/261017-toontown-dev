//! Independent final-round persistence variants.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review regressions")]
mod c13_support;
mod common;
use c13_support::*;
use common::*;
use serde_json::{Value, json};

fn sound_bundle(h: &tt_executor::ExecutorHandle) -> (String, Value) {
    let assets = ok(h, "doc.asset.list", json!({}));
    let id = assets["assets"].as_array().unwrap().iter().find(|a| a["kind"] == "sound").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut asset = get(h, &id);
    asset.as_object_mut().unwrap().shift_remove("path");
    (
        id,
        json!({"format":"animjson","formatVersion":"1.0","scenes":[],"elements":[],"drawingBanks":[],"palettes":[],"pens":[],"assets":[asset],"blobs":{}}),
    )
}

#[test]
fn label_and_real_reference_in_one_entry_are_distinguished() {
    for (ty, pointer, kind) in [
        (tt_model::ObjectType::Element, "/placements/0/assetId", "asset"),
        (tt_model::ObjectType::Element, "/placements/0/mouthTrack/producedFor/assetId", "asset"),
        (tt_model::ObjectType::Element, "/sequences/0/assetId", "asset"),
        (tt_model::ObjectType::Drawing, "/nodes/0/strokes/0/spine/pen/penId", "pen"),
    ] {
        assert_eq!(tt_model::schema::property_schema(ty, pointer).unwrap()["x-ref"], kind);
    }
    let h = fixture();
    let (id, bundle) = sound_bundle(&h);
    let doc = h.snapshot().unwrap().unwrap();
    let key = doc
        .entry_keys()
        .into_iter()
        .find(|k| doc.entry_raw_json(k).unwrap().pointer("/placements/0/mouthTrack").is_some_and(|v| !v.is_null()))
        .unwrap();
    let mut raw = doc.entry_raw_json(&key).unwrap();

    raw["placements"][0]["mouthTrack"]["labels"] = json!({"1":id});
    raw["meta"] = json!({"assetId":id});
    raw["ext"] = json!({"org.review":{"assetId":id}});
    let (doc, _) = doc
        .with_patched_entry(
            &key,
            &[tt_model::change::PatchOp {
                op: "replace",
                from: None,
                path: "".into(),
                range: None,
                value: Some(raw),
                old_value: None,
                elided: false,
            }],
        )
        .unwrap();
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(doc, None, false).error.is_none());
    let imported = ok(&h, "doc.import.json", json!({"source":bundle,"onConflict":"replace"}));
    let new_id = &imported["created"]["assets"][0];
    assert_ne!(new_id, &json!(id));
    let raw = h.snapshot().unwrap().unwrap().entry_raw_json(&key).unwrap();
    assert_eq!(&raw["placements"][0]["assetId"], new_id);
    assert_eq!(&raw["placements"][0]["mouthTrack"]["producedFor"]["assetId"], new_id);
    assert_eq!(raw["placements"][0]["mouthTrack"]["labels"]["1"], id);
    assert_eq!(raw["meta"]["assetId"], id);
    assert_eq!(raw["ext"]["org.review"]["assetId"], id);
}

#[test]
fn remove_export_three_undo_blob_close_clears_cache() {
    use std::sync::Arc;
    use tt_persist::container::{BlobData, Blobs, Form};
    let dir = tmp();
    let doc = tt_model::fixture::sample_document(9401).unwrap();
    let mut blobs = Blobs::default();
    for (n, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(n, BlobData::Bytes(Arc::new(vec![42; 4096])));
    }
    let source = dir.join("Source.animpkg");
    tt_persist::save::save(
        &source,
        Form::Package,
        &tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-29T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    let h = spawn(&dir.join("user"));
    job_ok(&h, "project.open", json!({"path":source}));
    let assets = ok(&h, "doc.asset.list", json!({}));
    let asset = assets["assets"].as_array().unwrap().iter().find(|a| a["storage"] == "embedded").unwrap();
    let id = asset["id"].clone();
    let filename = get(&h, id.as_str().unwrap())["entry"].as_str().unwrap().replace('/', "_");
    let before = ok(&h, "doc.blob.get", json!({"ref":id}))["base64"].clone();
    ok(&h, "doc.asset.remove", json!({"assetIds":[id],"force":true}));
    let observed_cache_roots = std::cell::RefCell::new(std::collections::BTreeSet::new());
    let count = || {
        let mut files = Vec::new();
        fn walk(p: &std::path::Path, wanted: &str, files: &mut Vec<std::path::PathBuf>) {
            if let Ok(entries) = std::fs::read_dir(p) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        walk(&p, wanted, files);
                    } else if entry.file_name() == wanted {
                        files.push(p);
                    }
                }
            }
        }
        for entry in std::fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
            if entry.file_name().to_string_lossy().starts_with(&format!("tt-cache-{}-", std::process::id())) {
                walk(&entry.path(), &filename, &mut files);
            }
        }
        for file in &files {
            let cache = file.ancestors().nth(3).unwrap();
            assert!(cache.file_name().unwrap().to_string_lossy().starts_with("tt-cache-"));
            observed_cache_roots.borrow_mut().insert(cache.to_path_buf());
        }
        files.len()
    };
    job_ok(&h, "doc.package.export", json!({"path":dir.join("export.animpkg")}));
    let once = count();
    assert!(once >= 1, "the omitted-media cache path was observed");
    for _ in 0..2 {
        job_ok(&h, "doc.package.export", json!({"path":dir.join("export.animpkg"),"onExists":"overwrite"}));
    }
    assert_eq!(count(), once, "each export copied the same omitted bytes to another random cache directory");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "doc.blob.get", json!({"ref":id}))["base64"], before);
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    assert_eq!(count(), 0);
    assert!(
        observed_cache_roots.borrow().iter().all(|root| !root.exists()),
        "close removes the observed cache directories"
    );
    drop(h);
    assert_eq!(count(), 0);
}
