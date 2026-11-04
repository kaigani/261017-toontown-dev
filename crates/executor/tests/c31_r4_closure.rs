//! Independent round-4 scoped closure variants; fixtures stay in temporary directories.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
fn package(dir: &std::path::Path, seed: u64, c: u8) -> std::path::PathBuf {
    let mut parts = tt_model::fixture::sample_parts(seed).unwrap();
    if c == 199 {
        let other = tt_model::fixture::sample_parts(seed + 1).unwrap();
        let mut p = serde_json::to_value(&parts.project).unwrap();
        p["id"] = serde_json::to_value(other.project.id()).unwrap();
        parts.project = tt_model::object::from_json(&p).unwrap();
    }
    let e = parts.elements.iter().find(|e| e.name() == "Background").unwrap();
    let b = parts.banks.iter().find(|b| Some(b.id()) == e.drawing_bank_id().as_ref()).unwrap();
    let at = parts.drawings.iter().position(|d| d.id() == &b.drawings().ids()[0]).unwrap();
    let png = tt_render::encode_png(&[c, 93, 127, 255], 1, 1).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let entry = d["nodes"][0]["image"]["entry"].as_str().unwrap().to_owned();
    d["nodes"][0]["width"] = json!(1);
    d["nodes"][0]["height"] = json!(1);
    d["nodes"][0]["image"] =
        json!({"entry":entry,"mediaType":"image/png","size":png.len(),"sha256":tt_persist::fsx::sha256(&png)});
    parts.drawings[at] = tt_model::object::from_json(&d).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    blobs.insert(entry, tt_persist::container::BlobData::Bytes(std::sync::Arc::new(png)));
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-11-03T19:00:00Z".into(),
    };
    let path = dir.join(format!("seed-{seed}-{c}.animpkg"));
    tt_persist::save::save_package(&path, &input, None).unwrap();
    path
}
fn image(h: &ExecutorHandle, d: &str) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":d}))["nodes"][0]["image"].clone()
}
fn copy(h: &ExecutorHandle) {
    ok(h, "cell.clipboard.set", json!({"elementIds":[BG],"from":1,"to":1}));
}

fn texture_package(dir: &std::path::Path, c: u8) -> std::path::PathBuf {
    let mut parts = tt_model::fixture::sample_parts(0x314400 + u64::from(c)).unwrap();
    let mut palette = serde_json::to_value(&parts.palettes[0]).unwrap();
    palette["swatches"][0]["kind"] = json!("texture");
    for value in palette["swatches"][0]["values"].as_object_mut().unwrap().values_mut() {
        *value = json!({"image":"textures/r3-shared.png","tile":true,"width":1,"height":1});
    }
    for drawing in &mut parts.drawings {
        let mut value = serde_json::to_value(&*drawing).unwrap();
        if value["colourRecovery"][0]["colourId"] == palette["swatches"][0]["colourId"] {
            value["colourRecovery"][0]["swatch"] = palette["swatches"][0].clone();
            *drawing = tt_model::object::from_json(&value).unwrap();
        }
    }

    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    let png = tt_render::encode_png(&[c, 81, 222, 255], 1, 1).unwrap();
    blobs.insert("textures/r3-shared.png".into(), tt_persist::container::BlobData::Bytes(std::sync::Arc::new(png)));
    blobs.insert(
        "textures/unreferenced.png".into(),
        tt_persist::container::BlobData::Bytes(std::sync::Arc::new(vec![9, 8, 7])),
    );
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-11-03T19:00:00Z".into(),
    };
    let path = dir.join(format!("texture-{c}.animpkg"));
    tt_persist::save::save_package(&path, &input, None).unwrap();
    let loaded = tt_persist::load::load(&path, &Default::default()).unwrap();
    assert!(!loaded.blobs.contains("textures/unreferenced.png"));
    for palette in loaded.doc.palettes() {
        assert!(
            !serde_json::to_string(palette).unwrap().contains("textures/r3-shared"),
            "live palettes do not reference the copied texture"
        );
    }
    assert!(loaded.blobs.contains("textures/r3-shared.png"));
    path
}

