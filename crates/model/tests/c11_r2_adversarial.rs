//! C1.1 round 2: independent fix-closure controls and permanent defect regressions.
#![allow(clippy::unwrap_used, clippy::panic, reason = "review regression assertions")]
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::sync::OnceLock;
use tt_model::document::{Document, RenderOptions};
use tt_model::entities::{
    ModelObject,
    asset::Asset,
    channel::{Channel, CurveType, Key},
    drawing::{Drawing, DrawingNode},
    element::Element,
    palette::{Palette, Swatch},
    pen::{Pen, PenKind},
};
use tt_model::{Id, IdGenerator, ObjectType, canonical, change, fixture, object, schema};

fn doc() -> &'static Document {
    static DOC: OnceLock<Document> = OnceLock::new();
    DOC.get_or_init(|| fixture::sample_document(1182712).unwrap())
}

fn raw(ty: ObjectType) -> Value {
    for entry in doc().entry_keys() {
        for o in change::own_objects(doc(), &entry).unwrap() {
            if o.ty == ty {
                return doc().object_json(&o.key, RenderOptions { depth: None, path: false }).unwrap();
            }
        }
    }
    panic!("fixture lacks {ty}")
}

fn id(s: &str) -> Id {
    Id::parse(s).unwrap()
}

fn op(kind: &'static str, path: &str, from: Option<&str>, value: Option<Value>, old: Option<Value>) -> change::PatchOp {
    change::PatchOp {
        op: kind,
        path: path.into(),
        from: from.map(str::to_owned),
        value,
        old_value: old,
        range: None,
        elided: false,
    }
}

fn roundtrip_patch(before: Value, patch: Vec<change::PatchOp>) {
    let after = change::apply(&before, &patch).unwrap();
    // Amended by the builder in round 2 per the orchestrator's C11-R2-02 ruling ("inverses must be computed against
    // the pre-state with resolved concrete indices (never `-`)"): the inverse is computed from `before`. The two
    // assertions are unchanged.
    let inverse = change::invert_against(&before, &patch).unwrap();
    assert_eq!(change::apply(&after, &inverse).unwrap(), before, "patch={patch:?}; inverse={inverse:?}");
}

#[test]
fn duplicate_split_entries_all_five_maps_are_rejected() {
    for which in 0..5 {
        let mut p = fixture::sample_parts(1182712).unwrap();
        match which {
            0 => p.palettes.push(p.palettes[0].clone()),
            1 => p.banks.push(p.banks[0].clone()),
            2 => p.drawings.push(p.drawings[0].clone()),
            3 => p.scenes.push(p.scenes[0].clone()),
            _ => p.elements.push(p.elements[0].clone()),
        }
        let errors = Document::from_parts(p).unwrap_err();
        assert!(errors.iter().any(|e| e.code() == "id.duplicate"), "map {which}: {errors:?}");
    }
}

#[test]
fn duplicate_inline_collections_and_cross_entry_global_ids_are_rejected() {
    let mut palette = raw(ObjectType::Palette);
    for field in ["styles", "swatches"] {
        let saved = palette[field].clone();
        let first = palette[field][0].clone();
        palette[field].as_array_mut().unwrap().push(first);
        assert!(object::from_json::<Palette>(&palette).is_err(), "{field}");
        palette[field] = saved;
    }
    let mut p = fixture::sample_parts(1182712).unwrap();
    let mut project = object::to_json(&p.project).unwrap();
    project["pens"][0]["id"] = json!(p.elements[0].id());
    p.project = serde_json::from_value(project).unwrap();
    assert!(Document::from_parts(p).unwrap_err().iter().any(|e| e.code() == "id.duplicate"));
}

#[test]
fn duplicate_local_ids_across_nested_nodes_share_drawing_scope() {
    let mut d = raw(ObjectType::Drawing);
    let node = json!({"id":"shared","type":"drawingNode","rev":1,"kind":"layer","hint":"brush","strokes":[]});
    d["nodes"] = json!([node, {"id":"group","type":"drawingNode","rev":1,"kind":"group","children":[node]}]);
    assert!(object::from_json::<Drawing>(&d).is_err());
    d["nodes"][1]["children"][0]["id"] = json!("different");
    assert!(object::from_json::<Drawing>(&d).is_ok());
}

#[test]
fn duplicate_keys_are_rejected_even_at_distinct_frames() {
    let mut ch = raw(ObjectType::Channel);
    ch["keys"][1]["id"] = ch["keys"][0]["id"].clone();
    assert!(object::from_json::<Channel>(&ch).is_err());
}

