//! Independent C1.3 round-1 persistence regressions. Deterministic inputs; no product edits.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review regression tests")]

mod c13_support;
mod common;

use c13_support::*;
use common::*;
use serde_json::json;
use tt_executor::InvokeOptions;

#[test]
fn dry_run_close_with_save_does_not_write_the_project_or_backups() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let target = dir.join("Dry.animpkg");
    ok(&h, "project.create", json!({"name":"Dry", "path":target}));
    let before = std::fs::read(&target).unwrap();
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"unsaved":true}}}));
    let state = ok(&h, "doc.state.get", json!({}));
    let env =
        with(&h, "project.close", json!({"onUnsaved":"save"}), InvokeOptions { dry_run: true, ..Default::default() });
    assert!(env.is_ok(), "{env:?}");
    let after_state = ok(&h, "doc.state.get", json!({}));
    assert!(
        std::fs::read(&target).unwrap() == before,
        "11 §7.2: dryRun wrote the project; state before={state}, after={after_state}"
    );
    assert_eq!(after_state, state);
    assert!(ok(&h, "project.backup.list", json!({})).as_array().unwrap().is_empty());
}

#[test]
fn save_as_current_path_does_not_overwrite_another_sessions_lock() {
    let dir = tmp();
    let target = dir.join("Locked.animpkg");
    let owner = spawn(&dir.join("owner"));
    ok(&owner, "project.create", json!({"name":"Owner", "path":target}));
    let before = std::fs::read(&target).unwrap();
    let other = spawn(&dir.join("other"));
    job_ok(&other, "project.open", json!({"path":target}));
    assert_eq!(ok(&other, "doc.state.get", json!({}))["readOnly"], true);
    let scene = first(&other, "scene");
    ok(&other, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"other":true}}}));
    let env = run(&other, "project.saveAs", json!({"path":target,"onExists":"overwrite"}));
    if let Some(started) = env.result {
        let rec = wait(&other, &started);
        assert_eq!(rec["state"], "failed", "LOCK-02: same-path Save As must respect the owner's live lock: {rec}");
    } else {
        assert!(env.error.is_some());
    }
    assert_eq!(std::fs::read(&target).unwrap(), before);
}

#[test]
fn folder_entry_change_is_detected_even_when_manifest_is_unchanged() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let target = dir.join("Folder.animpkg");
    std::fs::create_dir_all(&target).unwrap();
    ok(&h, "project.create", json!({"name":"Folder", "path":target,"onExists":"overwrite"}));
    let project_path = target.join("project.json");
    let mut external: serde_json::Value = serde_json::from_slice(&std::fs::read(&project_path).unwrap()).unwrap();
    external["meta"] = json!({"external":true});
    std::fs::write(&project_path, serde_json::to_vec_pretty(&external).unwrap()).unwrap();
    h.run_maintenance(false).unwrap();
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"local":true}}}));
    let env = run(&h, "project.save", json!({}));
    assert_eq!(
        env.error.as_ref().map(|e| e.code.as_str()),
        Some("project.changedOnDisk"),
        "RO-02: external entry edit must block save: {env:?}"
    );
}

#[test]
fn recovering_an_untitled_copy_claims_it_before_a_second_session_can_recover() {
    let dir = tmp();
    let user = dir.join("user");
    let root = user.join("Recovery");
    let doc = tt_model::fixture::sample_document(13).unwrap();
    let id = "rc_1234abcd";
    let info = tt_persist::recovery::RecoveryInfo {
        recovery_id: id.into(),
        project_id: doc.project().id().as_str().into(),
        project_name: "Orphan".into(),
        original_path: None,
        saved_at: "2000-01-01T00:00:00.000Z".into(),
        document_rev: doc.document_rev(),
        app_version: "0.0.1".into(),
        process_id: std::process::id(),
        host: tt_persist::host::host(),
        heartbeat_at: "2000-01-01T00:00:00.000Z".into(),
    };
    tt_persist::recovery::write(
        &root,
        &info,
        &tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &Default::default(),
            session: None,
            saved_at: info.saved_at.clone(),
        },
    )
    .unwrap();
    let a = spawn(&user);
    job_ok(&a, "project.recovery.open", json!({"recoveryId":id}));
    let b = spawn(&user);
    let env = run(&b, "project.recovery.open", json!({"recoveryId":id}));
    if let Some(started) = &env.result {
        let rec = wait(&b, started);
        assert_eq!(rec["state"], "failed", "REC-01: two running sessions recovered the same copy: {rec}");
    }
    assert_eq!(
        env.error.as_ref().map(|e| e.code.as_str()),
        Some("project.recoveryInUse"),
        "REC-01: recovered work must immediately belong to the running session: {env:?}"
    );
}

