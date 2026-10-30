//! Independent C1.3 round-1 format and interrupted-save regressions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review regression tests")]
use serde_json::{Value, json};
use tt_persist::container::{Blobs, Form};
use tt_persist::load::Preserved;
use tt_persist::save::{SaveInput, save};

fn tmp() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("tt-c13-r1-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn partial_folder_rollback_restores_every_already_replaced_entry() {
    // Round-2 orchestrator ruling: replace the unsatisfiable v1 fixture with v2 old copies.
    use tt_persist::journal::{Journal, Replace, Version};
    for done in 0..=3 {
        for mode in ["complete", "rollback", "unrecoverable"] {
            let root = tmp();
            std::fs::create_dir_all(root.join("palettes")).unwrap();
            let mut j = Journal::default();
            let targets = ["project.json", "palettes/p.json", "manifest.json"];
            for (i, target) in targets.iter().enumerate() {
                let parent = std::path::Path::new(target).parent().unwrap();
                let old = format!("old-{i}").into_bytes();
                let new = format!("new-{i}").into_bytes();
                let old_file = parent.join(format!(".entry{i}.old")).to_string_lossy().replace('\\', "/");
                let new_file = parent.join(format!(".entry{i}.tmp")).to_string_lossy().replace('\\', "/");
                std::fs::write(root.join(target), &old).unwrap();
                std::fs::write(root.join(&old_file), &old).unwrap();
                std::fs::write(root.join(&new_file), &new).unwrap();
                j.replace.push(Replace {
                    target: target.to_string(),
                    old: Some(Version {
                        file: old_file,
                        size: old.len() as u64,
                        sha256: tt_persist::fsx::sha256(&old),
                    }),
                    new: Version { file: new_file, size: new.len() as u64, sha256: tt_persist::fsx::sha256(&new) },
                });
            }
            let jp = root.join(".save-journal.json");
            std::fs::write(&jp, serde_json::to_vec(&j.to_json()).unwrap()).unwrap();
            for r in j.replace.iter().take(done) {
                std::fs::rename(root.join(&r.new.file), root.join(&r.target)).unwrap();
            }
            if mode != "complete" {
                // Lose one new version even after all renames: rollback must remain possible.
                let r = &j.replace[2];
                let f = if done == 3 { &r.target } else { &r.new.file };
                std::fs::remove_file(root.join(f)).unwrap();
            }
            if mode == "unrecoverable" {
                let r = &j.replace[0];
                std::fs::remove_file(root.join(&r.old.as_ref().unwrap().file)).unwrap();
                std::fs::write(root.join(&r.target), b"neither version").unwrap();
            }
            let before: Vec<_> = j
                .replace
                .iter()
                .flat_map(|r| [r.target.clone(), r.new.file.clone(), r.old.as_ref().unwrap().file.clone()])
                .chain([".save-journal.json".to_owned()])
                .map(|n| (n.clone(), std::fs::read(root.join(n)).ok()))
                .collect();
            let result = tt_persist::save::recover_journal(&root, false, &mut Default::default());
            if mode == "unrecoverable" {
                assert_eq!(result.unwrap_err().details()["reason"], "unrecoverableSave");
                for (n, bytes) in before {
                    assert_eq!(std::fs::read(root.join(n)).ok(), bytes);
                }
            } else {
                result.unwrap();
                for (i, target) in targets.iter().enumerate() {
                    let prefix = if mode == "complete" { "new" } else { "old" };
                    assert_eq!(
                        std::fs::read(root.join(target)).unwrap(),
                        format!("{prefix}-{i}").as_bytes(),
                        "{done} {mode} {target}"
                    );
                }
                assert!(!jp.exists());
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn folder_has_the_same_mimetype_entry_as_package() {
    let root = tmp();
    let mut ids = tt_model::IdGenerator::seeded(13, 1_760_486_400_000);
    let (doc, _) = tt_model::defaults::new_project(
        &tt_model::defaults::NewProject {
            name: "Format".into(),
            frame_rate: 24.0,
            camera_width: 1920,
            camera_height: 1080,
        },
        &mut ids,
        "2026-10-19T12:00:00.000Z",
    )
    .unwrap();
    save(
        &root,
        Form::Folder,
        &SaveInput {
            doc: &doc,
            preserved: &Preserved::default(),
            blobs: &Blobs::default(),
            session: None,
            saved_at: "2026-10-19T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(root.join("mimetype")).unwrap(),
        b"application/x-animpkg+zip",
        "FMT-01 identical entry trees"
    );
}

#[test]
fn malformed_journal_cannot_delete_an_arbitrary_project_entry() {
    let root = tmp();
    std::fs::write(root.join("project.json"), b"user content").unwrap();
    let journal: Value = json!({"format":"unrelated-format", "deletions":["project.json"]});
    std::fs::write(root.join(".save-journal.json"), serde_json::to_vec(&journal).unwrap()).unwrap();
    let mut issues = tt_persist::report::Issues::default();
    let _ = tt_persist::save::recover_journal(&root, false, &mut issues);
    assert_eq!(
        std::fs::read(root.join("project.json")).unwrap(),
        b"user content",
        "malformed journal must fail closed"
    );
}

fn fixture_folder(root: &std::path::Path) -> std::path::PathBuf {
    let folder = root.join("fixture");
    let doc = tt_model::fixture::sample_document(13).unwrap();
    save(
        &folder,
        Form::Folder,
        &SaveInput {
            doc: &doc,
            preserved: &Preserved::default(),
            blobs: &Blobs::default(),
            session: None,
            saved_at: "2026-10-19T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    folder
}

#[test]
fn newer_minor_unknown_properties_survive_on_the_same_frozen_entry() {
    let root = tmp();
    let folder = fixture_folder(&root);
    let manifest = folder.join("manifest.json");
    let mut m: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    m["formatVersion"] = json!("1.3");
    std::fs::write(&manifest, serde_json::to_vec(&m).unwrap()).unwrap();
    let idx = tt_persist::container::FolderIndex::scan(&folder, Default::default()).unwrap();
    let name = idx.files().keys().find(|n| n.contains("/elements/")).unwrap();
    let path = folder.join(name);
    let mut e: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    e["loopType"] = json!("futureLoop");
    e["futureProperty"] = json!({"content":[1,2,3]});
    e["transform"]["futureTransform"] = json!(17);
    std::fs::write(&path, serde_json::to_vec(&e).unwrap()).unwrap();
    let loaded = tt_persist::load(&folder, &Default::default()).unwrap();
    assert!(!loaded.preserved.frozen.is_empty());
    let out = root.join("out.animpkg");
    save(
        &out,
        Form::Package,
        &SaveInput {
            doc: &loaded.doc,
            preserved: &loaded.preserved,
            blobs: &loaded.blobs,
            session: None,
            saved_at: "2026-10-19T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    let c = tt_persist::Container::open(&out, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
    let actual: Value = serde_json::from_slice(&c.read(name, 1 << 20).unwrap()).unwrap();
    assert_eq!(actual["futureProperty"], e["futureProperty"], "VER-02: frozen entry lost its unknown properties");
    assert_eq!(actual["transform"]["futureTransform"], e["transform"]["futureTransform"]);
}

#[test]
fn existing_project_thumbnail_survives_save() {
    let root = tmp();
    let folder = fixture_folder(&root);
    std::fs::write(folder.join("thumbnail.png"), b"existing-thumbnail").unwrap();
    let loaded = tt_persist::load(&folder, &Default::default()).unwrap();
    let planned = tt_persist::save::plan(&SaveInput {
        doc: &loaded.doc,
        preserved: &loaded.preserved,
        blobs: &loaded.blobs,
        session: None,
        saved_at: "2026-10-19T12:00:00.000Z".into(),
    })
    .unwrap();
    assert!(planned.iter().any(|p| p.name == "thumbnail.png"), "ADR-0014 §17: existing thumbnail must be preserved");
}

#[test]
fn repairs_at_exhausted_document_revision_fail_closed() {
    let root = tmp();
    let folder = fixture_folder(&root);
    let mp = folder.join("manifest.json");
    let mut m: Value = serde_json::from_slice(&std::fs::read(&mp).unwrap()).unwrap();
    m["documentRev"] = json!(u64::MAX);
    std::fs::write(&mp, serde_json::to_vec(&m).unwrap()).unwrap();
    let pp = folder.join("project.json");
    let mut p: Value = serde_json::from_slice(&std::fs::read(&pp).unwrap()).unwrap();
    p["name"] = json!("Bad/Name");
    std::fs::write(&pp, serde_json::to_vec(&p).unwrap()).unwrap();
    let loaded = tt_persist::load(&folder, &Default::default());
    assert!(
        loaded.is_err(),
        "REV-03: repair needs a new revision; must not saturate and report clean: {:?}",
        loaded.as_ref().ok().map(|l| (l.repaired, l.doc.document_rev(), l.report.repaired_count))
    );
}

#[test]
fn package_and_folder_round_trip_all_fixture_entries_and_binary_bytes() {
    let root = std::env::var_os("TT_C13_R1_ARTIFACT_DIR")
        .map(|p| std::path::PathBuf::from(p).join("format-samples"))
        .unwrap_or_else(tmp);
    std::fs::create_dir_all(&root).unwrap();
    let doc = tt_model::fixture::sample_document(0xc13120261029).unwrap();
    let mut blobs = Blobs::default();
    for (entry, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(
            entry.clone(),
            tt_persist::container::BlobData::Bytes(std::sync::Arc::new(format!("binary\0\r\n{entry}").into_bytes())),
        );
    }
    let input = SaveInput {
        doc: &doc,
        preserved: &Preserved::default(),
        blobs: &blobs,
        session: None,
        saved_at: "2026-10-19T12:00:00.000Z".into(),
    };
    for (name, form) in [("byte-check.animpkg", Form::Package), ("byte-check-folder", Form::Folder)] {
        let path = root.join(name);
        save(&path, form, &input, None, true).unwrap();
        let loaded = tt_persist::load(&path, &Default::default()).unwrap();
        assert!(!loaded.repaired, "{:?}", loaded.report);
        assert_eq!(loaded.doc.document_rev(), doc.document_rev());
        for key in doc.entry_keys() {
            assert_eq!(loaded.doc.entry_canonical(&key).unwrap(), doc.entry_canonical(&key).unwrap());
        }
        for (entry, _, _, _) in tt_persist::load::blob_refs(&doc) {
            assert_eq!(loaded.blobs.read(&entry, 1 << 20).unwrap(), blobs.read(&entry, 1 << 20).unwrap());
        }
    }
}

#[test]
fn zip_names_reject_drive_prefixes_in_every_path_component() {
    for name in ["vendor/C:/escape.bin", "vendor/D:relative.bin"] {
        let joined = name.split('/').fold(std::path::PathBuf::from(r"C:\intended-root"), |p, s| p.join(s));
        assert!(
            !tt_persist::zip::safe_name(name),
            "ZIP-01: nested drive prefix accepted: {name}; joined path={joined:?}"
        );
    }
}
