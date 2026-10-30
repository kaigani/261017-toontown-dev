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
fn asset_replace_preserves_unrelated_strings_equal_to_the_old_id() {
    let h = fixture();
    let (id, bundle) = sound_bundle(&h);
    let scene = first(&h, "scene");
    let doc = h.snapshot().unwrap().unwrap();
    let key = tt_model::document::EntryKey::Scene(tt_model::Id::parse(&scene).unwrap());
    let (doc, _) = doc
        .with_patched_entry(
            &key,
            &[tt_model::change::PatchOp {
                op: "replace",
                from: None,
                path: "/name".into(),
                range: None,
                value: Some(json!(id)),
                old_value: None,
                elided: false,
            }],
        )
        .unwrap();
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    ok(&h, "doc.import.json", json!({"source":bundle,"onConflict":"replace"}));
    assert_eq!(get(&h, &scene)["name"], id, "display text is not an asset reference");
}

#[test]
fn asset_replace_with_palette_import_repoints_existing_scene_placements() {
    let h = fixture();
    let (_, mut bundle) = sound_bundle(&h);
    let palette = first(&h, "palette");
    let mut imported_palette = get(&h, &palette);
    imported_palette["name"] = json!("Additional palette");
    bundle["palettes"] = json!([imported_palette]);
    let result = run(&h, "doc.import.json", json!({"source":bundle,"onConflict":"replace"}));
    assert!(result.is_ok(), "a palette style patch must not suppress asset-reference updates: {result:?}");
    ok(&h, "doc.validate", json!({"minSeverity":"error"}));
}

#[test]
fn fixture_open_latency_twenty_samples() {
    let mut samples = Vec::new();
    for _ in 0..20 {
        let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
        let doc = tt_model::fixture::sample_document(1).unwrap();
        let start = std::time::Instant::now();
        assert!(h.open_document(doc, None, false).is_ok());
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reviews/evidence/M1-C1.3-r3/open-latency.json");
    std::fs::write(path, serde_json::to_vec_pretty(&json!({"milliseconds":samples,"method":"same in-memory fixture open_document as watchdog setup; default budget; excludes fixture construction"})).unwrap()).unwrap();
}

#[test]
fn failed_and_dry_atomic_save_variants_preserve_disk_revision_and_events() {
    for dry in [false, true] {
        for cmd in ["project.close", "app.quit"] {
            let dir = tmp();
            let h = spawn(&dir.join("user"));
            let path = dir.join("batch.animpkg");
            ok(&h, "project.create", json!({"name":"Variants","path":path}));
            let scene = first(&h, "scene");
            let before = std::fs::read(&path).unwrap();
            let state = ok(&h, "doc.state.get", json!({}));
            let events = h.subscribe();
            let mut steps = vec![
                json!({"id":"doc.object.patch","params":{"targets":[scene],"props":{"meta":{"variant":true}}}}),
                json!({"id":cmd,"params":{"onUnsaved":"save"}}),
            ];
            if !dry {
                steps.push(json!({"id":"doc.object.patch","params":{"targets":["missing"],"props":{"meta":{}}}}));
            }
            let result = batch(&h, json!({"atomic":true,"label":"Variant","dryRun":dry,"steps":steps}));
            assert_eq!(result.is_ok(), dry, "{result:?}");
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert_eq!(ok(&h, "doc.state.get", json!({})), state);
            assert!(events.try_iter().all(|e| e.name != "doc.changed"));
            std::fs::remove_dir_all(&dir).ok();
        }
    }
}

#[test]
fn repeated_exports_do_not_duplicate_omitted_media_in_session_cache() {
    use std::sync::Arc;
    use tt_persist::container::{BlobData, Blobs, Form};
    let dir = tmp();
    let doc = tt_model::fixture::sample_document(901).unwrap();
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
    ok(&h, "doc.asset.remove", json!({"assetIds":[id],"force":true}));
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
        files.len()
    };
    job_ok(&h, "doc.package.export", json!({"path":dir.join("export.animpkg")}));
    let once = count();
    assert!(once >= 1, "the omitted-media cache path was observed");
    for _ in 0..3 {
        job_ok(&h, "doc.package.export", json!({"path":dir.join("export.animpkg"),"onExists":"overwrite"}));
    }
    assert_eq!(count(), once, "each export copied the same omitted bytes to another random cache directory");
}

