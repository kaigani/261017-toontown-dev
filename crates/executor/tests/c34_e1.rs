//! M3 C3.4 (builder): what scenario E1 needed beyond C3.1-C3.3 — a swatch path where a colour ID is accepted (11 §5),
//! `wait: true` inside a sequential batch (09 §11.1 item 4) — and the E1 rig's conservative `anim.changed` coverage and
//! evaluation counts (readiness items 9 and 10).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::collections::BTreeMap;
use std::sync::Mutex;

use common::*;
use serde_json::{Value, json};
use tt_executor::{BatchRequest, ExecutorHandle, Origin};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

const BALL: &str = "scene:#1/element:\"Ball\"";
const PEG: &str = "scene:#1/element:\"Bounce\"";
const CAMERA: &str = "scene:#1/element:\"Camera\"";

/// A new 1920x1080, 24 fps project with E1's ball: a red circle under a peg with keys at 1, 12 and 24 and explicit
/// easing on the velocity channel.
fn e1() -> (ExecutorHandle, std::path::PathBuf, String) {
    let dir = c13_support::tmp();
    let h = c13_support::spawn(&dir);
    ok(
        &h,
        "project.create",
        json!({ "name": "E1", "path": dir.join("E1.animpkg").to_string_lossy(), "frameRate": 24, "cameraWidth": 1920, "cameraHeight": 1080 }),
    );
    let red = red_path(&h);
    for (cmd, params) in [
        ("element.create", json!({ "type": "drawing", "name": "Ball", "sceneId": "scene:#1" })),
        (
            "draw.ellipse.add",
            json!({ "elementId": BALL, "frame": 1, "from": { "x": -50, "y": -50 }, "to": { "x": 50, "y": 50 }, "circle": true, "layer": "new", "colourId": red }),
        ),
        (
            "colour.paint.apply",
            json!({ "elementId": BALL, "frame": 1, "points": [{ "x": 0, "y": 0 }], "colourId": red }),
        ),
        (
            "xsheet.exposures.set",
            json!({ "columns": [{ "elementId": BALL, "from": 1, "to": 30, "exposures": [{ "drawing": "1", "from": 1, "to": 30 }] }] }),
        ),
        ("element.create", json!({ "type": "peg", "name": "Bounce", "sceneId": "scene:#1" })),
        ("element.parent.set", json!({ "elementIds": [BALL], "parentId": PEG })),
        ("element.timing.set", json!({ "elementIds": [PEG, CAMERA], "duration": 30 })),
        (
            "anim.path.set",
            json!({ "nodeId": PEG, "points": [{ "kind": "key", "frame": 1, "x": 0, "y": 2.5, "z": 0 }, { "kind": "key", "frame": 12, "x": 0, "y": -2.5, "z": 0 }, { "kind": "key", "frame": 24, "x": 0, "y": 2.5, "z": 0 }] }),
        ),
        (
            "anim.function.preset.apply",
            json!({ "nodeId": PEG, "channel": "velocity", "preset": "slowAcceleration", "frame": 1, "scope": "segment" }),
        ),
        (
            "anim.function.preset.apply",
            json!({ "nodeId": PEG, "channel": "velocity", "preset": "slowDeceleration", "frame": 12, "scope": "segment" }),
        ),
    ] {
        ok(&h, cmd, params);
    }
    (h, dir, red)
}

/// The red swatch of the default palette as a path (`palette:#1/swatch:#N`).
fn red_path(h: &ExecutorHandle) -> String {
    let export = ok(h, "doc.export.json", json!({ "scope": "project", "binaries": "omit" }));
    let swatches = export["bundle"]["palettes"][0]["swatches"].as_array().unwrap();
    let i = swatches
        .iter()
        .position(|s| {
            s["values"]
                .as_object()
                .unwrap()
                .values()
                .any(|v| v["rgba"].as_str().is_some_and(|c| c.eq_ignore_ascii_case("#FF0000FF")))
        })
        .unwrap();
    format!("palette:#1/swatch:#{}", i + 1)
}