#[test]
fn duplicate_ids_across_split_types_and_inline_levels_are_rejected() {
    for which in 0..7 {
        let mut p = fixture::sample_parts(1182712).unwrap();
        let collision = json!(p.project.id());
        match which {
            0 => {
                let mut v = object::to_json(&p.palettes[0]).unwrap();
                v["styles"][0]["id"] = collision;
                p.palettes[0] = serde_json::from_value(v).unwrap();
            }
            1 => {
                let mut v = object::to_json(&p.project).unwrap();
                v["startTransition"]["id"] = collision;
                p.project = serde_json::from_value(v).unwrap();
            }
            2 => {
                let mut v = object::to_json(&p.scenes[0]).unwrap();
                v["outTransition"]["id"] = collision;
                p.scenes[0] = serde_json::from_value(v).unwrap();
            }
            3 => {
                let mut v = object::to_json(&p.project).unwrap();
                v["assets"][0]["id"] = collision;
                p.project = serde_json::from_value(v).unwrap();
            }
            4 => {
                let mut v = object::to_json(&p.library).unwrap();
                v["children"][0]["id"] = collision;
                p.library = serde_json::from_value(v).unwrap();
            }
            5 => {
                let mut v = object::to_json(&p.palettes[0]).unwrap();
                v["swatches"][0]["colourId"] = collision;
                p.palettes[0] = serde_json::from_value(v).unwrap();
            }
            _ => {
                let mut v = object::to_json(&p.scenes[0]).unwrap();
                v["id"] = collision;
                p.scenes[0] = serde_json::from_value(v).unwrap();
            }
        }
        assert!(Document::from_parts(p).is_err(), "containment case {which}");
    }
}

fn invalid_roundtrip<T: ModelObject + tt_model::validate::Check>(v: Value) {
    assert!(object::from_json::<T>(&v).is_err());
    for t in [serde_json::from_value::<T>(v.clone()), serde_json::from_str::<T>(&v.to_string())].into_iter().flatten() {
        assert!(object::revalidate(&t).is_err());
    }
}

#[test]
fn public_serde_entries_cannot_evade_revalidation() {
    let mut p = raw(ObjectType::Pen);
    p["maxSize"] = json!(101);
    invalid_roundtrip::<Pen>(p);
    let mut p = raw(ObjectType::Element);
    p["loops"] = json!(2147483648_u64);
    invalid_roundtrip::<Element>(p);
    let mut p = raw(ObjectType::Channel);
    p["keys"][0]["frame"] = json!(0);
    invalid_roundtrip::<Channel>(p);
    let mut p = raw(ObjectType::Swatch);
    p["name"] = json!("\u{0001}");
    invalid_roundtrip::<Swatch>(p);
    let mut p = raw(ObjectType::Asset);
    p["storage"] = json!("linked");
    p["entry"] = Value::Null;
    p["link"] =
        json!({"kind":"libraryItem","path":null,"absolutePath":null,"libraryId":null,"itemId":null,"versionId":null});
    invalid_roundtrip::<Asset>(p);
    let p = Pen::new(id("p"), "Pen".into(), PenKind::Brush, 101.0, 102.0, 0);
    assert!(object::revalidate(&p).is_err());
    let k = Key::new(id("k"), 0, 1.0);
    assert!(object::revalidate(&k).is_err());
    let ch = Channel::one_d(id("c"), "rotation".into(), CurveType::Spline, vec![k]);
    assert!(object::revalidate(&ch).is_err());
}

#[test]
fn every_document_entry_kind_revalidates_deserialized_fields() {
    for which in 0..7 {
        let mut p = fixture::sample_parts(1182712).unwrap();
        macro_rules! corrupt {
            ($slot:expr, $field:literal, $bad:expr) => {{
                let mut v = object::to_json(&$slot).unwrap();
                v[$field] = $bad;
                $slot = serde_json::from_value(v).unwrap();
            }};
        }
        match which {
            0 => corrupt!(p.project, "cameraWidth", json!(1)),
            1 => corrupt!(p.palettes[0], "name", json!("bad/name")),
            2 => {
                let mut v = object::to_json(&p.banks[0]).unwrap();
                let one = v["drawings"][0].clone();
                v["drawings"].as_array_mut().unwrap().push(one);
                p.banks[0] = serde_json::from_value(v).unwrap();
            }
            3 => corrupt!(p.drawings[0], "name", json!("bad.name")),
            4 => corrupt!(p.scenes[0], "name", json!("bad/name")),
            5 => corrupt!(p.elements[0], "columnWidth", json!(1)),
            _ => corrupt!(p.library, "name", json!("")),
        }
        assert!(Document::from_parts(p).is_err(), "entry {which}");
    }
}

#[test]
fn opaque_model_lookalikes_deep_arrays_and_patch_inverse_are_preserved() {
    let mut blob = json!({"type":"swatch","id":"alias","colourId":"different","name":" e\u{301} ","rev":9,"exposure":[[null,0]],"items":[{"type":"pen","id":"p"}],"nested":{"type":"channel","keys":[]}});
    for _ in 0..20 {
        blob = json!({"name":" e\u{301} ","children":[blob]});
    }
    let mut p = raw(ObjectType::Pen);
    p["meta"] = json!({"blob":blob});
    p["ext"] = json!({"org.review.test":blob});
    let a = object::to_json(&object::from_json::<Pen>(&p).unwrap()).unwrap();
    assert_eq!(a["meta"], p["meta"]);
    assert_eq!(a["ext"], p["ext"]);
    let mut b = a.clone();
    b["meta"]["blob"]["children"][0]["name"] = json!("changed");
    let patch = change::diff_json(&a, &b, "", ObjectType::Pen);
    assert_eq!(change::apply(&a, &patch).unwrap(), b);
    assert_eq!(change::apply(&b, &change::invert(&patch)).unwrap(), a);
}

