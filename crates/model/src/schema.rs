//! JSON Schemas of the object model (10 §2.7–§2.8; ADR-0011).
//!
//! Schemas are generated with `schemars` from the same Rust types the application deserialises and validates, then
//! completed by a deterministic post-processing pass:
//!
//! - **Presence.** `required` lists every property except those marked `x-omitDefault` (10 JSON-04) or
//!   `x-kindSpecific`; kind-specific properties are absent (never `null`) when they do not apply, and one `if/then`
//!   rule per `elementType`, drawing-node `kind`, stroke `kind`, channel kind and library-item kind states which
//!   apply (10 §4.6, §4.9, §4.12, §4.17).
//! - **Integer ranges.** Every integer gets explicit bounds of its Rust type (10 §1.1: `int` is 32-bit unless
//!   stated; revisions and byte sizes are 64-bit and travel losslessly, ADR-0012).
//! - **Names.** Every `name` gets the pattern and `maxLength` of its entity (10 §1.3).
//! - **Markers.** `readOnly`, `x-unit`, `x-command`, `x-ref` come from field attributes; derived (`x-derived`) and
//!   transient (`x-transient`) query-only properties are added per type (10 §2.7); every object schema carries
//!   `x-since` (the format version that introduced it) and `x-objectType`.
//!
//! The result is published three ways: [`object_schema`] (`doc.schema.get`), [`bundle`] (the file
//! `crates/model/schema/model-<version>.schema.json`, kept in sync by a test) and the generated front-end module
//! `desktop/src/ipc/generated/` (C1.1 readiness bullet 1).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, SchemaGenerator};
use serde_json::{Map, Value, json};

use crate::entities::effect::{
    BlurParams, ClippingParams, ColourTransformParams, DropShadowParams, ParticleParams, ShakeParams,
};
use crate::entities::element::{ElementType, PivotSet};
use crate::entities::{
    asset, channel, drawing, effect, element, library, palette, pen, project, scene, sound, transition,
};
use crate::error::{ModelError, codes};
use crate::name::NameRule;
use crate::types::ObjectType;

/// The JSON Schema dialect.
pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// Base of schema `$id`s.
pub const ID_BASE: &str = "https://schemas.toontown.dev/model";

/// The schema definition name of an object type (PascalCase of the type name).
#[must_use]
pub fn def_name(ty: ObjectType) -> String {
    let s = ty.as_str();
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    if let Some(c) = chars.next() {
        out.push(c.to_ascii_uppercase());
    }
    out.extend(chars);
    out
}

fn register<T: JsonSchema>(g: &mut SchemaGenerator) {
    let _ = g.subschema_for::<T>();
}

fn generate_raw() -> Map<String, Value> {
    let mut g = SchemaSettings::draft2020_12().into_generator();
    register::<project::Project>(&mut g);
    register::<palette::Palette>(&mut g);
    register::<palette::PaletteStyle>(&mut g);
    register::<palette::Swatch>(&mut g);
    register::<pen::Pen>(&mut g);
    register::<asset::Asset>(&mut g);
    register::<drawing::DrawingBank>(&mut g);
    register::<drawing::Drawing>(&mut g);
    register::<drawing::DrawingNode>(&mut g);
    register::<drawing::Stroke>(&mut g);
    register::<scene::Scene>(&mut g);
    register::<transition::Transition>(&mut g);
    register::<element::Element>(&mut g);
    register::<channel::Channel>(&mut g);
    register::<channel::Key>(&mut g);
    register::<channel::MotionPoint>(&mut g);
    register::<effect::ParticleBlock>(&mut g);
    register::<sound::SoundPlacement>(&mut g);
    register::<sound::MediaSequence>(&mut g);
    register::<library::Library>(&mut g);
    register::<library::LibraryFolder>(&mut g);
    register::<library::LibraryItem>(&mut g);
    register::<ColourTransformParams>(&mut g);
    register::<ClippingParams>(&mut g);
    register::<DropShadowParams>(&mut g);
    register::<BlurParams>(&mut g);
    register::<ShakeParams>(&mut g);
    register::<ParticleParams>(&mut g);
    register::<ObjectType>(&mut g);
    register::<crate::id::Id>(&mut g);
    g.take_definitions(true)
}

