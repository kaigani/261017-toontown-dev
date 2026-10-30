//! Permanent adversarial C1.1 requirements. Failures are intentional, never ignored.
#![allow(clippy::unwrap_used, clippy::panic, reason = "adversarial test helpers")]
use serde_json::{Value, json};
use tt_model::document::{Document, RenderOptions};
use tt_model::entities::{
    asset::Asset,
    channel::Channel,
    drawing::{Drawing, DrawingNode, Stroke},
    element::Element,
    palette::Swatch,
    pen::Pen,
};
use tt_model::{IdGenerator, ObjectType, canonical, change, fixture, object, schema};

fn raw(ty: ObjectType) -> Value {
    let doc = fixture::sample_document(711).unwrap();
    for key in doc.entry_keys() {
        for o in change::own_objects(&doc, &key).unwrap() {
            if o.ty == ty {
                return doc.object_json(&o.key, RenderOptions { depth: None, path: false }).unwrap();
            }
        }
    }
    panic!("fixture lacks {ty}");
}

fn pen() -> Value {
    raw(ObjectType::Pen)
}

#[test]
fn duplicate_entry_ids_must_not_silently_overwrite_content() {
    let mut p = fixture::sample_parts(711).unwrap();
    let mut duplicate = object::to_json(&p.elements[0]).unwrap();
    duplicate["name"] = json!("Overwritten original");
    p.elements.push(object::from_json::<Element>(&duplicate).unwrap());
    assert!(Document::from_parts(p).is_err(), "ID-01: duplicate entry disappeared before validation");
}

#[test]
fn extension_and_metadata_names_are_never_interpreted() {
    let mut p = pen();
    let opaque = json!({"type":"element", "name":"  e\u{301}  ", "payload": [1,2,3]});
    p["ext"] = json!({"com.example.tool": opaque.clone()});
    p["meta"] = json!({"userObject": opaque.clone()});
    let result = object::to_json(&object::from_json::<Pen>(&p).unwrap()).unwrap();
    assert_eq!(result["ext"]["com.example.tool"], opaque, "10 §2.2: ext verbatim");
    assert_eq!(result["meta"]["userObject"], opaque, "10 §2.2: meta never interpreted");
}

#[test]
fn nfc_applies_to_persisted_strings_beyond_names() {
    let mut p = fixture::sample_parts(711).unwrap();
    let mut e = object::to_json(&p.elements[0]).unwrap();
    e["note"] = json!("e\u{301}");
    p.elements[0] = object::from_json::<Element>(&e).unwrap();
    assert_eq!(object::to_json(&p.elements[0]).unwrap()["note"], json!("é"), "10 §1.1 string");
}

#[test]
fn swatch_id_alias_is_accepted_by_the_validated_input_path() {
    let mut s = raw(ObjectType::Swatch);
    let id = s.as_object_mut().unwrap().shift_remove("colourId").unwrap();
    s["id"] = id;
    assert!(object::from_json::<Swatch>(&s).is_ok(), "10 §2.2 explicitly accepts id as alias");
}

#[test]
fn depth_zero_results_validate_against_their_object_schema() {
    let doc = fixture::sample_document(711).unwrap();
    let stub = doc.object_json(doc.project().id().as_str(), RenderOptions { depth: Some(0), path: false }).unwrap();
    assert!(schema::is_valid(ObjectType::Project, &stub), "10 §2.2 both full objects and stubs validate: {stub}");
}

#[test]
fn document_assembly_rechecks_schema_ranges_after_public_deserialize() {
    let mut p = fixture::sample_parts(711).unwrap();
    let mut project = object::to_json(&p.project).unwrap();
    project["pens"][0]["minSize"] = json!(101);
    project["pens"][0]["maxSize"] = json!(102);
    p.project = serde_json::from_value(project).unwrap();
    assert!(Document::from_parts(p).is_err(), "10 §4.2: Document accepted pen sizes > 100 via public Deserialize");
}