#[test]
fn failed_package_export_overwrite_keeps_the_previous_destination() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let folder = dir.join("source-folder");
    std::fs::create_dir_all(&folder).unwrap();
    ok(&h, "project.create", json!({"name":"Export","path":folder,"onExists":"overwrite"}));
    ok(&h, "project.close", json!({}));
    std::fs::write(folder.join("vendor.bin"), b"vendor data").unwrap();
    let source = dir.join("source.animpkg");
    let container =
        tt_persist::Container::open(&folder, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
    tt_persist::save::container_to_package(&container, &source).unwrap();
    job_ok(&h, "project.open", json!({"path":source}));
    // An external truncation after indexing makes reading the preserved binary fail.
    std::fs::OpenOptions::new().write(true).open(&source).unwrap().set_len(1).unwrap();
    let target = dir.join("prior-export.animpkg");
    std::fs::write(&target, b"previous complete export").unwrap();
    let started = ok(&h, "doc.package.export", json!({"path":target,"onExists":"overwrite"}));
    let rec = wait(&h, &started);
    assert_eq!(rec["state"], "failed", "{rec}");
    assert_eq!(
        std::fs::read(&target).ok(),
        Some(b"previous complete export".to_vec()),
        "SAVE-01: failed export deleted its destination"
    );
}

#[test]
fn failed_atomic_batch_after_save_does_not_leak_disk_or_clean_marker_changes() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let target = dir.join("Batch.animpkg");
    ok(&h, "project.create", json!({"name":"Batch","path":target}));
    let before = std::fs::read(&target).unwrap();
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"unsaved":true}}}));
    let state = ok(&h, "doc.state.get", json!({}));
    let env = batch(
        &h,
        json!({"label":"Rollback persistence","atomic":true,"steps":[
            {"id":"project.close","params":{"onUnsaved":"save"}},
            {"id":"doc.object.patch","params":{"targets":["missing-reference"],"props":{"meta":{}}}}
        ]}),
    );
    assert!(env.is_error(), "{env:?}");
    let after = ok(&h, "doc.state.get", json!({}));
    assert!(
        std::fs::read(&target).unwrap() == before,
        "11 §7.3: failed atomic batch wrote file; before={state}, after={after}"
    );
    assert_eq!(after, state);
}

#[test]
fn import_replace_replaces_a_same_named_scene_instead_of_renaming_it() {
    let dir = tmp();
    let src = spawn(&dir.join("source"));
    ok(&src, "project.create", json!({"name":"Source"}));
    let scene = first(&src, "scene");
    let bundle =
        ok(&src, "doc.export.json", json!({"scope":"scenes","refs":[scene],"binaries":"omit"}))["bundle"].clone();
    let dst = spawn(&dir.join("destination"));
    ok(&dst, "project.create", json!({"name":"Destination"}));
    let prior = first(&dst, "scene");
    ok(&dst, "doc.import.json", json!({"source":bundle,"onConflict":"replace"}));
    let scenes = ok(&dst, "doc.object.find", json!({"type":"scene"}));
    assert_eq!(scenes["total"], 1, "10 §6.8 / ADR-0014 §7: replace must not append a renamed scene: {scenes}");
    assert_ne!(scenes["items"][0]["id"], prior);
}