/// Integer bounds by `format` (schemars annotates Rust integer types with these formats).
fn int_bounds(format: &str) -> Option<(Value, Value)> {
    Some(match format {
        "uint8" => (json!(0), json!(u8::MAX)),
        "uint16" => (json!(0), json!(u16::MAX)),
        // 10 §1.1: `int` is the 32-bit *signed* range unless a section states otherwise; unsigned Rust fields
        // (frames, durations, counts) take its non-negative part. Stated exceptions carry their own `maximum`
        // (particle seed: 0–2^32−1, 06 §3.8.3) or a 64-bit type (revisions, byte sizes: 10 §5 entries over 4 GiB).
        "uint32" => (json!(0), json!(i32::MAX)),
        "uint64" => (json!(0), json!(u64::MAX)),
        "int8" => (json!(i8::MIN), json!(i8::MAX)),
        "int16" => (json!(i16::MIN), json!(i16::MAX)),
        "int32" => (json!(i32::MIN), json!(i32::MAX)),
        "int64" => (json!(i64::MIN), json!(i64::MAX)),
        _ => return None,
    })
}

/// Recursively: integer bounds from `format`, and `format` removed (it is a Rust-type annotation, not a JSON Schema
/// format the validators should interpret).
fn fix_integers(v: &mut Value) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(f)) = m.get("format").cloned() {
                if let Some((lo, hi)) = int_bounds(&f) {
                    m.entry("minimum").or_insert(lo);
                    m.entry("maximum").or_insert(hi);
                    m.remove("format");
                } else if f == "double" || f == "float" {
                    m.remove("format");
                }
            }
            m.values_mut().for_each(fix_integers);
        }
        Value::Array(a) => a.iter_mut().for_each(fix_integers),
        _ => {}
    }
}

/// Applies the model's integer bounds to any generated schema (the `format` of Rust integer types becomes
/// `minimum`/`maximum`, `int` being signed 32-bit per 10 §1.1). Used for command parameter and result schemas, which
/// keep serde's own `required` set (optional and defaulted parameters are not required).
pub fn apply_integer_bounds(v: &mut Value) {
    fix_integers(v);
}

/// Removes the `null` alternative of a property schema (`type: [T, "null"]` or `anyOf: [S, {type: null}]`).
fn strip_null(p: &mut Value) {
    let Some(m) = p.as_object_mut() else { return };
    if let Some(Value::Array(types)) = m.get_mut("type") {
        types.retain(|t| t != "null");
        if types.len() == 1 {
            let t = types.remove(0);
            m.insert("type".into(), t);
        }
    }
    if let Some(Value::Array(any)) = m.get("anyOf").cloned() {
        let rest: Vec<Value> = any.into_iter().filter(|s| s != &json!({"type": "null"})).collect();
        if rest.len() == 1 {
            m.remove("anyOf");
            if let Some(Value::Object(inner)) = rest.into_iter().next() {
                for (k, v) in inner {
                    m.entry(k).or_insert(v);
                }
            }
        } else {
            m.insert("anyOf".into(), Value::Array(rest));
        }
    }
}

fn props_mut(def: &mut Value) -> Option<&mut Map<String, Value>> {
    def.get_mut("properties").and_then(Value::as_object_mut)
}