#[test]
fn image_import_options_are_opaque() {
    let p = fixture::sample_parts(1182712).unwrap();
    let image = p.elements.iter().find(|e| e.element_type().as_str() == "image").unwrap();
    let mut v = object::to_json(image).unwrap();
    v["importOptions"] =
        json!({"type":"swatch","id":"x","name":" e\u{301} ","points":[{"type":"element","name":"  raw  "}]});
    assert_eq!(
        object::to_json(&object::from_json::<Element>(&v).unwrap()).unwrap()["importOptions"],
        v["importOptions"]
    );
}

#[test]
fn ordinary_move_copy_test_and_array_index_shifts_invert() {
    roundtrip_patch(
        json!({"a":[0,1,2,3],"b":7}),
        vec![
            op("move", "/a/3", Some("/a/0"), None, None),
            op("copy", "/b", Some("/a/1"), None, Some(json!(7))),
            op("test", "/b", None, Some(json!(2)), None),
        ],
    );
    roundtrip_patch(json!({"a/b":{"~x":1},"z":9}), vec![op("move", "/z", Some("/a~1b/~0x"), None, Some(json!(9)))]);
    assert!(change::apply(&json!({}), &[op("replace", "/absent", None, Some(json!(1)), Some(json!(0)))]).is_err());
    assert_eq!(
        change::apply(&json!({"a":1}), &[op("test", "/a", None, Some(json!(2)), None)]).unwrap_err().code(),
        "doc.patchTestFailed"
    );
}

#[test]
fn array_append_patch_inverse_restores_input() {
    roundtrip_patch(json!({"a":[1,2]}), vec![op("add", "/a/-", None, Some(json!(3)), None)]);
}

#[test]
fn nested_array_move_with_overwrite_inverse_restores_input() {
    roundtrip_patch(
        json!({"a":[{"x":1},{"x":2},{"x":3}]}),
        vec![op("move", "/a/1/x", Some("/a/0"), None, Some(json!(3)))],
    );
}

#[test]
fn range_patch_rejects_nonstring_mouth_value_without_deleting_text() {
    let mut p = op("replace", "/mouths", None, Some(json!(42)), Some(json!("B")));
    p.range = Some(tt_model::value::FrameRange { from: 2, to: 2 });
    assert!(change::apply(&json!({"mouths":"ABC"}), &[p]).is_err(), "malformed replacement must not delete B");
}

#[test]
fn missing_copy_source_is_not_implicitly_the_document_root() {
    assert!(change::apply(&json!({"a":1}), &[op("copy", "/b", None, None, None)]).is_err());
}

#[test]
fn elided_and_incomplete_operations_fail_without_mutating_input() {
    let value = json!({"a":[0,1],"b":1});
    for k in ["add", "replace", "test"] {
        assert!(change::apply(&value, &[op(k, "/b", None, None, Some(json!(1)))]).is_err());
    }
    for k in ["add", "remove", "replace", "move", "copy", "test"] {
        let mut p = op(k, "/b", Some("/a/0"), Some(json!(2)), Some(json!(1)));
        p.elided = true;
        assert!(change::apply(&value, &[p.clone()]).is_err());
        if k != "test" {
            assert!(change::apply(&value, &change::invert(&[p])).is_err());
        }
    }
    assert_eq!(value, json!({"a":[0,1],"b":1}));
}

