//! M3 C3.2 round 1 (builder): the default camera (C32-R1-03), RFC 6902 numeric equality of `test` (C32-R1-04, the
//! common patch kernel of M1), the frame scope of a coalesced `anim.changed` (C32-R1-05) and typed refusals instead of
//! encoding failures for evaluator results (C32-R1-06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::Status;
use tt_model::defaults::{DEFAULT_FOV, DEFAULT_FOV_DISPLAY};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";

fn peg(h: &tt_executor::ExecutorHandle, name: &str) -> String {
    ok(h, "element.create", json!({ "type": "peg", "name": name }))["elementIds"][0].as_str().unwrap().to_owned()
}

fn width(camera: &Value) -> f64 {
    let c = camera["frameCornersAtZ0"].as_array().unwrap();
    c[1]["x"].as_f64().unwrap() - c[0]["x"].as_f64().unwrap()
}

// ----- C32-R1-03 ----------------------------------------------------------------------------------------------------

#[test]
fn a_new_camera_has_the_exact_default_and_the_tables_value_stays_as_set() {
    let h = fixture();
    // Default: the exact 2·atan(0.375), stored and read back at full precision; the frame is 12 fields wide.
    let id = ok(&h, "anim.camera.create", json!({ "name": "Default" }))["cameraId"].as_str().unwrap().to_owned();
    let cam = ok(&h, "anim.camera.get", json!({ "cameraId": id, "frame": 1 }));
    assert_eq!(cam["fov"].as_f64().unwrap().to_bits(), DEFAULT_FOV.to_bits(), "{cam}");
    assert_eq!(cam["effectiveFov"].as_f64().unwrap().to_bits(), DEFAULT_FOV.to_bits());
    assert!((width(&cam) - 12.0).abs() < 1e-12, "{}", width(&cam));
    assert_eq!(get(&h, &id)["camera"]["fov"].as_f64().unwrap().to_bits(), DEFAULT_FOV.to_bits());
    // An agent that sets the tables' 41.1121 gets 41.1121 back and projects with it (SI-104, ruled "no alias"):
    // 2·16·tan(41.1121°/2) ≈ 12.0000031 fields, not the default's 12.
    let set = ok(&h, "anim.camera.create", json!({ "name": "Table", "fov": DEFAULT_FOV_DISPLAY }))["cameraId"]
        .as_str()
        .unwrap()
        .to_owned();
    let cam = ok(&h, "anim.camera.get", json!({ "cameraId": set, "frame": 1 }));
    assert_eq!(cam["fov"], json!(41.1121));
    assert_eq!(cam["effectiveFov"], json!(41.1121));
    let want = 32.0 * tt_anim::num::tan_deg(DEFAULT_FOV_DISPLAY / 2.0);
    assert!((width(&cam) - want).abs() < 1e-12 && (width(&cam) - 12.000_003_045).abs() < 1e-8, "{}", width(&cam));
    // A new project's camera has the exact default too (M1's defaults).
    let fixture_camera = get(&h, "scene:\"Intro\"/element:\"Camera\"");
    assert_eq!(fixture_camera["camera"]["fov"].as_f64().unwrap().to_bits(), DEFAULT_FOV.to_bits());
}

// ----- C32-R1-04 ----------------------------------------------------------------------------------------------------

#[test]
fn rfc_test_compares_numbers_by_value_exactly_and_recursively() {
    let h = fixture();
    ok(&h, "anim.transform.set", json!({ "nodeIds": [PEG], "target": "static", "skew": 0 }));
    // The reviewer's case: integer and floating zero are the same number (RFC 6902 §4.6).
    for zero in [json!(0), json!(0.0), json!(-0.0)] {
        let r = rev(&h);
        ok(
            &h,
            "doc.object.patch",
            json!({ "targets": [PEG], "ops": [{ "op": "test", "path": "/transform/skew", "value": zero }] }),
        );
        assert_eq!(rev(&h), r, "a passing test changes nothing");
    }
    // Spellings of one value; integers beyond 2^53 are compared exactly, never through a double.
    let meta = json!({ "n": 9_007_199_254_740_993_u64, "list": [100, { "x": 0.5 }], "neg": -3 });
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "ops": [{ "op": "add", "path": "/meta", "value": meta }] }));
    let passes = [
        ("/meta/list/0", json!(1e2)),
        ("/meta/list/0", json!(100.0)),
        ("/meta/neg", json!(-3.0)),
        ("/meta/n", json!(9_007_199_254_740_993_u64)),
        // The same object with members in another order and numbers spelled as floats.
        ("/meta", json!({ "neg": -3.0, "list": [1e2, { "x": 0.5 }], "n": 9_007_199_254_740_993_u64 })),
    ];
    for (path, value) in passes {
        let env = run(
            &h,
            "doc.object.patch",
            json!({ "targets": [BOB], "ops": [{ "op": "test", "path": path, "value": value }] }),
        );
        assert_eq!(env.status, Status::Ok, "{path} {value}: {env:?}");
    }
    let fails = [
        ("/meta/n", json!(9_007_199_254_740_992_u64)),
        // 9007199254740993.0 is the double 9007199254740992: not the stored integer.
        ("/meta/n", json!(9_007_199_254_740_993_f64)),
        ("/meta/list/0", json!(100.5)),
        ("/meta/list/0", json!("100")),
        ("/meta/list", json!([100])),
        ("/meta/list", json!([100, { "x": 0.5, "y": 1 }])),
        ("/meta/neg", json!(null)),
    ];
    for (path, value) in fails {
        let r = rev(&h);
        let before = get(&h, BOB);
        // A failing test after a write rolls the write back.
        let e = err(
            &h,
            "doc.object.patch",
            json!({ "targets": [BOB], "ops": [
                { "op": "replace", "path": "/note", "value": "changed" },
                { "op": "test", "path": path, "value": value } ] }),
        );
        assert_eq!(e.code, "doc.patchTestFailed", "{path} {value}");
        assert_eq!((rev(&h), get(&h, BOB)), (r, before), "{path}: rolled back");
    }
}