#[test]
fn bundle_binary_references_cannot_read_outside_agent_roots() {
    let dir = tmp();
    let allowed = dir.join("allowed");
    std::fs::create_dir_all(&allowed).unwrap();
    let src = fixture();
    let mut bundle = ok(&src, "doc.export.json", json!({"scope":"project","binaries":"omit"}))["bundle"].clone();
    bundle["assets"][0]["entry"] = json!("../outside.bin");
    std::fs::write(dir.join("outside.bin"), b"outside-root-sentinel").unwrap();
    let source = allowed.join("bundle.animjson");
    std::fs::write(&source, serde_json::to_vec(&bundle).unwrap()).unwrap();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Root bounds"}));
    h.grant_roots("s1", vec![allowed]).unwrap();
    let env = h.invoke(tt_executor::Invocation {
        command: "doc.import.json".into(),
        params: json!({"source":source}),
        options: Default::default(),
        origin: agent(&[tt_registry::Scope::Read, tt_registry::Scope::Edit, tt_registry::Scope::FilesRead]),
    });
    assert!(env.is_error(), "11 §6.2: bundle import accepted an out-of-root binary reference: {env:?}");
}

#[test]
fn concurrent_stale_lock_takeover_has_only_one_owner() {
    use std::sync::{Arc, Barrier};
    use tt_persist::lock::{LockInfo, Locked};
    let dir = tmp();
    let target = dir.join("Concurrent.animpkg");
    std::fs::write(&target, b"placeholder").unwrap();
    for trial in 0..64 {
        let old = LockInfo {
            user: "review".into(),
            host: tt_persist::host::host(),
            process_id: std::process::id(),
            app_version: "0.0.1".into(),
            opened_at: "2000-01-01T00:00:00.000Z".into(),
            heartbeat_at: "2000-01-01T00:00:00.000Z".into(),
            lock_id: "stale".into(),
        };
        std::fs::write(tt_persist::lock::lock_path(&target), serde_json::to_vec(&old).unwrap()).unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let target = target.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    tt_persist::lock::acquire(&target)
                })
            })
            .collect();
        let outcomes: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        let owners = outcomes.iter().filter(|o| matches!(o, Locked::Acquired { .. })).count();
        assert_eq!(owners, 1, "LOCK-02: trial {trial}, concurrent stale takeover returned {owners} owners");
    }
}

#[test]
fn persistence_registry_declarations_have_valid_examples_and_record_their_schemas() {
    let mut rows = Vec::new();
    for e in tt_executor::registry()
        .unwrap()
        .entries()
        .filter(|e| e.id().starts_with("project.") || e.id().starts_with("doc.") || e.id() == "app.quit")
    {
        let c = e.command.unwrap();
        let schema = e.params_schema();
        let validator = jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&schema).unwrap();
        for ex in c.decl.examples {
            let v: serde_json::Value = serde_json::from_str(ex.params).unwrap();
            assert!(validator.is_valid(&v), "{} {}", e.id(), ex.title);
        }
        let mut row = e.describe();
        row["specParameters"] = json!(e.spec.params);
        row["specResult"] = json!(e.spec.result);
        row["specUndo"] = json!(e.spec.undo);
        rows.push(row);
    }
    assert_eq!(rows.len(), 48);
    if let Some(out) = std::env::var_os("TT_C13_R1_ARTIFACT_DIR") {
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("persistence-registry.json"), serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    }
}

