//! Independent C1.3 round-2 variants.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review regression tests")]
mod c13_support;
mod common;
use c13_support::*;
use common::*;
use serde_json::json;

#[test]
fn asset_name_conflict_replace_keeps_one_asset_and_undo_restores_original() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Assets"}));
    let mut bundle = json!({"format":"animjson","formatVersion":"1.0","scenes":[],"elements":[],"drawingBanks":[],"palettes":[],"pens":[],"blobs":{},"assets":[{"id":"as_01K7AAAAAAAAAAAAAAAAAAAAAA","type":"asset","rev":1,"name":"same.wav","kind":"sound","mediaType":"audio/wav","storage":"embedded","entry":"media/as_01K7AAAAAAAAAAAAAAAAAAAAAA.wav","link":null,"size":0,"sha256":"0000000000000000000000000000000000000000000000000000000000000000","importedAt":"2026-10-29T00:00:00.000Z","sourcePath":null}]});
    let bytes = b"reviewer media";
    bundle["assets"][0]["size"] = json!(bytes.len());
    bundle["assets"][0]["sha256"] = json!(tt_persist::fsx::sha256(bytes));
    bundle["blobs"]["media/as_01K7AAAAAAAAAAAAAAAAAAAAAA.wav"] =
        json!({"mediaType":"audio/wav","base64":tt_persist::b64::encode(bytes)});
    ok(&h, "doc.import.json", json!({"source":bundle}));
    let before = ok(&h, "doc.asset.list", json!({}));
    let result = run(&h, "doc.import.json", json!({"source":bundle,"onConflict":"replace"}));
    assert!(result.is_ok(), "{result:?}");
    let after = ok(&h, "doc.asset.list", json!({}));
    assert_eq!(after["assets"].as_array().unwrap().len(), 1, "replace left duplicate assets: {after}");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "doc.asset.list", json!({})), before);
}

#[test]
fn thirty_two_concurrent_lock_acquisitions_keep_exactly_one_owner() {
    use std::sync::{Arc, Barrier};
    use tt_persist::lock::{LockInfo, Locked};
    let dir = tmp();
    let target = dir.join("Concurrent.animpkg");
    std::fs::write(&target, b"placeholder").unwrap();
    for stale in [true, false] {
        for trial in 0..64 {
            let path = tt_persist::lock::lock_path(&target);
            let _ = std::fs::remove_file(&path);
            if stale {
                let old = LockInfo {
                    user: "review".into(),
                    host: tt_persist::host::host(),
                    process_id: std::process::id(),
                    app_version: "0.0.1".into(),
                    opened_at: "2000-01-01T00:00:00.000Z".into(),
                    heartbeat_at: "2000-01-01T00:00:00.000Z".into(),
                    lock_id: "stale".into(),
                };
                std::fs::write(path, serde_json::to_vec(&old).unwrap()).unwrap();
            }
            let barrier = Arc::new(Barrier::new(32));
            let threads: Vec<_> = (0..32)
                .map(|_| {
                    let target = target.clone();
                    let barrier = barrier.clone();
                    std::thread::spawn(move || {
                        barrier.wait();
                        tt_persist::lock::acquire(&target)
                    })
                })
                .collect();
            let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
            assert_eq!(
                results.iter().filter(|r| matches!(r, Locked::Acquired { .. })).count(),
                1,
                "stale={stale} trial={trial}: {results:?}"
            );
        }
    }
}

#[test]
fn folder_hidden_preserved_entry_change_blocks_save() {
    let dir = tmp();
    let target = dir.join("Folder");
    std::fs::create_dir_all(&target).unwrap();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Folder", "path":target,"onExists":"overwrite"}));
    ok(&h, "project.close", json!({}));
    std::fs::create_dir_all(target.join("vendor")).unwrap();
    std::fs::write(target.join("vendor/.metadata"), b"old").unwrap();
    job_ok(&h, "project.open", json!({"path":target}));
    std::fs::write(target.join("vendor/.metadata"), b"new-external").unwrap();
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"local":true}}}));
    let result = run(&h, "project.save", json!({}));
    assert_eq!(result.error.as_ref().map(|e| e.code.as_str()), Some("project.changedOnDisk"), "{result:?}");
}