fn reorder(v: &Value, seed: &mut u64) -> Value {
    match v {
        Value::Object(m) => {
            let mut pairs: Vec<_> = m.iter().collect();
            for i in (1..pairs.len()).rev() {
                *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let j = usize::try_from(*seed % u64::try_from(i + 1).unwrap()).unwrap();
                pairs.swap(i, j);
            }
            Value::Object(pairs.into_iter().map(|(k, v)| (k.clone(), reorder(v, seed))).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(|v| reorder(v, seed)).collect()),
        _ => v.clone(),
    }
}

#[test]
fn randomized_insertion_and_post_patch_revalidation_write_identical_bytes() {
    let mut p = raw(ObjectType::Pen);
    p["meta"] = json!({"z":[{"type":"element","name":" e\u{301} ","a":2}],"a":1,"é":3,"😀":4});
    p["ext"] = json!({"org.review.test":{"z":9,"a":1}});
    let expected = object::to_canonical(&object::from_json::<Pen>(&p).unwrap()).unwrap();
    let mut seed = 1182712;
    for _ in 0..100 {
        let candidate = reorder(&p, &mut seed);
        assert_eq!(object::to_canonical(&object::from_json::<Pen>(&candidate).unwrap()).unwrap(), expected);
        let patch = vec![op("replace", "/meta", None, Some(candidate["meta"].clone()), Some(p["meta"].clone()))];
        let patched = change::apply(&p, &patch).unwrap();
        assert_eq!(object::to_canonical(&object::from_json::<Pen>(&patched).unwrap()).unwrap(), expected);
    }
}

fn nested(n: usize) -> Value {
    (0..n).fold(json!(1), |v, _| json!([v]))
}

#[test]
fn model_depth_99_100_101_is_enforced_before_parsing() {
    for depth in [99, 100, 101] {
        let mut p = raw(ObjectType::Pen);
        p["meta"] = json!({"deep": nested(depth-2)});
        assert_eq!(tt_model::input::depth(&p, 200), depth);
        let result = object::from_json::<Pen>(&p);
        if depth <= 100 {
            let value = result.unwrap();
            let text = object::to_canonical(&value).unwrap();
            assert!(tt_model::json_text::parse::<Value>(&text).is_ok());
        } else {
            assert_eq!(result.unwrap_err()[0].code(), "doc.limitExceeded");
        }
    }
}

#[test]
fn canonical_writer_limits_are_explicit() {
    assert!(canonical::to_value(&nested(127)).is_ok());
    assert!(canonical::to_value(&nested(128)).is_ok());
    assert!(canonical::to_value(&nested(129)).is_err());
    assert!(canonical::to_canonical_string(&nested(127)).is_ok());
    assert!(canonical::to_canonical_string(&nested(129)).is_err());
}

#[test]
fn nonfinite_nested_structs_maps_and_model_constructors_are_rejected() {
    #[derive(serde::Serialize)]
    struct Nested {
        values: Vec<f32>,
        other: Option<f64>,
    }
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let nested = Nested { values: vec![1.0], other: Some(n) };
        assert!(canonical::check_finite(&nested).is_err());
        assert!(canonical::to_value(&nested).is_err());
        assert!(canonical::to_canonical(&nested).is_err());
        let p = Pen::new(id("p"), "Pen".into(), PenKind::Brush, 0.0, n, 0);
        assert!(object::to_json(&p).is_err());
        assert!(object::to_canonical(&p).is_err());
        assert!(object::revalidate(&p).is_err());
        let k = Key::new(id("k"), 1, n);
        assert!(object::to_json(&k).is_err());
    }
    assert!(canonical::to_value(&Nested { values: vec![f32::NAN], other: None }).is_err());
}

#[test]
fn cloned_generator_eight_threads_100k_each_are_unique() {
    for generator in [IdGenerator::system(), IdGenerator::seeded(1182712, 1)] {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let mut g = generator.clone();
                std::thread::spawn(move || {
                    let mut ids = Vec::with_capacity(100_000);
                    for _ in 0..100_000 {
                        ids.push(g.next(ObjectType::Element).unwrap());
                    }
                    assert!(ids.windows(2).all(|w| w[0].as_str() < w[1].as_str()));
                    ids
                })
            })
            .collect();
        let all: HashSet<_> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
        assert_eq!(all.len(), 800_000);
    }
}

#[test]
fn swatch_alias_equal_conflicting_and_nested_forms() {
    let mut sw = raw(ObjectType::Swatch);
    sw["id"] = sw["colourId"].clone();
    assert!(object::from_json::<Swatch>(&sw).is_ok());
    sw["id"] = json!("different");
    assert!(object::from_json::<Swatch>(&sw).is_err());
    let mut pal = raw(ObjectType::Palette);
    for s in pal["swatches"].as_array_mut().unwrap() {
        let id = s.as_object_mut().unwrap().shift_remove("colourId").unwrap();
        s["id"] = id;
    }
    assert!(object::from_json::<Palette>(&pal).is_ok());
}

#[test]
fn signed_int_boundaries_and_seed_exception() {
    let mut k = raw(ObjectType::Key);
    for n in [1_u64, 2147483647, 2147483648, 4294967295] {
        k["frame"] = json!(n);
        assert_eq!(schema::is_valid(ObjectType::Key, &k), n <= 2147483647, "{n}");
    }
    let mut b = raw(ObjectType::ParticleBlock);
    b["seed"] = json!(u32::MAX);
    assert!(schema::is_valid(ObjectType::ParticleBlock, &b));
    b["seed"] = json!(4294967296_u64);
    assert!(!schema::is_valid(ObjectType::ParticleBlock, &b));
}

#[test]
fn every_model_stub_and_full_result_validates_at_depth_zero_one_two_all() {
    let mut types = BTreeSet::new();
    for e in doc().entry_keys() {
        for o in change::own_objects(doc(), &e).unwrap() {
            types.insert(o.ty);
            for depth in [Some(0), Some(1), Some(2), None] {
                let v = doc().object_json(&o.key, RenderOptions { depth, path: true }).unwrap();
                assert!(schema::is_valid(o.ty, &v), "{} depth {depth:?}: {v}", o.key);
            }
        }
    }
    assert_eq!(types.len(), ObjectType::ALL.len());
}

