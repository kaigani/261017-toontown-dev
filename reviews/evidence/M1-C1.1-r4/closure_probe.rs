//! Fresh scoped closure probes; retained as evidence, not a permanent test target.
#![allow(clippy::unwrap_used, clippy::panic, reason = "review evidence assertions")]
use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::entities::element::Element;
use tt_model::validate::{Check, Issues};
use tt_model::value::FrameRange;
use tt_model::{ObjectType, fixture, object, schema};

fn replace(path: &str, value: Value, old: Value, range: Option<(u32, u32)>) -> PatchOp {
    PatchOp { op: "replace", path: path.into(), from: None, value: Some(value), old_value: Some(old),
        range: range.map(|(from, to)| FrameRange { from, to }), elided: false }
}

fn roundtrip(before: &Value, patch: &[PatchOp], expected: &Value) {
    let (after, inverse) = change::apply_with_inverse(before, patch).unwrap();
    assert_eq!(&after, expected);
    assert_eq!(inverse, change::invert_against(before, patch).unwrap());
    assert_eq!(change::apply(&after, &inverse).unwrap(), *before);
    let redo = change::invert_against(&after, &inverse).unwrap();
    assert_eq!(change::apply(before, &redo).unwrap(), after);
}

#[test]
fn opaque_text_with_and_without_range() {
    for namespace in ["meta", "ext"] {
        let before = json!({namespace: {"org.review/tool~": {"mouthTrack": {"mouths": "café 🎬"}, "exposure": "not runs", "note": "old"}}});
        for (slot, old) in [("mouthTrack/mouths", "café 🎬"), ("exposure", "not runs"), ("note", "old")] {
            let path = format!("/{namespace}/org.review~1tool~0/{slot}");
            let mut expected = before.clone();
            *expected.pointer_mut(&path).unwrap() = json!("new 👀 /~ e\u{301}");
            for ty in [ObjectType::Pen, ObjectType::SoundPlacement, ObjectType::Element] {
                let patch = change::diff_json(&before, &expected, "", ty);
                assert!(patch.iter().all(|p| p.range.is_none()));
                roundtrip(&before, &patch, &expected);
            }
            roundtrip(&before, &[replace(&path, json!("new 👀 /~ e\u{301}"), json!(old), None)], &expected);
            let bad = replace(&path, json!("X"), json!(old), Some((1, old.chars().count() as u32)));
            let error = change::apply(&before, &[bad]).unwrap_err();
            assert_eq!(error.code(), "arg.invalid");
            assert_eq!(error.details()["pointer"], path);
        }
    }
    let before = json!({"mouthTrack": {"mouths": "ABCD"}});
    roundtrip(&before, &[replace("/mouthTrack/mouths", json!("GX"), json!("BC"), Some((2, 3)))],
        &json!({"mouthTrack": {"mouths": "AGXD"}}));
    for (from, to, new, old) in [
        (0, 0, json!("X"), json!("A")),
        (3, 2, json!("X"), json!("C")),
        (2, 3, json!("X"), json!("B")),
        (5, 5, json!("X"), json!("A")),
        (2, 2, json!([]), json!("B")),
        (2, 2, json!("X"), json!(false)),
        (2, 2, json!("X"), json!("C")),
        (2, 2, json!("Z"), json!("B")),
    ] {
        let bad = replace("/mouthTrack/mouths", new, old, Some((from, to)));
        assert_eq!(change::apply(&before, &[bad]).unwrap_err().code(), "arg.invalid");
    }
}

#[test]
fn numeric_object_array_moves() {
    let cases = [
        (json!({"o":{"0":"s","1":{"x":"old"},"2":{"x":"keep"}}}), "/o/0", "/o/1/x", json!({"o":{"1":{"x":"s"},"2":{"x":"keep"}}})),
        (json!({"o":{"01":"s","1":"keep"},"a":["a","b"]}), "/o/01", "/a/1", json!({"o":{"1":"keep"},"a":["a","s","b"]})),
        (json!({"o":{"0":"old","2":"keep"},"a":["s","b"]}), "/a/0", "/o/0", json!({"o":{"0":"s","2":"keep"},"a":["b"]})),
        (json!({"o":{"0":["s","b"],"1":{"0":"old"},"2":"keep"}}), "/o/0/0", "/o/1/0", json!({"o":{"0":["b"],"1":{"0":"s"},"2":"keep"}})),
        (json!({"a":[{"0":"s"},{"0":"old"},{"0":"keep"}]}), "/a/0", "/a/1/0", json!({"a":[{"0":"old"},{"0":{"0":"s"}}]})),
        (json!({"a":["a","s","b"],"o":{"0":["keep"]}}), "/a/1", "/o/0/-", json!({"a":["a","b"],"o":{"0":["keep","s"]}})),
    ];
    for (before, from, path, expected) in cases {
        let patch = [PatchOp { op: "move", path: path.into(), from: Some(from.into()), value: None, old_value: None, range: None, elided: false }];
        roundtrip(&before, &patch, &expected);
    }
}