#[test]
fn image_element_bank_requires_exactly_one_bitmap_per_drawing() {
    let mut p = fixture::sample_parts(711).unwrap();
    let image = p.elements.iter().find(|e| e.element_type().as_str() == "image").unwrap();
    let bank = p.banks.iter().find(|b| Some(b.id()) == image.drawing_bank_id().as_ref()).unwrap();
    let drawing_id = bank.drawings().ids()[0].clone();
    let i = p.drawings.iter().position(|d| d.id() == &drawing_id).unwrap();
    let mut d = object::to_json(&p.drawings[i]).unwrap();
    d["nodes"] = json!([]);
    p.drawings[i] = object::from_json::<Drawing>(&d).unwrap();
    assert!(Document::from_parts(p).is_err(), "10 §4.10 empty image drawing accepted");
}

#[test]
fn linked_file_asset_requires_paths() {
    let mut a = raw(ObjectType::Asset);
    a["storage"] = json!("linked");
    a["entry"] = Value::Null;
    a["link"] = json!({"kind":"file","path":null,"absolutePath":null,"libraryId":null,"itemId":null,"versionId":null});
    assert!(object::from_json::<Asset>(&a).is_err(), "10 §4.16 linked file with no locator accepted");
}

#[test]
fn project_requires_a_visible_palette() {
    let mut p = fixture::sample_parts(711).unwrap();
    for palette in &mut p.palettes {
        let mut v = object::to_json(palette).unwrap();
        v["hidden"] = json!(true);
        *palette = object::from_json(&v).unwrap();
    }
    assert!(Document::from_parts(p).is_err(), "03 section 1.2 requires at least one visible palette");
}

#[test]
fn project_library_must_have_local_kind() {
    let mut p = fixture::sample_parts(711).unwrap();
    let mut v = object::to_json(&p.library).unwrap();
    v["kind"] = json!("global");
    p.library = object::from_json(&v).unwrap();
    assert!(Document::from_parts(p).is_err(), "10 sections 3 and 4.1 require the project-local library");
}

#[test]
fn brush_outline_must_be_closed() {
    let mut s = raw(ObjectType::Stroke);
    s["kind"] = json!("brush");
    s["closed"] = json!(false);
    s["lineStyle"] = Value::Null;
    assert!(object::from_json::<Stroke>(&s).is_err(), "10 §4.9: brush outlines are closed");
}

#[test]
fn fixed_text_requires_width() {
    let doc = fixture::sample_document(711).unwrap();
    let mut text = None;
    for k in doc.entry_keys() {
        for o in change::own_objects(&doc, &k).unwrap() {
            if o.ty == ObjectType::DrawingNode && o.json["kind"] == "text" {
                text = Some(doc.object_json(&o.key, RenderOptions { depth: None, path: false }).unwrap());
            }
        }
    }
    let mut n = text.unwrap();
    n["widthMode"] = json!("fixed");
    n.as_object_mut().unwrap().shift_remove("width");
    assert!(object::from_json::<DrawingNode>(&n).is_err(), "10 §4.9: fixed text without width");
}

#[test]
fn texture_paths_must_be_under_textures() {
    let mut s = raw(ObjectType::Swatch);
    s["kind"] = json!("texture");
    for v in s["values"].as_object_mut().unwrap().values_mut() {
        *v = json!({"image":"../outside.png","tile":true});
    }
    assert!(object::from_json::<Swatch>(&s).is_err(), "10 §4.13: texture entry outside textures/");
}

#[test]
fn canonical_map_order_does_not_depend_on_insertion_order() {
    let mut a = pen();
    let mut b = a.clone();
    a["meta"] = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
    b["meta"] = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
    assert_eq!(a, b);
    let a: Pen = object::from_json(&a).unwrap();
    let b: Pen = object::from_json(&b).unwrap();
    assert_eq!(
        object::to_canonical(&a).unwrap(),
        object::to_canonical(&b).unwrap(),
        "10 AG-06 identical documents identical JSON"
    );
}

