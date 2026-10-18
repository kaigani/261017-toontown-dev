//! Independent final-round C1.1 closure checks and permanent regressions.
#![allow(clippy::unwrap_used, clippy::panic, reason = "review test assertions and fixtures")]

use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, RenderOptions};
use tt_model::entities::{element::Element, pen::Pen, sound::SoundPlacement};
use tt_model::value::FrameRange;
use tt_model::{ObjectType, fixture, object, schema};

fn op(kind: &'static str, path: &str, from: Option<&str>, value: Option<Value>, old: Option<Value>) -> PatchOp {
    PatchOp {
        op: kind,
        path: path.into(),
        from: from.map(str::to_owned),
        value,
        old_value: old,
        range: None,
        elided: false,
    }
}

fn roundtrip(before: &Value, patch: &[PatchOp]) {
    let (after, inverse) = change::apply_with_inverse(before, patch).unwrap();
    assert_eq!(after, change::apply(before, patch).unwrap());
    assert_eq!(inverse, change::invert_against(before, patch).unwrap());
    assert_eq!(change::apply(&after, &inverse).unwrap(), *before, "{patch:?}; {inverse:?}");
    let redo = change::invert_against(&after, &inverse).unwrap();
    assert_eq!(change::apply(before, &redo).unwrap(), after);
}

fn raw(doc: &Document, ty: ObjectType) -> Value {
    for entry in doc.entry_keys() {
        for obj in change::own_objects(doc, &entry).unwrap() {
            if obj.ty == ty {
                return doc.object_json(&obj.key, RenderOptions { depth: None, path: false }).unwrap();
            }
        }
    }
    panic!("fixture lacks {ty}");
}

#[test]
fn append_move_remove_sequences_restore_nested_arrays() {
    for count in 1..24 {
        let before = json!({"a": (0..count).map(|x| json!({"x": [x, x + 1]})).collect::<Vec<_>>()});
        let patch = [
            op("add", "/a/-", None, Some(json!({"x": [71, 72]})), None),
            op("move", "/a/0/x/-", Some(&format!("/a/{count}/x/0")), None, None),
            op("remove", &format!("/a/{count}"), None, None, None),
            op("copy", "/a/-", Some("/a/0"), None, None),
        ];
        roundtrip(&before, &patch);
        let inverse = change::invert_against(&before, &patch).unwrap();
        assert!(inverse.iter().all(|p| !p.path.ends_with("/-")));
    }
}

#[test]
fn moving_into_own_subtree_fails_atomically_and_copying_parent_succeeds() {
    let before = json!({"a/b": {"~": {"leaf": [1, 2]}}});
    for to in ["/a~1b/~/child", "/a~1b/~0/leaf/0"] {
        let bad = op("move", to, Some("/a~1b"), None, None);
        assert!(change::apply(&before, &[bad]).is_err());
    }
    roundtrip(&before, &[op("copy", "/a~1b/~0/child", Some("/a~1b"), None, None)]);
    roundtrip(&before, &[op("copy", "/a~1b/~0/leaf/0", Some(""), None, None)]);
    assert_eq!(before, json!({"a/b": {"~": {"leaf": [1, 2]}}}));
}

#[test]
fn malformed_mouth_ranges_return_typed_errors() {
    let before = json!({"mouths": "ABCDEFGX"});
    for (from, to, new, old) in [
        (0, 0, json!("X"), json!("A")),
        (3, 2, json!("X"), json!("C")),
        (3, 4, json!("X"), json!("C")),
        (9, 9, json!("X"), json!("A")),
        (8, 9, json!("X"), json!("XA")),
        (2_147_483_648, 2_147_483_648, json!("X"), json!("A")),
        (2, 2, json!([]), json!("B")),
        (2, 2, json!("X"), json!(false)),
        (2, 2, json!("X"), json!("C")),
        (2, 2, json!("Z"), json!("B")),
    ] {
        let p =
            PatchOp { range: Some(FrameRange { from, to }), ..op("replace", "/mouths", None, Some(new), Some(old)) };
        let error = change::apply(&before, &[p]).unwrap_err();
        assert_eq!(error.code(), "arg.invalid");
    }
    for r in [
        json!({"from": -1, "to": 1}),
        json!({"from": 1.5, "to": 2}),
        json!({"from": 1, "to": -2}),
        json!({"from": 1, "to": 2.5}),
    ] {
        assert!(serde_json::from_value::<FrameRange>(r).is_err());
    }
}

#[test]
fn zero_length_exposure_runs_are_invalid_range_payloads() {
    let before = json!({"exposure": [["dr_a", 3]]});
    for new in [json!([["dr_b", 0]]), json!([["dr_b", 1], ["dr_c", 0]])] {
        let p = PatchOp {
            range: Some(FrameRange { from: 2, to: 2 }),
            ..op("replace", "/exposure", None, Some(new), Some(json!([["dr_a", 1]])))
        };
        let result = change::apply(&before, &[p]);
        assert!(result.is_err(), "zero counts are not exposure runs (10 section 4.7): {result:?}");
    }
}

#[test]
fn opaque_mouth_track_lookalikes_are_ordinary_json_changes() {
    let doc = fixture::sample_document(202610180311).unwrap();
    let mut failures = Vec::new();
    for field in ["meta", "ext"] {
        let mut before = raw(&doc, ObjectType::Pen);
        before[field] = json!({"com.example.tool": {"mouthTrack": {"mouths": "hello"}}});
        let mut after = before.clone();
        after[field]["com.example.tool"]["mouthTrack"]["mouths"] = json!("world");
        assert!(object::from_json::<Pen>(&before).is_ok());
        assert!(object::from_json::<Pen>(&after).is_ok());
        let patch = change::diff_json(&before, &after, "", ObjectType::Pen);
        match change::apply(&before, &patch) {
            Ok(actual) => {
                assert_eq!(actual, after);
                roundtrip(&before, &patch);
            }
            Err(e) => failures.push((field, e)),
        }
    }
    assert!(failures.is_empty(), "valid opaque changes failed: {failures:?}");
}