#[test]
fn a_swatch_path_is_accepted_where_a_colour_id_is() {
    let _t = turn();
    let (h, _dir, red) = e1();
    let colour = ok(&h, "doc.object.get", json!({ "ref": red }))["object"]["colourId"].as_str().unwrap().to_owned();
    let drawing = ok(&h, "draw.drawing.get", json!({ "drawingId": format!("{BALL}/drawing:#1") }));
    let text = drawing.to_string();
    assert!(text.matches(&colour).count() >= 2, "the circle's line and its painted zone use the swatch's colour ID");
    assert!(!text.contains("palette:"), "the path is resolved, never stored");
    // By name too, and the same refusals as before for anything else.
    let name = ok(&h, "doc.object.get", json!({ "ref": red }))["object"]["name"].as_str().unwrap().to_owned();
    let by_name = format!("palette:#1/swatch:\"{name}\"");
    ok(
        &h,
        "draw.ellipse.add",
        json!({ "elementId": BALL, "frame": 1, "from": { "x": 100, "y": 100 }, "to": { "x": 120, "y": 120 }, "layer": "new", "colourId": by_name }),
    );
    for bad in ["palette:#1/swatch:#999", "palette:\"Nope\"/swatch:#1", "col_00000000000000000000000000", "scene:#1"] {
        let e = err(
            &h,
            "draw.ellipse.add",
            json!({ "elementId": BALL, "frame": 1, "from": { "x": 0, "y": 0 }, "to": { "x": 9, "y": 9 }, "colourId": bad }),
        );
        assert_eq!(e.code, "colour.notFound", "{bad}: {e:?}");
        let e = err(
            &h,
            "colour.paint.apply",
            json!({ "elementId": BALL, "frame": 1, "points": [{ "x": 0, "y": 0 }], "colourId": bad }),
        );
        assert_eq!(e.code, "colour.notFound", "{bad}: {e:?}");
    }
}

#[test]
fn wait_true_in_a_sequential_batch_answers_the_manifest() {
    let _t = turn();
    let (h, dir, _) = e1();
    let out = dir.join("seq");
    let req: BatchRequest = serde_json::from_value(json!({
        "label": "E1 export",
        "atomic": false,
        "stopOnError": true,
        "steps": [
            { "id": "render.range.run", "params": { "from": 1, "to": 3, "scale": 0.1, "directory": out.to_string_lossy(), "createDirs": true, "wait": true } },
            { "id": "render.range.run", "params": { "from": 1, "to": 3, "scale": 0.1, "directory": dir.join("seq2").to_string_lossy(), "createDirs": true } }
        ]
    }))
    .unwrap();
    let env = h.batch(req, Origin::User);
    assert!(env.is_ok(), "{env:?}");
    let steps = env.steps.unwrap();
    let m = steps[0].result.as_ref().unwrap();
    assert_eq!(m["frameCount"], 3, "{m}");
    assert!(m["jobId"].as_str().is_some_and(|j| j.starts_with("job-")), "{m}");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 4, "the frames and the manifest exist when the step answers");
    let started = steps[1].result.as_ref().unwrap();
    assert!(started.get("frameCount").is_none() && started["jobId"].is_string(), "without wait: the job ID: {started}");
}

