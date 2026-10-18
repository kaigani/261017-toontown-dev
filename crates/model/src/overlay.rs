//! Unknown-property preservation (10 VER-02, ZIP-04 spirit, JSON-02 "unknown keys last in lexicographic order").
//!
//! Model types are strict (`deny_unknown_fields`). JSON from a newer minor format version may carry properties this
//! version does not know; they must survive load, edit and save verbatim. [`split`] walks a raw object alongside its
//! type's schema and moves every property the schema does not declare into an [`Overlay`] keyed by the JSON Pointer of
//! its containing object, leaving JSON the strict types accept. [`merge`] puts the properties back, after the known
//! ones, in lexicographic order. The load path (C1.3) applies this to entries of newer-minor files.
//!
//! Containing objects are addressed by pointer; inside arrays of objects that carry an `id`/`colourId` the pointer
//! segment is `@<id>` instead of the index, so the overlay follows objects that moved or whose siblings changed.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::schema::definitions;
use crate::types::ObjectType;

/// Unknown properties by containing object (pointer with `@id` segments for identified array members).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay(BTreeMap<String, BTreeMap<String, Value>>);

impl Overlay {
    /// Whether nothing was preserved.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The preserved properties (pointer → name → value).
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<String, BTreeMap<String, Value>> {
        &self.0
    }
}

fn deref<'a>(defs: &'a Map<String, Value>, mut s: &'a Value) -> &'a Value {
    for _ in 0..32 {
        match s.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/")).and_then(|n| defs.get(n)) {
            Some(next) => s = next,
            None => break,
        }
    }
    s
}

/// The object schemas a value at this position may follow (alternatives of `anyOf`/`oneOf` included).
fn object_schemas<'a>(defs: &'a Map<String, Value>, s: &'a Value, out: &mut Vec<&'a Value>, depth: usize) {
    if depth > 16 {
        return;
    }
    let s = deref(defs, s);
    if s.get("properties").is_some() {
        out.push(s);
    }
    for k in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(alts)) = s.get(k) {
            for a in alts {
                object_schemas(defs, a, out, depth + 1);
            }
        }
    }
}

fn member_key(v: &Value, i: usize) -> String {
    match v.get("id").or_else(|| v.get("colourId")).and_then(Value::as_str) {
        Some(id) => format!("@{id}"),
        None => i.to_string(),
    }
}

