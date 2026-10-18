//! Thirty implemented register rows not selected in R2: 111-124, 128-141, 146, 148.
#![allow(clippy::unwrap_used, clippy::panic, reason = "review fixture assertions")]
use serde_json::{Value, json};
use tt_model::entities::{drawing::Drawing, element::Element};
use tt_model::{fixture, object};

fn element(kind: &str) -> Value {
    let doc = fixture::sample_document(202610180312).unwrap();
    doc.elements().map(|e| object::to_json(e).unwrap()).find(|e| e["elementType"] == kind).unwrap()
}

fn valid(v: &Value) -> Value {
    let obj = object::from_json::<Element>(v).unwrap();
    object::to_json(&obj).unwrap()
}

fn field(kind: &str, path: &str, good: &[Value], bad: &[Value]) {
    let mut v = element(kind);
    valid(&v);
    for x in good {
        *v.pointer_mut(path).unwrap() = x.clone();
        assert_eq!(valid(&v).pointer(path), Some(x), "{kind} {path}");
    }
    for x in bad {
        *v.pointer_mut(path).unwrap() = x.clone();
        assert!(object::from_json::<Element>(&v).is_err(), "{kind} {path} accepts {x}");
    }
}

#[test]
fn rows_111_to_124_element_variant_blocks_are_required_and_typed() {
    let cases: [(&str, &[&str]); 14] = [
        ("sound", &["streamed", "showMouthShapes", "placements", "lipSyncMappings"]),
        ("media", &["sequences", "transform", "channels"]),
        ("peg", &["transform", "exportGroup", "channels"]),
        ("camera", &["camera", "transform", "channels"]),
        ("skeleton", &["skeleton", "transform", "channels"]),
        ("bone", &["bone", "channels"]),
        ("effectParameter", &["slot"]),
        ("colourTransform", &["params", "channels"]),
        ("clipping", &["params", "channels"]),
        ("dropShadow", &["params", "channels"]),
        ("blur", &["params", "channels"]),
        ("shake", &["params", "channels"]),
        ("particleNature", &["params", "blocks"]),
        ("particleImpact", &["params", "blocks"]),
    ];
    for (i, (kind, required)) in cases.iter().enumerate() {
        let original = element(kind);
        valid(&original);
        for key in *required {
            assert!(original.get(*key).is_some(), "row {} {key}", 111 + i);
            let mut changed = original.clone();
            changed.as_object_mut().unwrap().remove(*key);
            assert!(object::from_json::<Element>(&changed).is_err(), "{kind} missing {key}");
            changed[*key] = Value::Null;
            assert!(object::from_json::<Element>(&changed).is_err(), "{kind} null {key}");
        }
    }
}

#[test]
fn rows_128_to_137_transform_constraints_and_pivots() {
    field(
        "drawing",
        "/transform/scale/x",
        &[json!(-0.001), json!(0.001), json!(-9.5)],
        &[json!(0), json!(0.0009), json!(-0.0009)],
    );
    for path in
        ["/transform/keepProportions", "/transform/rotateAboutPivot", "/transform/lockDepth", "/transform/orientToPath"]
    {
        field("drawing", path, &[json!(true), json!(false)], &[json!("true"), json!(1)]);
    }
    for path in ["/transform/rotationPivot", "/transform/scalePivot"] {
        field("drawing", path, &[json!({"x": -2.75, "y": 4.125})], &[json!([1, 2]), json!({"x": "1", "y": 2})]);
    }
    field("peg", "/transform/pivot", &[json!({"x": -8.0, "y": 2.25})], &[json!(null), json!({"x": 1})]);
    field(
        "peg",
        "/transform/applyPivotTo",
        &[json!("rotation"), json!("scaling"), json!("both")],
        &[json!("none"), json!(false)],
    );
    field("peg", "/transform/useChildDrawingPivot", &[json!(true), json!(false)], &[json!(null), json!(1)]);
}

#[test]
fn rows_138_to_141_camera_export_skeleton_and_bone_blocks() {
    field("camera", "/camera/fov", &[json!(1.0), json!(179.0)], &[json!(0.99), json!(179.01)]);
    field("camera", "/camera/fovMode", &[json!("static"), json!("dynamic")], &[json!("unknown")]);
    for path in ["/exportGroup/groupOnVectorExport", "/exportGroup/addComposite"] {
        field("peg", path, &[json!(true), json!(false)], &[json!("false")]);
    }
    for path in [
        "/skeleton/showTrack",
        "/skeleton/showRoot",
        "/skeleton/showBones",
        "/skeleton/showInfluence",
        "/skeleton/deformationEnabled",
    ] {
        field("skeleton", path, &[json!(true), json!(false)], &[json!(1)]);
    }
    field(
        "skeleton",
        "/skeleton/vectorQuality",
        &[json!("low"), json!("medium"), json!("high"), json!("highest")],
        &[json!("ultra")],
    );
    field("skeleton", "/skeleton/boneColour", &[json!(null), json!("#AABBCCFF")], &[json!("red")]);
    field("bone", "/bone/rest/length", &[json!(0.011), json!(3.75)], &[json!(0), json!(0.01)]);
    field("bone", "/bone/influence/type", &[json!("infinite"), json!("elliptical")], &[json!("round")]);
}

#[test]
fn rows_146_and_148_drawing_pivot_and_ordered_nodes() {
    let doc = fixture::sample_document(202610180312).unwrap();
    let mut v = doc
        .drawings()
        .map(|d| object::to_json(d).unwrap())
        .find(|d| d["nodes"].as_array().unwrap().len() >= 2)
        .unwrap();
    v["pivot"] = json!({"x": -0.125, "y": 63.75});
    v["nodes"].as_array_mut().unwrap().reverse();
    let out = object::to_json(&object::from_json::<Drawing>(&v).unwrap()).unwrap();
    assert_eq!(out["pivot"], v["pivot"]);
    assert_eq!(out["nodes"], v["nodes"]);
    v["pivot"]["y"] = json!("63.75");
    assert!(object::from_json::<Drawing>(&v).is_err());
    v["pivot"]["y"] = json!(63.75);
    v["nodes"] = json!({});
    assert!(object::from_json::<Drawing>(&v).is_err());
}