#[test]
fn json_equal_is_rfc_numeric_equality() {
    use tt_model::change::json_equal;
    assert!(json_equal(&json!(0), &json!(0.0)));
    assert!(json_equal(&json!(u64::MAX), &json!(u64::MAX)));
    assert!(!json_equal(&json!(u64::MAX), &json!(u64::MAX - 1)));
    // u64::MAX as a double is 2^64, which no JSON integer equals.
    assert!(!json_equal(&json!(u64::MAX), &json!(18_446_744_073_709_551_615_f64)));
    assert!(json_equal(&json!(i64::MIN), &json!(-9_223_372_036_854_775_808_f64)));
    assert!(json_equal(&json!(-1), &json!(-1.0)) && !json_equal(&json!(-1), &json!(1)));
    assert!(!json_equal(&json!(1), &json!(true)) && !json_equal(&json!(0), &json!(null)));
    assert!(json_equal(&json!({ "a": [1, 2.0] }), &json!({ "a": [1.0, 2] })));
    assert!(!json_equal(&json!({ "a": 1 }), &json!({ "a": 1, "b": 1 })));
    assert!(!json_equal(&json!([1, 2]), &json!([2, 1])));
}

// ----- C32-R1-05 ----------------------------------------------------------------------------------------------------

fn changed_events(h: &tt_executor::ExecutorHandle, steps: Vec<Value>) -> Vec<Value> {
    let rx = h.subscribe();
    let env = batch(h, json!({ "label": "mixed", "atomic": true, "steps": steps }));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    rx.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload.clone()).collect()
}

fn channels(ev: &Value) -> Vec<&str> {
    let mut c: Vec<&str> = ev["channels"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    c.sort_unstable();
    c
}

fn frames(ev: &Value) -> Option<(f64, f64)> {
    ev.get("frames").map(|f| (f["from"].as_f64().unwrap(), f["to"].as_f64().unwrap()))
}

#[test]
fn an_all_frame_change_absorbs_a_keyed_range_in_either_order() {
    for reverse in [false, true] {
        let h = fixture();
        let mut steps = vec![
            json!({ "id": "anim.transform.set", "params": { "nodeIds": [PEG], "target": "static", "rotation": 13 } }),
            json!({ "id": "anim.transform.set", "params": { "nodeIds": [PEG], "frame": 7, "skew": 17 } }),
        ];
        if reverse {
            steps.reverse();
        }
        let events = changed_events(&h, steps);
        assert_eq!(events.len(), 1, "{events:?}");
        assert!(channels(&events[0]).contains(&"skew") && channels(&events[0]).len() == 2, "{events:?}");
        assert_eq!(frames(&events[0]), None, "reverse {reverse}: the static change covers every frame: {events:?}");
    }
}

#[test]
fn keyed_changes_report_their_interpolation_intervals_and_merge_them() {
    let h = fixture();
    // Bob's rotation keys: 1, 12, 24 (fixture), then 30 and 36.
    for (frame, value) in [(30, 5.0), (36, 9.0)] {
        ok(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "frame": frame, "rotation": value }));
    }
    // One key inside: its two neighbouring keys bound the change.
    let one = changed_events(
        &h,
        vec![json!({ "id": "anim.transform.set", "params": { "nodeIds": [BOB], "frame": 18, "rotation": 3 } })],
    );
    assert_eq!(one.len(), 1);
    assert_eq!(frames(&one[0]), Some((12.0, 24.0)), "{one:?}");
    ok(&h, "edit.undo", json!({}));
    // Two keyed edits in one step, both orders: the range spanning both intervals.
    for reverse in [false, true] {
        let mut steps = vec![
            json!({ "id": "anim.transform.set", "params": { "nodeIds": [BOB], "frame": 18, "rotation": 3 } }),
            json!({ "id": "anim.transform.set", "params": { "nodeIds": [BOB], "frame": 33, "rotation": 7 } }),
        ];
        if reverse {
            steps.reverse();
        }
        let events = changed_events(&h, steps);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(frames(&events[0]), Some((12.0, 36.0)), "reverse {reverse}: {events:?}");
        ok(&h, "edit.undo", json!({}));
    }
    // A key after the last one changes the hold to the end, and the first key of a channel changes every frame.
    let after = changed_events(
        &h,
        vec![json!({ "id": "anim.transform.set", "params": { "nodeIds": [BOB], "frame": 40, "rotation": 1 } })],
    );
    assert_eq!(frames(&after[0]), None, "{after:?}");
    let first = changed_events(
        &h,
        vec![json!({ "id": "anim.transform.set", "params": { "nodeIds": [PEG], "frame": 20, "rotation": 1 } })],
    );
    assert_eq!(frames(&first[0]), None, "{first:?}");
}