#[test]
fn package_inspection_reads_a_bounded_header() {
    if std::env::var_os("TT_C13_MEMORY_CHILD").is_some() {
        let dir = tmp();
        let file = dir.join("large.animpkg");
        std::fs::File::create(&file).unwrap().set_len(256 << 20).unwrap();
        let h = spawn(&dir.join("user"));
        let result = ok(&h, "doc.package.get", json!({"path":file,"validate":false}));
        assert_eq!(result["readable"], false);
        return;
    }
    let python = if cfg!(windows) { "python" } else { "python3" };
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reviews/evidence/M1-C1.3-r1/memory-probe.py");
    let output = std::process::Command::new(python).arg(script).arg(std::env::current_exe().unwrap()).output().unwrap();
    assert!(
        output.status.success(),
        "ZIP64 / never-block: inspection read the whole file to identify its header: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn default_preference_values_are_not_persisted_as_overrides() {
    let dir = tmp();
    let user = dir.join("user");
    let h = spawn(&user);
    ok(&h, "project.create", json!({"name":"Defaults"}));
    ok(&h, "project.properties.set", json!({"setAsDefault":true}));
    let settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(user.join("Settings/preferences.json")).unwrap()).unwrap();
    for key in ["project.defaults.frameRate", "project.defaults.cameraWidth", "project.defaults.cameraHeight"] {
        assert!(
            settings["data"].get(key).is_none(),
            "01 §11: default preference {key} was persisted as an override: {settings}"
        );
    }
}

#[test]
fn malformed_numeric_inputs_fail_closed_or_repair_deterministically_under_watchdog() {
    let dir = tmp();
    let folder = dir.join("numeric-folder");
    std::fs::create_dir_all(&folder).unwrap();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Numeric","path":folder,"onExists":"overwrite"}));
    ok(&h, "project.close", json!({}));
    let pp = folder.join("project.json");
    let original: serde_json::Value = serde_json::from_slice(&std::fs::read(&pp).unwrap()).unwrap();
    for number in ["NaN", "1e999", "1e300", "-1e300", "184467440737095516160000000000000000000"] {
        let mut value = original.clone();
        value["frameRate"] = json!("NUMBER_SENTINEL");
        let text = serde_json::to_string(&value).unwrap().replace("\"NUMBER_SENTINEL\"", number);
        std::fs::write(&pp, text).unwrap();
        let mut outcomes = Vec::new();
        for _ in 0..2 {
            let start = std::time::Instant::now();
            let started = ok(&h, "project.open", json!({"path":folder,"onUnsaved":"discard"}));
            let rec = wait(&h, &started);
            assert!(start.elapsed() < std::time::Duration::from_secs(60));
            if rec["state"] == "failed" {
                assert_eq!(rec["error"]["code"], "project.invalidFormat", "{number}: {rec}");
                assert!(h.snapshot().unwrap().is_none());
                outcomes.push(json!({"error":rec["error"]}));
            } else {
                assert_eq!(rec["state"], "succeeded", "{rec}");
                let doc = h.snapshot().unwrap().unwrap();
                let canonical: Vec<_> = doc.entry_keys().iter().map(|key| doc.entry_canonical(key).unwrap()).collect();
                let mut report = ok(&h, "doc.loadReport.get", json!({}));
                report.as_object_mut().unwrap().shift_remove("durationMs");
                outcomes.push(json!({"entries":canonical,"report":report}));
                ok(&h, "project.close", json!({"onUnsaved":"discard"}));
            }
        }
        assert_eq!(outcomes[0], outcomes[1], "LOAD-03 numeric case {number}");
    }
}

