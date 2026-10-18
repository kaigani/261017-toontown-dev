//! C1.1 conformance: schema conformance of every object and entry (10 §2.8), byte-identical round trips (10 JSON-05,
//! 11 §14.3 at the model level), golden canonical entries, path resolution (10 §2.4), change sets (10 §2.6).
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers outside #[test] functions")]

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::{Value, json};
use tt_model::change::{self, OwnChange};
use tt_model::document::{Document, DocumentParts, EntryKey, RenderOptions, Target};
use tt_model::entities::drawing::{Drawing, DrawingBank};
use tt_model::entities::element::Element;
use tt_model::entities::library::Library;
use tt_model::entities::palette::Palette;
use tt_model::entities::project::Project;
use tt_model::entities::scene::Scene;
use tt_model::fixture::{sample_document, sample_parts};
use tt_model::object::from_json;
use tt_model::{ObjectType, canonical, schema};

fn doc() -> Document {
    sample_document(1).unwrap()
}

fn all_keys(doc: &Document) -> Vec<(ObjectType, String)> {
    let mut out = Vec::new();
    for key in doc.entry_keys() {
        for o in change::own_objects(doc, &key).unwrap() {
            out.push((o.ty, o.key));
        }
    }
    out
}

#[test]
fn fixture_covers_every_object_and_element_type() {
    let d = doc();
    let types: BTreeSet<ObjectType> = all_keys(&d).into_iter().map(|(t, _)| t).collect();
    for t in ObjectType::ALL {
        assert!(types.contains(&t), "fixture lacks a {t}");
    }
    let element_types: BTreeSet<_> = d.elements().map(|e| *e.element_type()).collect();
    for t in tt_model::entities::element::ElementType::ALL {
        assert!(element_types.contains(&t), "fixture lacks a {} element", t.as_str());
    }
    assert!(d.issues().is_empty(), "{:?}", d.issues());
}

#[test]
fn every_object_validates_at_every_depth() {
    let d = doc();
    let keys = all_keys(&d);
    assert!(keys.len() > 80, "{}", keys.len());
    for (ty, key) in &keys {
        for depth in [Some(1), Some(2), None] {
            let v = d.object_json(key, RenderOptions { depth, path: true }).unwrap();
            let errs = schema::validate(*ty, &v, 10).unwrap();
            assert!(errs.is_empty(), "{ty} {key} depth {depth:?}: {errs:?}\n{v:#}");
            assert!(v.get("path").is_some(), "{key} has no path");
        }
        // depth 0 is a stub.
        let stub = d.object_json(key, RenderOptions { depth: Some(0), path: true }).unwrap();
        assert!(stub.get("rev").is_none());
    }
}

#[test]
fn every_entry_validates_and_has_no_query_only_properties() {
    let d = doc();
    for key in d.entry_keys() {
        let v = d.entry_json(&key).unwrap();
        let ty = ObjectType::parse(v["type"].as_str().unwrap()).unwrap();
        assert!(schema::validate(ty, &v, 10).unwrap().is_empty(), "{key:?}");
        assert!(!has_object_key(&v, "path"), "{key:?} carries a transient path");
    }
}

/// DM-1: the persisted JSON of an object equals its query JSON at the split depth minus transient/derived fields.
#[test]
fn persisted_json_equals_query_json_minus_transient() {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.shift_remove("path");
                m.values_mut().for_each(strip);
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let d = doc();
    // Palettes, drawings and elements have no split children: full-depth query JSON = entry JSON.
    for key in d.entry_keys() {
        if matches!(key, EntryKey::Palette(_) | EntryKey::Drawing(_) | EntryKey::Element(_)) {
            let entry = d.entry_json(&key).unwrap();
            let id = entry.get("id").and_then(Value::as_str).unwrap().to_owned();
            let mut query = d.object_json(&id, RenderOptions { depth: None, path: true }).unwrap();
            strip(&mut query);
            assert_eq!(query, entry, "{key:?}");
        }
    }
}

/// Whether any embedded model object (a JSON object with an object-type `type`) has the key.
fn has_object_key(v: &Value, key: &str) -> bool {
    match v {
        Value::Object(m) => {
            let is_object = m.get("type").and_then(Value::as_str).and_then(ObjectType::parse).is_some();
            (is_object && m.contains_key(key)) || m.values().any(|x| has_object_key(x, key))
        }
        Value::Array(a) => a.iter().any(|x| has_object_key(x, key)),
        _ => false,
    }
}