#[test]
fn separate_transactions_keep_separate_events() {
    let h = fixture();
    let rx = h.subscribe();
    ok(&h, "anim.transform.set", json!({ "nodeIds": [PEG], "target": "static", "rotation": 13 }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "frame": 18, "rotation": 3 }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [PEG], "frame": 7, "skew": 17 }));
    let events: Vec<Value> = rx.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload.clone()).collect();
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(frames(&events[0]), None);
    assert_eq!(frames(&events[1]), Some((12.0, 24.0)));
    assert_eq!(channels(&events[2]), ["skew"]);
}

// ----- C32-R1-06 ----------------------------------------------------------------------------------------------------

fn chain(h: &tt_executor::ExecutorHandle, n: usize, statics: &Value) -> String {
    let mut parent: Option<String> = None;
    for i in 0..n {
        let node = peg(h, &format!("N{i}"));
        if let Some(p) = &parent {
            ok(h, "element.parent.set", json!({ "elementIds": [node], "parentId": p }));
        }
        let mut params = json!({ "nodeIds": [node], "target": "static" });
        params.as_object_mut().unwrap().extend(statics.as_object().unwrap().clone());
        ok(h, "anim.transform.set", params);
        parent = Some(node);
    }
    parent.unwrap()
}

#[test]
fn a_finite_world_matrix_reads_back_and_a_non_finite_one_is_a_typed_refusal() {
    let h = fixture();
    // The reviewer's case: 26 nested pegs at the per-node bound, world scale 1e156.
    let leaf = chain(&h, 26, &json!({ "scale": { "x": 1e6, "y": 1e6 } }));
    let v = ok(&h, "anim.evaluate", json!({ "nodeIds": [leaf], "frames": [1], "include": ["world"] }));
    let w = &v["frames"][0]["nodes"][0]["world"];
    for axis in ["x", "y"] {
        let s = w["scale"][axis].as_f64().unwrap();
        assert!((s / 1e156 - 1.0).abs() < 1e-12, "{axis}: {s}");
    }
    // Scale 1e6 with a 45° turn and an 89° skew grows each level by about 4e7, so the 46 levels the tree-depth limit
    // allows pass 1e308. The refusal is typed, never `internal`.
    let h = fixture();
    let leaf = chain(&h, 46, &json!({ "scale": { "x": 1e6, "y": 1e6 }, "rotation": 45, "skew": 89 }));
    let e = err(&h, "anim.evaluate", json!({ "nodeIds": [leaf], "frames": [1], "include": ["world"] }));
    assert_eq!((e.code.as_str(), e.details["reason"].as_str()), ("anim.valueOutOfRange", Some("nonFinite")), "{e:?}");
    // It names the node whose world matrix first left the finite range (an ancestor of the leaf here).
    assert!(e.details["nodeId"].as_str().is_some_and(|n| n.starts_with("el_")), "{e:?}");
}

#[test]
fn a_non_finite_number_in_any_anim_result_is_a_typed_refusal() {
    // The encoding backstop behind each query's own checks: `anim.*` results refuse with the typed error.
    let e = tt_registry::encode_command_result("anim.evaluate", &(1.0, f64::INFINITY)).unwrap_err();
    assert_eq!(e.code, "anim.valueOutOfRange");
    assert_eq!(e.details["reason"], "nonFinite");
    // Elsewhere a non-finite result stays an internal error (10 §1.1).
    let e = tt_registry::encode_command_result("doc.object.get", &(f64::NAN,)).unwrap_err();
    assert_eq!(e.code, "internal");
    assert_eq!(tt_registry::encode_command_result("anim.evaluate", &(1.5,)).unwrap(), json!([1.5]));
}