#[test]
fn identical_embedded_asset_content_is_stored_once() {
    let dir = tmp();
    let file = dir.join("same.bin");
    let bytes = b"same media content";
    std::fs::write(&file, bytes).unwrap();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Dedup"}));
    let src = fixture();
    let mut bundle = ok(&src, "doc.export.json", json!({"scope":"project","binaries":"omit"}))["bundle"].clone();
    let mut ids = tt_model::IdGenerator::seeded(16, 1_760_486_400_000);
    let duplicate_id = ids.next(tt_model::ObjectType::Asset).unwrap();
    let mut duplicate = bundle["assets"][0].clone();
    duplicate["id"] = json!(duplicate_id);
    duplicate["name"] = json!("second.wav");
    bundle["assets"].as_array_mut().unwrap().push(duplicate);
    let mut added = false;
    'scenes: for scene in bundle["scenes"].as_array_mut().unwrap() {
        for element in scene["elements"].as_array_mut().unwrap() {
            if let Some(placements) = element.get_mut("placements").and_then(serde_json::Value::as_array_mut)
                && !placements.is_empty()
            {
                let mut placement = placements[0].clone();
                placement["id"] = json!(ids.next(tt_model::ObjectType::SoundPlacement).unwrap());
                placement["assetId"] = json!(duplicate_id);
                placement["startFrame"] = json!(9);
                placement["stopFrame"] = json!(16);
                placement["mouthTrack"] = serde_json::Value::Null;
                placements.push(placement);
                added = true;
                break 'scenes;
            }
        }
    }
    assert!(added);
    for asset in bundle["assets"].as_array_mut().unwrap().iter_mut().filter(|a| a["kind"] == "sound") {
        asset["storage"] = json!("linked");
        asset["entry"] = serde_json::Value::Null;
        asset["link"] =
            json!({"kind":"file","path":file,"absolutePath":file,"libraryId":null,"itemId":null,"versionId":null});
        asset["size"] = json!(bytes.len());
        asset["sha256"] = json!(tt_persist::fsx::sha256(bytes));
    }
    ok(&h, "doc.import.json", json!({"source":bundle}));
    assert!(ok(&h, "doc.asset.list", json!({"status":"unused"}))["assets"].as_array().unwrap().is_empty());
    h.run_maintenance(true).unwrap();
    let sound_ids: Vec<_> = ok(&h, "doc.asset.list", json!({"kind":"sound"}))["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].clone())
        .collect();
    assert_eq!(sound_ids.len(), 2);
    ok(&h, "doc.asset.embed", json!({"assetIds":sound_ids}));
    let target = dir.join("dedup.animpkg");
    job_ok(&h, "doc.package.export", json!({"path":target}));
    let c = tt_persist::Container::open(&target, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
    let media: Vec<_> = c.names().into_iter().filter(|n| n.starts_with("media/")).collect();
    assert_eq!(media.len(), 1, "MED-01: identical SHA-256 media was stored twice: {media:?}");
}

#[test]
fn external_change_reload_and_keep_mine_choices_work_on_owned_package() {
    let dir = tmp();
    let target = dir.join("choices.animpkg");
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Choices","path":target}));
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"local":true}}}));
    let external = || {
        let loaded = tt_persist::load(&target, &Default::default()).unwrap();
        let id = loaded.doc.scenes().next().unwrap().id().clone();
        let (doc, _) = loaded
            .doc
            .with_patched_entry(
                &tt_model::document::EntryKey::Scene(id),
                &[tt_model::change::PatchOp {
                    op: "add",
                    from: None,
                    path: "/meta".into(),
                    range: None,
                    value: Some(json!({"external":true})),
                    old_value: None,
                    elided: false,
                }],
            )
            .unwrap();
        tt_persist::save::save_package(
            &target,
            &tt_persist::save::SaveInput {
                doc: &doc,
                preserved: &loaded.preserved,
                blobs: &loaded.blobs,
                session: None,
                saved_at: "2026-10-29T19:00:00.000Z".into(),
            },
            None,
        )
        .unwrap();
    };
    external();
    h.run_maintenance(false).unwrap();
    assert_eq!(err(&h, "project.save", json!({})).code, "project.changedOnDisk");
    job_ok(&h, "project.saveAs", json!({"path":target,"onExists":"overwrite"}));
    assert_eq!(get(&h, &scene)["meta"], json!({"local":true}));
    external();
    h.run_maintenance(false).unwrap();
    job_ok(&h, "project.open", json!({"path":target,"onUnsaved":"discard"}));
    assert_eq!(get(&h, &scene)["meta"], json!({"external":true}));
}