fn reparse(d: &Document) -> Document {
    let mut parts = sample_parts(1).unwrap();
    let parse_entry = |k: &EntryKey| -> Value { serde_json::from_str(&d.entry_canonical(k).unwrap()).unwrap() };
    parts.project = from_json::<Project>(&parse_entry(&EntryKey::Project)).unwrap();
    parts.library = from_json::<Library>(&parse_entry(&EntryKey::Library)).unwrap();
    parts.palettes =
        d.palettes().map(|p| from_json::<Palette>(&parse_entry(&EntryKey::Palette(p.id().clone()))).unwrap()).collect();
    parts.banks =
        d.banks().map(|b| from_json::<DrawingBank>(&parse_entry(&EntryKey::Bank(b.id().clone()))).unwrap()).collect();
    parts.drawings =
        d.drawings().map(|x| from_json::<Drawing>(&parse_entry(&EntryKey::Drawing(x.id().clone()))).unwrap()).collect();
    parts.scenes =
        d.scenes().map(|s| from_json::<Scene>(&parse_entry(&EntryKey::Scene(s.id().clone()))).unwrap()).collect();
    parts.elements =
        d.elements().map(|e| from_json::<Element>(&parse_entry(&EntryKey::Element(e.id().clone()))).unwrap()).collect();
    Document::from_parts(parts).unwrap()
}

#[test]
fn canonical_round_trip_is_byte_identical() {
    let a = doc();
    let b = reparse(&a);
    let c = reparse(&b);
    for key in a.entry_keys() {
        let x = a.entry_canonical(&key).unwrap();
        assert_eq!(x, b.entry_canonical(&key).unwrap(), "{key:?}");
        assert_eq!(x, c.entry_canonical(&key).unwrap(), "{key:?}");
        assert!(x.ends_with('\n') && !x.contains('\r') && !x.starts_with('\u{feff}'));
    }
    // JSON-05: saving the same document twice is byte-identical; a fresh build with the same seed is too.
    let again = doc();
    for key in a.entry_keys() {
        assert_eq!(a.entry_canonical(&key).unwrap(), again.entry_canonical(&key).unwrap());
    }
}

fn entry_file_name(d: &Document, key: &EntryKey) -> String {
    match key {
        EntryKey::Project => "project.json".into(),
        EntryKey::Library => "library.json".into(),
        EntryKey::Palette(id) => format!("palettes/{}.json", d.palette(id).unwrap().name()),
        EntryKey::Bank(id) => format!("banks/{id}.json"),
        EntryKey::Drawing(id) => format!("drawings/{id}.json"),
        EntryKey::Scene(id) => format!("scenes/{}.json", d.scene(id).unwrap().name()),
        EntryKey::Element(id) => {
            let scene = d.scene(d.scene_of(id).unwrap()).unwrap().name();
            format!("elements/{scene}/{}.json", d.element(id).unwrap().name())
        }
    }
}

/// Golden canonical entries of the sample project (`TT_UPDATE_GOLDEN=1` rewrites them after an intended change).
#[test]
fn golden_canonical_entries() {
    let d = doc();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/sample");
    let update = std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let mut expected_files = BTreeSet::new();
    for key in d.entry_keys() {
        let name = entry_file_name(&d, &key);
        expected_files.insert(name.clone());
        let path = dir.join(&name);
        let text = d.entry_canonical(&key).unwrap();
        if update {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        } else {
            let golden = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e} (run with TT_UPDATE_GOLDEN=1)", path.display()));
            assert_eq!(text, golden, "{name} differs from its golden");
        }
    }
    // No stale goldens.
    let mut found = BTreeSet::new();
    let mut subs: Vec<String> =
        ["", "palettes", "banks", "drawings", "scenes"].iter().map(|s| (*s).to_owned()).collect();
    if let Ok(rd) = std::fs::read_dir(dir.join("elements")) {
        subs.extend(
            rd.flatten().filter(|e| e.path().is_dir()).map(|e| format!("elements/{}", e.file_name().to_string_lossy())),
        );
    }
    for sub in &subs {
        if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
            for e in rd.flatten() {
                if e.path().is_file() {
                    let rel = if sub.is_empty() {
                        e.file_name().to_string_lossy().into_owned()
                    } else {
                        format!("{sub}/{}", e.file_name().to_string_lossy())
                    };
                    found.insert(rel);
                }
            }
        }
    }
    assert_eq!(found, expected_files);
}

