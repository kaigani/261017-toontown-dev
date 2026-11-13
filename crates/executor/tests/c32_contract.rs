//! M3 C3.2 (builder): the common contract of every animation command (readiness item 12): one undo step, dry-run isolation,
//! atomic rollback, stale guards, request-owned cancellation, explicit and current targets, one `anim.changed` per
//! undo step, unchanged selection and zones, save/open. The spin hook is process-global, so the tests take turns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, InvokeOptions, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const CAMERA: &str = "scene:\"Intro\"/element:\"Camera\"";
const POINT: &str = "scene:\"Intro\"/element:\"Bob\"/channel:\"position\"/key:\"24\"";

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The whole scene as JSON, every level: what a command may change, compared without revisions.
fn state(h: &ExecutorHandle) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("rev");
                m.remove("modifiedAt");
                m.values_mut().for_each(strip);
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut v = ok(h, "doc.object.get", json!({ "ref": "scene:\"Intro\"", "depth": -1 }))["object"].clone();
    strip(&mut v);
    v
}

/// Every C3.2 action with parameters valid on the fixture.
fn actions() -> Vec<(&'static str, Value)> {
    vec![
        ("anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "rotation": 12 })),
        ("anim.transform.set", json!({ "nodeIds": [PEG], "frame": 12, "rotation": 45, "position": { "x": 1 } })),
        ("anim.loop.set", json!({ "nodeId": PEG, "loops": 2 })),
        (
            "anim.path.set",
            json!({ "nodeId": PEG, "points": [{ "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 }, { "kind": "key", "frame": 9, "x": 3, "y": 0, "z": 0 }] }),
        ),
        ("anim.path.point.add", json!({ "nodeId": BOB, "kind": "key", "frame": 6 })),
        ("anim.path.point.set", json!({ "pointIds": [POINT], "x": 7, "bias": 0.25 })),
        ("anim.path.point.remove", json!({ "pointIds": [POINT] })),
        ("anim.path.lockDepth.set", json!({ "nodeId": BOB, "frame": 1, "enabled": true })),
        ("anim.key.add", json!({ "nodeIds": [BOB], "frame": 6 })),
        ("anim.key.move", json!({ "nodeId": BOB, "fromFrame": 12, "toFrame": 14 })),
        ("anim.key.remove", json!({ "nodeIds": [BOB], "frame": 12 })),
        ("anim.key.remove", json!({ "nodeIds": [BOB], "all": true })),
        ("anim.segment.set", json!({ "nodeIds": [BOB], "frame": 1, "constant": true })),
        ("anim.segment.uniform.apply", json!({ "nodeId": BOB, "frame": 1, "side": "right" })),
        ("anim.function.set", json!({ "nodeId": BOB, "channel": "skew", "keys": [{ "frame": 1, "value": 5 }] })),
        ("anim.function.key.add", json!({ "nodeId": BOB, "channel": "rotation", "frame": 6, "value": 3 })),
        ("anim.function.key.set", json!({ "nodeId": BOB, "channel": "rotation", "frame": 12, "value": 3 })),
        ("anim.function.key.remove", json!({ "nodeId": BOB, "channel": "rotation", "frames": [12] })),
        ("anim.function.key.duplicate", json!({ "nodeId": BOB, "channel": "rotation", "frame": 12, "toFrame": 13 })),
        ("anim.function.curveType.set", json!({ "nodeId": BOB, "channel": "rotation", "curveType": "interpolation" })),
        (
            "anim.function.preset.apply",
            json!({ "nodeId": BOB, "channel": "rotation", "preset": "easeInOut", "scope": "all" }),
        ),
        ("anim.camera.create", json!({ "name": "Second", "makeCurrent": true })),
        ("anim.camera.set", json!({ "cameraId": CAMERA, "fov": 60 })),
    ]
}

#[test]
fn every_action_is_one_undo_step_and_a_dry_run_changes_nothing() {
    let _t = turn();
    let h = fixture();
    for (id, params) in actions() {
        let before = state(&h);
        let rev0 = rev(&h);
        let app = ok(&h, "app.state.get", json!({}));
        let steps = ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap();
        // A dry run changes nothing, and reports what the real call would change.
        let dry = with(&h, id, params.clone(), InvokeOptions { dry_run: true, ..InvokeOptions::default() });
        assert_eq!(dry.status, Status::Ok, "{id} dry run: {dry:?}");
        assert_eq!(rev(&h), rev0, "{id} dry run");
        assert_eq!(state(&h), before, "{id} dry run");
        assert_eq!(ok(&h, "app.state.get", json!({})), app, "{id} dry run");
        // The real call: one step, the one `anim.changed` the spec asks for (05 §19.4).
        let rx = h.subscribe();
        ok(&h, id, params.clone());
        let after = ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap();
        assert_eq!(after, steps + 1, "{id} {params}: exactly one undo step");
        let changed = state(&h);
        assert_ne!(changed, before, "{id} {params} changed something");
        if id != "anim.camera.create" {
            assert_eq!(rx.try_iter().filter(|e| e.name == "anim.changed").count(), 1, "{id} {params}");
        }
        ok(&h, "edit.undo", json!({}));
        assert_eq!(state(&h), before, "{id} {params}: undo restores");
        ok(&h, "edit.redo", json!({}));
        assert_eq!(state(&h), changed, "{id} {params}: redo repeats");
        ok(&h, "edit.undo", json!({}));
    }
}

#[test]
fn atomic_batches_roll_everything_back_and_share_one_step_and_one_event() {
    let _t = turn();
    let h = fixture();
    let before = state(&h);
    let rev0 = rev(&h);
    let steps: Vec<Value> = actions()
        .into_iter()
        .filter(|(id, _)| *id == "anim.transform.set" || *id == "anim.function.key.add")
        .map(|(id, params)| json!({ "id": id, "params": params }))
        .collect();
    let mut failing = steps.clone();
    failing.push(json!({ "id": "anim.function.key.add", "params": { "nodeId": BOB, "channel": "skew", "frame": 3, "value": 200 } }));
    let rx = h.subscribe();
    let env = batch(&h, json!({ "label": "pose", "atomic": true, "steps": failing }));
    assert_ne!(env.status, Status::Ok);
    assert_eq!(rev(&h), rev0);
    assert_eq!(state(&h), before);
    assert!(!rx.try_iter().any(|e| e.name == "anim.changed"), "a refused batch announces nothing");
    let rx = h.subscribe();
    let env = batch(&h, json!({ "label": "pose", "atomic": true, "steps": steps }));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "anim.changed").collect();
    assert_eq!(events.len(), 1, "one event per undo step: {events:?}");
    let nodes = events[0].payload["nodeIds"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "the union of nodes");
    let channels = events[0].payload["channels"].as_array().unwrap();
    assert!(
        channels.iter().any(|c| c == "rotation") && channels.iter().any(|c| c == "position"),
        "the union of channels"
    );
    ok(&h, "edit.undo", json!({}));
    assert_eq!(state(&h), before, "one undo step");
    // A later step reads what an earlier step staged: a key, then its value through evaluate-independent paths.
    let env = batch(
        &h,
        json!({ "label": "two", "atomic": true, "steps": [
            { "id": "anim.function.key.add", "params": { "nodeId": BOB, "channel": "skew", "frame": 3, "value": 2 } },
            { "id": "anim.function.key.set", "params": { "nodeId": BOB, "channel": "skew", "frame": 3, "value": 4 } } ] }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let v = ok(&h, "anim.function.get", json!({ "nodeId": BOB, "channel": "skew" }));
    assert_eq!(v["keys"][0]["value"], 4.0);
}

#[test]
fn stale_guards_and_guarded_replay_apply() {
    let _t = turn();
    let h = fixture();
    let opts = InvokeOptions { request_id: Some("c32-replay".into()), ..InvokeOptions::default() };
    let p = json!({ "nodeIds": [BOB], "frame": 3, "rotation": 1, "relative": true });
    let first = with(&h, "anim.transform.set", p.clone(), opts.clone());
    assert!(first.is_ok());
    let r = rev(&h);
    let again = with(&h, "anim.transform.set", p, opts);
    assert_eq!(again.result, first.result);
    assert_eq!(rev(&h), r, "applied once");
    let el = get(&h, BOB);
    let stale = InvokeOptions {
        expect_rev: [(el["id"].as_str().unwrap().to_owned(), el["rev"].as_u64().unwrap() - 1)].into_iter().collect(),
        ..InvokeOptions::default()
    };
    let env = with(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "frame": 4, "rotation": 2 }), stale);
    assert_eq!(env.status, Status::Error);
    assert_eq!(rev(&h), r);
}

#[test]
fn omitted_targets_use_the_selection_then_the_current_element_and_the_current_frame() {
    let _t = turn();
    let h = fixture();
    ok(&h, "element.select", json!({ "elementIds": [PEG] }));
    ok(&h, "app.frame.set", json!({ "frame": 7 }));
    let r = ok(&h, "anim.transform.set", json!({ "rotation": 33 }));
    assert_eq!(r["nodeIds"][0], get(&h, PEG)["id"]);
    assert_eq!(r["keyedChannels"], json!(["rotation"]));
    let e = ok(&h, "anim.evaluate", json!({ "nodeIds": [PEG], "include": ["animated"] }));
    assert_eq!(e["frames"][0]["frame"], 7.0);
    assert_eq!(e["frames"][0]["nodes"][0]["animated"]["rotation"], 33.0);
    let list = ok(&h, "anim.key.list", json!({ "nodeId": PEG }));
    let keyed = list["keys"].as_array().unwrap().iter().find(|k| k["frame"] == 7.0).unwrap().clone();
    assert!(keyed["channels"].as_array().unwrap().iter().any(|c| c == "rotation"), "{keyed}");
    // Queries and edits leave the selection and the current frame alone.
    let app = ok(&h, "app.state.get", json!({}));
    ok(&h, "anim.key.add", json!({ "frame": 9 }));
    ok(&h, "anim.evaluate", json!({ "nodeIds": [BOB] }));
    let after = ok(&h, "app.state.get", json!({}));
    assert_eq!(after["selection"], app["selection"]);
    assert_eq!(after["frame"], app["frame"]);
    // Collapsed hierarchies: descendants are keyed where their range holds the frame, and reported.
    let added = ok(&h, "anim.key.add", json!({ "nodeIds": [PEG], "frame": 3, "includeDescendants": true }));
    assert!(added["keys"].as_array().unwrap().len() >= 2, "{added}");
    let list = ok(&h, "anim.key.list", json!({ "nodeId": PEG, "includeDescendants": true }));
    assert!(list["keys"].as_array().unwrap().iter().any(|k| k["descendantKeyed"] == true), "{list}");
    // A full pose stores the current evaluated values; existing keys keep theirs (05 §10.1).
    let at = ok(&h, "anim.key.add", json!({ "nodeIds": [BOB], "frame": 6, "channels": ["rotation", "scale"] }));
    assert_eq!(at["keys"][0]["channels"], json!(["rotation", "scale.x", "scale.y"]), "`scale` names both axes");
    let again = ok(&h, "anim.key.add", json!({ "nodeIds": [BOB], "frame": 6, "channels": ["rotation"] }));
    assert_eq!(again["keys"], json!([]));
    // Bounds of the segment around a frame, across channels.
    let b = ok(
        &h,
        "anim.key.add",
        json!({ "nodeIds": [BOB], "frame": 8, "channels": ["skew"], "atSegmentBounds": "both" }),
    );
    assert_eq!(b["keys"].as_array().unwrap().len(), 2, "{b}");
    // Removal keeps velocity and pivot keys unless they are named.
    ok(&h, "anim.function.key.add", json!({ "nodeId": PEG, "channel": "pivot.x", "frame": 20, "value": 1 }));
    ok(&h, "anim.function.key.add", json!({ "nodeId": PEG, "channel": "rotation", "frame": 20, "value": 1 }));
    let r = ok(&h, "anim.key.remove", json!({ "nodeIds": [PEG], "frame": 20 }));
    assert_eq!(r["removed"], 1);
    let pivots = ok(&h, "anim.function.get", json!({ "nodeId": PEG, "channel": "pivot.x" }));
    assert!(pivots["keys"].as_array().unwrap().iter().any(|k| k["frame"] == 20.0), "{pivots}");
}

#[test]
fn a_running_evaluation_is_cancelled_by_its_callers_budget_and_frees_the_worker() {
    let _t = turn();
    let budget = Duration::from_millis(400);
    let h = fixture_with(ExecutorConfig { budget });
    let rev0 = rev(&h);
    let frames: Vec<f64> = (0..5_000).map(|i| 1.0 + f64::from(i) * 0.25).collect();
    tt_vector::work::hooks::set_spin_micros(2_000);
    let t = Instant::now();
    let e = run(&h, "anim.evaluate", json!({ "nodeIds": [BOB], "frames": frames }));
    let answered = t.elapsed();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(e.error.as_ref().map(|x| x.code.as_str()), Some("command.timeout"), "{e:?}");
    assert!(answered < budget * 3, "{answered:?}");
    let read = run(&h, "doc.state.get", json!({}));
    assert!(read.is_ok(), "the worker is free: {read:?}");
    assert_eq!(rev(&h), rev0);
    // An edit cancelled while running commits nothing, late or otherwise.
    let start = state(&h);
    tt_vector::work::hooks::set_spin_micros(300_000);
    let e = run(&h, "anim.transform.set", json!({ "nodeIds": [BOB, PEG, CAMERA], "frame": 1, "rotation": 1 }));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(e.error.as_ref().map(|x| x.code.as_str()), Some("command.timeout"), "{e:?}");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(rev(&h), rev0, "no late commit");
    assert_eq!(state(&h), start);
    // And a queued call keeps its own deadline.
    tt_vector::work::hooks::set_spin_micros(2_000);
    let a = h.clone();
    let frames: Vec<f64> = (0..5_000).map(|i| 1.0 + f64::from(i) * 0.25).collect();
    let running = std::thread::spawn(move || run(&a, "anim.evaluate", json!({ "nodeIds": [BOB], "frames": frames })));
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    let queued = run(&h, "anim.camera.list", json!({}));
    let waited = t.elapsed();
    let running = running.join().unwrap();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(running.error.as_ref().map(|x| x.code.as_str()), Some("command.timeout"));
    assert!(waited < budget * 2, "{waited:?}");
    if queued.status != Status::Ok {
        assert_eq!(queued.error.as_ref().map(|x| x.code.as_str()), Some("command.timeout"), "{queued:?}");
    }
}

#[test]
fn oversized_requests_are_refused_before_the_watchdog() {
    let _t = turn();
    let h = fixture();
    let rev0 = rev(&h);
    // Frames × nodes over the result limit.
    let frames: Vec<f64> = (0..100_000).map(|i| 1.0 + f64::from(i)).collect();
    let nodes: Vec<&str> = vec![BOB; 11];
    let t = Instant::now();
    let e = err(&h, "anim.evaluate", json!({ "nodeIds": nodes, "frames": frames }));
    assert_eq!(e.code, "limit.exceeded", "{e:?}");
    assert!(t.elapsed() < Duration::from_secs(20));
    let e = err(&h, "anim.path.sample", json!({ "nodeId": BOB, "from": 1, "to": 100_000, "step": 0.001 }));
    assert_eq!(e.code, "limit.exceeded");
    let e = err(
        &h,
        "anim.function.sample",
        json!({ "nodeId": BOB, "channel": "rotation", "from": 1, "to": 100_000, "step": 0.001 }),
    );
    assert_eq!(e.code, "limit.exceeded");
    assert_eq!(rev(&h), rev0);
}

#[test]
fn channels_survive_save_and_open_with_their_ids() {
    let _t = turn();
    let h = fixture();
    ok(
        &h,
        "anim.function.set",
        json!({ "nodeId": BOB, "channel": "rotation", "curveType": "spline", "keys": [
        { "frame": 1, "value": 0, "right": { "dt": 3, "dv": 1 }, "handleMode": "broken" }, { "frame": 12, "value": 9, "constantSegment": true }, { "frame": 20, "value": 3 } ] }),
    );
    ok(&h, "anim.path.point.add", json!({ "nodeId": BOB, "kind": "control", "frame": 15.5 }));
    ok(&h, "anim.function.key.add", json!({ "nodeId": BOB, "channel": "velocity", "frame": 18, "value": 2 }));
    ok(&h, "anim.loop.set", json!({ "nodeId": PEG, "loops": 2, "loopType": "fill" }));
    ok(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fovMode": "dynamic", "fov": 50, "frame": 1 }));
    let before = state(&h);
    let frames: Vec<f64> = (0..48).map(|i| 1.0 + f64::from(i) * 0.5).collect();
    let values = ok(&h, "anim.evaluate", json!({ "nodeIds": [BOB, PEG, CAMERA], "frames": frames }));
    let dir = tmp();
    let out = dir.join("channels.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": out }));
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    job_ok(&h, "project.open", json!({ "path": out }));
    assert_eq!(state(&h), before);
    assert_eq!(ok(&h, "anim.evaluate", json!({ "nodeIds": [BOB, PEG, CAMERA], "frames": frames })), values);
}

#[test]
fn queries_answer_their_shapes() {
    let _t = turn();
    let h = fixture();
    let list = ok(&h, "anim.function.list", json!({ "nodeId": BOB }));
    let position = list["channels"].as_array().unwrap().iter().find(|c| c["channel"] == "position").unwrap().clone();
    assert_eq!(position["keyCount"], 2);
    assert_eq!(position["curveType"], Value::Null);
    let rotation = list["channels"].as_array().unwrap().iter().find(|c| c["channel"] == "rotation").unwrap().clone();
    assert_eq!(rotation["keyCount"], 3);
    assert_eq!(rotation["allowedCurveTypes"].as_array().unwrap().len(), 3, "velocityBased with a path");
    let f = ok(&h, "anim.function.get", json!({ "nodeId": BOB, "channel": "velocity" }));
    assert_eq!(f["pins"].as_array().unwrap().len(), 2);
    assert_eq!(f["keys"].as_array().unwrap().len(), 1);
    assert_eq!(ok(&h, "anim.path.get", json!({ "nodeId": PEG }))["points"], json!([]));
    assert_eq!(
        err(&h, "anim.path.get", json!({ "nodeId": "scene:\"Intro\"/element:\"Dialogue\"" })).code,
        "anim.notTransformable"
    );
    assert_eq!(err(&h, "anim.path.point.set", json!({ "pointIds": ["no/such"], "x": 1 })).code, "anim.notFound");
    let k = ok(&h, "anim.key.list", json!({ "nodeId": BOB, "from": 2, "to": 12 }));
    let frames: Vec<f64> = k["keys"].as_array().unwrap().iter().map(|x| x["frame"].as_f64().unwrap()).collect();
    assert_eq!(frames, vec![12.0]);
    assert_eq!(
        err(&h, "anim.function.key.add", json!({ "nodeId": PEG, "channel": "velocity", "frame": 3 })).code,
        "anim.channelUnsupported"
    );
}