#[test]
fn checkpoint_restore_inside_atomic_batch_is_refused_without_saving() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let path = dir.join("Checkpoint.animpkg");
    ok(&h, "project.create", json!({"name":"Checkpoint","path":path}));
    let scene = first(&h, "scene");
    ok(&h, "checkpoint.create", json!({"name":"base"}));
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"before":true}}}));
    let rev_before = rev(&h);
    let events = h.subscribe();
    let result = batch(
        &h,
        json!({"label":"Restore and save","steps":[
            {"id":"doc.object.patch","params":{"targets":[scene],"expectRev":obj_rev(&h,&scene),"props":{"meta":{"staged":true}}}},
            {"id":"checkpoint.restore","params":{"name":"base"}},
            {"id":"project.close","params":{"onUnsaved":"save"}}
        ]}),
    );
    assert!(result.is_error(), "{result:?}");
    assert_eq!(result.error.unwrap().details["error"]["code"], "command.disabled");
    assert_eq!(rev(&h), rev_before);
    let saved = tt_persist::load(&path, &Default::default()).unwrap();
    assert!(saved.doc.document_rev() < rev_before);
    assert_eq!(events.try_iter().filter(|e| e.name == "doc.changed").count(), 0);
}

#[test]
fn save_as_job_and_stale_expect_rev_inside_atomic_batch_roll_back() {
    for save_as in [true, false] {
        let dir = tmp();
        let h = spawn(&dir.join("user"));
        let path = dir.join("Source.animpkg");
        ok(&h, "project.create", json!({"name":"Guards","path":path}));
        let scene = first(&h, "scene");
        let state = ok(&h, "doc.state.get", json!({}));
        let before = std::fs::read(&path).unwrap();
        let last = if save_as {
            json!({"id":"project.saveAs","params":{"path":dir.join("Elsewhere.animpkg")}})
        } else {
            json!({"id":"doc.object.patch","params":{"targets":[scene],"expectRev":0,"props":{"meta":{}}}})
        };
        let result = batch(
            &h,
            json!({"label":"Guards","steps":[
                {"id":"doc.object.patch","params":{"targets":[scene],"props":{"meta":{"edit":true}}}},last
            ]}),
        );
        assert!(result.is_error(), "{result:?}");
        let error = result.error.unwrap();
        if save_as {
            assert_eq!(error.code, "batch.jobNotAllowed");
        } else {
            assert_eq!(error.details["error"]["code"], "rev.conflict");
        }
        assert_eq!(ok(&h, "doc.state.get", json!({})), state);
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(!dir.join("Elsewhere.animpkg").exists());
    }
}

#[test]
fn folder_fingerprint_includes_nested_hidden_names_but_excludes_root_controls_and_session() {
    for name in [
        ".control",
        ".git/nested",
        "session.json",
        "vendor/.hidden",
        "vendor/.deep/nested/.leaf",
        "vendor/session.json",
    ] {
        let dir = tmp();
        let target = dir.join("Folder");
        std::fs::create_dir_all(&target).unwrap();
        let h = spawn(&dir.join("user"));
        ok(&h, "project.create", json!({"name":"Hidden","path":target,"onExists":"overwrite"}));
        ok(&h, "project.close", json!({}));
        let file = target.join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"{}").unwrap();
        job_ok(&h, "project.open", json!({"path":target}));
        std::fs::write(&file, b"{\"external\":true}").unwrap();
        let result = run(&h, "project.save", json!({}));
        if name.starts_with("vendor/") {
            assert_eq!(result.error.unwrap().code, "project.changedOnDisk", "{name}");
        } else {
            assert!(result.is_ok(), "{name}: {result:?}");
            wait(&h, result.result.as_ref().unwrap());
        }
    }
}