fn undo_bytes(original_form: tt_persist::container::Form, same: bool) {
    use std::sync::Arc;
    use tt_persist::container::{BlobData, Blobs};

    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let doc = tt_model::fixture::sample_document(13).unwrap();
    let mut blobs = Blobs::default();
    for (n, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(n, BlobData::Bytes(Arc::new(b"retained reviewer bytes".to_vec())));
    }
    let source = dir.join("original.animpkg");
    tt_persist::save::save(
        &source,
        original_form,
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
    job_ok(&h, "project.open", json!({"path":source}));
    let assets = ok(&h, "doc.asset.list", json!({}));
    let id = assets["assets"].as_array().unwrap().iter().find(|a| a["storage"] == "embedded").unwrap()["id"].clone();
    let before = ok(&h, "doc.blob.get", json!({"ref":id}))["base64"].clone();
    ok(&h, "doc.asset.remove", json!({"assetIds":[id],"force":true}));
    if same {
        job_ok(&h, "project.save", json!({}));
    } else {
        job_ok(&h, "project.saveAs", json!({"path":dir.join("different.animpkg")}));
    }
    ok(&h, "edit.undo", json!({}));
    let got = run(&h, "doc.blob.get", json!({"ref":id}));
    assert!(got.is_ok(), "MED-04 undo bytes lost: original={original_form:?} same={same}: {got:?}");
    assert_eq!(got.result.unwrap()["base64"], before);
}

#[test]
fn save_as_package_retains_removed_asset_bytes_for_undo() {
    undo_bytes(tt_persist::container::Form::Package, false);
}
#[test]
fn save_as_folder_retains_removed_asset_bytes_for_undo() {
    undo_bytes(tt_persist::container::Form::Folder, false);
}
#[test]
fn same_path_package_save_retains_removed_asset_bytes_for_undo() {
    undo_bytes(tt_persist::container::Form::Package, true);
}
#[test]
fn same_path_folder_save_retains_removed_asset_bytes_for_undo() {
    undo_bytes(tt_persist::container::Form::Folder, true);
}

#[test]
fn dry_run_persistence_paths_do_not_change_disk_state_or_clean_marker() {
    use tt_executor::InvokeOptions;
    for cmd in ["project.close", "app.quit", "project.saveAs", "project.save"] {
        let dir = tmp();
        let h = spawn(&dir.join("user"));
        let path = dir.join("source.animpkg");
        ok(&h, "project.create", json!({"name":"Dry paths","path":path}));
        let scene = first(&h, "scene");
        ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"dirty":true}}}));
        let state = ok(&h, "doc.state.get", json!({}));
        let before = std::fs::read(&path).unwrap();
        let params = match cmd {
            "project.saveAs" => json!({"path":dir.join("copy.animpkg")}),
            "project.save" => json!({}),
            _ => json!({"onUnsaved":"save"}),
        };
        let result = with(&h, cmd, params, InvokeOptions { dry_run: true, ..Default::default() });
        assert!(result.is_ok(), "{cmd}: {result:?}");
        assert_eq!(ok(&h, "doc.state.get", json!({})), state);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!dir.join("copy.animpkg").exists());
    }
}

#[test]
fn gigabyte_package_inspection_is_bounded_with_and_without_validation() {
    if std::env::var_os("TT_C13_MEMORY_CHILD").is_some() {
        let dir = tmp();
        let file = dir.join("large.animpkg");
        std::fs::File::create(&file).unwrap().set_len(1 << 30).unwrap();
        let h = spawn(&dir.join("user"));
        for validate in [false, true] {
            assert_eq!(ok(&h, "doc.package.get", json!({"path":file,"validate":validate}))["readable"], false);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
        return;
    }
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reviews/evidence/M1-C1.3-r2/memory-probe.py");
    let result = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(script)
        .arg(std::env::current_exe().unwrap())
        .arg("gigabyte_package_inspection_is_bounded_with_and_without_validation")
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
}

#[test]
fn successful_atomic_close_with_save_commits_and_emits_the_saved_revision() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let path = dir.join("atomic.animpkg");
    ok(&h, "project.create", json!({"name":"Atomic save","path":path}));
    let scene = first(&h, "scene");
    // Ensure resolve_unsaved sees a dirty live project as well as the staged edit.
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"before":true}}}));
    let before = ok(&h, "doc.state.get", json!({}))["documentRev"].as_u64().unwrap();
    let events = h.subscribe();
    let result = batch(
        &h,
        json!({"label":"Save atomically","atomic":true,"steps":[
            {"id":"doc.object.patch","params":{"targets":[scene],"props":{"meta":{"batch":true}}}},
            {"id":"project.close","params":{"onUnsaved":"save"}}
        ]}),
    );
    assert!(result.is_ok(), "{result:?}");
    let saved = tt_persist::load(&path, &Default::default()).unwrap();
    let changes: Vec<_> = events.try_iter().filter(|e| e.name == "doc.changed").collect();
    assert!(
        changes.iter().any(|e| e.payload["documentRev"] == saved.doc.document_rev()),
        "no successful commit event for written revision {}; before={before}, envelope={result:?}, events={changes:?}",
        saved.doc.document_rev()
    );
}