#[test]
fn recovery_only_collision_bytes_survive_two_reopens() {
    let dir = tmp();
    let h = fixture();
    let mut clips = vec![];
    for c in [41, 209] {
        let path = texture_package(&dir, c);
        job_ok(&h, "project.open", json!({"path":path,"onUnsaved":"discard"}));
        ok(&h, "cell.clipboard.set", json!({"elementIds":["scene:\"Intro\"/element:\"Bob\""],"from":1,"to":1}));
        let clip = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
        assert!(clip["payload"]["blobs"]["textures/r3-shared.png"].is_string());
        clips.push(clip);
    }
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    assert!(h.open_document(tt_model::fixture::sample_document(0x313937).unwrap(), None, false).is_ok());
    let target = ok(&h, "element.create", json!({"type":"drawing","name":"Textures928"}))["elementIds"][0].clone();
    let mut refs = vec![];
    for (i, clip) in clips.iter().enumerate() {
        ok(&h, "edit.clipboard.set", json!({"content":clip}));
        let made = ok(&h, "cell.clipboard.apply", json!({"elementId":target,"frame":1+i*3,"mode":"newDrawings"}));
        let drawing = get(&h, made["createdDrawingIds"][0].as_str().unwrap());
        let swatch = &drawing["colourRecovery"][0]["swatch"];
        refs.push(swatch["values"].as_object().unwrap().values().next().unwrap()["image"].as_str().unwrap().to_owned());
    }
    assert_ne!(refs[0], refs[1]);
    for palette in h.snapshot().unwrap().unwrap().palettes() {
        assert!(!serde_json::to_string(palette).unwrap().contains("textures/r3-shared"));
    }
    let output = dir.join("textures.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":output}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    job_ok(&h, "project.open", json!({"path":output}));
    let second = dir.join("textures-second.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":second}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    job_ok(&h, "project.open", json!({"path":second}));
    let loaded = tt_persist::load::load(&second, &Default::default()).unwrap();
    assert!(!loaded.blobs.contains("textures/unreferenced.png"));
    println!(
        "texture readback {}",
        json!({"path":output,"copiedEntries":refs,"hasOriginal":loaded.blobs.contains(&refs[0]),"hasRemapped":loaded.blobs.contains(&refs[1])})
    );
    for (entry, c) in refs.iter().zip([41, 209]) {
        assert!(loaded.blobs.contains(entry), "saved texture recovery reference {entry} must retain its copied bytes");
        assert_eq!(
            loaded.blobs.read(entry, 1 << 20).unwrap(),
            tt_render::encode_png(&[c, 81, 222, 255], 1, 1).unwrap()
        );
    }
}
#[test]
fn ordered_chain_final_missing_source_rolls_back_all_targets() {
    let h = fixture();
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    ok(&h, "doc.object.patch", json!({"targets":[bob,BG],"props":{"meta":{"a":"old"}}}));
    let before = [get(&h, bob), get(&h, BG)];
    let revision = rev(&h);
    let history = ok(&h, "edit.history.get", json!({}));
    let r = run(
        &h,
        "doc.object.patch",
        json!({"targets":[bob,BG],"ops":[
            {"op":"replace","path":"/meta/a","value":"new"},
            {"op":"copy","from":"/meta/a","path":"/note"},
            {"op":"test","path":"/note","value":"new"},
            {"op":"move","from":"/note","path":"/meta/c"},
            {"op":"remove","path":"/meta/a"},
            {"op":"copy","from":"/meta/a","path":"/name"}
        ]}),
    );
    assert_eq!(r.status, tt_executor::Status::Error, "{r:?}");
    assert_eq!(r.error.unwrap().code, "arg.invalid");
    assert_eq!([get(&h, bob), get(&h, BG)], before);
    assert_eq!(rev(&h), revision);
    assert_eq!(ok(&h, "edit.history.get", json!({})), history);
}

