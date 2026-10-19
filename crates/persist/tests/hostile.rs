//! Malformed and hostile packages (10 §6.2 ZIP-01…ZIP-06, §6.6 VER-02, §7.2; M0 readiness bullet 5): unsafe entry
//! names, size lies, decompression bombs, duplicate and overlapping entries, encryption, missing or wrong `mimetype`,
//! truncated files, newer versions, and the load repairs (duplicate IDs, dangling references, unreadable entries,
//! invalid trees, names, values), each deterministic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use tt_model::document::Document;
use tt_persist::codes;
use tt_persist::container::{BlobData, Blobs, Form};
use tt_persist::load::{LoadOptions, Preserved, blob_refs, load};
use tt_persist::save::{SaveInput, save};

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-hostile-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// One raw zip entry, with knobs to lie.
#[derive(Clone, Default)]
struct Raw {
    name: String,
    data: Vec<u8>,
    deflate: bool,
    flags: u16,
    declared_size: Option<u32>,
    local_name: Option<String>,
    extra: Vec<u8>,
}

fn raw(name: &str, data: &[u8]) -> Raw {
    Raw { name: name.into(), data: data.to_vec(), ..Raw::default() }
}

/// Writes entries without any safety check (the attacker's writer).
fn raw_zip(entries: &[Raw]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let offset = out.len() as u32;
        let body = if e.deflate {
            let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(&e.data).unwrap();
            enc.finish().unwrap()
        } else {
            e.data.clone()
        };
        let crc = crc32fast::hash(&e.data);
        let usize_ = e.declared_size.unwrap_or(e.data.len() as u32);
        let lname = e.local_name.clone().unwrap_or_else(|| e.name.clone());
        let method: u16 = if e.deflate { 8 } else { 0 };
        let mut h = Vec::new();
        h.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        h.extend_from_slice(&20u16.to_le_bytes());
        h.extend_from_slice(&(e.flags | 0x0800).to_le_bytes());
        h.extend_from_slice(&method.to_le_bytes());
        h.extend_from_slice(&[0, 0, 0x21, 0]);
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&(body.len() as u32).to_le_bytes());
        h.extend_from_slice(&usize_.to_le_bytes());
        h.extend_from_slice(&(lname.len() as u16).to_le_bytes());
        h.extend_from_slice(&(e.extra.len() as u16).to_le_bytes());
        h.extend_from_slice(lname.as_bytes());
        h.extend_from_slice(&e.extra);
        out.extend_from_slice(&h);
        out.extend_from_slice(&body);
        let mut c = Vec::new();
        c.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&(e.flags | 0x0800).to_le_bytes());
        c.extend_from_slice(&method.to_le_bytes());
        c.extend_from_slice(&[0, 0, 0x21, 0]);
        c.extend_from_slice(&crc.to_le_bytes());
        c.extend_from_slice(&(body.len() as u32).to_le_bytes());
        c.extend_from_slice(&usize_.to_le_bytes());
        c.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
        c.extend_from_slice(&0u16.to_le_bytes());
        c.extend_from_slice(&0u16.to_le_bytes());
        c.extend_from_slice(&0u16.to_le_bytes());
        c.extend_from_slice(&0u16.to_le_bytes());
        c.extend_from_slice(&0u32.to_le_bytes());
        c.extend_from_slice(&offset.to_le_bytes());
        c.extend_from_slice(e.name.as_bytes());
        central.extend_from_slice(&c);
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

const MIME: &[u8] = b"application/x-animpkg+zip";

fn sample() -> (Document, Blobs) {
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let mut blobs = Blobs::default();
    for (entry, _, _, _) in blob_refs(&doc) {
        blobs.insert(entry.clone(), BlobData::Bytes(Arc::new(format!("bytes of {entry}").into_bytes())));
    }
    (doc, blobs)
}

