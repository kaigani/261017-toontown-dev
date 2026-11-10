//! M3 C3.3 (builder; readiness items 4, 5, 12; ADR-0020): golden images of fixture scenes built by commands, compared
//! byte for byte with the committed PNGs in `tests/golden/c33/` on every OS (`check` in debug, `c33 render release` in
//! release). No text and no fonts (machine fonts would differ, ADR-0016).
//!
//! `TT_UPDATE_GOLDEN=1 cargo test -p tt-executor --test c33_goldens` re-records the PNGs and `index.json`; a legitimate
//! change is declared in the checkpoint's handoff with the old and new SHA-256 of each file (plan §6). On a mismatch the
//! actual image is written beside the target directory under `target/c33-golden-diff/`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c33_support;
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use c33_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/c33")
}

fn sha(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The staging scene: a back square, a rotated, mirrored and skewed front square at Z = 1 over it, a pencil line and a
/// brush stroke on a third element under a rotated peg with a pivot, and the two colours of the overlap.
fn staging(h: &ExecutorHandle) {
    clear(h);
    let c = solids(h, 3);
    let back = square(h, "Back", 150.0, &c[0].0);
    let front = square(h, "Front", 90.0, &c[1].0);
    ok(
        h,
        "anim.transform.set",
        json!({ "nodeIds": [front], "target": "static", "position": { "x": 1, "y": 0.5, "z": 1 }, "rotation": 25, "scale": { "x": -1.2, "y": 0.9 }, "skew": 15 }),
    );
    let lines = ok(h, "element.create", json!({ "type": "drawing", "name": "Lines", "sceneId": INTRO }))["elementIds"]
        [0]
    .as_str()
    .unwrap()
    .to_owned();
    ok(
        h,
        "draw.stroke.add",
        json!({ "elementId": lines, "frame": 1, "kind": "pencil", "fit": false, "layer": "new", "thickness": 6, "colourId": c[1].0,
        "points": [{"x": -300, "y": -100}, {"x": -150, "y": 50}, {"x": 0, "y": -100}] }),
    );
    ok(
        h,
        "draw.stroke.add",
        json!({ "elementId": lines, "frame": 1, "kind": "brush", "colourId": c[0].0,
        "points": [{"x": 100, "y": -200, "pressure": 1}, {"x": 250, "y": -150, "pressure": 0.5}],
        "pen": { "tipShape": "round", "minSize": 4, "maxSize": 16, "tipAngle": 0 } }),
    );
    let peg = ok(h, "element.create", json!({ "type": "peg", "name": "C33 Peg", "sceneId": INTRO }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(h, "element.parent.set", json!({ "elementIds": [lines], "parentId": peg }));
    ok(
        h,
        "anim.transform.set",
        json!({ "nodeIds": [peg], "target": "static", "rotation": -12, "position": { "x": -1.5, "y": -1 }, "pivot": { "x": 0.5, "y": 0.25 } }),
    );
    let _ = back;
}

/// A golden: name, scene setup, command and parameters.
type Case = (&'static str, fn(&ExecutorHandle), &'static str, Value);

/// The scenes and frames of the goldens.
fn cases() -> Vec<Case> {
    fn roll(h: &ExecutorHandle) {
        staging(h);
        ok(
            h,
            "anim.camera.set",
            json!({ "cameraId": CAMERA, "roll": 17, "fovMode": "dynamic", "fov": 35, "frame": 1 }),
        );
        ok(h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 55, "frame": 13 }));
    }
    fn moving(h: &ExecutorHandle) {
        staging(h);
        let doc = h.snapshot().unwrap().unwrap();
        let front = doc.elements().find(|e| e.name() == "Front").unwrap().id().as_str().to_owned();
        ok(h, "anim.transform.set", json!({ "nodeIds": [front], "frame": 1, "position": { "x": -2 } }));
        ok(
            h,
            "anim.transform.set",
            json!({ "nodeIds": [front], "frame": 24, "position": { "x": 2.5 }, "rotation": 120 }),
        );
    }
    vec![
        ("staging", staging, "render.frame.run", json!({ "frame": 1, "scale": 0.25 })),
        (
            "staging-transparent",
            staging,
            "render.frame.run",
            json!({ "frame": 1, "width": 320, "height": 320, "background": "transparent" }),
        ),
        ("roll-dynamic-fov", roll, "render.frame.run", json!({ "frame": 7, "scale": 0.25 })),
        ("moving-frame-9", moving, "render.frame.run", json!({ "frame": 9, "scale": 0.25 })),
        (
            "moving-contact-sheet",
            moving,
            "inspect.contactSheet.render",
            json!({ "frames": { "from": 1, "to": 24, "step": 6 }, "cellWidth": 120 }),
        ),
    ]
}

#[test]
fn golden_images_match_byte_for_byte() {
    let update = std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let index_path = dir().join("index.json");
    let mut index: BTreeMap<String, Value> = if update {
        BTreeMap::new()
    } else {
        serde_json::from_slice(
            &std::fs::read(&index_path).expect("tests/golden/c33/index.json (TT_UPDATE_GOLDEN=1 records it)"),
        )
        .unwrap()
    };
    let mut mismatches = Vec::new();
    for (name, setup, command, params) in cases() {
        let h = fixture();
        setup(&h);
        let r = ok(&h, command, params.clone());
        let png = tt_persist::b64::decode(r["data"].as_str().or(r["png"].as_str()).unwrap()).unwrap();
        // A repeat in the same process gives the same bytes.
        let again = ok(&h, command, params.clone());
        assert_eq!(again["sha256"], r["sha256"], "{name}: repeated render");
        let file = dir().join(format!("{name}.png"));
        if update {
            std::fs::create_dir_all(dir()).unwrap();
            std::fs::write(&file, &png).unwrap();
            index.insert(
                name.to_owned(),
                json!({ "command": command, "params": params, "sha256": sha(&png), "bytes": png.len(), "width": r["width"], "height": r["height"] }),
            );
            continue;
        }
        let want = std::fs::read(&file).unwrap_or_default();
        if want != png {
            let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/c33-golden-diff");
            std::fs::create_dir_all(&out).unwrap();
            std::fs::write(out.join(format!("{name}.png")), &png).unwrap();
            mismatches.push(format!("{name}: {} vs golden {}", sha(&png), sha(&want)));
        }
        assert_eq!(index[name]["sha256"].as_str(), Some(sha(&want).as_str()), "{name}: index and file agree");
    }
    if update {
        std::fs::write(&index_path, serde_json::to_string_pretty(&index).unwrap() + "\n").unwrap();
    }
    assert!(
        mismatches.is_empty(),
        "golden mismatches (actual images in target/c33-golden-diff/):\n{}",
        mismatches.join("\n")
    );
}

/// The render gate's scene (`tests/fixtures/c33/gate-scene.json`), replayed in process on a new project: its images are
/// recorded in `tests/golden/c33/gate-index.json`, and `scripts/c33-render-gate.py` checks that the release headless
/// binary renders the same bytes (in-process and headless agree, readiness item 4).
#[test]
fn gate_scene_images_match_the_gate_index() {
    let update = std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let gate: Value = serde_json::from_slice(
        &std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/c33/gate-scene.json"))
            .unwrap(),
    )
    .unwrap();
    let tmp = std::env::temp_dir().join(format!("tt-c33-gate-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&tmp).unwrap();
    let h = ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
        config: tt_executor::ExecutorConfig { budget: std::time::Duration::from_secs(60) },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_persist::settings::UserDirs::under(&tmp.join("user"))),
            maintenance: tt_executor::Maintenance { tick: std::time::Duration::ZERO, ..Default::default() },
            ..Default::default()
        },
    })
    .unwrap();
    ok(&h, "project.create", json!({ "name": "Gate", "path": tmp.join("Gate.animpkg").to_string_lossy() }));
    for step in gate["steps"].as_array().unwrap() {
        let mut params = step["params"].clone();
        swatches(&h, &mut params);
        ok(&h, step["id"].as_str().unwrap(), params);
    }
    let index_path = dir().join("gate-index.json");
    let mut index: BTreeMap<String, Value> = if update {
        BTreeMap::new()
    } else {
        serde_json::from_slice(&std::fs::read(&index_path).expect("gate-index.json (TT_UPDATE_GOLDEN=1 records it)"))
            .unwrap()
    };
    let mut mismatches = Vec::new();
    for out in gate["outputs"].as_array().unwrap() {
        let (name, id) = (out["name"].as_str().unwrap(), out["id"].as_str().unwrap());
        let r = ok(&h, id, out["params"].clone());
        let Some(b64) = r["data"].as_str().or(r["png"].as_str()) else {
            // A JSON read-back (the passage frames): compared across operating systems by the gate script.
            assert!(r["points"].as_array().is_some_and(|p| p.len() == 4), "{name}: {r}");
            continue;
        };
        let png = tt_persist::b64::decode(b64).unwrap();
        let file = dir().join(format!("{name}.png"));
        if update {
            std::fs::write(&file, &png).unwrap();
            index.insert(name.to_owned(), json!({ "sha256": sha(&png), "bytes": png.len() }));
        } else if index.get(name).and_then(|e| e["sha256"].as_str()) != Some(sha(&png).as_str()) {
            mismatches.push(format!("{name}: {}", sha(&png)));
        }
    }
    if update {
        std::fs::write(&index_path, serde_json::to_string_pretty(&index).unwrap() + "\n").unwrap();
    }
    drop(h);
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(mismatches.is_empty(), "gate images differ from gate-index.json:\n{}", mismatches.join("\n"));
}

/// Replaces `$swatch:N` strings by the colour ID of the first palette's N-th swatch (as the gate script does).
fn swatches(h: &ExecutorHandle, v: &mut Value) {
    match v {
        Value::String(s) if s.starts_with("$swatch:") => {
            let r = format!("palette:#1/swatch:#{}", &s["$swatch:".len()..]);
            *s = ok(h, "doc.object.get", json!({ "ref": r }))["object"]["colourId"].as_str().unwrap().to_owned();
        }
        Value::Array(a) => a.iter_mut().for_each(|x| swatches(h, x)),
        Value::Object(m) => m.values_mut().for_each(|x| swatches(h, x)),
        _ => {}
    }
}