#[test]
fn resolved_stateless_move_inverse_does_not_shift_numeric_object_keys() {
    let before = json!({"a": {"0": "source", "1": {"x": "old"}, "2": {"x": "untouched"}}});
    let patch = [op("move", "/a/1/x", Some("/a/0"), None, Some(json!("old")))];
    roundtrip(&before, &patch);
    let after = change::apply(&before, &patch).unwrap();
    let inverse = change::invert(&patch);
    assert_eq!(
        change::apply(&after, &inverse).unwrap(),
        before,
        "resolved paths and oldValue meet invert's documented contract: {inverse:?}"
    );
}

fn property_validator(ty: ObjectType, path: &str) -> jsonschema::Validator {
    let mut s = schema::property_schema(ty, path).unwrap();
    s["$defs"] = json!(schema::definitions());
    jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&s).unwrap()
}

#[test]
fn property_schema_preserves_nullable_value_domains() {
    for (ty, path, nonnull) in [
        (ObjectType::Pen, "/colourId", json!("col_test")),
        (ObjectType::Stroke, "/fillMatrix", json!([1, 0, 0, 1, 0, 0])),
        (ObjectType::SoundPlacement, "/mouthTrack", json!(null)),
    ] {
        let v = property_validator(ty, path);
        assert!(v.is_valid(&Value::Null), "{ty} {path}: {:?}", schema::property_schema(ty, path));
        assert!(v.is_valid(&nonnull), "{ty} {path}");
    }
}

#[test]
fn property_schema_nested_conditionals_and_ref_annotations_survive() {
    for path in ["/params/transform/origin/x", "/params/transform/yAxis/y", "/channels/rotation/keys/0/value"] {
        let s = schema::property_schema(ObjectType::Element, path).unwrap();
        assert_eq!(s["type"], "number", "{path}: {s}");
    }
    for ty in ObjectType::ALL {
        let key = if ty == ObjectType::Swatch { "/colourId" } else { "/id" };
        let s = schema::property_schema(ty, key).unwrap();
        assert_eq!(s["readOnly"], true, "{ty}");
        assert_eq!(s["x-since"], "1.0", "{ty}");
    }
}

#[test]
fn property_schema_preserves_every_enum_alternative() {
    let v = property_validator(ObjectType::Pen, "/tip");
    for tip in ["round", "oval", "square", "flat"] {
        assert!(v.is_valid(&json!(tip)), "valid pen tip {tip} rejected by discovered schema");
    }
}

#[test]
fn anonymous_variant_property_declarations_have_since() {
    // These are declarations, not repeated conditional predicates: DynamicParam is a persisted untagged enum
    // whose two anonymous object alternatives introduce value/keyframes (10 section 2.8; 06 section 8.4).
    let dynamic = &schema::definitions()["DynamicParam"]["anyOf"];
    let missing: Vec<&str> = [(0, "value"), (1, "keyframes")]
        .into_iter()
        .filter_map(|(index, name)| (dynamic[index]["properties"][name]["x-since"] != "1.0").then_some(name))
        .collect();
    assert!(missing.is_empty(), "anonymous property declarations lack x-since: {missing:?}");
}

#[test]
fn frame_map_keys_and_nfc_values_at_fresh_boundaries() {
    let doc = fixture::sample_document(202610180311).unwrap();
    let mut drawing = raw(&doc, ObjectType::Element);
    // Select the drawing element, independent of fixture iteration order.
    for entry in doc.entry_keys() {
        let v = doc.entry_json(&entry).unwrap();
        if v.get("elementType") == Some(&json!("drawing")) {
            drawing = v;
            break;
        }
    }
    let mut placement = raw(&doc, ObjectType::SoundPlacement);
    placement["startFrame"] = json!(1);
    placement["stopFrame"] = json!(2_147_483_647u32);
    // Labels are placement relative; a stale track can retain a shorter mouths string.
    for key in ["-1", "0", "2147483647", "2147483648", "01", "1\u{0301}"] {
        drawing["cellNotes"] = json!({key: "A\u{030a} e\u{0301}"});
        let valid = object::from_json::<Element>(&drawing);
        assert_eq!(valid.is_ok(), key == "2147483647", "cellNotes {key}: {valid:?}");
        if let Ok(v) = valid {
            assert_eq!(object::to_json(&v).unwrap()["cellNotes"][key], "Å é");
        }
    }
    // Use the actual placement length for label validation, then exercise max-int at the schema boundary.
    placement = raw(&doc, ObjectType::SoundPlacement);
    for key in ["-1", "0", "2147483647", "2147483648", "01", "1\u{0301}", "1"] {
        placement["mouthTrack"]["labels"] = json!({key: "A\u{030a} e\u{0301}"});
        let valid = object::from_json::<SoundPlacement>(&placement);
        assert_eq!(valid.is_ok(), key == "1", "labels {key}: {valid:?}");
        if let Ok(v) = valid {
            assert_eq!(object::to_json(&v).unwrap()["mouthTrack"]["labels"][key], "Å é");
        }
        if key == "2147483647" {
            assert!(schema::validate(ObjectType::SoundPlacement, &placement, 10).unwrap().is_empty());
        }
    }
}