#[test]
fn property_lookup_follows_effect_condition_without_discarding_it() {
    let s = schema::property_schema(ObjectType::Element, "/params/transform/origin/x");
    assert!(s.is_some(), "the drop-shadow transform is declared under Element then/allOf");
    assert_eq!(s.unwrap()["type"], "number");
}

#[test]
fn property_lookup_preserves_ref_property_annotations() {
    let p = schema::property_schema(ObjectType::Pen, "/id").unwrap();
    assert_eq!(p["readOnly"], true, "resolving Id must retain the owning property's readOnly");
    assert_eq!(p["x-since"], "1.0");
}

#[test]
fn spline_clamps_both_sides_for_irregular_key_spacing() {
    let mut ch = raw(ObjectType::Channel);
    ch["curveType"] = json!("spline");
    let template = ch["keys"][0].clone();
    ch["keys"] = Value::Array(
        [1, 7, 90]
            .into_iter()
            .map(|frame| {
                let mut k = template.clone();
                k["id"] = json!(format!("k{frame}"));
                k["frame"] = json!(frame);
                k["handles"]["left"]["dt"] = json!(-1e9);
                k["handles"]["right"]["dt"] = json!(1e9);
                k
            })
            .collect(),
    );
    let out = object::to_json(&object::from_json::<Channel>(&ch).unwrap()).unwrap();
    assert_eq!(out["keys"][0]["handles"]["right"]["dt"], 6.0);
    assert_eq!(out["keys"][1]["handles"]["left"]["dt"], -6.0);
    assert_eq!(out["keys"][1]["handles"]["right"]["dt"], 83.0);
    assert_eq!(out["keys"][2]["handles"]["left"]["dt"], -83.0);
}

#[test]
fn fixed_text_width_and_texture_paths_reject_new_invalid_cases() {
    let mut text = json!({"id":"text","type":"drawingNode","rev":1,"kind":"text","transform":[1,0,0,1,0,0],"widthMode":"fixed","text":"","runs":[],"paragraphs":[]});
    assert!(object::from_json::<DrawingNode>(&text).is_err());
    text["width"] = json!(10);
    assert!(object::from_json::<DrawingNode>(&text).is_ok());
    let mut sw = raw(ObjectType::Swatch);
    sw["kind"] = json!("texture");
    for path in ["textures/../bad.png", "textures//a.png", "textures/./a.png", "C:/a.png", "textures\\a.png"] {
        for v in sw["values"].as_object_mut().unwrap().values_mut() {
            *v = json!({"image":path,"tile":true});
        }
        assert!(object::from_json::<Swatch>(&sw).is_err(), "{path}");
    }
    for v in sw["values"].as_object_mut().unwrap().values_mut() {
        *v = json!({"image":"textures/valid.png","tile":true});
    }
    assert!(object::from_json::<Swatch>(&sw).is_ok());
}

fn element(kind: &str) -> Value {
    let p = fixture::sample_parts(1182712).unwrap();
    object::to_json(p.elements.iter().find(|e| e.element_type().as_str() == kind).unwrap()).unwrap()
}

#[test]
fn cell_note_frame_map_keys_obey_signed_int_bounds() {
    let mut e = element("drawing");
    e["cellNotes"] = json!({"2147483647":"last signed int"});
    assert!(object::from_json::<Element>(&e).is_ok());
    e["cellNotes"] = json!({"2147483648":"outside signed int"});
    assert!(object::from_json::<Element>(&e).is_err(), "10 §1.1 frame is int even when used as a map key");
}

#[test]
fn mouth_label_frame_map_keys_are_positive_and_within_int() {
    use tt_model::entities::sound::SoundPlacement;
    let mut p = raw(ObjectType::SoundPlacement);
    assert!(p["mouthTrack"].is_object());
    p["mouthTrack"]["labels"] = json!({"0":"not a frame","2147483648":"out of int"});
    assert!(object::from_json::<SoundPlacement>(&p).is_err());
}

#[test]
fn new_entity_semantic_boundary_controls() {
    remaining_entity_semantic_boundary_controls();
    use tt_model::entities::sound::SoundPlacement;
    let mut p = raw(ObjectType::SoundPlacement);
    let frames = p["stopFrame"].as_u64().unwrap() - p["startFrame"].as_u64().unwrap() + 1;
    p["mouthTrack"]["manual"] = json!([{"from":1,"to":frames}]);
    assert!(object::from_json::<SoundPlacement>(&p).is_ok());
    for range in [json!({"from":0,"to":1}), json!({"from":1,"to":frames+1})] {
        p["mouthTrack"]["manual"] = json!([range]);
        assert!(object::from_json::<SoundPlacement>(&p).is_err());
    }
    let mut p = fixture::sample_parts(1182712).unwrap();
    let im = p.elements.iter().find(|e| e.element_type().as_str() == "image").unwrap();
    let bank = p.banks.iter().find(|b| Some(b.id()) == im.drawing_bank_id().as_ref()).unwrap();
    let which = p.drawings.iter().position(|d| d.id() == &bank.drawings().ids()[0]).unwrap();
    let mut drawing = object::to_json(&p.drawings[which]).unwrap();
    let mut bitmap = drawing["nodes"][0].clone();
    bitmap["id"] = json!("secondBitmap");
    drawing["nodes"].as_array_mut().unwrap().push(bitmap);
    p.drawings[which] = object::from_json(&drawing).unwrap();
    assert!(Document::from_parts(p).is_err(), "two bitmaps also violate image invariant");
    let mut a = raw(ObjectType::Asset);
    a["storage"] = json!("linked");
    a["entry"] = Value::Null;
    a["link"] = json!({"kind":"libraryItem","path":null,"absolutePath":null,"libraryId":"lib","itemId":"item","versionId":null});
    assert!(object::from_json::<Asset>(&a).is_ok());
    a["link"]["path"] = json!("wrong-kind");
    assert!(object::from_json::<Asset>(&a).is_err());
}

