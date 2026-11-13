//! M3 C3.3 round 1 (builder): depth-tie groups (C33-R1-01), `wait: true` (C33-R1-02), transactional publication
//! (C33-R1-03), size limits before work (C33-R1-04), `include`/`exclude` (C33-R1-05), `renderMs` (C33-R1-06), the
//! staging sweep (C33-R1-08), and one evaluation per node per frame for a render-list query.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod c33_support;
mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use c33_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Status};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Spin;

impl Drop for Spin {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
    }
}

fn order(h: &ExecutorHandle) -> Vec<String> {
    ok(h, "inspect.frame.describe", json!({ "frame": 1, "detail": "full" }))["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["elementId"].as_str().unwrap().to_owned())
        .collect()
}

fn z(h: &ExecutorHandle, id: &str, z: f64) {
    ok(h, "anim.transform.set", json!({ "nodeIds": [id], "target": "static", "position": { "z": z } }));
}

/// Back to front = displayed Timeline rows bottom to top, for the given elements.
fn timeline_back_to_front(h: &ExecutorHandle, ids: &[String]) -> Vec<String> {
    let doc = h.snapshot().unwrap().unwrap();
    let scene = doc.scenes().find(|s| s.name() == "Intro").unwrap();
    let mut rows: Vec<String> =
        scene.tree().iter().map(|n| n.id.as_str().to_owned()).filter(|i| ids.contains(i)).collect();
    rows.reverse();
    rows
}

#[test]
fn depth_ties_group_from_the_smallest_depth_and_follow_the_timeline() {
    // Serialised with the evaluation-count test: the evaluator's test recorder is process-wide.
    let _t = turn();
    let h = fixture();
    clear(&h);
    let c = solids(&h, 1)[0].0.clone();
    let ids: Vec<String> = (0..3).map(|i| square(&h, &format!("Tie {i}"), 50.0, &c)).collect();
    let timeline = timeline_back_to_front(&h, &ids);
    // Two depths 2e-8 apart on either side of 0.5e-6, in both assignments: one group, Timeline order (05 §2.8).
    for (a, b) in [(0.000_000_49, 0.000_000_51), (0.000_000_51, 0.000_000_49)] {
        z(&h, &ids[0], a);
        z(&h, &ids[1], b);
        z(&h, &ids[2], 5.0);
        let got = order(&h);
        let pair: Vec<String> = timeline.iter().filter(|i| **i != ids[2]).cloned().collect();
        assert_eq!(got[..2], pair[..], "a tie follows the Timeline");
        assert_eq!(got[2], ids[2]);
    }
    // A chain 0, 0.6e-6, 1.2e-6: the group anchored at 0 takes 0.6e-6 (under 1e-6 from the anchor) but not 1.2e-6,
    // whose distance from the anchor reaches 1e-6 although it is only 0.6e-6 from its neighbour (ADR-0020, SI-106).
    let depths = [0.0, 0.000_000_6, 0.000_001_2];
    for (id, d) in ids.iter().zip(depths) {
        z(&h, id, d);
    }
    let got = order(&h);
    let group: Vec<String> = timeline.iter().filter(|i| **i != ids[2]).cloned().collect();
    assert_eq!(got[..2], group[..], "the anchored group in Timeline order");
    assert_eq!(got[2], ids[2], "the third depth starts a new group");
}

#[test]
fn a_render_list_query_evaluates_each_node_once_per_frame() {
    let _t = turn();
    let h = fixture();
    clear(&h);
    let c = solids(&h, 1)[0].0.clone();
    let leaf = square(&h, "Leaf", 40.0, &c);
    let mut parent = String::new();
    let mut nodes = vec![leaf.clone()];
    for i in 0..4 {
        let peg = ok(&h, "element.create", json!({ "type": "peg", "name": format!("Chain {i}"), "sceneId": INTRO }))["elementIds"][0]
            .as_str()
            .unwrap()
            .to_owned();
        if !parent.is_empty() {
            ok(&h, "element.parent.set", json!({ "elementIds": [peg], "parentId": parent }));
        }
        ok(&h, "anim.transform.set", json!({ "nodeIds": [peg], "target": "static", "rotation": 10 }));
        nodes.push(peg.clone());
        parent = peg;
    }
    ok(&h, "element.parent.set", json!({ "elementIds": [leaf], "parentId": parent }));
    for (command, params) in [
        ("inspect.frame.describe", json!({ "frame": 5, "detail": "full" })),
        ("render.frame.run", json!({ "frame": 5, "scale": 0.1 })),
        ("inspect.hitTest", json!({ "space": "image", "point": { "x": 960, "y": 540 }, "frame": 5 })),
    ] {
        tt_anim::eval::hooks::start();
        ok(&h, command, params);
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (id, f) in tt_anim::eval::hooks::take() {
            if nodes.contains(&id) {
                assert_eq!(f, 5.0);
                *counts.entry(id).or_default() += 1;
            }
        }
        assert_eq!(counts.len(), nodes.len(), "{command}: every node of the chain is evaluated");
        assert!(counts.values().all(|n| *n == 1), "{command}: once per node per frame: {counts:?}");
    }
}

#[test]
fn wait_true_answers_the_manifest_and_a_timeout_keeps_the_job_addressable() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let m = ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 2, "width": 48, "directory": dir.to_string_lossy(), "wait": true }),
    );
    assert_eq!(m["frameCount"], 2, "{m}");
    assert!(m["jobId"].as_str().is_some_and(|j| j.starts_with("job-")), "{m}");
    assert_eq!(m["files"].as_array().unwrap().len(), 2);
    // Without `wait`, the job ID as before.
    let other = c13_support::tmp();
    let started =
        ok(&h, "render.range.run", json!({ "from": 1, "to": 1, "width": 48, "directory": other.to_string_lossy() }));
    assert!(started.get("frameCount").is_none() && started["jobId"].is_string());
    c13_support::job_ok(&h, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60000 }));
    // A job slower than the call's budget: `job.waitTimeout` with the job ID, which stays addressable.
    let slow = fixture_with(ExecutorConfig { budget: Duration::from_millis(500) });
    let out = c13_support::tmp();
    let _spin = Spin;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let e = err(
        &slow,
        "render.range.run",
        json!({ "from": 1, "to": 48, "width": 48, "directory": out.to_string_lossy(), "wait": true }),
    );
    assert_eq!(e.code, "job.waitTimeout", "{e:?}");
    let job = e.details["jobId"].as_str().unwrap().to_owned();
    ok(&slow, "job.cancel", json!({ "jobId": job }));
    tt_vector::work::hooks::set_spin_micros(0);
    let rec = slow.jobs().wait(&job, Duration::from_secs(60)).unwrap().0;
    assert_eq!(rec.state, tt_executor::JobState::Cancelled, "{rec:?}");
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn a_failed_publication_restores_the_destination_and_publishes_nothing() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    std::fs::write(dir.join("shot-1.png"), b"the original frame 1").unwrap();
    std::fs::create_dir(dir.join("shot-2.png")).unwrap();
    let started = ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 2, "width": 32, "directory": dir.to_string_lossy(), "namePattern": "shot-{frame}", "onConflict": "overwrite" }),
    );
    let rec = c13_support::wait(&h, &started);
    assert_eq!(rec["state"], "failed", "{rec}");
    assert_eq!(rec["error"]["code"], "export.targetInvalid", "{rec}");
    assert_eq!(std::fs::read(dir.join("shot-1.png")).unwrap(), b"the original frame 1", "the original is kept");
    assert_eq!(files(&dir), vec!["shot-1.png", "shot-2.png"], "no staging, backup, frame or manifest is left");
    // With every target replaceable, the overwrite succeeds and leaves no backup.
    std::fs::remove_dir(dir.join("shot-2.png")).unwrap();
    std::fs::write(dir.join("shot-2.png"), b"the original frame 2").unwrap();
    let m = c13_support::job_ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 2, "width": 32, "directory": dir.to_string_lossy(), "namePattern": "shot-{frame}", "onConflict": "overwrite" }),
    );
    assert_eq!(m["frameCount"], 2);
    assert_ne!(std::fs::read(dir.join("shot-1.png")).unwrap(), b"the original frame 1");
    assert_eq!(files(&dir), vec!["Intro_0001-0002.manifest.json", "shot-1.png", "shot-2.png"]);
}