#[test]
fn canonical_scene_map_order_is_not_document_state() {
    let mut a = object::to_json(&fixture::sample_parts(711).unwrap().scenes[0]).unwrap();
    a["paletteStyles"] = serde_json::from_str(r#"{"pal_a":"style_a","pal_b":"style_b"}"#).unwrap();
    let mut b = a.clone();
    b["paletteStyles"] = serde_json::from_str(r#"{"pal_b":"style_b","pal_a":"style_a"}"#).unwrap();
    assert_eq!(a, b);
    let a: tt_model::entities::scene::Scene = object::from_json(&a).unwrap();
    let b: tt_model::entities::scene::Scene = object::from_json(&b).unwrap();
    assert_eq!(object::to_canonical(&a).unwrap(), object::to_canonical(&b).unwrap());
}

#[test]
fn containment_and_scene_limit_controls() {
    for case in 0..5 {
        let mut p = fixture::sample_parts(711).unwrap();
        if case == 4 {
            p.document_rev = 0;
            assert!(Document::from_parts(p).is_err());
            continue;
        }
        let mut s = object::to_json(&p.scenes[0]).unwrap();
        match case {
            0 => s["tree"] = json!([]),
            1 => {
                let n = s["tree"][0].clone();
                s["tree"].as_array_mut().unwrap().push(n);
            }
            2 => {
                let n = s["tree"][0].clone();
                s["tree"][0]["children"] = json!([n]);
            }
            _ => s["tree"][0]["id"] = json!("dangling_element"),
        }
        if let Ok(scene) = object::from_json(&s) {
            p.scenes[0] = scene;
            assert!(Document::from_parts(p).is_err(), "accepted bad tree case {case}");
        }
    }
    let mut p = fixture::sample_parts(711).unwrap();
    let index = p.elements.iter().position(|e| e.element_type().as_str() == "peg").unwrap();
    let mut e = object::to_json(&p.elements[index]).unwrap();
    e["duration"] = json!(100001);
    if let Ok(element) = object::from_json(&e) {
        p.elements[index] = element;
        assert!(Document::from_parts(p).is_err());
    }
}

#[test]
fn metadata_depth_is_enforced_at_model_input() {
    let mut p = pen();
    let mut deep = json!(0);
    for _ in 0..140 {
        deep = json!({"next":deep});
    }
    p["meta"] = json!({"deep":deep});
    let parsed = object::from_json::<Pen>(&p);
    if let Ok(valid) = &parsed {
        assert!(object::to_canonical(valid).is_err(), "the repro must exceed the actual canonical writer limit");
    }
    assert!(parsed.is_err(), "accepted object cannot be written by its canonical writer");
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

#[test]
fn add_over_existing_object_property_is_invertible() {
    let a = json!({"a":1});
    let patch = vec![op("add", "/a", None, Some(json!(2)), Some(json!(1)))];
    let b = change::apply(&a, &patch).unwrap();
    // Amended by the builder in round 3 per the orchestrator's C11-R3-02 ruling (stateless `invert` removed from the
    // public API; `invert_against` is the only inversion, "update ... every caller"): the inverse is computed from the
    // pre-state. The assertion is unchanged.
    assert_eq!(
        change::apply(&b, &change::invert_against(&a, &patch).unwrap()).unwrap(),
        a,
        "RFC6902 add replaces existing property; oldValue must restore it"
    );
}

#[test]
fn move_over_existing_object_property_is_invertible() {
    let a = json!({"a":1,"b":2});
    let patch = vec![op("move", "/b", Some("/a"), None, Some(json!(2)))];
    let b = change::apply(&a, &patch).unwrap();
    // Amended by the builder in round 3 per the orchestrator's C11-R3-02 ruling (stateless `invert` removed from the
    // public API; `invert_against` is the only inversion, "update ... every caller"): the inverse is computed from the
    // pre-state. The assertion is unchanged.
    assert_eq!(
        change::apply(&b, &change::invert_against(&a, &patch).unwrap()).unwrap(),
        a,
        "10 §2.6 every patch invertible"
    );
}

#[test]
fn root_add_is_rfc6902_replacement() {
    let patch = vec![op("add", "", None, Some(json!([2])), Some(json!([1])))];
    assert_eq!(change::apply(&json!([1]), &patch).unwrap(), json!([2]));
}

#[test]
fn elided_patch_must_not_write_null() {
    let a = json!({"a":1});
    let mut p = op("replace", "/a", None, None, None);
    p.elided = true;
    assert!(change::apply(&a, &[p]).is_err(), "10 §2.6: elided value requires reread, never replacement with null");
}

#[test]
fn schema_integer_boundary_does_not_round_u64_max_up() {
    let mut p = pen();
    p["rev"] = serde_json::from_str("18446744073709551616").unwrap();
    assert!(!schema::is_valid(ObjectType::Pen, &p), "schema accepted 2^64 above u64::MAX");
}

#[test]
fn int_means_signed_32_bit_unless_spec_states_otherwise() {
    let mut ch = raw(ObjectType::Channel);
    assert_ne!(ch["name"], "position", "fixture must exercise a scalar channel");
    ch["keys"][0]["frame"] = json!(2147483648u64);
    ch["keys"].as_array_mut().unwrap().truncate(1);
    assert!(object::from_json::<Channel>(&ch).is_err(), "10 §1.1 int is signed 32-bit; frame is int");
}

#[test]
fn million_system_ids_unique_ordered_and_correctly_prefixed() {
    let mut g = IdGenerator::system();
    let mut previous = String::new();
    for _ in 0..1_000_000 {
        let id = g.next(ObjectType::Element).unwrap();
        let s = id.as_str();
        assert_eq!(s.len(), 29);
        assert!(s.starts_with("el_"));
        assert!(s[3..].bytes().all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b)));
        assert!(s > previous.as_str());
        previous = s.into();
    }
}

#[test]
fn cloning_a_system_generator_cannot_duplicate_ids() {
    let mut original = IdGenerator::system();
    for _ in 0..1024 {
        let _ = original.next(ObjectType::Element).unwrap();
        let mut cloned = original.clone();
        let a = original.next(ObjectType::Element).unwrap();
        let b = cloned.next(ObjectType::Element).unwrap();
        assert_ne!(a, b, "Clone duplicates the monotonic random counter within a millisecond");
    }
}

#[test]
fn hostile_path_and_id_controls() {
    for s in ["", "a/b/c", "../a", "é", "ｅｌ_1", "a\0b"] {
        assert!(tt_model::Id::parse(s).is_err());
    }
    // Opaque input IDs legitimately need not use the generated ULID format (ID-02).
    for s in ["lowercase", "wrongPrefix_000", "s42"] {
        assert!(tt_model::Id::parse(s).is_ok());
    }
    let long = vec!["element:\"A\""; 10_000].join("/");
    let _ = tt_model::Path::parse(&long);
    for s in ["scene:\"A\"/../element:\"B\"", "scene:#0", "scene:\"\\n\"", "scene:\"A\"//camera"] {
        assert!(tt_model::Path::parse(s).is_err(), "{s}");
    }
    for x in [-0.0, 1e21, 0.1 + 0.2, f64::MIN_POSITIVE, 5e-324] {
        let text = canonical::format_f64(x).unwrap();
        assert_eq!(text.parse::<f64>().unwrap(), x);
    }
    assert!(canonical::format_f64(f64::NAN).is_err());
    assert!(canonical::format_f64(f64::INFINITY).is_err());
}

#[test]
fn diff_does_not_treat_opaque_arrays_as_child_collections() {
    let a = json!({"meta":{"items":[{"id":"a","type":"pen"}]}});
    let b = json!({"meta":{"items":[{"id":"a","type":"scene"}]}});
    let patch = change::diff_json(&a, &b, "", ObjectType::Pen);
    assert_eq!(change::apply(&a, &patch).unwrap(), b, "metadata edits disappeared from generated patch");
}

#[test]
fn every_valid_deep_library_object_has_a_resolvable_canonical_path() {
    let mut parts = fixture::sample_parts(711).unwrap();
    let mut lib = object::to_json(&parts.library).unwrap();
    let mut node = json!({"id":"folder_leaf","type":"libraryFolder","rev":1,"name":"Leaf","children":[]});
    for i in 0..33 {
        node = json!({"id":format!("folder_{i}"),"type":"libraryFolder","rev":1,"name":format!("Folder {i}"),"children":[node]});
    }
    lib["children"] = json!([node]);
    parts.library = object::from_json(&lib).unwrap();
    let doc = Document::from_parts(parts).unwrap();
    let path = doc.canonical_path("folder_leaf").unwrap();
    assert!(doc.resolve_str(&path, &()).is_ok(), "unresolvable canonical path: {path}");
}

#[test]
fn child_stub_type_must_agree_with_the_containment_slot() {
    let mut p = raw(ObjectType::Project);
    p["palettes"] = json!([{"id":"p","type":"scene","name":"Wrong child"}]);
    assert!(!schema::is_valid(ObjectType::Project, &p), "palette slot schema accepted scene stub");
}

#[test]
fn schema_properties_have_format_introduction_annotations() {
    let s = schema::object_schema(ObjectType::Pen);
    assert_eq!(s["properties"]["minSize"]["x-since"], "1.0", "10 section 2.8 annotates properties, not only objects");
    assert_eq!(s["properties"]["type"]["readOnly"], true);
}

#[test]
fn property_schema_resolves_nested_child_and_tuple_fields() {
    assert!(
        schema::property_schema(ObjectType::Element, "/channels/rotation/keys/0/value").is_some(),
        "child stub/full union hides a real field"
    );
    let point = schema::property_schema(ObjectType::Stroke, "/points/0/0").unwrap();
    assert_eq!(point["type"], "number", "tuple prefixItems are addressable properties");
}

#[test]
fn schema_mutations_cover_all_object_types_and_root_properties() {
    let doc = fixture::sample_document(711).unwrap();
    let mut types = std::collections::BTreeSet::new();
    let mut mutations = 0;
    for key in doc.entry_keys() {
        for own in change::own_objects(&doc, &key).unwrap() {
            let v = doc.object_json(&own.key, RenderOptions { depth: None, path: false }).unwrap();
            assert!(schema::is_valid(own.ty, &v));
            types.insert(own.ty.as_str());
            let mut bad = v.clone();
            bad["unknownReviewerField"] = json!(1);
            assert!(!schema::is_valid(own.ty, &bad), "unknown key accepted for {}", own.ty);
            mutations += 1;
            let s = schema::object_schema(own.ty);
            for field in s["required"].as_array().unwrap() {
                let field = field.as_str().unwrap();
                let mut bad = v.clone();
                bad.as_object_mut().unwrap().shift_remove(field);
                assert!(!schema::is_valid(own.ty, &bad), "{} accepted missing {field}", own.ty);
                mutations += 1;
            }
            for (field, value) in v.as_object().unwrap() {
                if field == "meta" || field == "ext" || value.is_null() {
                    continue;
                }
                let mut bad = v.clone();
                bad[field] = if value.is_boolean() { json!("wrong") } else { json!(false) };
                assert!(!schema::is_valid(own.ty, &bad), "{} accepted wrong type at {field}", own.ty);
                mutations += 1;
            }
        }
    }
    assert_eq!(types.len(), 22);
    println!("review schema sweep: {} types, {mutations} mutations", types.len());
}

#[test]
fn ten_megabyte_drawing_validation_measurement() {
    let mut d = raw(ObjectType::Drawing);
    let base = raw(ObjectType::Stroke);
    let mut strokes = Vec::new();
    for i in 0..19_000 {
        let mut s = base.clone();
        s["id"] = json!(format!("s{i}"));
        strokes.push(s);
    }
    d["nodes"] = json!([{"id":"layer","type":"drawingNode","rev":1,"kind":"layer","hint":"brush","strokes":strokes}]);
    let bytes = serde_json::to_vec(&d).unwrap();
    let start = std::time::Instant::now();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap();
    let result = object::from_json::<Drawing>(&parsed).unwrap();
    assert_eq!(result.nodes().len(), 1);
    println!("review drawing validation: bytes={}, ms={:.3}", bytes.len(), start.elapsed().as_secs_f64() * 1000.0);
}

#[test]
fn jsonschema_integer_multiple_of_does_not_round_before_testing() {
    let s = json!({"type":"integer","multipleOf":5});
    // Ported by the builder to the jsonschema 0.59 API after the C11-R1-18 upgrade (`JSONSchema::options().compile`
    // no longer exists); the schema, the draft and both assertions are unchanged.
    let validator = jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&s).unwrap();
    assert!(validator.is_valid(&json!(9007199254740995u64)), "exact integer divisible by 5 rejected");
    assert!(!validator.is_valid(&json!(9007199254740999u64)), "integer not divisible by 5 accepted");
}