fn walk(defs: &Map<String, Value>, schema: &Value, v: &mut Value, at: &str, out: &mut Overlay, depth: usize) {
    if depth > crate::MAX_JSON_DEPTH {
        return;
    }
    match v {
        Value::Object(m) => {
            let mut schemas = Vec::new();
            object_schemas(defs, schema, &mut schemas, 0);
            if schemas.is_empty() {
                // A map (e.g. `channels`): walk its values with the value schema. A free-form object (`meta`,
                // `ext`, opaque `params`) has none, and nothing in it is unknown.
                if let Some(value_schema) = deref(defs, schema).get("additionalProperties").filter(|a| a.is_object()) {
                    for (k, child) in m.iter_mut() {
                        walk(defs, value_schema, child, &format!("{at}/{}", crate::pointer::escape(k)), out, depth + 1);
                    }
                }
                return;
            }
            let open = schemas.iter().any(|s| s.get("additionalProperties").is_some_and(|a| a != &Value::Bool(false)));
            let known = |k: &str| schemas.iter().any(|s| s.get("properties").and_then(|p| p.get(k)).is_some());
            if !open {
                let unknown: Vec<String> = m.keys().filter(|k| !known(k)).cloned().collect();
                if !unknown.is_empty() {
                    let entry = out.0.entry(at.to_owned()).or_default();
                    for k in unknown {
                        if let Some(val) = m.shift_remove(&k) {
                            entry.insert(k, val);
                        }
                    }
                }
            }
            for (k, child) in m.iter_mut() {
                let sub: Vec<&Value> =
                    schemas.iter().filter_map(|s| s.get("properties").and_then(|p| p.get(k))).collect();
                let target =
                    sub.first().copied().or_else(|| schemas.iter().find_map(|s| s.get("additionalProperties")));
                if let Some(t) = target {
                    walk(defs, t, child, &format!("{at}/{}", crate::pointer::escape(k)), out, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            let s = deref(defs, schema);
            let item_schema = s.get("items");
            for (i, item) in items.iter_mut().enumerate() {
                let key = member_key(item, i);
                let t = s.get("prefixItems").and_then(|p| p.get(i)).or(item_schema);
                if let Some(t) = t {
                    walk(defs, t, item, &format!("{at}/{key}"), out, depth + 1);
                }
            }
        }
        _ => {}
    }
}

/// Moves every property the type's schema does not declare out of `raw` into an overlay.
#[must_use]
pub fn split(ty: ObjectType, raw: &Value) -> (Value, Overlay) {
    let defs = definitions();
    let mut v = raw.clone();
    let mut out = Overlay::default();
    if let Some(schema) = defs.get(&crate::schema::def_name(ty)) {
        walk(defs, schema, &mut v, "", &mut out, 0);
    }
    (v, out)
}

fn locate<'a>(v: &'a mut Value, pointer: &str) -> Option<&'a mut Value> {
    if pointer.is_empty() {
        return Some(v);
    }
    let mut cur = v;
    for raw in pointer.strip_prefix('/')?.split('/') {
        let seg = raw.replace("~1", "/").replace("~0", "~");
        cur = match cur {
            Value::Object(m) => m.get_mut(&seg)?,
            Value::Array(a) => {
                if let Some(id) = seg.strip_prefix('@') {
                    a.iter_mut()
                        .find(|x| x.get("id").or_else(|| x.get("colourId")).and_then(Value::as_str) == Some(id))?
                } else {
                    a.get_mut(seg.parse::<usize>().ok()?)?
                }
            }
            _ => return None,
        };
    }
    Some(cur)
}

/// Puts preserved properties back after the known ones, in lexicographic order (JSON-02). Properties whose
/// containing object no longer exists (it was deleted) are dropped, as the object's own data is.
#[must_use]
pub fn merge(mut value: Value, overlay: &Overlay) -> Value {
    for (pointer, props) in &overlay.0 {
        if let Some(Value::Object(m)) = locate(&mut value, pointer) {
            for (k, v) in props {
                if !m.contains_key(k) {
                    m.insert(k.clone(), v.clone());
                }
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::sample_document;
    use crate::object::from_json;
    use serde_json::json;

    #[test]
    fn newer_minor_properties_survive_a_parse_and_write_in_lexicographic_order() {
        let doc = sample_document(1).unwrap();
        let bob = doc.elements().find(|e| e.name() == "Bob").unwrap().id().as_str().to_owned();
        let mut raw = doc.object_json(&bob, crate::document::RenderOptions { depth: None, path: false }).unwrap();
        raw["zNewFlag"] = json!(true);
        raw["aNewThing"] = json!({"x": 1});
        raw["transform"]["wobble"] = json!(0.5);
        raw["channels"]["rotation"]["keys"][1]["tension"] = json!(0.25);
        raw["meta"] = json!({"free": "form stays"});
        let (clean, overlay) = split(ObjectType::Element, &raw);
        assert!(!overlay.is_empty());
        assert!(clean.get("zNewFlag").is_none() && clean["transform"].get("wobble").is_none());
        let parsed: crate::entities::element::Element = from_json(&clean).unwrap();
        let written = merge(crate::object::to_json(&parsed).unwrap(), &overlay);
        let keys: Vec<&String> = written.as_object().unwrap().keys().collect();
        let n = keys.len();
        assert_eq!(keys[n - 2..], [&"aNewThing".to_owned(), &"zNewFlag".to_owned()], "unknown keys last, sorted");
        assert_eq!(written["transform"]["wobble"], json!(0.5));
        assert_eq!(written["channels"]["rotation"]["keys"][1]["tension"], json!(0.25));
        assert_eq!(written["meta"], json!({"free": "form stays"}));
        let key_id = &overlay.entries().keys().find(|k| k.contains("keys")).unwrap().clone();
        assert!(key_id.contains("/keys/@"), "array members are addressed by id: {key_id}");
    }
}