#[test]
fn folder_visible_mutations_are_detected_without_waiting_for_maintenance_and_own_save_is_clean() {
    for change in ["rename", "delete", "add", "touch", "palette"] {
        let dir = tmp();
        let h = spawn(&dir.join("user"));
        let path = dir.join("folder");
        std::fs::create_dir_all(&path).unwrap();
        ok(&h, "project.create", json!({"name":"Watch","path":path,"onExists":"overwrite"}));
        let scene = first(&h, "scene");
        ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"own":true}}}));
        job_ok(&h, "project.save", json!({}));
        h.run_maintenance(true).unwrap();
        job_ok(&h, "project.save", json!({}));
        match change {
            "rename" => std::fs::rename(path.join("mimetype"), path.join("renamed-mimetype")).unwrap(),
            "delete" => std::fs::remove_file(path.join("mimetype")).unwrap(),
            "add" => std::fs::write(path.join("added.bin"), b"external").unwrap(),
            "touch" => {
                let f = std::fs::File::options().write(true).open(path.join("mimetype")).unwrap();
                f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(10)).unwrap();
            }
            "palette" => {
                let palette = std::fs::read_dir(path.join("palettes")).unwrap().next().unwrap().unwrap().path();
                std::fs::write(palette, b"external palette").unwrap();
            }
            _ => unreachable!(),
        }
        let result = run(&h, "project.save", json!({}));
        assert_eq!(
            result.error.as_ref().map(|e| e.code.as_str()),
            Some("project.changedOnDisk"),
            "{change}: {result:?}"
        );
    }
}

#[test]
fn failed_atomic_quit_save_leaves_disk_and_revision_unchanged() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let path = dir.join("quit.animpkg");
    ok(&h, "project.create", json!({"name":"Quit batch","path":path}));
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"unsaved":true}}}));
    let before = std::fs::read(&path).unwrap();
    let state = ok(&h, "doc.state.get", json!({}));
    let result = batch(
        &h,
        json!({"label":"Quit rollback","atomic":true,"steps":[{"id":"app.quit","params":{"onUnsaved":"save"}},{"id":"doc.object.patch","params":{"targets":["missing-reference"],"props":{"meta":{}}}}]}),
    );
    assert!(result.is_error());
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(ok(&h, "doc.state.get", json!({})), state);
}

fn staged_close_save(command: &str) {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let path = dir.join("clean-batch.animpkg");
    ok(&h, "project.create", json!({"name":"Clean atomic save","path":path}));
    let scene = first(&h, "scene");
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], false);
    let result = batch(
        &h,
        json!({"label":"Edit and save","atomic":true,"steps":[
            {"id":"doc.object.patch","params":{"targets":[scene],"props":{"meta":{"batch":true}}}},
            {"id":command,"params":{"onUnsaved":"save"}}
        ]}),
    );
    assert!(result.is_ok(), "{result:?}");
    let saved = tt_persist::load(&path, &Default::default()).unwrap();
    let key = tt_model::document::EntryKey::Scene(tt_model::Id::parse(scene.as_str()).unwrap());
    assert_eq!(
        saved.doc.entry_raw_json(&key).unwrap()["meta"]["batch"],
        true,
        "{command} discarded the staged edit despite onUnsaved=save: {result:?}"
    );
}
#[test]
fn clean_project_atomic_close_saves_staged_edits() {
    staged_close_save("project.close");
}
#[test]
fn clean_project_atomic_quit_saves_staged_edits() {
    staged_close_save("app.quit");
}
