//! SAVE-02 fault injection (C13-R1-03/04): a folder save interrupted after any number of renames is recovered to
//! exactly the new version when every new entry is present, and to exactly the previous version when a pending new
//! entry was lost; a journal that names neither complete version changes nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};
use tt_persist::container::{Blobs, Form};
use tt_persist::load::Preserved;
use tt_persist::report::Issues;
use tt_persist::save::{FolderOptions, SaveInput, save, save_folder};

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-journal-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn input<'a>(d: &'a Document, p: &'a Preserved, b: &'a Blobs) -> SaveInput<'a> {
    SaveInput { doc: d, preserved: p, blobs: b, session: None, saved_at: "2026-10-29T12:00:00.000Z".into() }
}

/// Every file of the folder except control files, by relative path.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn patch(doc: &Document, key: &EntryKey, path: &str, value: Value) -> Document {
    doc.with_patched_entry(
        key,
        &[tt_model::change::PatchOp {
            op: "add",
            from: None,
            path: path.into(),
            range: None,
            value: Some(value),
            old_value: None,
            elided: false,
        }],
    )
    .unwrap()
    .0
}

/// The previous version and the changed version (three entries changed).
fn versions() -> (Document, Document) {
    let doc = tt_model::fixture::sample_document(7).unwrap();
    let mut next = doc.clone();
    let mut changed = 0;
    for key in doc.entry_keys() {
        if matches!(key, EntryKey::Element(_) | EntryKey::Palette(_) | EntryKey::Project) && changed < 3 {
            next = patch(&next, &key, "/meta/journalTest", json!(changed));
            changed += 1;
        }
    }
    (doc, next)
}