#[test]
fn forty_previously_implemented_register_rows_retain_constraints() {
    use ObjectType::*;
    // Round-1 register row IDs. Every mutation starts with a valid fixture object; property-schema presence alone
    // does not count as this check. Colour normalization and calendar timestamps also go through their parsers.
    let cases = [
        (1, Pen, "/id", json!("bad/id")),
        (5, Scene, "/visible", json!(0)),
        (6, Stroke, "/kind", json!("bad")),
        (7, Scene, "/backgroundColour", json!("#GG0000")),
        (8, Drawing, "/pivot/x", json!("bad")),
        (9, Element, "/transform/offset/z", json!("bad")),
        (10, Stroke, "/fillMatrix", json!([1, 0, 0, 1, 0])),
        (13, SoundPlacement, "/inPoint", json!(-1)),
        (15, Drawing, "/createdAt", json!("2026-01-01")),
        (23, Project, "/name", json!("Bad.")),
        (24, Scene, "/name", json!("Bad/name")),
        (25, Element, "/name", json!("Bad.name")),
        (26, Drawing, "/name", json!("Bad.name")),
        (27, Palette, "/name", json!("Bad/name")),
        (28, Swatch, "/name", json!("x".repeat(129))),
        (29, Pen, "/name", json!("x".repeat(65))),
        (30, LibraryFolder, "/name", json!("Bad/name")),
        (65, Project, "/frameRate", json!(120.1)),
        (66, Project, "/cameraWidth", json!(15)),
        (67, Project, "/cameraHeight", json!(8193)),
        (76, Pen, "/minSize", json!(-1)),
        (77, Pen, "/maxSize", json!(100.1)),
        (78, Pen, "/smoothness", json!(11)),
        (79, Pen, "/tip", json!("bad")),
        (80, Pen, "/colourId", json!("bad/id")),
        (82, Scene, "/visible", json!("true")),
        (83, Scene, "/backgroundColour", json!(42)),
        (90, Element, "/elementType", json!("bad")),
        (95, Element, "/loopType", json!("bad")),
        (96, Element, "/positionLoop", json!("bad")),
        (99, Element, "/visibleInScene", json!(0)),
        (101, Element, "/locked", json!(0)),
        (102, Element, "/collapsed", json!(0)),
        (103, Element, "/markerColour", json!("bad")),
        (104, Element, "/backgroundColour", json!("bad")),
        (106, Element, "/columnWidth", json!(1001)),
        (125, Element, "/transform/offset/x", json!("bad")),
        (126, Element, "/transform/rotation", json!("bad")),
        (127, Element, "/transform/skew", json!(90)),
        (147, Drawing, "/mouthShape", json!("H")),
    ];
    assert_eq!(cases.len(), 40);
    for (row, ty, pointer, bad) in cases {
        let mut value = if ty == Element { element("drawing") } else { raw(ty) };
        assert!(schema::is_valid(ty, &value), "valid control row {row}");
        if ty == Stroke && pointer == "/fillMatrix" {
            value["fillMatrix"] = bad;
        } else {
            *value.pointer_mut(pointer).unwrap_or_else(|| panic!("row {row}: missing {pointer}")) = bad;
        }
        assert!(!schema::is_valid(ty, &value), "row {row}: {pointer} accepted invalid value");
        println!("register spot-check {row}: pass");
    }
    assert_eq!(tt_model::value::Colour::parse("#a1b2c3").unwrap().to_string(), "#A1B2C3FF");
    assert!(tt_model::value::Timestamp::parse("2025-02-29T00:00:00.000Z").is_err());
}

