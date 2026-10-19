//! Evidence-only fresh closure probes; staged temporarily by run_probe.py.
use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::value::FrameRange;

fn op(path: &str, value: Value, old: Value, window: Option<(u32, u32)>) -> PatchOp {
    PatchOp { op: "replace", path: path.into(), from: None, value: Some(value), old_value: Some(old),
        range: window.map(|(from, to)| FrameRange { from, to }), elided: false }
}

fn limited(before: &Value, patch: &[PatchOp], pointer: &str) {
    let bytes = serde_json::to_vec(before).unwrap();
    for result in [change::apply(before, patch), change::apply_with_inverse(before, patch).map(|x| x.0)] {
        let e = result.unwrap_err();
        assert_eq!(e.code(), "doc.limitExceeded", "{e:?}");
        assert_eq!(e.details()["pointer"], pointer);
        assert_eq!(e.details()["maxFrames"], 100_000);
        assert_eq!(serde_json::to_vec(before).unwrap(), bytes);
    }
    assert!(change::invert_against(before, patch).is_err());
}

#[test]
fn split_run_totals_at_and_over_limit() {
    let before = json!({"exposure": [["dr_fresh", 7]], "sentinel": {"unchanged": true}});
    for range in [None, Some((1, 100_000))] {
        let patch = op("/exposure", json!([["dr_other", 49_999], [null, 1], ["dr_last", 50_000]]), before["exposure"].clone(), range);
        let (after, inverse) = change::apply_with_inverse(&before, &[patch]).unwrap();
        assert_eq!(change::apply(&after, &inverse).unwrap(), before);
    }
    // A multi-run total, not one oversized count.
    limited(&before, &[op("/exposure", json!([["dr_other", 50_000], [null, 50_001]]), before["exposure"].clone(), None)], "/exposure");
    limited(&before, &[op("/exposure", json!([["dr_other", 50_000], [null, 50_001]]), before["exposure"].clone(), Some((1, 100_000)))], "/exposure");
}

#[test]
fn window_edges_and_integer_overflow_are_rejected_without_mutation() {
    let before = json!({"exposure": [["dr_fresh", 7]]});
    for frame in [100_001, u32::MAX - 1, u32::MAX] {
        limited(&before, &[op("/exposure", json!([["dr_new", 1]]), json!([[null, 1]]), Some((frame, frame)))], "/exposure");
    }
    for runs in [json!([["dr_a", u32::MAX], ["dr_b", 2]]), json!([["dr_a", u32::MAX], [null, u32::MAX]])] {
        limited(&before, &[op("/exposure", runs.clone(), before["exposure"].clone(), None)], "/exposure");
        limited(&before, &[op("/exposure", runs.clone(), json!([["dr_fresh", 1]]), Some((1, 1)))], "/exposure");
        limited(&before, &[op("/exposure", json!([["dr_b", 1]]), runs.clone(), Some((1, 1)))], "/exposure");
        limited(&json!({"exposure": runs}), &[op("/exposure", json!([["dr_b", 1]]), json!([["dr_a", 1]]), Some((1, 1)))], "/exposure");
    }
    for n in [i64::MAX as u64 - 1, i64::MAX as u64, u64::MAX] {
        let bytes = serde_json::to_vec(&before).unwrap();
        let e = change::apply(&before, &[op("/exposure", json!([["dr_a", n]]), before["exposure"].clone(), None)]).unwrap_err();
        assert_eq!(e.code(), "arg.invalid"); // Counts outside u32 are malformed, not narrowed.
        assert_eq!(e.details()["pointer"], "/exposure");
        assert_eq!(serde_json::to_vec(&before).unwrap(), bytes);
        assert!(serde_json::from_value::<FrameRange>(json!({"from": 1, "to": n})).is_err());
    }
}

#[test]
fn result_length_and_cumulative_range_batches_are_bounded() {
    let before = json!({"exposure": [["dr_a", 99_999]], "marker": 0});
    let marker = op("/marker", json!(1), json!(0), None);
    let first = op("/exposure", json!([["dr_b", 2]]), json!([["dr_a", 1]]), Some((1, 2)));
    let at = change::apply(&before, &[marker.clone(), first.clone()]).unwrap();
    assert_eq!(at["exposure"], json!([["dr_b", 2], ["dr_a", 99_998]]));
    let second = op("/exposure", json!([["dr_c", 2]]), json!([["dr_a", 1]]), Some((3, 4)));
    limited(&before, &[marker, first, second], "/exposure");
    let full = json!({"exposure": [["dr_a", 100_000]]});
    limited(&full, &[op("/exposure", json!([["dr_b", 2]]), json!([["dr_a", 1]]), Some((42, 43)))], "/exposure");
}

