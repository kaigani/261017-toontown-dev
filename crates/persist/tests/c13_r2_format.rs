//! Independent persistence edge cases for C1.3 round 2.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review regression tests")]
use serde_json::{Value, json};
use tt_persist::{
    container::{Blobs, Form},
    load::Preserved,
    save::{SaveInput, save},
};
fn tmp() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("tt-c13-r2-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn write(root: &std::path::Path, form: Form, d: &tt_model::document::Document, p: &Preserved, b: &Blobs) {
    save(
        root,
        form,
        &SaveInput { doc: d, preserved: p, blobs: b, session: None, saved_at: "2026-10-29T12:00:00.000Z".into() },
        None,
        false,
    )
    .unwrap();
}
#[test]
fn portable_entry_names_reject_windows_special_forms() {
    for n in [
        "a/../b",
        "a/./b",
        "a/NUL.bin",
        "CON",
        "a/COM1.txt",
        "a/name:stream",
        "a/name.",
        "a/name ",
        r"\\server\share",
        r"\\?\C:\file",
        r"a/b\c",
    ] {
        assert!(!tt_persist::zip::safe_name(n), "{n}");
    }
    for n in ["vendor/caf\u{e9}.bin", "vendor/cafe\u{301}.bin"] {
        assert!(tt_persist::zip::safe_name(n));
    }
}
#[test]
fn overlong_component_is_not_a_portable_folder_entry() {
    assert!(
        !tt_persist::zip::safe_name(&format!("vendor/{}.bin", "a".repeat(256))),
        "a component beyond 255 characters cannot be written in the Windows folder form"
    );
}
#[test]
fn document_revision_boundaries_with_and_without_repairs() {
    for rev in [u64::MAX - 1, u64::MAX] {
        for repair in [false, true] {
            let root = tmp();
            let d = tt_model::fixture::sample_document(13).unwrap();
            write(&root, Form::Folder, &d, &Default::default(), &Default::default());
            let mp = root.join("manifest.json");
            let mut m: Value = serde_json::from_slice(&std::fs::read(&mp).unwrap()).unwrap();
            m["documentRev"] = json!(rev);
            std::fs::write(mp, serde_json::to_vec(&m).unwrap()).unwrap();
            if repair {
                let pp = root.join("project.json");
                let mut p: Value = serde_json::from_slice(&std::fs::read(&pp).unwrap()).unwrap();
                p["name"] = json!("Bad/Name");
                std::fs::write(pp, serde_json::to_vec(&p).unwrap()).unwrap();
            }
            let r = tt_persist::load(&root, &Default::default());
            if rev == u64::MAX && repair {
                assert_eq!(r.unwrap_err().details()["reason"], "revisionExhausted");
            } else {
                let l = r.unwrap();
                assert_eq!(l.repaired, repair);
                assert_eq!(l.doc.document_rev(), rev + u64::from(repair));
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
#[test]
fn frozen_entry_nested_unknown_data_survives_other_entry_edit_both_forms_and_reopen() {
    use tt_model::document::EntryKey;
    let root = tmp();
    let d = tt_model::fixture::sample_document(13).unwrap();
    let source = root.join("source");
    write(&source, Form::Folder, &d, &Default::default(), &Default::default());
    let mp = source.join("manifest.json");
    let mut m: Value = serde_json::from_slice(&std::fs::read(&mp).unwrap()).unwrap();
    m["formatVersion"] = json!("1.3");
    std::fs::write(mp, serde_json::to_vec(&m).unwrap()).unwrap();
    let c = tt_persist::Container::open(&source, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
    let n = c.names().into_iter().find(|n| n.contains("/elements/")).unwrap();
    let mut e: Value = serde_json::from_slice(&c.read(&n, 1 << 20).unwrap()).unwrap();
    e["loopType"] = json!("futureLoop");
    e["transform"]["unknownNested"] = json!({"list":[{"future":[1,null,"z"]}]});
    e["futureRoot"] = json!([{"new":true}]);
    e["meta"] = json!({"futureLoop":"valid metadata","nested":[{"x":2}]});
    e["ext"] = json!({"org.example":{"unknown":[3,4]}});
    std::fs::write(source.join(&n), serde_json::to_vec(&e).unwrap()).unwrap();
    let l = tt_persist::load(&source, &Default::default()).unwrap();
    let (edited, _) = l
        .doc
        .with_patched_entry(
            &EntryKey::Project,
            &[tt_model::change::PatchOp {
                op: "add",
                from: None,
                path: "/meta/reviewer".into(),
                range: None,
                value: Some(json!(true)),
                old_value: None,
                elided: false,
            }],
        )
        .unwrap();
    for (f, form) in [("folder", Form::Folder), ("package.animpkg", Form::Package)] {
        let out = root.join(f);
        write(&out, form, &edited, &l.preserved, &l.blobs);
        let c = tt_persist::Container::open(&out, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
        let actual: Value = serde_json::from_slice(&c.read(&n, 1 << 20).unwrap()).unwrap();
        assert_eq!(actual, e, "frozen entry changed in {form:?}");
        let reopened = tt_persist::load(&out, &Default::default()).unwrap();
        assert!(!reopened.preserved.frozen.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn crash_before_journal_publication_leaves_old_document_loadable() {
    let root = tmp();
    let d = tt_model::fixture::sample_document(13).unwrap();
    write(&root, Form::Folder, &d, &Default::default(), &Default::default());
    let before = tt_persist::load(&root, &Default::default()).unwrap();
    // The writer has staged an old copy and new temporary, but has not published its journal.
    let n = format!("palettes/{}.json", d.palettes().next().unwrap().id());
    tt_persist::journal::keep_old(&root, &n).unwrap();
    std::fs::write(root.join("palettes/.interrupted.tmp"), b"pending new bytes").unwrap();
    let after = tt_persist::load(&root, &Default::default()).unwrap();
    for k in before.doc.entry_keys() {
        assert_eq!(before.doc.entry_canonical(&k).unwrap(), after.doc.entry_canonical(&k).unwrap());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_claim_expires_when_actual_child_owner_exits_and_release_restores_metadata() {
    use tt_persist::recovery::{self, Claim, RecoveryInfo};
    if let Some(root) = std::env::var_os("TT_C13_R2_CLAIM_CHILD") {
        assert!(matches!(recovery::claim(std::path::Path::new(&root), "rc_11223344"), Claim::Claimed(_)));
        return;
    }
    let root = tmp();
    let doc = tt_model::fixture::sample_document(13).unwrap();
    let info = RecoveryInfo {
        recovery_id: "rc_11223344".into(),
        project_id: doc.project().id().to_string(),
        project_name: "Claim".into(),
        original_path: None,
        saved_at: "2000-01-01T00:00:00.000Z".into(),
        document_rev: doc.document_rev(),
        app_version: "0.0.1".into(),
        process_id: std::process::id(),
        host: tt_persist::host::host(),
        heartbeat_at: "2000-01-01T00:00:00.000Z".into(),
    };
    recovery::write(
        &root,
        &info,
        &SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &Default::default(),
            session: None,
            saved_at: info.saved_at.clone(),
        },
    )
    .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "recovery_claim_expires_when_actual_child_owner_exits_and_release_restores_metadata",
            "--nocapture",
        ])
        .env("TT_C13_R2_CLAIM_CHILD", &root)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    let dead = recovery::info(&root, &info.recovery_id).unwrap();
    assert_ne!(dead.process_id, std::process::id());
    let Claim::Claimed(previous) = recovery::claim(&root, &info.recovery_id) else {
        panic!("dead child claim did not expire")
    };
    assert_eq!(previous, dead);
    assert!(matches!(recovery::claim(&root, &info.recovery_id), Claim::InUse(_)));
    recovery::release(&root, &previous);
    assert_eq!(recovery::info(&root, &info.recovery_id).unwrap(), dead);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn actual_folder_writer_keeps_manifest_after_removal_renames() {
    let root = tmp();
    let old = tt_model::fixture::sample_document(13).unwrap();
    write(&root, Form::Folder, &old, &Default::default(), &Default::default());
    let stray = root.join("palettes/pl_stray.json");
    std::fs::write(&stray, b"{}\n").unwrap();
    let new = old.with_revisions(old.document_rev() + 1, &[]).unwrap();
    let result = tt_persist::save::save_folder(
        &root,
        &SaveInput {
            doc: &new,
            preserved: &Default::default(),
            blobs: &Default::default(),
            session: None,
            saved_at: "2026-10-29T13:00:00.000Z".into(),
        },
        None,
        tt_persist::save::FolderOptions { crash_after_renames: Some(1), ..Default::default() },
    );
    assert!(result.is_err());
    let manifest: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert!(
        manifest["documentRev"] == old.document_rev() || !stray.exists(),
        "SAVE-02: new manifest published before the pending entry removal"
    );
}