#[test]
fn canonical_serialization_rejects_nonfinite_values() {
    assert!(canonical::to_canonical(&f64::NAN).is_err());
}

#[test]
fn mouth_track_manual_ranges_stay_within_the_placement() {
    let mut s = raw(ObjectType::SoundPlacement);
    assert!(s["mouthTrack"].is_object());
    s["mouthTrack"]["manual"] = json!([{"from":2,"to":1}]);
    assert!(object::from_json::<tt_model::entities::sound::SoundPlacement>(&s).is_err());
}

#[test]
fn spline_handle_offsets_are_clamped_to_adjacent_key_times() {
    let mut ch = raw(ObjectType::Channel);
    ch["curveType"] = json!("spline");
    ch["keys"][0]["handles"]["right"]["dt"] = json!(1_000_000);
    let result = object::from_json::<Channel>(&ch);
    if let Ok(value) = result {
        assert!(
            object::to_json(&value).unwrap()["keys"][0]["handles"]["right"]["dt"].as_f64().unwrap() < 1_000_000.0,
            "10 section 4.12 and 05 section 10.3 require handle time clamping"
        );
    }
}

#[test]
fn nested_closed_value_objects_reject_unknown_fields() {
    fn walk(
        s: &Value,
        v: &Value,
        path: &str,
        defs: &serde_json::Map<String, Value>,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        if !v.is_object() && !v.is_array() {
            return;
        }
        if let Some(r) = s["$ref"].as_str().and_then(|r| r.strip_prefix("#/$defs/")) {
            walk(&defs[r], v, path, defs, out);
            return;
        }
        if s["additionalProperties"] == false && v.is_object() {
            out.insert(path.to_owned());
        }
        if let Some(props) = s["properties"].as_object() {
            for (k, sub) in props {
                if let Some(child) = v.get(k) {
                    walk(sub, child, &format!("{path}/{}", tt_model::pointer::escape(k)), defs, out);
                }
            }
        }
        if let Some(items) = v.as_array() {
            for (i, child) in items.iter().enumerate() {
                let sub = s["prefixItems"].get(i).unwrap_or(&s["items"]);
                walk(sub, child, &format!("{path}/{i}"), defs, out);
            }
        }
        if s["additionalProperties"].is_object()
            && let Some(m) = v.as_object()
        {
            for (k, child) in m {
                walk(&s["additionalProperties"], child, &format!("{path}/{}", tt_model::pointer::escape(k)), defs, out);
            }
        }
        for k in ["anyOf", "oneOf", "allOf"] {
            if let Some(alts) = s[k].as_array() {
                for sub in alts {
                    walk(sub, v, path, defs, out);
                }
            }
        }
        if let Some(sub) = s.get("then") {
            walk(sub, v, path, defs, out);
        }
    }
    let doc = fixture::sample_document(711).unwrap();
    let mut count = 0;
    for key in doc.entry_keys() {
        let v = doc.entry_raw_json(&key).unwrap();
        let ty = ObjectType::parse(v["type"].as_str().unwrap()).unwrap();
        let mut pointers = std::collections::BTreeSet::new();
        walk(&schema::object_schema(ty), &v, "", schema::definitions(), &mut pointers);
        for pointer in pointers {
            let mut bad = v.clone();
            bad.pointer_mut(&pointer).unwrap()["unknownReviewerField"] = json!(1);
            assert!(!schema::is_valid(ty, &bad), "{ty} accepted nested unknown at {pointer}");
            count += 1;
        }
    }
    println!("review nested-schema sweep: {count} closed-object mutations");
    assert!(count > 100);
}