// The staging-recovery test of round 1 is replaced by `c33_round2.rs` (sidecar ownership, C33-R2-01).

#[test]
fn include_and_exclude_override_visibility_without_touching_the_document() {
    let _t = turn();
    let h = fixture();
    let rev0 = rev(&h);
    let sha =
        |h: &ExecutorHandle, params: Value| ok(h, "render.frame.run", params)["sha256"].as_str().unwrap().to_owned();
    // exclude Bob = hiding Bob.
    let excluded = sha(&h, json!({ "frame": 1, "scale": 0.25, "exclude": [BOB] }));
    let sheet_excluded =
        ok(&h, "inspect.contactSheet.render", json!({ "frames": [1, 9], "cellWidth": 48, "exclude": [BOB] }))["sha256"]
            .clone();
    let described = ok(&h, "inspect.frame.describe", json!({ "frame": 1, "exclude": [BOB] }));
    assert_eq!(rev(&h), rev0, "the document is not changed");
    let shown = sha(&h, json!({ "frame": 1, "scale": 0.25 }));
    assert_ne!(excluded, shown, "Bob is in the frame");
    let reasons: Vec<&str> =
        described["omitted"].as_array().unwrap().iter().map(|o| o["reason"].as_str().unwrap()).collect();
    assert!(reasons.contains(&"excluded"), "{reasons:?}");
    let dir = c13_support::tmp();
    let m = c13_support::job_ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 1, "scale": 0.25, "directory": dir.to_string_lossy(), "exclude": [BOB] }),
    );
    ok(&h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": false }));
    assert_eq!(sha(&h, json!({ "frame": 1, "scale": 0.25 })), excluded, "the same image as hiding the element");
    assert_eq!(
        ok(&h, "inspect.contactSheet.render", json!({ "frames": [1, 9], "cellWidth": 48 }))["sha256"],
        sheet_excluded
    );
    assert_eq!(m["files"][0]["sha256"].as_str(), Some(excluded.as_str()));
    // include shows a hidden element (Bob is hidden now): the same image as showing it.
    assert_eq!(sha(&h, json!({ "frame": 1, "scale": 0.25, "include": [BOB] })), shown);
    // Excluding a peg leaves out its descendants; both lists name the element: excluded.
    let child = {
        ok(&h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": true }));
        let c = solids(&h, 1)[0].0.clone();
        let sq = square(&h, "Under peg", 60.0, &c);
        ok(&h, "element.parent.set", json!({ "elementIds": [sq], "parentId": PEG }));
        sq
    };
    let d = ok(&h, "inspect.frame.describe", json!({ "frame": 1, "exclude": [PEG] }));
    let omitted: Vec<&str> = d["omitted"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["reason"] == "excluded")
        .map(|o| o["elementId"].as_str().unwrap())
        .collect();
    assert!(omitted.contains(&child.as_str()), "{omitted:?}");
    let both = ok(&h, "inspect.frame.describe", json!({ "frame": 1, "include": [BOB], "exclude": [BOB] }));
    assert!(both["omitted"].as_array().unwrap().iter().any(|o| o["reason"] == "excluded"));
    assert_eq!(
        err(&h, "render.frame.run", json!({ "exclude": ["scene:\"Intro\"/element:\"Nobody\""] })).code,
        "arg.invalid"
    );
}

#[test]
fn known_sizes_are_refused_before_work_and_render_ms_is_reported() {
    let _t = turn();
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(100) });
    let _spin = Spin;
    tt_vector::work::hooks::set_spin_micros(300_000);
    for (command, params) in [
        ("render.frame.run", json!({ "width": 2049, "height": 1 })),
        ("render.frame.run", json!({ "scale": 2 })),
        ("inspect.contactSheet.render", json!({ "frames": [1, 2, 3, 4], "columns": 2, "cellWidth": 2048 })),
    ] {
        let e = err(&h, command, params.clone());
        assert_eq!(e.code, "render.inlineTooLarge", "{command} {params}: {e:?}");
    }
    let e = err(&h, "render.frame.run", json!({ "width": 8192, "scale": 2 }));
    assert_eq!(e.code, "limit.exceeded");
    tt_vector::work::hooks::set_spin_micros(0);
    let h = fixture();
    let r = ok(&h, "render.frame.run", json!({ "frame": 1, "scale": 0.1 }));
    assert!(r["renderMs"].is_u64(), "{r}");
    let again = ok(&h, "render.frame.run", json!({ "frame": 1, "scale": 0.1 }));
    assert_eq!(again["sha256"], r["sha256"], "renderMs is volatile; the image bytes are the contract");
    let env = run(
        &h,
        "render.frame.run",
        json!({ "width": 4096, "height": 2, "output": "file", "path": c13_support::tmp().join("wide.png").to_string_lossy() }),
    );
    assert_eq!(env.status, Status::Ok, "file output is not limited by the inline size: {env:?}");
}