#[test]
fn props_source_move_is_current_and_one_undo() {
    let h = fixture();
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let before = get(&h, bob);
    let revision = rev(&h);
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[bob],"props":{"meta":{"source":"R4 props"}},"ops":[
            {"op":"move","from":"/meta/source","path":"/note"},
            {"op":"test","path":"/note","value":"R4 props"}
        ]}),
    );
    assert_eq!(get(&h, bob)["note"], "R4 props");
    assert!(get(&h, bob)["meta"].get("source").is_none());
    assert_eq!(rev(&h), revision + 1);
    ok(&h, "edit.undo", json!({}));
    for field in ["meta", "note"] {
        assert_eq!(get(&h, bob)[field], before[field]);
    }
}

fn open(h: &ExecutorHandle, path: &std::path::Path) {
    job_ok(h, "project.open", json!({"path":path,"onUnsaved":"discard"}));
}
fn read_package(path: &std::path::Path, refs: &[(String, Vec<u8>)]) {
    let loaded = tt_persist::load::load(path, &Default::default()).unwrap();
    for (entry, bytes) in refs {
        assert_eq!(&loaded.blobs.read(entry, 1 << 20).unwrap(), bytes, "{entry}");
    }
}
#[test]
fn colliding_projects_atomic_recopy_return_trip_and_whole_elements() {
    let dir = tmp();
    let a = package(&dir, 0x314401, 21);
    let b = package(&dir, 0x314401, 199);
    let h = fixture();
    open(&h, &a);
    let source_project = first(&h, "project");
    let source_bank = get(&h, BG)["drawingBankId"].clone();
    let source_id = get(&h, BG)["exposure"][0][0].as_str().unwrap().to_owned();
    let source = image(&h, &source_id);
    let bytes_a = tt_persist::load::load(&a, &Default::default())
        .unwrap()
        .blobs
        .read(source["entry"].as_str().unwrap(), 1 << 20)
        .unwrap();
    copy(&h);
    let cells = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
    ok(&h, "edit.copy", json!({"context":"staging","objectIds":[BG]}));
    let whole = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
    open(&h, &b);
    assert_ne!(first(&h, "project"), source_project);
    assert_eq!(get(&h, BG)["drawingBankId"], source_bank);
    assert_eq!(get(&h, BG)["exposure"][0][0], source_id);
    let destination = image(&h, &source_id);
    let bytes_b = tt_persist::load::load(&b, &Default::default())
        .unwrap()
        .blobs
        .read(destination["entry"].as_str().unwrap(), 1 << 20)
        .unwrap();
    assert_ne!(bytes_a, bytes_b);
    ok(&h, "edit.clipboard.set", json!({"content":cells}));
    let steps = json!([
        {"id":"cell.clipboard.apply","params":{"elementId":BG,"frame":29}},
        {"id":"cell.clipboard.set","params":{"elementIds":[BG],"from":29,"to":29}}
    ]);
    let source_entry = source["entry"].as_str().unwrap();
    let (stem, ext) = source_entry.rsplit_once('.').unwrap();
    let remapped = format!("{stem}-{}.{}", &tt_persist::fsx::sha256(&bytes_a)[..16], ext);
    assert_eq!(err(&h, "doc.blob.get", json!({"entry":remapped})).code, "project.mediaMissing");
    let before = get(&h, BG);
    let revision = rev(&h);
    for dry in [true, false] {
        let mut s = steps.as_array().unwrap().clone();
        if !dry {
            s.push(json!({"id":"element.rename","params":{"elementId":BG,"name":"bad.name"}}));
        }
        let r = batch(&h, json!({"label":"R4 discard","atomic":true,"dryRun":dry,"steps":s}));
        assert_eq!(r.status == tt_executor::Status::Ok, dry, "{r:?}");
        assert_eq!(get(&h, BG), before);
        assert_eq!(rev(&h), revision);
        assert_eq!(ok(&h, "edit.clipboard.get", json!({"includeData":true})), cells);
        assert_eq!(err(&h, "doc.blob.get", json!({"entry":remapped})).code, "project.mediaMissing");
        // Package read-back also verifies rejection has left no persisted bytes.
        let out = dir.join(format!("discard-{dry}.animpkg"));
        job_ok(&h, "project.saveAs", json!({"path":out}));
        let loaded = tt_persist::load::load(&out, &Default::default()).unwrap();
        assert!(!loaded.blobs.contains(&remapped));
        read_package(&out, &[(destination["entry"].as_str().unwrap().into(), bytes_b.clone())]);
    }
    let r = batch(&h, json!({"label":"R4 accepted","atomic":true,"steps":steps}));
    assert_eq!(r.status, tt_executor::Status::Ok, "{r:?}");
    let recopy = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
    assert_eq!(recopy["payload"]["blobs"].as_object().unwrap().len(), 1);
    let pasted =
        recopy["payload"]["drawings"].as_object().unwrap().values().next().unwrap()["nodes"][0]["image"].clone();
    assert_eq!(pasted["entry"], remapped);
    assert_eq!(ok(&h, "doc.blob.get", json!({"entry":remapped}))["sha256"], tt_persist::fsx::sha256(&bytes_a));
    let out_b = dir.join("B-verified.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":out_b}));
    open(&h, &out_b);
    read_package(
        &out_b,
        &[
            (destination["entry"].as_str().unwrap().into(), bytes_b.clone()),
            (pasted["entry"].as_str().unwrap().into(), bytes_a.clone()),
        ],
    );
    open(&h, &a);
    let made = ok(&h, "cell.clipboard.apply", json!({"elementId":BG,"frame":41}));
    let returned = image(&h, made["createdDrawingIds"][0].as_str().unwrap());
    let out_a = dir.join("A-verified.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":out_a}));
    open(&h, &out_a);
    read_package(
        &out_a,
        &[
            (source["entry"].as_str().unwrap().into(), bytes_a.clone()),
            (returned["entry"].as_str().unwrap().into(), bytes_a.clone()),
        ],
    );
    open(&h, &b);
    ok(&h, "edit.clipboard.set", json!({"content":whole}));
    let made = ok(&h, "edit.paste", json!({"context":"staging"}));
    let element = made["createdIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| get(&h, id.as_str().unwrap()))
        .find(|e| e["elementType"] == "image")
        .unwrap();
    assert_ne!(element["drawingBankId"], get(&h, BG)["drawingBankId"]);
    let whole_image = image(&h, element["exposure"][0][0].as_str().unwrap());
    let out = dir.join("whole-elements.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":out}));
    open(&h, &out);
    read_package(
        &out,
        &[
            (destination["entry"].as_str().unwrap().into(), bytes_b),
            (whole_image["entry"].as_str().unwrap().into(), bytes_a),
        ],
    );
}