/// A palette entry no palette owns: the next save removes it (LAY-03), so the journal also has a removal.
fn stray(folder: &Path) {
    std::fs::write(folder.join("palettes").join("pl_stray.json"), b"{}
").unwrap();
}

#[test]
fn a_save_interrupted_after_any_rename_recovers_to_one_whole_version() {
    let (old, new) = versions();
    let (p, b) = (Preserved::default(), Blobs::default());
    let reference = tmp();
    save(&reference.join("old"), Form::Folder, &input(&old, &p, &b), None, false).unwrap();
    stray(&reference.join("old"));
    save(&reference.join("new"), Form::Folder, &input(&new, &p, &b), None, false).unwrap();
    let (old_files, new_files) = (files(&reference.join("old")), files(&reference.join("new")));
    assert_ne!(old_files, new_files);
    let mut steps = 0;
    loop {
        for lose_pending in [false, true] {
            let dir = tmp();
            let folder = dir.join("F.animpkg");
            save(&folder, Form::Folder, &input(&old, &p, &b), None, false).unwrap();
            stray(&folder);
            let r = save_folder(
                &folder,
                &input(&new, &p, &b),
                None,
                FolderOptions { crash_after_renames: Some(steps), ..Default::default() },
            );
            let journal_path = folder.join(".save-journal.json");
            if r.is_ok() {
                // No more steps to interrupt.
                assert_eq!(files(&folder), new_files);
                let _ = std::fs::remove_dir_all(&dir);
                return assert!(steps >= 4, "three replacements and a removal: {steps}");
            }
            let journal: Value = serde_json::from_slice(&std::fs::read(&journal_path).unwrap()).unwrap();
            let mut lost = false;
            if lose_pending {
                // A pending temporary is lost: the previous version must come back.
                for r in journal["replace"].as_array().unwrap() {
                    let t = tt_persist::journal::path(&folder, r["new"]["file"].as_str().unwrap());
                    if t.is_file() {
                        std::fs::remove_file(t).unwrap();
                        lost = true;
                        break;
                    }
                }
            }
            let mut issues = Issues::default();
            tt_persist::save::recover_journal(&folder, false, &mut issues).unwrap();
            assert!(!journal_path.exists());
            let expected = if lost { &old_files } else { &new_files };
            assert_eq!(&files(&folder), expected, "interrupted after {steps} renames, lost pending: {lost}");
            let _ = std::fs::remove_dir_all(&dir);
        }
        steps += 1;
    }
}

#[test]
fn a_journal_whose_versions_are_both_incomplete_changes_nothing() {
    let (old, new) = versions();
    let (p, b) = (Preserved::default(), Blobs::default());
    let dir = tmp();
    let folder = dir.join("F.animpkg");
    save(&folder, Form::Folder, &input(&old, &p, &b), None, false).unwrap();
    let _ = save_folder(
        &folder,
        &input(&new, &p, &b),
        None,
        FolderOptions { crash_after_renames: Some(1), ..Default::default() },
    );
    let journal: Value = serde_json::from_slice(&std::fs::read(folder.join(".save-journal.json")).unwrap()).unwrap();
    // Lose a pending temporary and the old copy of the entry already replaced.
    let replace = journal["replace"].as_array().unwrap();
    let done = &replace[0];
    std::fs::remove_file(tt_persist::journal::path(&folder, done["old"]["file"].as_str().unwrap())).unwrap();
    std::fs::remove_file(tt_persist::journal::path(&folder, replace[1]["new"]["file"].as_str().unwrap())).unwrap();
    let before = files(&folder);
    let mut issues = Issues::default();
    let e = tt_persist::save::recover_journal(&folder, false, &mut issues).unwrap_err();
    assert_eq!(e.details()["reason"], "unrecoverableSave", "{e:?}");
    assert_eq!(files(&folder), before, "nothing changed");
    // Loading fails the same way (fail closed), and still changes nothing.
    let e = tt_persist::load(&folder, &Default::default()).unwrap_err();
    assert_eq!(e.details()["reason"], "unrecoverableSave");
    assert_eq!(files(&folder), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn journals_are_validated_before_anything_changes() {
    let dir = tmp();
    std::fs::write(dir.join("project.json"), b"user content").unwrap();
    let sha = "0".repeat(64);
    let cases = [
        json!({"format": "unrelated-format", "deletions": ["project.json"]}),
        json!({"format": "animpkg-save-journal", "renames": [], "deletions": ["project.json"]}),
        json!({"format": "animpkg-save-journal", "formatVersion": "2.0", "replace": [], "remove": [
            {"target": "project.json", "old": {"file": "../.x.old", "size": 1, "sha256": sha}}]}),
        json!({"format": "animpkg-save-journal", "formatVersion": "2.0", "replace": [], "remove": [
            {"target": "project.json", "old": {"file": "palettes/.project.json.x.old", "size": 1, "sha256": sha}}]}),
        json!({"format": "animpkg-save-journal", "formatVersion": "2.0", "replace": [
            {"target": ".lock", "new": {"file": "..lock.x.tmp", "size": 1, "sha256": sha}, "old": null}], "remove": []}),
        json!({"format": "animpkg-save-journal", "formatVersion": "2.0", "replace": [
            {"target": "vendor/C:/x", "new": {"file": "vendor/C:/.x.a.tmp", "size": 1, "sha256": sha}, "old": null}], "remove": []}),
        json!({"format": "animpkg-save-journal", "formatVersion": "2.0", "replace": [], "remove": [
            {"target": "project.json", "old": {"file": ".project.json.a.old", "size": 1, "sha256": "XYZ"}}]}),
        json!([1, 2, 3]),
    ];
    for c in cases {
        std::fs::write(dir.join(".save-journal.json"), serde_json::to_vec(&c).unwrap()).unwrap();
        let mut issues = Issues::default();
        let e = tt_persist::save::recover_journal(&dir, false, &mut issues).unwrap_err();
        assert_eq!(e.details()["reason"], "journal", "{c}");
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), b"user content", "{c}");
        assert!(dir.join(".save-journal.json").is_file(), "the journal is kept: {c}");
    }
    std::fs::write(dir.join(".save-journal.json"), b"not json").unwrap();
    let mut issues = Issues::default();
    assert!(tt_persist::save::recover_journal(&dir, false, &mut issues).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