#[test]
fn scheduled_autosave_of_untitled_work_preserves_history_and_location() {
    let dir = tmp();
    let h = tt_executor::ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
        config: tt_executor::ExecutorConfig { budget: std::time::Duration::from_secs(60) },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_executor::UserDirs::under(&dir)),
            maintenance: tt_executor::Maintenance {
                tick: std::time::Duration::from_millis(20),
                autosave: Some(std::time::Duration::from_millis(100)),
                ..Default::default()
            },
            ..Default::default()
        },
    })
    .unwrap();
    ok(&h, "project.create", json!({"name":"Untitled autosave"}));
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"unsaved":true}}}));
    let state = ok(&h, "doc.state.get", json!({}));
    let history = ok(&h, "edit.history.get", json!({}));
    let events = h.subscribe();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let event = loop {
        assert!(std::time::Instant::now() < deadline, "scheduled autosave did not complete");
        if let Ok(e) = events.recv_timeout(std::time::Duration::from_millis(200))
            && e.name == "project.autosaved"
        {
            break e;
        }
    };
    assert_eq!(ok(&h, "doc.state.get", json!({})), state);
    assert_eq!(ok(&h, "edit.history.get", json!({})), history);
    let id = event.payload["recoveryId"].as_str().unwrap();
    let info = tt_persist::recovery::info(&dir.join("Recovery"), id).unwrap();
    assert_eq!(info.original_path, None);
    let loaded = tt_persist::load(&dir.join("Recovery").join(id).join("project"), &Default::default()).unwrap();
    assert_eq!(loaded.doc.document_rev(), state["documentRev"].as_u64().unwrap());
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    assert!(!dir.join("Recovery").join(id).exists());
}

#[test]
fn import_and_repair_undo_labels_match_the_command_table() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Labels"}));
    let source = json!({"format":"animjson","formatVersion":"1.0","scenes":[],"elements":[],"drawingBanks":[],"palettes":[],"pens":[],"blobs":{},"assets":[{"id":"as_01K7AAAAAAAAAAAAAAAAAAAAAA","type":"asset","rev":1,"name":"unused.wav","kind":"sound","mediaType":"audio/wav","storage":"embedded","entry":"media/as_01K7AAAAAAAAAAAAAAAAAAAAAA.wav","link":null,"size":0,"sha256":"0000000000000000000000000000000000000000000000000000000000000000","importedAt":"2026-10-29T00:00:00.000Z","sourcePath":null}]});
    let imported = run(&h, "doc.import.json", json!({"source":source}));
    assert!(imported.is_ok(), "{imported:?}");
    let repaired = run(&h, "doc.repair", json!({"codes":["unusedAssets"]}));
    assert!(repaired.is_ok(), "{repaired:?}");
    assert_eq!(
        (imported.undo.unwrap().label, repaired.undo.unwrap().label),
        ("Import JSON".into(), "Repair Project".into()),
        "10 §10.2 command-table undo labels"
    );
}

#[test]
fn unreferenced_embedded_media_is_removed_from_the_saved_package() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name":"Unused media"}));
    let bytes = b"unused media";
    let source = json!({"format":"animjson","formatVersion":"1.0","scenes":[],"elements":[],"drawingBanks":[],"palettes":[],"pens":[],"blobs":{"media/as_01K7AAAAAAAAAAAAAAAAAAAAAA.wav":{"mediaType":"audio/wav","base64":tt_persist::b64::encode(bytes)}},"assets":[{"id":"as_01K7AAAAAAAAAAAAAAAAAAAAAA","type":"asset","rev":1,"name":"unused.wav","kind":"sound","mediaType":"audio/wav","storage":"embedded","entry":"media/as_01K7AAAAAAAAAAAAAAAAAAAAAA.wav","link":null,"size":bytes.len(),"sha256":tt_persist::fsx::sha256(bytes),"importedAt":"2026-10-29T00:00:00.000Z","sourcePath":null}]});
    ok(&h, "doc.import.json", json!({"source":source}));
    let unused = ok(&h, "doc.asset.list", json!({"status":"unused"}));
    assert_eq!(unused["assets"].as_array().unwrap().len(), 1);
    let target = dir.join("unused.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":target}));
    let c = tt_persist::Container::open(&target, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
    assert!(!c.names().iter().any(|n| n.starts_with("media/")), "MED-04: package retained media with zero references");
}