#[test]
fn media_survives_cross_form_save_as_and_recovery_deletion_with_two_undos() {
    use std::sync::Arc;
    use tt_persist::container::{BlobData, Blobs, Form};
    for recovered in [false, true] {
        for source_form in [Form::Package, Form::Folder] {
            let dir = tmp();
            let user = dir.join("user");
            let doc = tt_model::fixture::sample_document(13).unwrap();
            let mut blobs = Blobs::default();
            for (name, _, _, _) in tt_persist::load::blob_refs(&doc) {
                blobs.insert(name, BlobData::Bytes(Arc::new(b"undo across forms".to_vec())));
            }
            let input = tt_persist::save::SaveInput {
                doc: &doc,
                preserved: &Default::default(),
                blobs: &blobs,
                session: None,
                saved_at: "2000-01-01T00:00:00.000Z".into(),
            };
            let h = spawn(&user);
            let recovery_folder = user.join("Recovery/rc_1234abcd");
            if recovered {
                let info = tt_persist::recovery::RecoveryInfo {
                    recovery_id: "rc_1234abcd".into(),
                    project_id: doc.project().id().as_str().into(),
                    project_name: "Media".into(),
                    original_path: None,
                    saved_at: input.saved_at.clone(),
                    document_rev: doc.document_rev(),
                    app_version: "0.0.1".into(),
                    process_id: std::process::id(),
                    host: tt_persist::host::host(),
                    heartbeat_at: input.saved_at.clone(),
                };
                tt_persist::recovery::write(&user.join("Recovery"), &info, &input).unwrap();
                job_ok(&h, "project.recovery.open", json!({"recoveryId":"rc_1234abcd"}));
            } else {
                let source = dir.join("source.animpkg");
                tt_persist::save::save(&source, source_form, &input, None, false).unwrap();
                job_ok(&h, "project.open", json!({"path":source}));
            }
            let assets = ok(&h, "doc.asset.list", json!({}));
            let list = assets["assets"].as_array().unwrap();
            assert!(list.len() >= 2);
            let embedded = list.iter().find(|a| a["storage"] == "embedded").unwrap()["id"].clone();
            let before = ok(&h, "doc.blob.get", json!({"ref":embedded}))["base64"].clone();
            for asset in list.iter().take(2) {
                ok(&h, "doc.asset.remove", json!({"assetIds":[asset["id"]],"force":true}));
            }
            let target = dir.join("destination.animpkg");
            if source_form == Form::Package {
                std::fs::create_dir_all(&target).unwrap();
            }
            job_ok(&h, "project.saveAs", json!({"path":target,"onExists":"overwrite"}));
            if recovered {
                assert!(!recovery_folder.exists(), "AUTO-04 deleted old source");
            }
            ok(&h, "edit.undo", json!({}));
            ok(&h, "edit.undo", json!({}));
            assert_eq!(ok(&h, "doc.asset.list", json!({})), assets);
            assert_eq!(ok(&h, "doc.blob.get", json!({"ref":embedded}))["base64"], before);
        }
    }
}

#[test]
fn movie_asset_replace_repoints_sequences_and_one_undo_restores_exact_entries() {
    fn remove_volatile_revisions(value: &mut Value) {
        match value {
            Value::Object(map) => {
                if map.contains_key("id") && map.contains_key("type") {
                    map.shift_remove("rev");
                }
                for (key, value) in map {
                    if key != "meta" && key != "ext" {
                        remove_volatile_revisions(value);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(remove_volatile_revisions),
            _ => {}
        }
    }
    for preserve in [false, true] {
        let h = fixture();
        let movie = ok(&h, "doc.asset.list", json!({}))["assets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["kind"] == "movie")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut asset = get(&h, &movie);
        asset.as_object_mut().unwrap().shift_remove("path");
        let before = h.snapshot().unwrap().unwrap();
        let bundle = json!({"format":"animjson","formatVersion":"1.0","scenes":[],"elements":[],"drawingBanks":[],"palettes":[],"pens":[],"assets":[asset],"blobs":{}});
        let imported = ok(
            &h,
            "doc.import.json",
            json!({"source":bundle,"onConflict":"replace","idPolicy":if preserve {"preserve"} else {"new"}}),
        );
        let new_id = imported["created"]["assets"][0].as_str().unwrap();
        let sequences = ok(&h, "doc.object.find", json!({"type":"mediaSequence"}));
        assert!(!sequences["items"].as_array().unwrap().is_empty());
        for sequence in sequences["items"].as_array().unwrap() {
            assert_eq!(get(&h, sequence["id"].as_str().unwrap())["assetId"], new_id);
        }
        ok(&h, "edit.undo", json!({}));
        let after = h.snapshot().unwrap().unwrap();
        for key in before.entry_keys() {
            let mut expected = before.entry_raw_json(&key).unwrap();
            let mut actual = after.entry_raw_json(&key).unwrap();
            remove_volatile_revisions(&mut expected);
            remove_volatile_revisions(&mut actual);
            assert_eq!(actual, expected, "undo {key:?}");
        }
    }
}