fn flag(p: &Value, key: &str) -> bool {
    p.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Derived (`x-derived`) and transient (`x-transient`) properties of a type (10 §2.7 and the per-type "Derived:"
/// lists of 10 §4). Every type also has the transient canonical `path`.
fn query_only_properties(ty: ObjectType) -> Vec<(&'static str, Value)> {
    let derived = |schema: Value| {
        let mut s = schema;
        if let Value::Object(m) = &mut s {
            m.insert("x-derived".into(), json!(true));
            m.insert("readOnly".into(), json!(true));
        }
        s
    };
    let transient = |schema: Value| {
        let mut s = schema;
        if let Value::Object(m) = &mut s {
            m.insert("x-transient".into(), json!(true));
            m.insert("readOnly".into(), json!(true));
        }
        s
    };
    let ids = || json!({"type": "array", "items": {"$ref": "#/$defs/Id"}});
    let nullable_id = || json!({"anyOf": [{"$ref": "#/$defs/Id"}, {"type": "null"}]});
    // Derived frames, lengths and counts are `int`s (10 §1.1: signed 32-bit), non-negative.
    let count = || json!({"type": "integer", "minimum": 0, "maximum": i32::MAX});
    let mut out = vec![("path", transient(json!({"type": "string", "description": "Canonical path (10 §2.4)."})))];
    match ty {
        ObjectType::Project => {
            out.push(("sceneStartFrames", derived(json!({"type": "array", "items": count()}))));
            out.push(("globalLength", derived(count())));
            out.push(("thumbnail", derived(json!({"anyOf": [{"$ref": "#/$defs/Blob"}, {"type": "null"}]}))));
        }
        ObjectType::Scene => {
            out.push(("length", derived(json!({"type": "integer", "minimum": 1, "maximum": 100_000}))));
            out.push(("paddedLength", derived(count())));
        }
        ObjectType::Element => {
            out.push(("parentId", derived(nullable_id())));
            out.push(("childIds", derived(ids())));
            out.push(("depth", derived(count())));
            out.push(("timelineIndex", derived(count())));
            out.push(("xsheetIndex", derived(json!({"anyOf": [count(), {"type": "null"}]}))));
            out.push(("cloneGroupId", derived(nullable_id())));
            out.push(("cloneIds", derived(ids())));
            out.push(("drawingCount", derived(count())));
            out.push(("hiddenByParent", derived(json!({"type": "boolean"}))));
            out.push(("effectiveMarkerColour", derived(json!({"$ref": "#/$defs/Colour"}))));
        }
        ObjectType::DrawingBank => out.push(("elementIds", derived(ids()))),
        ObjectType::Drawing => {
            out.push(("thumbnail", derived(json!({"anyOf": [{"$ref": "#/$defs/Blob"}, {"type": "null"}]}))));
            out.push((
                "usage",
                derived(json!({"type": "array", "items": {"type": "object", "properties": {
                    "elementId": {"$ref": "#/$defs/Id"},
                    "ranges": {"type": "array", "items": {"$ref": "#/$defs/FrameRange"}}},
                    "required": ["elementId", "ranges"], "additionalProperties": false}})),
            ));
            out.push(("isEmpty", derived(json!({"type": "boolean"}))));
            out.push(("zones", derived(json!({"type": "array", "items": {"type": "object"}}))));
        }
        ObjectType::Asset => {
            out.push(("info", derived(json!({"type": "object"}))));
            out.push(("refs", derived(ids())));
            out.push(("status", transient(json!({"enum": ["ok", "missing", "changed", "unsupported"]}))));
        }
        ObjectType::SoundPlacement => out.push(("naturalEndFrame", derived(count()))),
        ObjectType::MediaSequence => {
            out.push(("sourceLength", derived(count())));
            out.push(("missing", derived(json!({"type": "boolean"}))));
        }
        ObjectType::ParticleBlock => out.push(("index", derived(count()))),
        ObjectType::Library => {
            out.push(("readOnly", transient(json!({"type": "boolean"}))));
            out.push(("writeAccessHeldBy", transient(json!({"type": ["string", "null"]}))));
            out.push(("location", transient(json!({"type": ["string", "null"]}))));
        }
        ObjectType::LibraryItem => out.push(("broken", transient(json!({"type": "boolean"})))),
        _ => {}
    }
    out
}

fn type_of_def(def: &Value) -> Option<ObjectType> {
    def.get("x-objectType").and_then(Value::as_str).and_then(ObjectType::parse)
}

/// Completes the schema of one object type.
/// Completes any struct schema (`additionalProperties: false`): kind-specific and omit-default optional properties
/// are absent when not applicable, never `null`; every other property is required (10 JSON-04: written
/// explicitly).
fn complete_struct(def: &mut Value) {
    if def.get("additionalProperties") != Some(&json!(false)) {
        return;
    }
    let Some(props) = props_mut(def) else { return };
    for p in props.values_mut() {
        if flag(p, "x-kindSpecific") || flag(p, "x-omitDefault") {
            strip_null(p);
        }
    }
    let required: Vec<Value> = props
        .iter()
        .filter(|(_, p)| !flag(p, "x-omitDefault") && !flag(p, "x-kindSpecific"))
        .map(|(k, _)| json!(k))
        .collect();
    if let Value::Object(m) = def {
        m.insert("required".into(), Value::Array(required));
    }
}

fn complete_object(ty: ObjectType, def: &mut Value) {
    let Some(props) = props_mut(def) else { return };
    // Names (10 §1.3).
    if let Some(name) = props.get_mut("name").and_then(Value::as_object_mut) {
        let rule = NameRule::for_type(ty);
        name.insert("minLength".into(), json!(1));
        name.insert("maxLength".into(), json!(rule.max_len));
        name.insert("pattern".into(), json!(rule.pattern()));
        name.remove("type");
        name.insert("type".into(), json!("string"));
    }
    if ty == ObjectType::Channel
        && let Some(name) = props.get_mut("name").and_then(Value::as_object_mut)
    {
        name.insert("pattern".into(), json!("^[a-z][A-Za-z]*(\\.[a-z][A-Za-z]*)?$"));
        name.insert("maxLength".into(), json!(64));
    }
    // The envelope `type` is persisted read-only (10 §2.7).
    if let Some(t) = props.get_mut("type").and_then(Value::as_object_mut) {
        t.insert("readOnly".into(), json!(true));
    }
    for (k, s) in query_only_properties(ty) {
        props.insert(k.to_owned(), s);
    }
    if let Value::Object(m) = def {
        m.insert("x-since".into(), json!(crate::FORMAT_VERSION));
    }
    match ty {
        ObjectType::Element => add_element_rules(def),
        ObjectType::DrawingNode => add_node_rules(def),
        ObjectType::Stroke => add_stroke_rules(def),
        ObjectType::Channel => add_channel_rules(def),
        ObjectType::LibraryItem => add_library_item_rules(def),
        _ => {}
    }
    full_or_stub(ty, def);
}

/// Makes an object schema accept the full object or its stub (10 §2.2: "Both forms validate against the schema";
/// a full object always contains `rev`, an object without `rev` is a stub). The schema's own `type` constant then
/// restricts a stub to this type, so every child slot accepts only stubs of its own type. Full-object requirements
/// and the kind rules move under `then`; the stub branch allows only `id`/`colourId`, `type` and `name`.
fn full_or_stub(ty: ObjectType, def: &mut Value) {
    let Value::Object(m) = def else { return };
    let full_required = m.remove("required").unwrap_or_else(|| json!([]));
    let mut rules: Vec<Value> = match m.remove("allOf") {
        Some(Value::Array(a)) => a,
        _ => Vec::new(),
    };
    if let Some(cond) = m.remove("if") {
        let mut r = Map::new();
        r.insert("if".into(), cond);
        if let Some(t) = m.remove("then") {
            r.insert("then".into(), t);
        }
        if let Some(e) = m.remove("else") {
            r.insert("else".into(), e);
        }
        rules.push(Value::Object(r));
    }
    // A stub's `name` is optional: entries reference split children by `{id, type}` and the document serialiser adds
    // the name when it renders a stub (10 §2.2).
    m.insert("required".into(), json!([ty.id_field(), "type"]));
    m.insert("if".into(), json!({ "required": ["rev"] }));
    let mut then = Map::new();
    then.insert("required".into(), full_required);
    if !rules.is_empty() {
        then.insert("allOf".into(), Value::Array(rules));
    }
    m.insert("then".into(), Value::Object(then));
    m.insert("else".into(), json!({ "propertyNames": { "enum": [ty.id_field(), "type", "name"] } }));
}

/// Every property of every definition records the format version that introduced it (10 §2.8 `x-since`).
///
/// The pass descends into anonymous declarations too — the object alternatives of an untagged enum such as
/// `DynamicParam`, inline objects under properties, items and map values (C11-R3-06) — but not into conditional
/// predicates (`if`/`then`/`else`), which constrain properties declared elsewhere.
fn annotate_since(def: &mut Value) {
    fn walk(v: &mut Value, depth: usize) {
        let Value::Object(m) = v else { return };
        if depth > 32 {
            return;
        }
        if let Some(Value::Object(props)) = m.get_mut("properties") {
            for p in props.values_mut() {
                if let Value::Object(pm) = p {
                    pm.entry("x-since").or_insert(json!(crate::FORMAT_VERSION));
                }
                walk(p, depth + 1);
            }
        }
        for k in ["items", "additionalProperties"] {
            if let Some(sub) = m.get_mut(k) {
                walk(sub, depth + 1);
            }
        }
        for k in ["prefixItems", "anyOf", "oneOf", "allOf"] {
            if let Some(Value::Array(subs)) = m.get_mut(k) {
                subs.iter_mut().for_each(|sub| walk(sub, depth + 1));
            }
        }
        if let Some(Value::Object(pp)) = m.get_mut("patternProperties") {
            pp.values_mut().for_each(|sub| walk(sub, depth + 1));
        }
    }
    walk(def, 0);
}

/// The decimal form of a `frame` used as a map key: 1–2147483647 (10 §1.1 `frame` = `int ≥ 1`, `int` signed 32-bit;
/// §4.7 cell notes, §4.14 mouth-track labels).
pub const FRAME_KEY_PATTERN: &str = "^(?:[1-9][0-9]{0,8}|1[0-9]{9}|20[0-9]{8}|21[0-3][0-9]{7}|214[0-6][0-9]{6}|2147[0-3][0-9]{5}|21474[0-7][0-9]{4}|214748[0-2][0-9]{3}|2147483[0-5][0-9]{2}|21474836[0-3][0-9]|214748364[0-7])$";

fn kind_rule(property: &str, value: &str, then: Value) -> Value {
    json!({ "if": { "properties": { property: { "const": value } }, "required": [property] }, "then": then })
}

fn add_element_rules(def: &mut Value) {
    if let Some(props) = props_mut(def) {
        // Exposure counts are ≥ 1; cell-note keys are frames ≥ 1.
        if let Some(count) =
            props.get_mut("exposure").and_then(|e| e.pointer_mut("/items/prefixItems/1")).and_then(Value::as_object_mut)
        {
            count.insert("minimum".into(), json!(1));
        }
        if let Some(notes) = props.get_mut("cellNotes").and_then(Value::as_object_mut) {
            notes.insert(
                "patternProperties".into(),
                json!({FRAME_KEY_PATTERN: {"type": "string", "maxLength": 10_000}}),
            );
        }
        if let Some(start) = props.get_mut("start").and_then(Value::as_object_mut) {
            start.insert(
                "description".into(),
                json!("Non-cell elements: first frame of the bar (persisted). Cell-based elements: derived from the exposure (query results only)."),
            );
        }
    }
    let mut rules = Vec::new();
    for t in ElementType::ALL {
        let allowed = t.specific_properties();
        let mut required: Vec<&str> = allowed.iter().copied().filter(|p| *p != "cellNotes").collect();
        let mut forbid: Vec<&str> =
            element::SPECIFIC_PROPERTIES.iter().copied().filter(|p| !allowed.contains(p)).collect();
        if t.is_cell_based() {
            required.push("visibleInXsheet");
        } else {
            required.extend(["start", "duration"]);
            forbid.push("visibleInXsheet");
        }
        let mut then_props = Map::new();
        for p in forbid {
            then_props.insert(p.to_owned(), json!(false));
        }
        if let Some(set) = PivotSet::of(t) {
            let all =
                ["rotationPivot", "scalePivot", "rotateAboutPivot", "pivot", "applyPivotTo", "useChildDrawingPivot"];
            let present = set.fields();
            let mut tp = Map::new();
            for f in all.iter().filter(|f| !present.contains(f)) {
                tp.insert((*f).to_owned(), json!(false));
            }
            then_props.insert("transform".into(), json!({ "required": present, "properties": tp }));
        }
        if allowed.contains(&"channels") {
            then_props.insert("channels".into(), json!({ "propertyNames": { "enum": t.allowed_channels() } }));
        }
        let params = match t {
            ElementType::ColourTransform => Some("ColourTransformParams"),
            ElementType::Clipping => Some("ClippingParams"),
            ElementType::DropShadow => Some("DropShadowParams"),
            ElementType::Blur => Some("BlurParams"),
            ElementType::Shake => Some("ShakeParams"),
            ElementType::ParticleNature | ElementType::ParticleImpact => Some("ParticleParams"),
            _ => None,
        };
        if let Some(p) = params {
            then_props.insert("params".into(), json!({ "$ref": format!("#/$defs/{p}") }));
        }
        if matches!(t, ElementType::ParticleNature | ElementType::ParticleImpact) {
            let types: Vec<&str> = if t == ElementType::ParticleNature {
                vec!["rain", "snow", "hail", "fog", "cloud", "eruption"]
            } else {
                vec!["shatter", "magic", "explosion", "splash", "fire", "celestial"]
            };
            then_props
                .insert("blocks".into(), json!({ "items": { "properties": { "particleType": { "enum": types } } } }));
        }
        rules.push(kind_rule("elementType", t.as_str(), json!({ "required": required, "properties": then_props })));
    }
    if let Value::Object(m) = def {
        m.insert("allOf".into(), Value::Array(rules));
    }
}

fn add_node_rules(def: &mut Value) {
    let all = [
        "hint",
        "strokes",
        "children",
        "transform",
        "widthMode",
        "width",
        "height",
        "text",
        "runs",
        "paragraphs",
        "image",
    ];
    let kinds: [(&str, &[&str], &[&str]); 4] = [
        ("layer", &["hint", "strokes"], &["hint", "strokes"]),
        ("group", &["children"], &["children"]),
        (
            "text",
            &["transform", "widthMode", "text", "runs", "paragraphs"],
            &["transform", "widthMode", "width", "text", "runs", "paragraphs"],
        ),
        ("bitmap", &["image", "width", "height", "transform"], &["image", "width", "height", "transform"]),
    ];
    let mut rules = Vec::new();
    for (kind, required, allowed) in kinds {
        let mut props = Map::new();
        for p in all.iter().filter(|p| !allowed.contains(p)) {
            props.insert((*p).to_owned(), json!(false));
        }
        if kind == "bitmap" {
            props.insert("width".into(), json!({"type": "integer", "minimum": 1, "maximum": 16384}));
            props.insert("height".into(), json!({"type": "integer", "minimum": 1, "maximum": 16384}));
        }
        if kind == "text" {
            props.insert("width".into(), json!({"type": "number", "minimum": 0, "x-unit": "du"}));
        }
        rules.push(kind_rule("kind", kind, json!({ "required": required, "properties": props })));
    }
    if let Value::Object(m) = def {
        m.insert("allOf".into(), Value::Array(rules));
    }
}

fn add_stroke_rules(def: &mut Value) {
    let rules = vec![
        kind_rule(
            "kind",
            "pencil",
            json!({"properties": {"lineStyle": {"$ref": "#/$defs/LineStyle"}, "spine": {"type": "null"}, "fillMatrix": {"type": "null"}}}),
        ),
        kind_rule(
            "kind",
            "invisible",
            json!({"properties": {"lineStyle": {"type": "null"}, "spine": {"type": "null"}, "fillMatrix": {"type": "null"}, "colourId": {"type": "null"}}}),
        ),
        kind_rule("kind", "brush", json!({"properties": {"lineStyle": {"type": "null"}}})),
    ];
    if let Value::Object(m) = def {
        m.insert("allOf".into(), Value::Array(rules));
    }
}

fn add_channel_rules(def: &mut Value) {
    if let Value::Object(m) = def {
        m.insert(
            "if".into(),
            json!({ "properties": { "name": { "const": channel::POSITION } }, "required": ["name"] }),
        );
        m.insert(
            "then".into(),
            json!({ "required": ["points"], "properties": { "curveType": false, "keys": false, "points": { "minItems": 1 } } }),
        );
        m.insert(
            "else".into(),
            json!({ "required": ["curveType", "keys"], "properties": { "points": false, "keys": { "minItems": 1 } } }),
        );
    }
}

fn add_library_item_rules(def: &mut Value) {
    let template =
        ["versionId", "artist", "copyright", "createdAt", "modifiedAt", "sizeBytes", "formatVersion", "summary"];
    let mut forbid = Map::new();
    for p in template.iter().chain(std::iter::once(&"source")) {
        forbid.insert((*p).to_owned(), json!(false));
    }
    if let Value::Object(m) = def {
        m.insert(
            "if".into(),
            json!({ "properties": { "itemKind": { "const": "template" } }, "required": ["itemKind"] }),
        );
        m.insert("then".into(), json!({ "required": template }));
        m.insert("else".into(), json!({ "properties": forbid }));
    }
}

fn build_definitions() -> Map<String, Value> {
    let mut defs = generate_raw();
    for def in defs.values_mut() {
        fix_integers(def);
    }
    let names: Vec<String> = defs.keys().cloned().collect();
    for name in names {
        if let Some(def) = defs.get_mut(&name) {
            complete_struct(def);
            if let Some(ty) = type_of_def(def) {
                complete_object(ty, def);
            }
            annotate_since(def);
        }
    }
    // Inserted after the struct pass: a stub's `name` and its `id`/`colourId` alternatives are not all required.
    defs.insert("Stub".into(), crate::entities::stub_schema());
    // MouthTrack carries the derived `stale` flag (10 §4.14); its labels are keyed by frame.
    if let Some(props) = defs.get_mut("MouthTrack").and_then(props_mut) {
        props.insert("stale".into(), json!({"type": "boolean", "x-derived": true, "readOnly": true}));
        if let Some(labels) = props.get_mut("labels").and_then(Value::as_object_mut) {
            labels.insert("patternProperties".into(), json!({FRAME_KEY_PATTERN: {"type": "string"}}));
        }
    }
    // Every property added above carries `x-since` too (C11-R2-11): annotate after all additions.
    for def in defs.values_mut() {
        annotate_since(def);
    }
    // Deterministic order of definitions.
    let sorted: BTreeMap<String, Value> = defs.into_iter().collect();
    sorted.into_iter().collect()
}

/// Generates and post-processes the schema definitions of other (non-model) types, e.g. the engine's IPC wire
/// types: integer bounds of their Rust types, `required` = every property not marked optional, sorted definitions.
pub fn struct_definitions(register: impl FnOnce(&mut SchemaGenerator)) -> Map<String, Value> {
    let mut g = SchemaSettings::draft2020_12().into_generator();
    register(&mut g);
    let mut defs = g.take_definitions(true);
    for def in defs.values_mut() {
        fix_integers(def);
        complete_struct(def);
    }
    let sorted: BTreeMap<String, Value> = defs.into_iter().collect();
    sorted.into_iter().collect()
}

/// All schema definitions (`$defs`), post-processed. Built once.
#[must_use]
pub fn definitions() -> &'static Map<String, Value> {
    static DEFS: OnceLock<Map<String, Value>> = OnceLock::new();
    DEFS.get_or_init(build_definitions)
}

/// The self-contained JSON Schema of an object type, as returned by `doc.schema.get` (10 §2.8).
#[must_use]
pub fn object_schema(ty: ObjectType) -> Value {
    let name = def_name(ty);
    let mut root = Map::new();
    root.insert("$schema".into(), json!(DIALECT));
    root.insert("$id".into(), json!(format!("{ID_BASE}/{}/{}.schema.json", crate::FORMAT_VERSION, ty.as_str())));
    if let Some(Value::Object(def)) = definitions().get(&name) {
        for (k, v) in def {
            root.insert(k.clone(), v.clone());
        }
    }
    root.insert("$defs".into(), Value::Object(definitions().clone()));
    Value::Object(root)
}

/// The schema of one property of a type, addressed by a JSON Pointer into the object (10 §10.2 `doc.schema.get
/// {property}`). The lookup keeps **every** schema that may apply on the way down (C11-R2-06): `$ref` targets,
/// `allOf`/`anyOf`/`oneOf` members and both branches of every conditional (there is no instance to choose one), and
/// at each token the property, tuple position (`prefixItems`), array item (`items`), map value
/// (`additionalProperties`) or pattern-map value (`patternProperties`) of each. The result merges the final
/// candidates in order — a declaration's own annotations (`readOnly`, `x-since`, `description`, …) before the keywords
/// of the definition it references — so annotations beside a `$ref` survive and a field declared only inside a kind
/// rule (an effect's typed `params`) is found. `None` when no schema declares the pointer.
#[must_use]
pub fn property_schema(ty: ObjectType, pointer: &str) -> Option<Value> {
    let tokens = crate::pointer::parse(pointer).ok()?;
    let defs = definitions();
    let mut current: Vec<&Value> = vec![defs.get(&def_name(ty))?];
    for t in &tokens {
        let index = crate::pointer::array_index(t);
        let mut expanded = Vec::new();
        for c in &current {
            expand(defs, c, &mut expanded, 0);
        }
        let mut next: Vec<&Value> = Vec::new();
        let mut push = |v: Option<&'static Value>| {
            if let Some(v) = v.filter(|v| v.is_object()) {
                next.push(v);
            }
        };
        for c in expanded {
            push(c.get("properties").and_then(|p| p.get(t)));
            if let Some(i) = index {
                push(c.get("prefixItems").and_then(|p| p.get(i)).or_else(|| c.get("items")));
            }
            push(c.get("additionalProperties"));
            if let Some(Value::Object(pp)) = c.get("patternProperties") {
                for v in pp.values() {
                    push(Some(v));
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        current = next;
    }
    // The first candidate is the property's own declaration (definitions come before their conditional branches): its
    // keywords are kept whole, following only its `$ref` chain, so alternatives (`anyOf` of an ID and null, a `oneOf`
    // of enum constants) keep their logic instead of being flattened into one narrower schema (C11-R3-04). Later
    // candidates (kind rules) contribute annotations only.
    let annotation =
        |k: &str| k.starts_with("x-") || matches!(k, "description" | "title" | "readOnly" | "writeOnly" | "deprecated");
    let mut merged = Map::new();
    for (i, c) in current.into_iter().enumerate() {
        for l in ref_chain(defs, c) {
            if let Value::Object(m) = l {
                for (k, v) in m {
                    if k != "$ref" && !merged.contains_key(k) && (i == 0 || annotation(k)) {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    }
    Some(Value::Object(merged))
}

/// A schema followed by the targets of its `$ref` chain (a declaration's own keywords first).
fn ref_chain<'a>(defs: &'a Map<String, Value>, mut v: &'a Value) -> Vec<&'a Value> {
    let mut out = vec![v];
    for _ in 0..32 {
        match v.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/")).and_then(|n| defs.get(n)) {
            Some(d) => {
                out.push(d);
                v = d;
            }
            None => break,
        }
    }
    out
}

fn deref<'a>(defs: &'a Map<String, Value>, mut v: &'a Value) -> &'a Value {
    for _ in 0..32 {
        match v.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/")).and_then(|n| defs.get(n)) {
            Some(d) => v = d,
            None => break,
        }
    }
    v
}

/// Every schema a schema may defer to: its `$ref` target, `anyOf`/`oneOf`/`allOf` members and conditional branches
/// (all of them: the lookup is by property name, not by instance), recursively.
fn expand<'a>(defs: &'a Map<String, Value>, s: &'a Value, out: &mut Vec<&'a Value>, depth: usize) {
    if depth > 16 {
        return;
    }
    let s = deref(defs, s);
    out.push(s);
    for k in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(alts)) = s.get(k) {
            for a in alts {
                expand(defs, a, out, depth + 1);
            }
        }
    }
    for k in ["then", "else"] {
        if let Some(b) = s.get(k) {
            expand(defs, b, out, depth + 1);
        }
    }
}

/// The schema bundle published for other consumers: every definition plus the object-type index.
#[must_use]
pub fn bundle() -> Value {
    let types: Map<String, Value> =
        ObjectType::ALL.iter().map(|t| (t.as_str().to_owned(), json!(format!("#/$defs/{}", def_name(*t))))).collect();
    json!({
        "$schema": DIALECT,
        "$id": format!("{ID_BASE}/{}/model.schema.json", crate::FORMAT_VERSION),
        "title": "Toontown object model",
        "formatVersion": crate::FORMAT_VERSION,
        "x-objectTypes": types,
        "$defs": definitions(),
    })
}

/// One schema violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// JSON Pointer into the validated value.
    pub pointer: String,
    /// JSON Pointer into the schema.
    pub schema_path: String,
    /// Message.
    pub message: String,
}

impl Violation {
    /// The `doc.schemaViolation` error for this violation.
    #[must_use]
    pub fn to_error(&self) -> ModelError {
        ModelError::with_details(
            codes::SCHEMA_VIOLATION,
            self.message.clone(),
            json!({ "pointer": self.pointer, "schemaPath": self.schema_path }),
        )
    }
}

fn validator(ty: ObjectType) -> Result<&'static jsonschema::Validator, ModelError> {
    static VALIDATORS: OnceLock<BTreeMap<ObjectType, Result<jsonschema::Validator, String>>> = OnceLock::new();
    let all = VALIDATORS.get_or_init(|| {
        ObjectType::ALL
            .iter()
            .map(|t| {
                let schema = object_schema(*t);
                // Draft 2020-12; compiling also checks the schema itself (a defect is reported, not hidden).
                let v = jsonschema::options()
                    .with_draft(jsonschema::Draft::Draft202012)
                    .should_validate_formats(false)
                    .build(&schema)
                    .map_err(|e| e.to_string());
                (*t, v)
            })
            .collect()
    });
    match all.get(&ty) {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(ModelError::new(codes::INTERNAL, format!("schema of `{ty}` does not compile: {e}"))),
        None => Err(ModelError::new(codes::INTERNAL, format!("no schema for `{ty}`"))),
    }
}

/// Validates a JSON value against the schema of an object type. Returns every violation (at most `limit`).
///
/// # Errors
/// `internal` if the schema itself fails to compile (a build defect covered by tests).
pub fn validate(ty: ObjectType, value: &Value, limit: usize) -> Result<Vec<Violation>, ModelError> {
    let v = validator(ty)?;
    Ok(v.iter_errors(value)
        .take(limit)
        .map(|e| Violation {
            pointer: e.instance_path().to_string(),
            schema_path: e.schema_path().to_string(),
            message: e.to_string(),
        })
        .collect())
}

/// Whether a value is valid for an object type.
#[must_use]
pub fn is_valid(ty: ObjectType, value: &Value) -> bool {
    validator(ty).map(|v| v.is_valid(value)).unwrap_or(false)
}