#[test]
fn mouth_ranges_at_over_limit_and_cumulative_batch() {
    let before = json!({"mouthTrack": {"mouths": "AB"}, "marker": 0});
    let full = op("/mouthTrack/mouths", json!("G".repeat(100_000)), json!("AB"), Some((1, 100_000)));
    let (after, inverse) = change::apply_with_inverse(&before, &[full]).unwrap();
    assert_eq!(after["mouthTrack"]["mouths"].as_str().unwrap().len(), 100_000);
    assert_eq!(change::apply(&after, &inverse).unwrap(), before);
    let edge = op("/mouthTrack/mouths", json!("A"), json!("G"), Some((100_000, 100_000)));
    assert!(change::apply(&after, &[edge]).is_ok());
    limited(&before, &[op("/mouthTrack/mouths", json!("X".repeat(100_001)), json!("AB"), Some((1, 100_001)))], "/mouthTrack/mouths");
    limited(&after, &[op("/mouthTrack/mouths", json!("XX"), json!("G"), Some((42, 43)))], "/mouthTrack/mouths");
    limited(&after, &[op("/mouthTrack/mouths", json!("A"), json!(""), Some((100_001, 100_001)))], "/mouthTrack/mouths");
    let almost = json!({"mouthTrack": {"mouths": "A".repeat(99_999)}, "marker": 0});
    let first = op("/mouthTrack/mouths", json!("BC"), json!("A"), Some((1, 2)));
    assert!(change::apply(&almost, std::slice::from_ref(&first)).is_ok());
    let second = op("/mouthTrack/mouths", json!("DE"), json!("A"), Some((3, 4)));
    limited(&almost, &[op("/marker", json!(1), json!(0), None), first, second], "/mouthTrack/mouths");
}

#[test]
fn opaque_namespaces_remain_uninterpreted() {
    let before = json!({"meta": {"exposure": [], "mouthTrack": {"mouths": ""}}, "ext": {"vendor/name": {"exposure": []}}});
    for (path, value, old) in [("/meta/exposure", json!([["opaque", u32::MAX]]), json!([])),
        ("/ext/vendor~1name/exposure", json!([["opaque", u32::MAX]]), json!([])),
        ("/meta/mouthTrack/mouths", json!("λ".repeat(100_001)), json!(""))] {
        let (after, inverse) = change::apply_with_inverse(&before, &[op(path, value, old, None)]).unwrap();
        assert_eq!(change::apply(&after, &inverse).unwrap(), before);
    }
}

#[test]
fn inspect_other_frame_bearing_write_paths() {
    let before = json!({"exposure": [["dr_a", 100_000]], "mouthTrack": {"mouths": "AB"}, "meta": {"saved": [["dr_b", 100_001]]}});
    let mut copy = op("/exposure", Value::Null, Value::Null, None);
    copy.op = "copy"; copy.from = Some("/meta/saved".into()); copy.value = None; copy.old_value = None;
    let mut moved = copy.clone(); moved.op = "move";
    let mut mouth_add = op("/mouthTrack/mouths", json!("X".repeat(100_001)), json!("AB"), None); mouth_add.op = "add";
    let mut add = op("/exposure/-", json!(["dr_b", 1]), Value::Null, None);
    add.op = "add"; add.old_value = None;
    for (name, patch) in [
        ("whole mouth string replace", op("/mouthTrack/mouths", json!("X".repeat(100_001)), json!("AB"), None)),
        ("whole mouth string add", mouth_add),
        ("nested exposure count", op("/exposure/0/1", json!(100_001), json!(100_000), None)),
        ("append exposure run", add), ("copy opaque runs into exposure", copy), ("move opaque runs into exposure", moved)] {
        match change::apply(&before, &[patch]) {
            Ok(after) => println!("OBSERVATION {name}: ACCEPTED exposure_total={} mouth_len={}", after["exposure"].as_array().unwrap().iter().map(|r| r[1].as_u64().unwrap()).sum::<u64>(), after["mouthTrack"]["mouths"].as_str().unwrap().len()),
            Err(e) => println!("OBSERVATION {name}: REJECTED {} {:?}", e.code(), e.details()),
        }
    }
}