#[test]
fn persisted_recovery_only_texture_remains_reachable_on_two_resaves() {
    let dir = tmp();
    let mut path = texture_package(&dir, 41);
    let expected = tt_render::encode_png(&[41, 81, 222, 255], 1, 1).unwrap();
    // Exercise the shared persistence reader/planner without the executor's separate unresolved-colour recovery,
    // which correctly creates live swatches when a missing colour is opened in the editor.
    for generation in 0..=2 {
        let loaded = tt_persist::load::load(&path, &Default::default()).unwrap();
        for palette in loaded.doc.palettes() {
            assert!(!serde_json::to_string(palette).unwrap().contains("textures/r3-shared"));
        }
        assert_eq!(loaded.blobs.read("textures/r3-shared.png", 1 << 20).unwrap(), expected);
        assert!(!loaded.blobs.contains("textures/unreferenced.png"));
        assert!(tt_persist::load::saved_blob_refs(&loaded.doc).iter().any(|r| r.0 == "textures/r3-shared.png"));
        if generation < 2 {
            let next = dir.join(format!("recovery-only-{generation}.animpkg"));
            let input = tt_persist::save::SaveInput {
                doc: &loaded.doc,
                preserved: &loaded.preserved,
                blobs: &loaded.blobs,
                session: None,
                saved_at: "2026-11-03T19:00:00Z".into(),
            };
            tt_persist::save::save_package(&next, &input, None).unwrap();
            path = next;
        }
    }
}
