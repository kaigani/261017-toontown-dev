//! C1.3 `doc.*` commands through the executor: the round trip of 11 §14.3 (objects created by a command, saved,
//! reopened, read back identically), the interchange bundle, assets and blobs, packages, validation and repair,
//! references, candidate validation, and the load report of a repaired project.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;
#[path = "c13_projects.rs"]
#[allow(dead_code)]
mod projects;

use std::collections::BTreeMap;
use std::path::Path;

use common::*;
use projects::{job_ok, spawn, tmp};
use serde_json::{Value, json};
use tt_model::ObjectType;

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Every object of the open project by type: id → its `doc.object.get` JSON (depth 1).
fn all_objects(h: &tt_executor::ExecutorHandle) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for ty in ObjectType::ALL {
        let mut offset = 0;
        loop {
            let found = ok(h, "doc.object.find", json!({ "type": ty.as_str(), "limit": 1000, "offset": offset }));
            for item in found["items"].as_array().unwrap() {
                let id = item["id"].as_str().unwrap().to_owned();
                out.insert(id.clone(), get(h, &id));
            }
            match found["nextOffset"].as_u64() {
                Some(n) => offset = n,
                None => break,
            }
        }
    }
    out
}

fn without(mut v: Value, keys: &[&str]) -> Value {
    fn strip(v: &mut Value, keys: &[&str]) {
        match v {
            Value::Object(m) => {
                for k in keys {
                    m.shift_remove(*k);
                }
                m.values_mut().for_each(|x| strip(x, keys));
            }
            Value::Array(a) => a.iter_mut().for_each(|x| strip(x, keys)),
            _ => {}
        }
    }
    strip(&mut v, keys);
    v
}