#[test]
fn all_entity_serde_entry_points_reject_unknown_fields_and_revalidate() {
    use tt_model::entities::*;
    fn check<T: ModelObject + tt_model::validate::Check>() {
        let mut v = if T::TYPE.is_entry() {
            doc()
                .entry_keys()
                .iter()
                .find_map(|e| {
                    let v = doc().entry_json(e).unwrap();
                    (v["type"] == T::TYPE.as_str()).then_some(v)
                })
                .unwrap()
        } else {
            raw(T::TYPE)
        };
        let typed: T = serde_json::from_value(v.clone()).unwrap();
        assert!(object::revalidate(&typed).is_ok(), "{} valid control", T::TYPE);
        v["unknownReviewProperty"] = json!(true);
        assert!(serde_json::from_value::<T>(v.clone()).is_err(), "{} from_value", T::TYPE);
        assert!(serde_json::from_str::<T>(&v.to_string()).is_err(), "{} from_str", T::TYPE);
        assert!(tt_model::json_text::parse::<T>(&v.to_string()).is_err(), "{} json_text", T::TYPE);
        assert!(object::from_json::<T>(&v).is_err(), "{} from_json", T::TYPE);
    }
    macro_rules! cases { ($($t:ty),+) => { $(check::<$t>();)+ }; }
    cases!(
        project::Project,
        palette::Palette,
        palette::PaletteStyle,
        palette::Swatch,
        pen::Pen,
        asset::Asset,
        drawing::DrawingBank,
        drawing::Drawing,
        drawing::DrawingNode,
        drawing::Stroke,
        scene::Scene,
        transition::Transition,
        element::Element,
        channel::Channel,
        channel::Key,
        channel::MotionPoint,
        effect::ParticleBlock,
        sound::SoundPlacement,
        sound::MediaSequence,
        library::Library,
        library::LibraryFolder,
        library::LibraryItem
    );
}

#[test]
fn all_public_entity_constructors_are_checked_before_document_use() {
    use tt_model::entities::{
        drawing::{DrawingBank, LayerHint},
        element::{ElementInit, ElementType, ParamSlot},
        library::Library,
        palette::PaletteStyle,
        project::{Project, ProjectInit},
        scene::{Scene, SceneInit},
        transition::Transition,
    };
    use tt_model::value::{Colour, Timestamp};
    let time = Timestamp::parse("2026-10-18T00:00:00.000Z").unwrap();
    assert!(object::revalidate(&PaletteStyle::new(id("sty"), "".into())).is_err());
    assert!(object::revalidate(&Swatch::solid(id("sw"), "".into(), &[id("sty")], Colour::WHITE)).is_err());
    assert!(object::revalidate(&Palette::new(id("pal"), "P".into(), vec![], vec![])).is_err());
    assert!(object::revalidate(&Library::local(id("lib"), "".into())).is_err());
    assert!(object::revalidate(&Drawing::empty(id("draw"), "bad.name".into(), time)).is_err());
    assert!(object::revalidate(&DrawingBank::new(id("bank"), vec![id("d"), id("d")])).is_err());
    assert!(object::revalidate(&DrawingNode::layer(id("layer"), LayerHint::Brush)).is_ok());
    assert!(object::revalidate(&Transition::none(id("transition"))).is_ok());
    assert!(
        object::revalidate(&Scene::new(SceneInit {
            id: id("scene"),
            name: "bad/name".into(),
            out_transition: Transition::none(id("transition")),
            palette_styles: Default::default(),
            elements: vec![],
            tree: vec![],
            column_order: vec![],
            current_camera_id: None
        }))
        .is_err()
    );
    let project = Project::new(ProjectInit {
        id: id("project"),
        name: "P".into(),
        created_at: time,
        frame_rate: f64::NAN,
        camera_size: (1920, 1080),
        start_transition: Transition::none(id("transition")),
        palettes: vec![id("p")],
        pens: vec![Pen::new(id("pen"), "Pen".into(), PenKind::Brush, 1.0, 2.0, 0)],
        drawing_banks: vec![],
        scenes: vec![id("scene")],
        library: id("lib"),
    });
    assert!(object::to_json(&project).is_err());
    assert!(object::revalidate(&project).is_err());
    for kind in ElementType::ALL {
        let e = Element::new(ElementInit {
            id: id("element"),
            element_type: kind,
            name: "bad.name".into(),
            created_at: time,
            drawing_bank_id: Some(id("bank")),
            bar: (1, 24),
            slot: Some(ParamSlot::Mask),
        });
        assert!(object::revalidate(&e).is_err(), "{}", kind.as_str());
    }
    let mut g = IdGenerator::seeded(1182712, 1);
    for p in Pen::default_set(|| g.next(ObjectType::Pen).unwrap()) {
        assert!(object::revalidate(&p).is_ok());
    }
}

#[test]
fn duplicate_placement_sequence_block_and_channel_ids_are_rejected() {
    for (kind, field) in [("sound", "placements"), ("media", "sequences"), ("particleNature", "blocks")] {
        let mut p = fixture::sample_parts(1182712).unwrap();
        let i = p.elements.iter().position(|e| e.element_type().as_str() == kind).unwrap();
        let mut v = object::to_json(&p.elements[i]).unwrap();
        let first = v[field][0].clone();
        assert!(first.is_object());
        v[field].as_array_mut().unwrap().push(first);
        p.elements[i] = serde_json::from_value(v).unwrap();
        assert!(Document::from_parts(p).is_err(), "{field}");
    }
    let mut p = fixture::sample_parts(1182712).unwrap();
    let i = p.elements.iter().position(|e| e.channels().as_ref().is_some_and(|c| c.len() > 1)).unwrap();
    let mut v = object::to_json(&p.elements[i]).unwrap();
    let map = v["channels"].as_object_mut().unwrap();
    let ids: Vec<_> = map.keys().cloned().collect();
    let collision = map[&ids[0]]["id"].clone();
    map.get_mut(&ids[1]).unwrap()["id"] = collision;
    p.elements[i] = serde_json::from_value(v).unwrap();
    assert!(Document::from_parts(p).is_err());
}