#[test]
fn exposure_counts_and_window_consistency() {
    let before = json!({"exposure": [["dr_review", 4]]});
    for count in [json!(0), json!(-2), json!(0.25), json!(1.5)] {
        for side in ["value", "old", "target"] {
            let malformed = json!([["dr_review", count]]);
            let mut current = before.clone();
            let mut patch = replace("/exposure", json!([["dr_new", 1]]), json!([["dr_review", 1]]), Some((2, 2)));
            match side { "value" => patch.value = Some(malformed), "old" => patch.old_value = Some(malformed), _ => current["exposure"] = malformed }
            let snapshot = current.clone();
            let error = change::apply(&current, &[patch]).unwrap_err();
            assert_eq!(error.code(), "arg.invalid", "{side}: {count}");
            assert_eq!(error.details()["pointer"], "/exposure");
            assert_eq!(current, snapshot);
        }
    }
    for patch in [
        replace("/exposure", json!([["dr_new", 5]]), json!([["dr_review", 1]]), Some((2, 2))),
        replace("/exposure", json!([["dr_new", 1]]), json!([["dr_review", 2]]), Some((2, 2))),
    ] { assert_eq!(change::apply(&before, &[patch]).unwrap_err().code(), "arg.invalid"); }
}

#[test]
fn empty_exposure_lists_are_canonical_and_invertible() {
    let before = json!({"exposure": []});
    roundtrip(&before, &[replace("/exposure", json!([["dr_new", 2]]), json!([[null, 2]]), Some((1, 2)))],
        &json!({"exposure": [["dr_new", 2]]}));
    roundtrip(&before, &[replace("/exposure", json!([]), json!([]), Some((1, 1)))], &before);
    let before = json!({"exposure": [["dr_review", 4]]});
    roundtrip(&before, &[replace("/exposure", json!([]), json!([["dr_review", 4]]), None)], &json!({"exposure": []}));
    roundtrip(&before, &[replace("/exposure", json!([]), json!([["dr_review", 1]]), Some((2, 2)))],
        &json!({"exposure": [["dr_review", 3]]}));
}

#[test]
fn model_integrity_rejects_invalid_counts_and_excessive_totals() {
    let parts = fixture::sample_parts(202610180419).unwrap();
    let element = parts.elements.iter().find(|e| e.exposure().as_ref().is_some_and(|r| !r.is_empty())).unwrap();
    let base = object::to_json(element).unwrap();
    for count in [json!(0), json!(-1), json!(0.5), json!(100001)] {
        let mut v = base.clone();
        v["exposure"][0][1] = count.clone();
        assert!(object::from_json::<Element>(&v).is_err(), "{count}");
        if let Ok(typed) = serde_json::from_value::<Element>(v) {
            let mut issues = Issues::new(100);
            typed.check("", &mut issues);
            let errors = issues.into_errors();
            let rule = if count == 0 { "runCount" } else { "limit" };
            assert!(errors.iter().any(|e| e.details()["rule"] == rule), "{errors:?}");
        }
    }
    let mut empty = base;
    empty["exposure"] = json!([]);
    assert!(object::from_json::<Element>(&empty).is_ok());
}

#[test]
fn inspect_patch_scene_limit() {
    // Diagnostic observation first; an accepted result is promoted to a permanent regression only after review.
    let before = json!({"exposure": [["dr_review", 4]]});
    for (from, to, value, old) in [
        (1, 100001, json!([["dr_new", 100001]]), json!([["dr_review", 4]])),
        (100001, 100001, json!([["dr_new", 1]]), json!([[null, 1]])),
    ] {
        let patch = replace("/exposure", value, old, Some((from, to)));
        match change::apply(&before, &[patch]) {
            Ok(value) => println!("scene-limit range {from}..{to} ACCEPTED: {value}"),
            Err(error) => println!("scene-limit range {from}..{to} REJECTED: {error:?}"),
        }
    }
}

#[test]
fn property_discovery_and_anonymous_annotations() {
    for (path, values) in [("/tip", vec![json!("flat"), json!("square"), json!("oval"), json!("round")]),
        ("/colourId", vec![Value::Null, json!("col_review")])] {
        let mut property = schema::property_schema(ObjectType::Pen, path).unwrap();
        property["$defs"] = json!(schema::definitions());
        let validator = jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&property).unwrap();
        for value in values { assert!(validator.is_valid(&value), "{path}: {value}"); }
        assert!(!validator.is_valid(&json!(42)));
    }
    fn walk(value: &Value, path: &str, count: &mut usize) {
        let Some(map) = value.as_object() else { return };
        if let Some(properties) = map.get("properties").and_then(Value::as_object) {
            for (name, property) in properties {
                let at = format!("{path}/properties/{name}");
                assert_eq!(property["x-since"], "1.0", "{at}");
                *count += 1;
                walk(property, &at, count);
            }
        }
        for key in ["items", "additionalProperties"] { if let Some(v) = map.get(key) { walk(v, &format!("{path}/{key}"), count); } }
        for key in ["prefixItems", "anyOf", "oneOf", "allOf"] {
            if let Some(values) = map.get(key).and_then(Value::as_array) {
                for (i, v) in values.iter().enumerate() { walk(v, &format!("{path}/{key}/{i}"), count); }
            }
        }
        if let Some(values) = map.get("patternProperties").and_then(Value::as_object) {
            for (key, v) in values { walk(v, &format!("{path}/patternProperties/{key}"), count); }
        }
    }
    let mut count = 0;
    for (name, value) in schema::definitions() { walk(value, &format!("/$defs/{name}"), &mut count); }
    println!("All {count} recursively discovered declarations carry x-since=1.0");
}
