//! Save → load round trips of both forms (10 FMT-01, JSON-05, LAY-04, REV-04).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tt_model::document::Document;
use tt_persist::container::{BlobData, Blobs, Form};
use tt_persist::load::{LoadOptions, Preserved, blob_refs, load};
use tt_persist::save::{SaveInput, save};

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-rt-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The sample document with bytes for every binary entry it references.
fn sample() -> (Document, Blobs) {
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let mut blobs = Blobs::default();
    for (entry, owner, _, _) in blob_refs(&doc) {
        blobs.insert(entry.clone(), BlobData::Bytes(Arc::new(format!("bytes of {entry} for {owner}").into_bytes())));
    }
    (doc, blobs)
}

fn entries(doc: &Document) -> BTreeMap<String, String> {
    doc.entry_keys()
        .iter()
        .map(|k| (tt_persist::layout::entry_path(doc, k).unwrap(), doc.entry_canonical(k).unwrap()))
        .collect()
}

fn input<'a>(doc: &'a Document, blobs: &'a Blobs, preserved: &'a Preserved) -> SaveInput<'a> {
    SaveInput { doc, preserved, blobs, session: None, saved_at: "2026-10-19T12:00:00.000Z".into() }
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let idx = tt_persist::container::FolderIndex::scan(root, tt_persist::zip::Limits::default()).unwrap();
    idx.files()
        .keys()
        .map(|k| (k.clone(), std::fs::read(k.split('/').fold(root.to_path_buf(), |p, s| p.join(s))).unwrap()))
        .collect()
}

#[test]
fn package_and_folder_round_trip_to_the_identical_document() {
    let (doc, blobs) = sample();
    let preserved = Preserved::default();
    let dir = tmp();
    for form in [Form::Package, Form::Folder] {
        let path = dir.join(format!("Sample-{}.animpkg", form.as_str()));
        save(&path, form, &input(&doc, &blobs, &preserved), None, true).unwrap();
        let loaded = load(&path, &LoadOptions::default()).unwrap();
        assert_eq!(entries(&loaded.doc), entries(&doc), "{form:?}: every entry reads back identically");
        assert_eq!(loaded.doc.document_rev(), doc.document_rev(), "REV-04: documentRev persists");
        assert!(!loaded.repaired, "{:#?}", loaded.report.issues);
        assert!(
            loaded.missing.iter().all(|m| m.kind == "linked"),
            "only the linked movie is missing: {:?}",
            loaded.missing
        );
        for (entry, _, _, _) in blob_refs(&doc) {
            assert_eq!(loaded.blobs.read(&entry, 1 << 20).unwrap(), blobs.read(&entry, 1 << 20).unwrap(), "{entry}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_forms_hold_identical_entry_bytes_and_saves_are_deterministic() {
    let (doc, blobs) = sample();
    let preserved = Preserved::default();
    let dir = tmp();
    let pkg = dir.join("A.animpkg");
    let folder = dir.join("B.animpkg");
    save(&pkg, Form::Package, &input(&doc, &blobs, &preserved), None, true).unwrap();
    save(&folder, Form::Folder, &input(&doc, &blobs, &preserved), None, true).unwrap();
    let z = tt_persist::zip::ZipArchive::open(&pkg, Some(tt_persist::manifest::PACKAGE_MIMETYPE), Default::default())
        .unwrap();
    let in_folder = files(&folder);
    for e in z.entries() {
        if e.name == "manifest.json" {
            continue;
        }
        assert_eq!(z.read(&e.name, 1 << 30).unwrap(), in_folder[&e.name], "FMT-01: `{}` differs between forms", e.name);
    }
    let names: Vec<&String> = in_folder.keys().filter(|k| k.as_str() != "manifest.json").collect();
    assert_eq!(names.len(), z.entries().len() - 1, "same entry trees, `mimetype` included (C13-R1-12)");
    // JSON-05: saving the same document twice produces byte-identical entries (and here, an identical package).
    let pkg2 = dir.join("C.animpkg");
    save(&pkg2, Form::Package, &input(&doc, &blobs, &preserved), None, true).unwrap();
    assert_eq!(std::fs::read(&pkg).unwrap(), std::fs::read(&pkg2).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unchanged_folder_save_rewrites_nothing_and_a_change_rewrites_one_entry() {
    let (doc, blobs) = sample();
    let preserved = Preserved::default();
    let dir = tmp();
    let folder = dir.join("F.animpkg");
    save(&folder, Form::Folder, &input(&doc, &blobs, &preserved), None, true).unwrap();
    let again = save(&folder, Form::Folder, &input(&doc, &blobs, &preserved), None, true).unwrap();
    assert!(again.written.is_empty() && again.removed.is_empty(), "LAY-04: {:?} {:?}", again.written, again.removed);
    // Change one element: only its entry (and the manifest, which carries documentRev) is rewritten.
    let el = doc.elements().find(|e| e.name() == "Bob").unwrap().id().clone();
    let (changed, _) = doc
        .with_patched_entry(
            &tt_model::document::EntryKey::Element(el.clone()),
            &[tt_model::change::PatchOp {
                op: "add",
                from: None,
                path: "/note".into(),
                range: None,
                value: Some(serde_json::json!("changed")),
                old_value: None,
                elided: false,
            }],
        )
        .unwrap();
    let third = save(&folder, Form::Folder, &input(&changed, &blobs, &preserved), None, true).unwrap();
    let scene = changed.scene_of(&el).unwrap();
    assert_eq!(third.written, vec![format!("scenes/{scene}/elements/{el}.json")], "{:?}", third.written);
    assert!(folder.join(".gitignore").is_file() && folder.join(".gitattributes").is_file(), "FMT-03");
    assert!(!folder.join(".save-journal.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