#[test]
fn deeper_library_paths_near_model_limit_resolve() {
    for count in [35, 45, 47] {
        let mut p = fixture::sample_parts(1182712).unwrap();
        let mut lib = object::to_json(&p.library).unwrap();
        let mut node = json!({"id":"leaf","type":"libraryFolder","rev":1,"name":"Leaf","children":[]});
        for i in 0..count {
            node = json!({"id":format!("folder_{i}"),"type":"libraryFolder","rev":1,"name":format!("Folder {i}"),"children":[node]});
        }
        lib["children"] = json!([node]);
        p.library = object::from_json(&lib).unwrap();
        let d = Document::from_parts(p).unwrap();
        let path = d.canonical_path("leaf").unwrap();
        assert!(d.resolve_str(&path, &()).is_ok(), "{count}: {path}");
    }
}

#[test]
fn every_envelope_type_is_readonly() {
    for ty in ObjectType::ALL {
        let s = schema::object_schema(ty);
        assert_eq!(s["properties"]["type"]["readOnly"], true, "{ty}");
    }
}

#[test]
fn every_schema_property_has_since_and_envelope_type_is_readonly() {
    fn walk(v: &Value) {
        if let Some(properties) = v.get("properties").and_then(Value::as_object) {
            for (name, property) in properties {
                assert_eq!(property["x-since"], "1.0", "{name}: {property}");
            }
        }
        // Conditional predicates constrain existing properties; they need not
        // repeat the declaration's annotations. Check declarations in each def.
    }
    for ty in ObjectType::ALL {
        let s = schema::object_schema(ty);
        walk(&s);
        for def in s["$defs"].as_object().unwrap().values() {
            walk(def);
        }
        assert_eq!(s["properties"]["type"]["readOnly"], true, "{ty}");
    }
}

#[test]
fn multiple_of_is_exact_near_u64_and_signed_limits() {
    let s = json!({"type":"integer","multipleOf":5});
    let validator = jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&s).unwrap();
    for n in [u64::MAX, u64::MAX - 1, 9007199254740990, 9007199254740991] {
        assert_eq!(validator.is_valid(&json!(n)), n % 5 == 0, "{n}");
    }
    for n in [i64::MIN, i64::MIN + 3, -9007199254740990, -9007199254740991] {
        assert_eq!(validator.is_valid(&json!(n)), n % 5 == 0, "{n}");
    }
}

#[test]
fn nfc_is_applied_to_declared_non_name_strings_without_touching_opaque_values() {
    let mut v = element("drawing");
    v["note"] = json!("A\u{030a} e\u{0301}");
    v["cellNotes"] = json!({"1":"A\u{030a} e\u{0301}"});
    v["meta"] = json!({"note":"A\u{030a} e\u{0301}"});
    let e: Element = object::from_json(&v).unwrap();
    let out = object::to_json(&e).unwrap();
    assert_eq!(out["note"], "\u{00c5} \u{00e9}");
    assert_eq!(out["cellNotes"]["1"], "\u{00c5} \u{00e9}");
    assert_eq!(out["meta"], v["meta"]);
}

fn remaining_entity_semantic_boundary_controls() {
    for visible_count in [0, 1] {
        let mut p = fixture::sample_parts(1182712).unwrap();
        for (i, palette) in p.palettes.iter_mut().enumerate() {
            let mut v = object::to_json(palette).unwrap();
            v["hidden"] = json!(i >= visible_count);
            *palette = object::from_json(&v).unwrap();
        }
        assert_eq!(Document::from_parts(p).is_ok(), visible_count > 0);
    }
    for kind in ["opened", "starter"] {
        let mut p = fixture::sample_parts(1182712).unwrap();
        let mut v = object::to_json(&p.library).unwrap();
        v["kind"] = json!(kind);
        p.library = object::from_json(&v).unwrap();
        assert!(Document::from_parts(p).is_err(), "project library {kind}");
    }
    let mut s = raw(ObjectType::Stroke);
    s["kind"] = json!("brush");
    s["lineStyle"] = Value::Null;
    s["closed"] = json!(true);
    let mut d = raw(ObjectType::Drawing);
    d["nodes"] =
        json!([{"id":"review_layer","type":"drawingNode","rev":1,"kind":"layer","hint":"brush","strokes":[s]}]);
    assert!(object::from_json::<Drawing>(&d).is_ok());
    d["nodes"][0]["strokes"][0]["closed"] = json!(false);
    assert!(object::from_json::<Drawing>(&d).is_err(), "nested brush outline must be closed");
}