fn shots(h: &ExecutorHandle, frames: &[u32]) -> Vec<String> {
    frames
        .iter()
        .map(|f| ok(h, "render.frame.run", json!({ "frame": f, "scale": 0.1 }))["sha256"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn e1_edits_announce_every_frame_whose_render_changes() {
    let _t = turn();
    let (h, _dir, _) = e1();
    let frames: Vec<u32> = (1..=30).collect();
    for (cmd, params) in [
        (
            "anim.function.preset.apply",
            json!({ "nodeId": PEG, "channel": "velocity", "preset": "easeInOut", "frame": 12, "scope": "segment" }),
        ),
        (
            "anim.function.preset.apply",
            json!({ "nodeId": PEG, "channel": "velocity", "preset": "linear", "frame": 1, "scope": "segment" }),
        ),
        (
            "anim.path.set",
            json!({ "nodeId": PEG, "points": [{ "kind": "key", "frame": 1, "x": 0, "y": 2.0, "z": 0 }, { "kind": "key", "frame": 12, "x": 0, "y": -2.5, "z": 0 }, { "kind": "key", "frame": 24, "x": 0, "y": 2.0, "z": 0 }] }),
        ),
    ] {
        let before = shots(&h, &frames);
        let rx = h.subscribe();
        ok(&h, cmd, params.clone());
        let events: Vec<Value> = rx.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload).collect();
        let after = shots(&h, &frames);
        let changed: Vec<u32> =
            frames.iter().zip(before.iter().zip(&after)).filter(|(_, (a, b))| a != b).map(|(f, _)| *f).collect();
        assert!(!changed.is_empty(), "{cmd} {params}: the edit changes renders");
        assert!(!events.is_empty(), "{cmd}: one anim.changed");
        for f in &changed {
            let covered = events.iter().any(|ev| match ev.get("frames") {
                None => true,
                Some(r) => r["from"].as_f64().unwrap() <= f64::from(*f) && f64::from(*f) <= r["to"].as_f64().unwrap(),
            });
            assert!(covered, "{cmd}: frame {f} changed outside {events:?}");
        }
    }
    // Undo and redo announce their frames too, and restore the renders exactly.
    let before = shots(&h, &frames);
    let rx = h.subscribe();
    ok(&h, "edit.undo", json!({}));
    assert!(rx.try_iter().any(|e| e.name == "anim.changed"), "undo announces anim.changed");
    assert_ne!(shots(&h, &frames), before);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(shots(&h, &frames), before, "redo restores every frame's bytes");
}

#[test]
fn e1_queries_evaluate_each_node_once_per_frame() {
    let _t = turn();
    let (h, _dir, _) = e1();
    let ids: Vec<String> = [BALL, PEG]
        .iter()
        .map(|r| ok(&h, "doc.object.get", json!({ "ref": r }))["object"]["id"].as_str().unwrap().to_owned())
        .collect();
    let sheet_frames: Vec<u32> = (1..=24).collect();
    for (cmd, params, frames) in [
        ("render.frame.run", json!({ "frame": 12, "scale": 0.1 }), vec![12]),
        ("inspect.frame.describe", json!({ "frame": 12, "detail": "full" }), vec![12]),
        ("inspect.hitTest", json!({ "space": "image", "point": { "x": 960, "y": 940 }, "frame": 12 }), vec![12]),
        ("inspect.contactSheet.render", json!({ "frames": sheet_frames, "cellWidth": 48 }), sheet_frames.clone()),
    ] {
        tt_anim::eval::hooks::start();
        ok(&h, cmd, params);
        let mut counts: BTreeMap<(String, u64), usize> = BTreeMap::new();
        for (id, f) in tt_anim::eval::hooks::take() {
            if ids.contains(&id) {
                *counts.entry((id, f.to_bits())).or_default() += 1;
            }
        }
        assert_eq!(counts.len(), ids.len() * frames.len(), "{cmd}: every E1 node at every frame: {counts:?}");
        assert!(counts.values().all(|n| *n == 1), "{cmd}: once per node per frame: {counts:?}");
    }
}

#[test]
fn a_dead_jobs_sidecar_names_this_host_in_any_letter_case() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let gone = {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        child.wait().unwrap();
        child.id()
    };
    let host = tt_persist::host::host();
    let flipped: String = host
        .chars()
        .map(|c| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() })
        .collect();
    let token = "c34c34c34c34";
    let side = json!({ "kind": "toontown.render.range.staging", "version": 1, "token": token, "jobId": "job-dead", "head": "unknown", "productVersion": "0.0.1", "host": flipped, "pid": gone, "published": false, "targets": ["old.png"], "staged": [format!(".old.png.tt-partial-{token}")], "backups": [format!(".old.png.tt-backup-{token}")] });
    std::fs::write(dir.join(format!(".tt-partial-{token}.job")), side.to_string()).unwrap();
    std::fs::write(dir.join(format!(".old.png.tt-backup-{token}")), b"original").unwrap();
    let m = c13_support::job_ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 1, "width": 32, "directory": dir.to_string_lossy() }),
    );
    let w = m["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["code"] == "render.staleStagingRemoved")
        .expect("recovered");
    assert_eq!(w["restored"], json!(["old.png"]), "{w}");
    assert_eq!(std::fs::read(dir.join("old.png")).unwrap(), b"original");
    assert!(tt_persist::host::is_this_host(&flipped) && !tt_persist::host::is_this_host("another-machine"));
}