#[test]
fn derived_properties_are_output_only_on_validated_entity_input() {
    fn check<T: tt_model::entities::ModelObject + tt_model::validate::Check>(v: Value) -> usize {
        assert!(object::from_json::<T>(&v).is_ok(), "control input for {}", T::TYPE);
        let schema = schema::object_schema(T::TYPE);
        let mut count = 0;
        for (field, def) in schema["properties"].as_object().unwrap() {
            if def["x-derived"] != true {
                continue;
            }
            let mut found = false;
            for candidate in
                [Value::Null, json!(false), json!(0), json!(1), json!("x"), json!("#000000FF"), json!([]), json!({})]
            {
                let mut input = v.clone();
                input[field] = candidate;
                if !schema::is_valid(T::TYPE, &input) {
                    continue;
                }
                assert!(object::from_json::<T>(&input).is_err(), "{} accepted derived {field}", T::TYPE);
                found = true;
                count += 1;
                break;
            }
            assert!(found, "test needs a valid output value for {}.{field}", T::TYPE);
        }
        count
    }
    use tt_model::entities::{
        asset, channel, drawing, effect, element, library, palette, pen, project, scene, sound, transition,
    };
    let p = fixture::sample_parts(711).unwrap();
    let mut n = 0;
    n += check::<project::Project>(object::to_json(&p.project).unwrap());
    n += check::<scene::Scene>(object::to_json(&p.scenes[0]).unwrap());
    n += check::<drawing::DrawingBank>(object::to_json(&p.banks[0]).unwrap());
    macro_rules! one {
        ($t:ty,$kind:ident) => {
            n += check::<$t>(raw(ObjectType::$kind));
        };
    }
    one!(palette::Palette, Palette);
    one!(palette::PaletteStyle, PaletteStyle);
    one!(palette::Swatch, Swatch);
    one!(pen::Pen, Pen);
    one!(asset::Asset, Asset);
    one!(drawing::Drawing, Drawing);
    one!(drawing::DrawingNode, DrawingNode);
    one!(drawing::Stroke, Stroke);
    one!(transition::Transition, Transition);
    one!(element::Element, Element);
    one!(channel::Channel, Channel);
    one!(channel::Key, Key);
    one!(channel::MotionPoint, MotionPoint);
    one!(effect::ParticleBlock, ParticleBlock);
    one!(sound::SoundPlacement, SoundPlacement);
    one!(sound::MediaSequence, MediaSequence);
    one!(library::Library, Library);
    one!(library::LibraryFolder, LibraryFolder);
    one!(library::LibraryItem, LibraryItem);
    println!("review derived-input sweep: 22 types, {n} output-only properties");
    assert!(n > 10);
}