#[test]
fn spec_example_paths_resolve() {
    let d = doc();
    let r = |s: &str| d.resolve_str(s, &()).unwrap_or_else(|e| panic!("{s}: {}", e.to_wire()));
    let key = |s: &str| match r(s) {
        Target::Object { key, .. } => key,
        Target::Cell { .. } => panic!("cell"),
    };
    assert_eq!(key("project"), d.project().id().as_str());
    let bob = d.elements().find(|e| e.name() == "Bob").unwrap().id().as_str().to_owned();
    assert_eq!(key("scene:\"Intro\"/element:\"Bob\""), bob);
    assert_eq!(key("scene:\"intro\"/element:\"bob\""), bob, "case-insensitive fallback");
    assert_eq!(key("project/scene:\"Intro\"/element:\"Bob\""), bob);
    let d3 = key("scene:\"Intro\"/element:\"Bob\"/drawing:\"3\"".replace('3', "10").as_str());
    assert_eq!(d.drawing(&tt_model::Id::parse(&d3).unwrap()).unwrap().name(), "10");
    assert_eq!(key("scene:\"Intro\"/element:\"Bob\"/drawing:#3"), d3, "natural order: 1, 2, 10");
    assert!(matches!(r("palette:\"Main\"/swatch:\"Skin\""), Target::Object { ty: ObjectType::Swatch, .. }));
    assert!(matches!(r("palette:\"Main\"/style:\"Night\""), Target::Object { ty: ObjectType::PaletteStyle, .. }));
    assert!(matches!(
        r("scene:\"Intro\"/element:\"Bob-P\"/channel:\"skew\"/key:\"48\""),
        Target::Object { ty: ObjectType::Key, .. }
    ));
    assert!(matches!(
        r("scene:\"Intro\"/element:\"Bob\"/channel:\"position\"/key:\"24\""),
        Target::Object { ty: ObjectType::MotionPoint, .. }
    ));
    assert!(matches!(
        r("scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"/layer:#1/stroke:#2"),
        Target::Object { ty: ObjectType::Stroke, .. }
    ));
    assert!(matches!(
        r("scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"/layer:\"Body\""),
        Target::Object { ty: ObjectType::DrawingNode, .. }
    ));
    assert_eq!(
        r("scene:\"Intro\"/element:\"Bob\"/cell:#12"),
        Target::Cell { element: tt_model::Id::parse(&bob).unwrap(), frame: 12 }
    );
    assert!(matches!(r("pen:\"Pen 1\""), Target::Object { ty: ObjectType::Pen, .. }));
    assert!(matches!(r("scene:\"Intro\"/camera"), Target::Object { ty: ObjectType::Element, .. }));
    assert!(matches!(r("scene:\"Intro\"/transition"), Target::Object { ty: ObjectType::Transition, .. }));
    assert!(matches!(r("transition"), Target::Object { ty: ObjectType::Transition, .. }));
    assert!(matches!(
        r("library:local/folder:\"props\"/template:\"WALK\""),
        Target::Object { ty: ObjectType::LibraryItem, .. }
    ));
    assert!(matches!(
        r("scene:\"Intro\"/element:\"Dialogue\"/placement:#1"),
        Target::Object { ty: ObjectType::SoundPlacement, .. }
    ));
    // A key referenced by its compound ID.
    let k = key("scene:\"Intro\"/element:\"Bob-P\"/channel:\"skew\"/key:\"1\"");
    assert!(k.contains('/'));
    assert_eq!(key(&k), k);
}

#[test]
fn reference_errors_carry_the_spec_codes() {
    let d = doc();
    let code = |s: &str| d.resolve_str(s, &()).unwrap_err().code();
    assert_eq!(code("scene:\"Nope\""), "ref.notFound");
    assert_eq!(code("el_00000000000000000000000000"), "ref.notFound");
    assert_eq!(code("scene:\"Intro\"/element:\"Bob\"/cell:#0"), "arg.invalid");
    assert_eq!(code("scene:\"Intro\"/element:\"Camera\"/cell:#3"), "ref.wrongType");
    assert_eq!(code("library:global"), "ref.notFound");
    assert_eq!(code("@current/element:\"Bob\""), "ref.notFound");
    let bob = d.elements().find(|e| e.name() == "Bob").unwrap().id().as_str().to_owned();
    assert_eq!(d.resolve_as(&bob, ObjectType::Scene, &()).unwrap_err().code(), "ref.wrongType");
    assert_eq!(code(&format!("scene:@{bob}")), "ref.wrongType");
    assert_eq!(code("scene:\"Intro\"/drawing:\"1\""), "ref.notFound");
}

#[test]
fn ambiguous_names_list_candidates() {
    let mut parts = sample_parts(1).unwrap();
    // Two swatches named "Skin" (names may repeat in a palette, 03 §1.4).
    let pal = &mut parts.palettes[0];
    let mut v = serde_json::to_value(&*pal).unwrap();
    let mut extra = v["swatches"][0].clone();
    extra["colourId"] = json!("col_extra");
    v["swatches"].as_array_mut().unwrap().push(extra);
    *pal = from_json::<Palette>(&v).unwrap();
    let d = Document::from_parts(parts).unwrap();
    let e = d.resolve_str("palette:\"Main\"/swatch:\"Skin\"", &()).unwrap_err();
    assert_eq!(e.code(), "ref.ambiguous");
    let candidates = e.details()["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    for c in candidates {
        let path = c["path"].as_str().unwrap();
        assert!(path.contains("swatch:@"), "ambiguous names make canonical paths use @id: {path}");
        assert!(d.resolve_str(path, &()).is_ok());
    }
}

/// Every object's canonical path resolves back to the same object (10 §2.4).
#[test]
fn canonical_paths_resolve_to_their_objects() {
    let d = doc();
    for (ty, key) in all_keys(&d) {
        let path = d.canonical_path(&key).unwrap();
        match d.resolve_str(&path, &()) {
            Ok(Target::Object { ty: t, key: k }) => {
                assert_eq!((t, k.as_str()), (ty, key.as_str()), "{path}");
            }
            other => panic!("{path} → {other:?}"),
        }
    }
}

fn modified_parts(base: &DocumentParts) -> DocumentParts {
    let mut parts = base.clone();
    let edit = |v: &mut Value| {
        // Rename Bob, change exposure in the middle, add a key, remove the note, add ext data.
        v["name"] = json!("Robert");
        let runs = v["exposure"].clone();
        let mut runs: Vec<Value> = runs.as_array().unwrap().clone();
        runs[2] = json!([runs[2][0].clone(), 10]);
        runs.insert(3, json!([null, 2]));
        v["exposure"] = Value::Array(runs);
        v.as_object_mut().unwrap().shift_remove("note");
        v["ext"] = json!({"com.example.tool": {"x": 1}});
        let keys = v["channels"]["rotation"]["keys"].as_array_mut().unwrap();
        let mut k = keys[0].clone();
        k["id"] = json!("ky_new");
        k["frame"] = json!(30);
        keys.push(k);
    };
    let bob = parts.elements.iter().position(|e| e.name() == "Bob").unwrap();
    let mut v = serde_json::to_value(&parts.elements[bob]).unwrap();
    edit(&mut v);
    parts.elements[bob] = from_json::<Element>(&v).unwrap();
    // Reorder scenes and delete a swatch.
    let mut p = serde_json::to_value(&parts.project).unwrap();
    p["scenes"].as_array_mut().unwrap().reverse();
    parts.project = from_json::<Project>(&p).unwrap();
    let mut pal = serde_json::to_value(&parts.palettes[0]).unwrap();
    pal["swatches"].as_array_mut().unwrap().remove(4);
    parts.palettes[0] = from_json::<Palette>(&pal).unwrap();
    // The pen used the deleted swatch's colour: unbind it.
    let mut p = serde_json::to_value(&parts.project).unwrap();
    for pen in p["pens"].as_array_mut().unwrap() {
        pen["colourId"] = Value::Null;
    }
    parts.project = from_json::<Project>(&p).unwrap();
    parts
}

#[test]
fn change_sets_describe_and_invert_edits() {
    let base = sample_parts(1).unwrap();
    let a = Document::from_parts(base.clone()).unwrap();
    let b = Document::assemble(modified_parts(&base));
    let changes = change::diff(&a, &b).unwrap();
    let kinds: Vec<(ObjectType, &str)> = changes
        .iter()
        .map(|(c, o)| {
            (
                o.ty,
                match c {
                    OwnChange::Created => "created",
                    OwnChange::Modified(_) => "modified",
                    OwnChange::Deleted => "deleted",
                },
            )
        })
        .collect();
    assert!(kinds.contains(&(ObjectType::Element, "modified")));
    assert!(kinds.contains(&(ObjectType::Key, "created")));
    assert!(kinds.contains(&(ObjectType::Swatch, "deleted")));
    assert!(kinds.contains(&(ObjectType::Palette, "modified")), "membership of swatches changed");
    assert!(kinds.contains(&(ObjectType::Channel, "modified")), "membership of keys changed");
    assert!(kinds.contains(&(ObjectType::Project, "modified")), "scene order changed");
    // Unchanged entries contribute nothing: the second palette is untouched.
    assert!(
        !changes
            .iter()
            .any(|(_, o)| o.ty == ObjectType::Palette && o.key == a.palettes().nth(1).unwrap().id().as_str())
    );
    let old: std::collections::HashMap<String, Value> =
        a.entry_keys().iter().flat_map(|k| change::own_objects(&a, k).unwrap()).map(|o| (o.key, o.json)).collect();
    for (c, o) in &changes {
        if let OwnChange::Modified(patch) = c {
            let before = &old[&o.key];
            assert_eq!(&change::apply(before, patch).unwrap(), &o.json, "{}: {patch:?}", o.key);
            assert_eq!(&change::apply(&o.json, &change::invert(patch)).unwrap(), before, "{} inverse", o.key);
            for op in patch {
                assert!(op.old_value.is_some() || op.op == "add" || op.op == "move", "{op:?} lacks oldValue");
            }
        }
    }
    // The element's exposure change is a range patch; the name change a plain replace with oldValue.
    let (_, bob) = changes.iter().find(|(_, o)| o.ty == ObjectType::Element).unwrap();
    let OwnChange::Modified(patch) = &changes.iter().find(|(_, o)| o.key == bob.key).unwrap().0 else { panic!() };
    let exp = patch.iter().find(|p| p.path == "/exposure").unwrap();
    assert!(exp.range.is_some(), "{exp:?}");
    assert_eq!(exp.range.unwrap().from, 19);
    let name = patch.iter().find(|p| p.path == "/name").unwrap();
    assert_eq!(
        (name.op, name.old_value.clone(), name.value.clone()),
        ("replace", Some(json!("Bob")), Some(json!("Robert")))
    );
    let wire = serde_json::to_value(OwnChange::Modified(patch.clone()).to_change(bob, 7)).unwrap();
    assert_eq!(wire["rev"], 7);
    assert_eq!(wire["type"], "element");
}

#[test]
fn x_command_annotations_name_registered_commands() {
    let registry =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/A_command_registry.md"))
            .unwrap();
    let mut seen = BTreeSet::new();
    fn walk(v: &Value, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) => {
                if let Some(Value::String(c)) = m.get("x-command") {
                    out.insert(c.clone());
                }
                m.values().for_each(|x| walk(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(&schema::bundle(), &mut seen);
    assert!(seen.len() > 10);
    for c in seen {
        assert!(registry.contains(&format!("`{c}`")), "x-command `{c}` is not in A_command_registry.md");
    }
}

/// The published schema file equals the generated bundle (`TT_UPDATE_GOLDEN=1` rewrites it).
#[test]
fn published_schema_bundle_is_current() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("schema/model-{}.schema.json", tt_model::FORMAT_VERSION));
    let text = canonical::to_canonical_string(&schema::bundle()).unwrap();
    if std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(committed == text, "{} is stale; run `TT_UPDATE_GOLDEN=1 cargo test -p tt-model`", path.display());
}

#[test]
fn schemas_are_valid_draft_2020_12() {
    for t in ObjectType::ALL {
        // Compiling under draft 2020-12 checks each schema; a defect surfaces as `internal` here.
        assert!(schema::validate(t, &json!({}), 1).is_ok(), "{t}");
    }
    assert_eq!(schema::property_schema(ObjectType::Element, "/transform/skew").unwrap()["maximum"], 89.0);
    assert_eq!(
        schema::property_schema(ObjectType::Project, "/frameRate").unwrap()["x-command"],
        "project.properties.set"
    );
}

/// REV-01/REV-02: no object may carry a revision newer than the document's `documentRev`.
#[test]
fn revisions_newer_than_the_document_are_integrity_errors() {
    let mut parts = sample_parts(1).unwrap();
    let bob = parts.elements.iter().position(|e| e.name() == "Bob").unwrap();
    let mut v = serde_json::to_value(&parts.elements[bob]).unwrap();
    v["channels"]["rotation"]["keys"][0]["rev"] = json!(99);
    parts.elements[bob] = from_json::<Element>(&v).unwrap();
    let errors = Document::from_parts(parts).unwrap_err();
    assert!(errors.iter().any(|e| e.details()["pointer"] == "/rev" && e.message().contains("rev 99")), "{errors:?}");
}