#[test]
fn objects_created_by_import_survive_save_and_reopen_identically() {
    let dir = tmp();
    // The source: the sample project, which holds every object type the bundle carries.
    let src = spawn(&dir.join("src"));
    assert!(src.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let bundle = ok(&src, "doc.export.json", json!({ "scope": "project", "binaries": "embed" }))["bundle"].clone();
    let source_objects = all_objects(&src);
    // The target: a new project; the bundle's scenes, banks, palettes, pens and assets are imported by command.
    let h = spawn(&dir.join("dst"));
    ok(&h, "project.create", json!({ "name": "Round trip" }));
    let r = ok(&h, "doc.import.json", json!({ "source": bundle, "idPolicy": "preserve" }));
    assert_eq!(r["created"]["scenes"].as_array().unwrap().len(), 2, "{r}");
    let imported = all_objects(&h);
    // Import fidelity: every imported object equals its source, revisions and paths aside.
    let mut types = std::collections::BTreeSet::new();
    for (id, v) in &source_objects {
        let t = v["type"].as_str().unwrap().to_owned();
        if matches!(t.as_str(), "project" | "library" | "libraryFolder" | "libraryItem") {
            continue;
        }
        let got = imported.get(id).unwrap_or_else(|| panic!("{t} `{id}` was not imported"));
        let scrub = |x: &Value| without(x.clone(), &["rev", "path", "name"]);
        assert_eq!(scrub(got), scrub(v), "{t} `{id}`");
        types.insert(t);
    }
    for t in ["palette", "paletteStyle", "swatch", "pen", "asset", "drawingBank", "drawing", "drawingNode", "stroke", "scene", "transition", "element", "channel", "key", "motionPoint", "particleBlock", "soundPlacement", "mediaSequence"] {
        assert!(types.contains(t), "the round trip covers {t}: {types:?}");
    }
    // 11 §14.3: save → reopen → get_object returns identical JSON, for every object.
    let before = all_objects(&h);
    let target = dir.join("RT.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": s(&target) }));
    let saved = all_objects(&h);
    ok(&h, "project.close", json!({}));
    job_ok(&h, "project.open", json!({ "path": s(&target) }));
    let after = all_objects(&h);
    // modifiedAt of the project is volatile (set by the save).
    let vol = |m: &BTreeMap<String, Value>| m.iter().map(|(k, v)| (k.clone(), without(v.clone(), &["modifiedAt"]))).collect::<BTreeMap<_, _>>();
    assert_eq!(after, saved, "reopened == saved, revisions included");
    assert_eq!(vol(&after).len(), vol(&before).len());
    // The folder form too.
    let folder = dir.join("RT-folder.animpkg");
    job_ok(&h, "doc.package.export", json!({ "path": s(&folder), "form": "folder" }));
    ok(&h, "project.close", json!({}));
    job_ok(&h, "project.open", json!({ "path": s(&folder) }));
    assert_eq!(all_objects(&h), saved);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bundles_import_into_a_scene_at_a_frame_with_fresh_ids_and_renames() {
    let dir = tmp();
    let h = spawn(&dir.join("u"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let bob = ok(&h, "doc.object.resolve", json!({ "refs": ["scene:\"Intro\"/element:\"Bob\""] }))["items"][0]["id"].as_str().unwrap().to_owned();
    let b = ok(&h, "doc.export.json", json!({ "scope": "elements", "refs": [bob], "binaries": "omit" }))["bundle"].clone();
    assert_eq!(b["scope"]["kind"], "elements");
    let before = get(&h, &bob);
    let scene2 = ok(&h, "doc.object.resolve", json!({ "refs": ["scene:\"Scene-2\""] }))["items"][0]["id"].as_str().unwrap().to_owned();
    let r = ok(&h, "doc.import.json", json!({ "source": b, "target": scene2, "atFrame": 11 }));
    let new_ids: Vec<String> = r["created"]["elements"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_owned()).collect();
    assert!(!new_ids.contains(&bob), "fresh IDs");
    let copy = new_ids.iter().map(|i| get(&h, i)).find(|e| e["elementType"] == "drawing").unwrap();
    assert_eq!(copy["exposure"][0], json!([null, 10]), "the copy starts at frame 11: {}", copy["exposure"]);
    assert_eq!(copy["exposure"][1], before["exposure"][0]);
    // Again: names conflict in the target scene and are renamed.
    let r2 = ok(&h, "doc.import.json", json!({ "source": b, "target": scene2 }));
    assert!(!r2["renamed"].as_array().unwrap().is_empty(), "{r2}");
    let e = err(&h, "doc.import.json", json!({ "source": b, "target": scene2, "onConflict": "fail" }));
    assert_eq!(e.code, "doc.importConflict");
    // A dry run changes nothing; one import is one undo step.
    let rev0 = rev(&h);
    let d = run(&h, "doc.import.json", json!({ "source": b, "target": scene2, "dryRun": true }));
    assert!(d.is_ok());
    assert_eq!(rev(&h), rev0);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap() + 1, ok(&h, "edit.history.get", json!({}))["entries"].as_array().unwrap().len() as u64);
    // Writing a bundle to a file and importing it from there.
    let file = dir.join("bob.animjson");
    let w = ok(&h, "doc.export.json", json!({ "scope": "elements", "refs": [bob], "path": s(&file) }));
    assert!(w["bytes"].as_u64().unwrap() > 0 && file.is_file());
    assert_eq!(err(&h, "doc.export.json", json!({ "scope": "elements", "refs": [bob], "path": s(&file) })).code, "fs.exists");
    ok(&h, "doc.import.json", json!({ "source": s(&file), "target": scene2 }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn linked_media_relink_embed_blob_export_and_remove() {
    let dir = tmp();
    let h = spawn(&dir.join("u"));
    ok(&h, "project.create", json!({ "name": "Media" }));
    job_ok(&h, "project.saveAs", json!({ "path": s(&dir.join("M.animpkg")) }));
    let media = dir.join("media");
    std::fs::create_dir_all(&media).unwrap();
    let wav = media.join("line.wav");
    std::fs::write(&wav, b"RIFF fake wave bytes").unwrap();
    let sha = tt_persist::fsx::sha256(b"RIFF fake wave bytes");
    // A bundle carrying one linked asset whose file is not where the link says.
    let bundle = json!({
        "format": "animjson", "formatVersion": "1.0", "scenes": [], "elements": [], "drawingBanks": [], "palettes": [],
        "pens": [], "blobs": {},
        "assets": [{ "id": "as_01K7AAAAAAAAAAAAAAAAAAAAAA", "type": "asset", "rev": 1, "name": "line.wav", "kind": "sound",
            "mediaType": "audio/wav", "storage": "linked", "entry": null,
            "link": { "kind": "file", "path": "elsewhere/line.wav", "absolutePath": null, "libraryId": null, "itemId": null, "versionId": null },
            "size": 20, "sha256": sha, "importedAt": "2026-10-19T00:00:00.000Z", "sourcePath": null }]
    });
    ok(&h, "doc.import.json", json!({ "source": bundle, "idPolicy": "preserve" }));
    let list = ok(&h, "doc.asset.list", json!({ "status": "unused" }));
    assert_eq!(list["assets"].as_array().unwrap().len(), 1);
    let r = ok(&h, "doc.asset.relink", json!({ "searchFolder": s(&media), "matchBy": "hash" }));
    assert_eq!(r["relinked"][0]["assetId"], "as_01K7AAAAAAAAAAAAAAAAAAAAAA", "{r}");
    let listed = ok(&h, "doc.asset.list", json!({}));
    assert_eq!(listed["assets"][0]["status"], "ok", "{listed}");
    let e = ok(&h, "doc.asset.embed", json!({ "all": true }));
    assert_eq!(e["embedded"][0], "as_01K7AAAAAAAAAAAAAAAAAAAAAA");
    let blob = ok(&h, "doc.blob.get", json!({ "ref": "as_01K7AAAAAAAAAAAAAAAAAAAAAA" }));
    assert_eq!(tt_persist::b64::decode(blob["base64"].as_str().unwrap()).unwrap(), b"RIFF fake wave bytes");
    assert_eq!(blob["sha256"], sha);
    assert_eq!(err(&h, "doc.blob.get", json!({ "ref": "as_01K7AAAAAAAAAAAAAAAAAAAAAA", "maxBytes": 4 })).code, "doc.blobTooLarge");
    // The embedded file is in the saved package (MED-01) after the linked original is gone.
    std::fs::remove_file(&wav).unwrap();
    job_ok(&h, "project.save", json!({}));
    ok(&h, "project.close", json!({}));
    job_ok(&h, "project.open", json!({ "path": s(&dir.join("M.animpkg")) }));
    let out = dir.join("out.wav");
    ok(&h, "doc.asset.export", json!({ "assetId": "as_01K7AAAAAAAAAAAAAAAAAAAAAA", "path": s(&out) }));
    assert_eq!(std::fs::read(&out).unwrap(), b"RIFF fake wave bytes");
    // Unused media: validated, repaired by the default fix, restored by undo.
    let v = ok(&h, "doc.validate", json!({ "checks": ["unusedAssets"] }));
    assert_eq!(v["issues"][0]["code"], "unusedAssets");
    let fixed = ok(&h, "doc.repair", json!({ "codes": ["unusedAssets"] }));
    assert_eq!(fixed["fixed"].as_array().unwrap().len(), 1, "{fixed}");
    assert!(ok(&h, "doc.asset.list", json!({}))["assets"].as_array().unwrap().is_empty());
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "doc.asset.list", json!({}))["assets"].as_array().unwrap().len(), 1);
    // MED-04: removed and saved, the entry leaves the package.
    ok(&h, "doc.asset.remove", json!({ "unused": true }));
    job_ok(&h, "project.save", json!({}));
    let z = tt_persist::zip::ZipArchive::open(&dir.join("M.animpkg"), None, Default::default()).unwrap();
    assert!(z.entries().iter().all(|e| !e.name.starts_with("media/")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn packages_templates_palettes_and_libraries_are_inspected_without_opening() {
    let dir = tmp();
    let h = spawn(&dir.join("u"));
    ok(&h, "project.create", json!({ "name": "Inspect" }));
    let pkg = dir.join("I.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": s(&pkg) }));
    let info = ok(&h, "doc.package.get", json!({ "path": s(&pkg), "validate": true }));
    assert_eq!((info["kind"].clone(), info["form"].clone(), info["readable"].clone()), (json!("project"), json!("package"), json!(true)));
    assert_eq!(info["name"], "Inspect");
    assert_eq!(info["projectId"], first(&h, "project"));
    let bad = dir.join("bad.animpkg");
    std::fs::write(&bad, b"PK\x03\x04garbage").unwrap();
    let bi = ok(&h, "doc.package.get", json!({ "path": s(&bad) }));
    assert_eq!(bi["readable"], false);
    assert_eq!(bi["issues"][0]["code"], "project.invalidFormat");
    let pal = dir.join("p.animpal");
    tt_persist::library::write_palette_file(&pal, &json!({ "format": "animpal", "formatVersion": "1.0", "palettes": [{ "name": "Skin" }] }), &Default::default(), true).unwrap();
    let pi = ok(&h, "doc.package.get", json!({ "path": s(&pal) }));
    assert_eq!((pi["kind"].clone(), pi["readable"].clone(), pi["name"].clone()), (json!("palette"), json!(true), json!("Skin")));
    let tpl = dir.join("t.animtpl");
    tt_persist::library::write_template(&tpl, &json!({ "id": "tp_1", "name": "Walk" }), &json!({ "format": "animjson" }), &Default::default(), None).unwrap();
    let ti = ok(&h, "doc.package.get", json!({ "path": s(&tpl) }));
    assert_eq!((ti["kind"].clone(), ti["name"].clone()), (json!("template"), json!("Walk")));
    let lib = dir.join("Lib");
    std::fs::create_dir_all(lib.join("files")).unwrap();
    let library: tt_model::entities::library::Library = tt_model::object::from_json(&json!({
        "id": "lib_01K7BBBBBBBBBBBBBBBBBBBBBB", "type": "library", "rev": 1, "name": "Shared", "kind": "global", "formatVersion": "1.0",
        "children": [{ "id": "li_01K7CCCCCCCCCCCCCCCCCCCCCC", "type": "libraryItem", "rev": 1, "name": "Gone", "itemKind": "image", "file": "gone.png", "thumbnail": null }]
    })).unwrap();
    tt_persist::library::write_library_index(&lib, &library).unwrap();
    let li = ok(&h, "doc.package.get", json!({ "path": s(&lib) }));
    assert_eq!((li["kind"].clone(), li["name"].clone()), (json!("library"), json!("Shared")));
    assert_eq!(li["issues"][0]["code"], "brokenLibraryItems");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn validation_references_and_candidates() {
    let h = fixture();
    let v = ok(&h, "doc.validate", json!({}));
    let unavailable: Vec<&str> = v["unavailable"].as_array().unwrap().iter().map(|u| u["check"].as_str().unwrap()).collect();
    assert_eq!(unavailable, ["unpaintedZones", "staleLipSync", "transitionPadding", "missingFonts", "outdatedTemplateLinks"]);
    assert!(v["issues"].as_array().unwrap().iter().all(|i| i["issueId"].as_str().unwrap().starts_with("is_")));
    assert_eq!(err(&h, "doc.validate", json!({ "checks": ["nope"] })).code, "arg.invalid");
    let errors = ok(&h, "doc.validate", json!({ "minSeverity": "error" }));
    assert!(errors["issues"].as_array().unwrap().iter().all(|i| i["severity"] == "error"));
    // References: the drawing element's bank is used by it.
    let bob = ok(&h, "doc.object.resolve", json!({ "refs": ["scene:\"Intro\"/element:\"Bob\""] }))["items"][0]["id"].as_str().unwrap().to_owned();
    let bank = get(&h, &bob)["drawingBankId"].as_str().unwrap().to_owned();
    let incoming = ok(&h, "doc.object.refs", json!({ "ref": bank }));
    assert!(incoming["refs"].as_array().unwrap().iter().any(|r| r["id"] == bob && r["property"] == "/drawingBankId"), "{incoming}");
    let outgoing = ok(&h, "doc.object.refs", json!({ "ref": bob, "direction": "outgoing" }));
    assert!(outgoing["refs"].as_array().unwrap().iter().any(|r| r["id"] == bank));
    // Candidates.
    let good = json!({ "name": "Ink", "kind": "brush", "minSize": 1, "maxSize": 4, "smoothness": 3, "tip": "round", "tipAngle": 0, "colourId": null });
    assert_eq!(ok(&h, "doc.schema.validate", json!({ "type": "pen", "object": good, "mode": "create" }))["valid"], true);
    let bad = json!({ "name": "Ink", "kind": "brush", "minSize": 9, "maxSize": 4, "smoothness": 3, "tip": "round", "tipAngle": 0, "colourId": null });
    let r = ok(&h, "doc.schema.validate", json!({ "type": "pen", "object": bad, "mode": "create" }));
    assert_eq!(r["valid"], false);
    let mut el = get(&h, &bob);
    el.as_object_mut().unwrap().shift_remove("path");
    assert_eq!(ok(&h, "doc.schema.validate", json!({ "type": "element", "object": el.clone() }))["valid"], true);
    el["drawingBankId"] = json!("db_01K7DDDDDDDDDDDDDDDDDDDDDD");
    let r = ok(&h, "doc.schema.validate", json!({ "type": "element", "object": el }));
    assert_eq!(r["valid"], false, "a replacement must keep the document's references valid: {r}");
}

#[test]
fn a_repaired_load_is_one_non_undoable_step_and_reported() {
    let dir = tmp();
    let h = spawn(&dir.join("u"));
    ok(&h, "project.create", json!({ "name": "Damaged" }));
    let folder = dir.join("D.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": format!("{}/", s(&folder)) }));
    ok(&h, "project.close", json!({}));
    // Make the scene list an element that does not exist.
    let p: Value = serde_json::from_slice(&std::fs::read(folder.join("project.json")).unwrap()).unwrap();
    let sid = p["scenes"][0]["id"].as_str().unwrap().to_owned();
    let sp = folder.join("scenes").join(&sid).join("scene.json");
    let mut sc: Value = serde_json::from_slice(&std::fs::read(&sp).unwrap()).unwrap();
    sc["elements"].as_array_mut().unwrap().push(json!({ "id": "el_01K7EEEEEEEEEEEEEEEEEEEEEE", "type": "element" }));
    std::fs::write(&sp, serde_json::to_vec(&sc).unwrap()).unwrap();
    let events = h.subscribe();
    let r = job_ok(&h, "project.open", json!({ "path": s(&folder) }));
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w["id"] == "ref.dangling"), "{r}");
    let changed: Vec<_> = events.try_iter().filter(|e| e.name == "doc.changed").collect();
    assert!(changed.iter().any(|e| e.payload["origin"] == "load" && e.payload["label"] == "Repair on Open"), "REP-01: {changed:?}");
    let report = ok(&h, "doc.loadReport.get", json!({}));
    assert!(report["repairedCount"].as_u64().unwrap() >= 1);
    assert_eq!(report["issues"][0]["code"], "ref.dangling");
    let st = ok(&h, "doc.state.get", json!({}));
    assert_eq!(st["dirty"], true, "REP-01: the project is modified");
    assert_eq!(st["documentRev"].as_u64().unwrap(), st["savedRev"].as_u64().unwrap() + 1);
    let hist = ok(&h, "edit.history.get", json!({}));
    assert_eq!(hist["entries"][0]["label"], "Repair on Open");
    assert_eq!(err(&h, "edit.undo", json!({})).code, "edit.nothingToUndo", "the repair cannot be undone into an invalid document");
    job_ok(&h, "project.save", json!({}));
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], false);
    let _ = std::fs::remove_dir_all(&dir);
}