/// The sample saved as a folder, returned with its entry files as JSON (for mutation).
fn sample_folder(dir: &Path) -> PathBuf {
    let (doc, blobs) = sample();
    let p = dir.join("S.animpkg");
    let pres = Preserved::default();
    save(
        &p,
        Form::Folder,
        &SaveInput {
            doc: &doc,
            preserved: &pres,
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-19T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    p
}

fn read_json(root: &Path, entry: &str) -> Value {
    serde_json::from_slice(&std::fs::read(entry.split('/').fold(root.to_path_buf(), |p, s| p.join(s))).unwrap())
        .unwrap()
}

fn write_json(root: &Path, entry: &str, v: &Value) {
    let p = entry.split('/').fold(root.to_path_buf(), |p, s| p.join(s));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, tt_model::canonical::to_canonical_string(v).unwrap()).unwrap();
}

fn reason(path: &Path) -> String {
    let e = load(path, &LoadOptions::default()).unwrap_err();
    assert_eq!(e.code(), codes::INVALID_FORMAT, "{e:?}");
    e.details()["reason"].as_str().unwrap_or_default().to_owned()
}

fn codes_of(l: &tt_persist::Loaded) -> Vec<String> {
    l.report.issues.iter().map(|i| i.code.clone()).collect()
}

fn element_path(root: &Path, name: &str) -> String {
    let idx = tt_persist::container::FolderIndex::scan(root, Default::default()).unwrap();
    idx.files().keys().find(|k| k.contains("/elements/") && read_json(root, k)["name"] == name).unwrap().clone()
}

// ----- container rules -----------------------------------------------------------------------------------------

#[test]
fn unsafe_entry_names_are_refused_before_extracting_anything() {
    let dir = tmp();
    for bad in ["../evil.json", "/abs.json", "C:/x.json", "a\\b.json", "a/../../b", "./x", "a//b", "con\u{7}trol"] {
        let p = dir.join("bad.animpkg");
        std::fs::write(&p, raw_zip(&[raw("mimetype", MIME), raw(bad, b"{}"), raw("manifest.json", b"{}")])).unwrap();
        assert_eq!(reason(&p), "unsafeEntry", "{bad}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn local_header_naming_another_entry_is_refused() {
    let dir = tmp();
    let p = dir.join("x.animpkg");
    let mut e = raw("project.json", b"{}");
    e.local_name = Some("../../x".into());
    std::fs::write(&p, raw_zip(&[raw("mimetype", MIME), e])).unwrap();
    assert_eq!(reason(&p), "unsafeEntry");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicate_entries_encryption_and_unknown_methods_are_refused() {
    let dir = tmp();
    let p = dir.join("x.animpkg");
    std::fs::write(&p, raw_zip(&[raw("mimetype", MIME), raw("project.json", b"{}"), raw("project.json", b"{}")]))
        .unwrap();
    assert_eq!(reason(&p), "duplicateEntry");
    let mut enc = raw("project.json", b"{}");
    enc.flags = 1;
    std::fs::write(&p, raw_zip(&[raw("mimetype", MIME), enc])).unwrap();
    assert_eq!(reason(&p), "encrypted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mimetype_must_come_first_uncompressed_and_exact() {
    let dir = tmp();
    let p = dir.join("x.animpkg");
    std::fs::write(&p, raw_zip(&[raw("project.json", b"{}"), raw("mimetype", MIME)])).unwrap();
    assert_eq!(reason(&p), "noMimetype");
    let mut m = raw("mimetype", MIME);
    m.deflate = true;
    std::fs::write(&p, raw_zip(&[m])).unwrap();
    assert_eq!(reason(&p), "noMimetype");
    let mut m = raw("mimetype", MIME);
    m.extra = vec![0x55, 0x54, 0, 0];
    std::fs::write(&p, raw_zip(&[m])).unwrap();
    assert_eq!(reason(&p), "noMimetype");
    std::fs::write(&p, raw_zip(&[raw("mimetype", b"application/zip")])).unwrap();
    assert_eq!(reason(&p), "wrongMimetype");
    std::fs::write(&p, b"PK\x03\x04 this is not a zip").unwrap();
    assert_eq!(reason(&p), "notZip");
    std::fs::write(&p, b"").unwrap();
    assert_eq!(reason(&p), "notZip");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn size_lies_and_decompression_bombs_abort_the_load() {
    let dir = tmp();
    let p = dir.join("x.animpkg");
    // A deflate bomb: 64 MiB of zeros declared as 100 bytes.
    let mut bomb = raw("project.json", &vec![b' '; 64 << 20]);
    bomb.deflate = true;
    bomb.declared_size = Some(100);
    let manifest = json!({"format": "animpkg", "formatVersion": "1.0", "entries": []}).to_string();
    std::fs::write(&p, raw_zip(&[raw("mimetype", MIME), bomb, raw("manifest.json", manifest.as_bytes())])).unwrap();
    assert_eq!(reason(&p), "sizeLimit");
    // A stored entry whose two sizes disagree.
    let mut lie = raw("project.json", b"{}");
    lie.declared_size = Some(5);
    std::fs::write(&p, raw_zip(&[raw("mimetype", MIME), lie])).unwrap();
    assert_eq!(reason(&p), "sizeLimit");
    // The manifest bounds what an entry may inflate to (ZIP-06): a listed size of 10 bytes against 1 MiB of data.
    let (doc, blobs) = sample();
    let good = dir.join("good.animpkg");
    let pres = Preserved::default();
    save(
        &good,
        Form::Package,
        &SaveInput {
            doc: &doc,
            preserved: &pres,
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-19T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    let z = tt_persist::zip::ZipArchive::open(&good, None, Default::default()).unwrap();
    let mut entries: Vec<Raw> = Vec::new();
    for e in z.entries() {
        let mut data = z.read(&e.name, 1 << 30).unwrap();
        if e.name == "project.json" {
            // Valid JSON with 200 KiB of white space: more than the manifest's 10 bytes plus the margin.
            data.extend(std::iter::repeat_n(b' ', 200 * 1024));
        }
        let mut r = raw(&e.name, &data);
        r.deflate = e.name.ends_with(".json");
        if e.name == "manifest.json" {
            let mut m: Value = serde_json::from_slice(&data).unwrap();
            for x in m["entries"].as_array_mut().unwrap() {
                if x["path"] == "project.json" {
                    x["size"] = json!(10);
                }
            }
            r.data = m.to_string().into_bytes();
        }
        entries.push(r);
    }
    let p2 = dir.join("lying-manifest.animpkg");
    std::fs::write(&p2, raw_zip(&entries)).unwrap();
    assert_eq!(reason(&p2), "sizeLimit");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn truncated_and_corrupted_packages_are_invalid_and_never_panic() {
    let (doc, blobs) = sample();
    let dir = tmp();
    let good = dir.join("good.animpkg");
    let pres = Preserved::default();
    save(
        &good,
        Form::Package,
        &SaveInput {
            doc: &doc,
            preserved: &pres,
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-19T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    let bytes = std::fs::read(&good).unwrap();
    // Every truncation point, and a byte flipped at many positions: an error or a repaired load, never a panic.
    let p = dir.join("cut.animpkg");
    let step = (bytes.len() / 97).max(1);
    for cut in (0..bytes.len()).step_by(step) {
        std::fs::write(&p, &bytes[..cut]).unwrap();
        assert!(load(&p, &LoadOptions::default()).is_err(), "truncated at {cut} of {}", bytes.len());
    }
    for pos in (0..bytes.len()).step_by(step) {
        let mut b = bytes.clone();
        b[pos] ^= 0x5a;
        std::fs::write(&p, &b).unwrap();
        if let Ok(l) = load(&p, &LoadOptions::default()) {
            assert!(l.doc.issues().iter().all(|i| i.severity != tt_model::document::Severity::Error));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hash_mismatches_are_warnings_and_the_entry_is_still_read() {
    let (doc, blobs) = sample();
    let dir = tmp();
    let good = dir.join("good.animpkg");
    let pres = Preserved::default();
    save(
        &good,
        Form::Package,
        &SaveInput {
            doc: &doc,
            preserved: &pres,
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-19T12:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    let z = tt_persist::zip::ZipArchive::open(&good, None, Default::default()).unwrap();
    let mut entries: Vec<Raw> = Vec::new();
    for e in z.entries() {
        let mut data = z.read(&e.name, 1 << 30).unwrap();
        if e.name.starts_with("palettes/") {
            // Same length, other bytes: a different rev digit is still a valid palette.
            let t = String::from_utf8(data).unwrap().replace("\"rev\": 1,", "\"rev\": 1 ,");
            data = t.into_bytes();
        }
        entries.push(raw(&e.name, &data));
    }
    let p = dir.join("mismatch.animpkg");
    std::fs::write(&p, raw_zip(&entries)).unwrap();
    let l = load(&p, &LoadOptions::default()).unwrap();
    assert!(codes_of(&l).iter().any(|c| c == codes::HASH_MISMATCH), "{:?}", codes_of(&l));
    assert_eq!(l.doc.palettes().count(), doc.palettes().count());
    let _ = std::fs::remove_dir_all(&dir);
}

// ----- versions ----------------------------------------------------------------------------------------------

#[test]
fn newer_major_versions_reader_versions_and_unknown_features_refuse_to_open() {
    let dir = tmp();
    let folder = sample_folder(&dir);
    let mut m = read_json(&folder, "manifest.json");
    for (field, value) in [
        ("formatVersion", json!("2.0")),
        ("minReaderVersion", json!("1.1")),
        ("requiredFeatures", json!(["holograms"])),
    ] {
        let mut x = m.clone();
        x[field] = value;
        if field == "minReaderVersion" {
            x["formatVersion"] = json!("1.1");
        }
        write_json(&folder, "manifest.json", &x);
        let e = load(&folder, &LoadOptions::default()).unwrap_err();
        assert_eq!(e.code(), codes::UNSUPPORTED_VERSION, "{field}");
    }
    m["format"] = json!("animtpl");
    write_json(&folder, "manifest.json", &m);
    assert_eq!(load(&folder, &LoadOptions::default()).unwrap_err().code(), codes::INVALID_FORMAT);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn newer_minor_content_is_preserved_verbatim_and_unknown_values_freeze_their_entry() {
    let dir = tmp();
    let folder = sample_folder(&dir);
    let mut m = read_json(&folder, "manifest.json");
    m["formatVersion"] = json!("1.3");
    m["zFuture"] = json!({"x": 1});
    write_json(&folder, "manifest.json", &m);
    // Unknown properties on an element, inside its transform and on a key; an unknown enum value on another element.
    let bob = element_path(&folder, "Bob");
    let mut e = read_json(&folder, &bob);
    e["zNew"] = json!([1, 2]);
    e["transform"]["wobble"] = json!(0.5);
    write_json(&folder, &bob, &e);
    let arm = element_path(&folder, "Arm");
    let mut a = read_json(&folder, &arm);
    a["loopType"] = json!("pingPong");
    write_json(&folder, &arm, &a);
    std::fs::write(folder.join("vendor-notes.txt"), b"kept").unwrap();
    let l = load(&folder, &LoadOptions::default()).unwrap();
    assert!(codes_of(&l).contains(&codes::NEWER_MINOR.to_owned()));
    assert!(codes_of(&l).contains(&codes::UNKNOWN_VALUE.to_owned()));
    assert!(!l.repaired, "preservation is not a repair: {:?}", codes_of(&l));
    // Save as a package and as a folder: the unknown content comes back verbatim.
    for form in [Form::Package, Form::Folder] {
        let out = dir.join(format!("out-{}.animpkg", form.as_str()));
        save(
            &out,
            form,
            &SaveInput {
                doc: &l.doc,
                preserved: &l.preserved,
                blobs: &l.blobs,
                session: None,
                saved_at: "2026-10-19T13:00:00.000Z".into(),
            },
            None,
            false,
        )
        .unwrap();
        let back = load(&out, &LoadOptions::default()).unwrap();
        let c = back.blobs.base().unwrap();
        let e2: Value = serde_json::from_slice(&c.read(&bob, 1 << 20).unwrap()).unwrap();
        assert_eq!(e2["zNew"], json!([1, 2]), "{form:?}");
        assert_eq!(e2["transform"]["wobble"], json!(0.5), "{form:?}");
        let a2 = c.read(&arm, 1 << 20).unwrap();
        assert_eq!(
            a2,
            std::fs::read(arm.split('/').fold(folder.clone(), |p, s| p.join(s))).unwrap(),
            "{form:?}: frozen entry written as read"
        );
        assert_eq!(c.read("vendor-notes.txt", 100).unwrap(), b"kept", "{form:?}: ZIP-04");
        let m2: Value = serde_json::from_slice(&c.read("manifest.json", 1 << 20).unwrap()).unwrap();
        assert_eq!(m2["formatVersion"], "1.3", "{form:?}: the newer version is kept");
        assert_eq!(m2["zFuture"], json!({"x": 1}), "{form:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ----- repairs -------------------------------------------------------------------------------------------------

#[test]
fn project_json_missing_or_invalid_is_fatal() {
    let dir = tmp();
    let folder = sample_folder(&dir);
    std::fs::write(folder.join("project.json"), b"{ not json").unwrap();
    assert_eq!(reason(&folder), "project");
    std::fs::remove_file(folder.join("project.json")).unwrap();
    assert_eq!(reason(&folder), "project");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicate_ids_dangling_references_and_unreadable_entries_are_repaired_deterministically() {
    let dir = tmp();
    let folder = sample_folder(&dir);
    // A second element entry carrying Bob's ID, in another scene's folder.
    let bob = element_path(&folder, "Bob");
    let bob_json = read_json(&folder, &bob);
    let scene2 = read_json(&folder, "project.json")["scenes"][1]["id"].as_str().unwrap().to_owned();
    let mut twin = bob_json.clone();
    twin["name"] = json!("Bob twin");
    write_json(&folder, &format!("scenes/{scene2}/elements/{}.json", bob_json["id"].as_str().unwrap()), &twin);
    // Duplicate pen IDs inside the project entry.
    let mut project = read_json(&folder, "project.json");
    let first_pen = project["pens"][0]["id"].clone();
    project["pens"][1]["id"] = first_pen;
    // A listed palette that does not exist, and a name with forbidden characters.
    project["palettes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "pal_01K7ZZZZZZZZZZZZZZZZZZZZZZ", "type": "palette"}));
    project["name"] = json!("Bad/Name?");
    write_json(&folder, "project.json", &project);
    // An exposure naming a drawing outside the element's bank; a placement to a missing asset.
    let mut b = read_json(&folder, &bob);
    b["exposure"][0][0] = json!("dr_01K7YYYYYYYYYYYYYYYYYYYYYY");
    write_json(&folder, &bob, &b);
    // An unreadable element and an unreadable drawing; a drawing entry removed; a value out of range.
    let arm = element_path(&folder, "Arm");
    std::fs::write(arm.split('/').fold(folder.clone(), |p, s| p.join(s)), b"{\"broken\": ").unwrap();
    let idx = tt_persist::container::FolderIndex::scan(&folder, Default::default()).unwrap();
    let drawings: Vec<String> = idx
        .files()
        .keys()
        .filter(|k| k.starts_with("banks/") && k.ends_with(".json") && !k.ends_with("bank.json"))
        .cloned()
        .collect();
    std::fs::write(drawings[0].split('/').fold(folder.clone(), |p, s| p.join(s)), b"[1,2").unwrap();
    std::fs::remove_file(drawings[1].split('/').fold(folder.clone(), |p, s| p.join(s))).unwrap();
    let cam = element_path(&folder, "Camera");
    let mut c = read_json(&folder, &cam);
    c["camera"]["fov"] = json!(500);
    write_json(&folder, &cam, &c);

    let l1 = load(&folder, &LoadOptions::default()).unwrap();
    let l2 = load(&folder, &LoadOptions::default()).unwrap();
    let canon = |l: &tt_persist::Loaded| -> BTreeMap<String, String> {
        l.doc.entry_keys().iter().map(|k| (format!("{k:?}"), l.doc.entry_canonical(k).unwrap())).collect()
    };
    assert_eq!(canon(&l1), canon(&l2), "LOAD-03: same files, same document, IDs and repairs");
    assert_eq!(l1.report.issues.len(), l2.report.issues.len());
    assert!(l1.repaired);
    assert_eq!(l1.doc.document_rev(), l1.saved_rev + 1, "repairs are one transaction after the file's revision");
    let found = codes_of(&l1);
    for c in [
        codes::ID_DUPLICATE,
        codes::REF_DANGLING,
        codes::SCHEMA_VIOLATION,
        codes::CONTENT_MISSING,
        codes::EXPOSURE_INVALID,
        codes::NAME_REPAIRED,
        codes::VALUE_CLAMPED,
    ] {
        assert!(found.contains(&c.to_owned()), "{c} missing from {found:?}");
    }
    assert!(l1.doc.issues().iter().all(|i| i.severity != tt_model::document::Severity::Error));
    assert_eq!(l1.doc.project().name(), "Bad_Name_");
    // REP-02: the unreadable element's raw content is kept in its scene's ext.
    let kept = l1.doc.scenes().any(|s| {
        l1.doc.entry_json(&tt_model::document::EntryKey::Scene(s.id().clone())).unwrap()["ext"]["app.unreadable"][&arm]
            .is_string()
    });
    assert!(kept, "the unreadable element is kept");
    // The twin got a new ID and is an element of its own scene.
    let twin_el = l1.doc.elements().find(|e| e.name() == "Bob twin").unwrap();
    assert_ne!(twin_el.id().as_str(), bob_json["id"].as_str().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_trees_are_repaired_without_losing_elements() {
    let dir = tmp();
    let folder = sample_folder(&dir);
    let scene_id = read_json(&folder, "project.json")["scenes"][0]["id"].as_str().unwrap().to_owned();
    let sp = format!("scenes/{scene_id}/scene.json");
    let mut s = read_json(&folder, &sp);
    let before = s["elements"].as_array().unwrap().len();
    // Drop the first top-level node (its elements go missing from the tree), list another twice, add an unknown one.
    let first = s["tree"][0].clone();
    let second_id = s["tree"][1]["id"].clone();
    s["tree"].as_array_mut().unwrap().remove(0);
    s["tree"].as_array_mut().unwrap().push(json!({"id": second_id, "children": []}));
    s["tree"].as_array_mut().unwrap().push(json!({"id": "el_01K7XXXXXXXXXXXXXXXXXXXXXX", "children": [first]}));
    s["columnOrder"] = json!([]);
    write_json(&folder, &sp, &s);
    let l = load(&folder, &LoadOptions::default()).unwrap();
    assert!(codes_of(&l).contains(&codes::TREE_INVALID.to_owned()), "{:?}", codes_of(&l));
    let scene = l.doc.scene(&tt_model::Id::parse(&scene_id).unwrap()).unwrap();
    assert_eq!(l.doc.tree_order(scene).len(), before, "every element is in the tree exactly once");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn interrupted_folder_saves_complete_or_roll_back_on_open() {
    let (doc, blobs) = sample();
    let dir = tmp();
    let folder = dir.join("J.animpkg");
    let pres = Preserved::default();
    fn input<'a>(d: &'a Document, pres: &'a Preserved, blobs: &'a Blobs) -> SaveInput<'a> {
        SaveInput { doc: d, preserved: pres, blobs, session: None, saved_at: "2026-10-19T12:00:00.000Z".into() }
    }
    save(&folder, Form::Folder, &input(&doc, &pres, &blobs), None, false).unwrap();
    let el = doc.elements().find(|e| e.name() == "Bob").unwrap().id().clone();
    let (changed, _) = doc
        .with_patched_entry(
            &tt_model::document::EntryKey::Element(el.clone()),
            &[tt_model::change::PatchOp {
                op: "add",
                from: None,
                path: "/note".into(),
                range: None,
                value: Some(json!("after crash")),
                old_value: None,
                elided: false,
            }],
        )
        .unwrap();
    // Crash after the journal: the next open completes the save.
    let crash = tt_persist::save::save_folder(
        &folder,
        &input(&changed, &pres, &blobs),
        None,
        tt_persist::save::FolderOptions { git_files: false, crash_after_journal: true },
    );
    assert!(crash.is_err());
    assert!(folder.join(".save-journal.json").is_file());
    let l = load(&folder, &LoadOptions::default()).unwrap();
    assert!(codes_of(&l).contains(&codes::SAVE_INTERRUPTED.to_owned()));
    assert_eq!(l.doc.element(&el).unwrap().note(), "after crash", "completed");
    assert!(!folder.join(".save-journal.json").exists());
    // Crash again, then damage a temporary: the open rolls back to the previous entries.
    let (changed2, _) = l
        .doc
        .with_patched_entry(
            &tt_model::document::EntryKey::Element(el.clone()),
            &[tt_model::change::PatchOp {
                op: "replace",
                from: None,
                path: "/note".into(),
                range: None,
                value: Some(json!("lost")),
                old_value: None,
                elided: false,
            }],
        )
        .unwrap();
    let _ = tt_persist::save::save_folder(
        &folder,
        &SaveInput { doc: &changed2, preserved: &l.preserved, blobs: &l.blobs, session: None, saved_at: "x".into() },
        None,
        tt_persist::save::FolderOptions { git_files: false, crash_after_journal: true },
    );
    let journal: Value = serde_json::from_slice(&std::fs::read(folder.join(".save-journal.json")).unwrap()).unwrap();
    let tmp0 = journal["renames"][0]["tmp"].as_str().unwrap();
    std::fs::write(tmp0.split('/').fold(folder.clone(), |p, s| p.join(s)), b"damaged").unwrap();
    let l2 = load(&folder, &LoadOptions::default()).unwrap();
    assert_eq!(l2.doc.element(&el).unwrap().note(), "after crash", "rolled back");
    let issue = l2
        .report
        .issues
        .iter()
        .find(|i| i.code == codes::SAVE_INTERRUPTED)
        .unwrap_or_else(|| panic!("{:#?}", l2.report.issues));
    assert_eq!(issue.details["completed"], false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn folder_entries_may_not_be_symbolic_links() {
    let dir = tmp();
    let folder = sample_folder(&dir);
    std::os::unix::fs::symlink("/etc/passwd", folder.join("media-link")).unwrap();
    assert_eq!(reason(&folder), "unsafeEntry");
    let _ = std::fs::remove_dir_all(&dir);
}
