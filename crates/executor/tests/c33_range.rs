//! M3 C3.3 (builder; readiness item 9): `render.range.run` — frame ranges and file names, bytes equal to single-frame
//! renders, the manifest and its companion file delivered with the frames, conflicts, formats, repeated bytes, and a
//! cancelled job that publishes nothing (no frame, no companion, no staged file).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod c33_support;
mod common;

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use c33_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn animated() -> ExecutorHandle {
    let h = fixture();
    clear(&h);
    let (c, _) = solids(&h, 1)[0].clone();
    let a = square(&h, "Mover", 60.0, &c);
    ok(&h, "anim.transform.set", json!({ "nodeIds": [a], "frame": 1, "position": { "x": -3 } }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [a], "frame": 24, "position": { "x": 3 }, "rotation": 90 }));
    h
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn run_range(h: &ExecutorHandle, params: Value) -> Value {
    c13_support::job_ok(h, "render.range.run", params)
}

#[test]
fn a_range_writes_named_frames_equal_to_single_renders_and_a_companion_manifest() {
    let _t = turn();
    let h = animated();
    let dir = c13_support::tmp();
    let m = run_range(&h, json!({ "from": 1, "to": 9, "step": 4, "scale": 0.1, "directory": dir.to_string_lossy() }));
    assert_eq!(
        files(&dir),
        vec!["Intro_0001-0009.manifest.json", "Intro_0001.png", "Intro_0005.png", "Intro_0009.png"]
    );
    assert_eq!(m["frameCount"], 3);
    assert_eq!(m["format"], "png");
    assert_eq!((m["width"].as_u64(), m["height"].as_u64()), (Some(192), Some(108)));
    for (i, f) in [1u64, 5, 9].iter().enumerate() {
        let entry = &m["files"][i];
        assert_eq!((entry["role"].as_str(), entry["frame"].as_u64()), (Some("frame"), Some(*f)));
        let bytes = std::fs::read(entry["path"].as_str().unwrap()).unwrap();
        assert_eq!(entry["bytes"].as_u64().unwrap() as usize, bytes.len());
        let single = render(&h, json!({ "frame": f, "scale": 0.1 }));
        assert_eq!(bytes, single.png, "frame {f}: the same bytes as render.frame.run");
        assert_eq!(entry["sha256"], single.result["sha256"]);
    }
    // The companion is the manifest itself.
    let companion: Value =
        serde_json::from_slice(&std::fs::read(dir.join("Intro_0001-0009.manifest.json")).unwrap()).unwrap();
    assert_eq!(companion, m);
    // Repeated: the same bytes, the same manifest.
    let again = c13_support::tmp();
    let m2 =
        run_range(&h, json!({ "from": 1, "to": 9, "step": 4, "scale": 0.1, "directory": again.to_string_lossy() }));
    for i in 0..3 {
        assert_eq!(m["files"][i]["sha256"], m2["files"][i]["sha256"]);
    }
}

#[test]
fn names_conflicts_and_formats() {
    let _t = turn();
    let h = animated();
    let dir = c13_support::tmp();
    let p =
        json!({ "from": 2, "to": 3, "scale": 0.1, "directory": dir.to_string_lossy(), "namePattern": "shot-{frame}" });
    run_range(&h, p.clone());
    assert_eq!(files(&dir), vec!["Intro_0002-0003.manifest.json", "shot-2.png", "shot-3.png"]);
    let e = err(&h, "render.range.run", p.clone());
    assert_eq!(e.code, "export.fileExists");
    assert_eq!(e.details["paths"].as_array().unwrap().len(), 3, "every conflicting path, before anything runs");
    let mut renamed = p.clone();
    renamed["onConflict"] = json!("rename");
    let m = run_range(&h, renamed);
    assert!(m["files"][0]["path"].as_str().unwrap().ends_with("shot-2 (2).png"), "{m}");
    let mut over = p.clone();
    over["onConflict"] = json!("overwrite");
    run_range(&h, over);
    // Bad patterns and directories.
    let mut bad = p.clone();
    bad["namePattern"] = json!("same");
    assert_eq!(err(&h, "render.range.run", bad).code, "arg.invalid", "two frames, one name");
    let missing = dir.join("not-yet");
    assert_eq!(
        err(&h, "render.range.run", json!({ "from": 1, "to": 1, "directory": missing.to_string_lossy() })).code,
        "arg.invalid"
    );
    run_range(
        &h,
        json!({ "from": 1, "to": 1, "scale": 0.1, "directory": missing.to_string_lossy(), "createDirs": true }),
    );
    assert_eq!(files(&missing).len(), 2);
    assert_eq!(
        err(&h, "render.range.run", json!({ "composition": "xsheet", "directory": dir.to_string_lossy() })).code,
        "render.compositionUnavailable"
    );
    // Other formats decode to the same size.
    for (format, ext) in [("jpeg", "jpg"), ("tga", "tga"), ("bmp", "bmp")] {
        let out = c13_support::tmp();
        let m = run_range(
            &h,
            json!({ "from": 4, "to": 4, "scale": 0.1, "format": format, "directory": out.to_string_lossy() }),
        );
        let path = m["files"][0]["path"].as_str().unwrap().to_owned();
        assert!(path.ends_with(&format!("Intro_0004.{ext}")), "{path}");
        let img = tt_io::image::decode(&std::fs::read(&path).unwrap(), Some(ext), &tt_io::image::LIMITS).unwrap();
        assert_eq!((img.width, img.height), (192, 108), "{format}");
    }
}

#[test]
fn a_cancelled_range_publishes_nothing() {
    let _t = turn();
    let h = animated();
    let dir = c13_support::tmp();
    // Slow every work charge so the job is still rendering when it is cancelled.
    tt_vector::work::hooks::set_spin_micros(20_000);
    let started =
        ok(&h, "render.range.run", json!({ "from": 1, "to": 48, "scale": 0.1, "directory": dir.to_string_lossy() }));
    let job = started["jobId"].as_str().unwrap().to_owned();
    std::thread::sleep(Duration::from_millis(300));
    ok(&h, "job.cancel", json!({ "jobId": job }));
    let rec = h.jobs().wait(&job, Duration::from_secs(60)).unwrap().0;
    tt_vector::work::hooks::set_spin_micros(0);
    assert_ne!(rec.state, tt_executor::JobState::Succeeded, "{rec:?}");
    assert!(files(&dir).is_empty(), "no frame, companion or staged file: {:?}", files(&dir));
}
