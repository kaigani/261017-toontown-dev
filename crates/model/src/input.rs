//! Schema-directed input normalisation (10 §1.1, §1.3, §2.2, §6.5; C11-R1-03/R1-06/R1-12/R1-15).
//!
//! Every object entering the model passes through [`prepare`] before schema validation. The walk follows the type's
//! generated schema (with `$ref`s, alternatives and the generated `if`/`then`/`else` kind rules), so it only touches
//! positions the schema declares:
//!
//! - **Strings** at declared positions are NFC-normalised (10 §1.1 "strings are NFC"); the `name` of a model object
//!   is also trimmed (10 §1.3).
//! - **Swatch alias.** A swatch given with `id` instead of `colourId` is accepted (10 §2.2); `id` becomes
//!   `colourId`. Both present and equal is the same; both present and different is left for validation to reject.
//! - **Key order** is a pure function of content (10 AG-06, JSON-02): object keys in schema order, keys the schema
//!   does not declare and the keys of maps (`paletteStyles`, swatch `values`, `channels`, …) in lexicographic
//!   (code point) order.
//! - **Opaque values** — `meta`, every `ext` namespace, `importOptions` and anything else whose schema declares no
//!   structure — are never interpreted: no string inside them is normalised and no embedded `type`/`name` is read.
//!   Only their object keys are put in lexicographic order, recursively (JSON object members are unordered, so
//!   this changes no value; it makes the canonical text independent of insertion order).

use serde_json::{Map, Value};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::is_nfc;

use crate::schema;
use crate::types::ObjectType;

/// Maximum nesting depth of an object accepted by the model (objects and arrays; the object itself is depth 1).
/// Lower than every reader's and writer's limit ([`crate::json_text::MAX_TEXT_DEPTH`], [`crate::MAX_JSON_DEPTH`]),
/// so an accepted object can always be written, wrapped in an envelope and read back (C11-R1-09).
pub const MAX_MODEL_DEPTH: usize = 100;

/// The nesting depth of a JSON value (a scalar is 0, `[]` is 1), stopping once `limit` is exceeded.
#[must_use]
pub fn depth(v: &Value, limit: usize) -> usize {
    fn go(v: &Value, d: usize, limit: usize) -> usize {
        if d > limit {
            return d;
        }
        match v {
            Value::Array(a) => a.iter().map(|x| go(x, d + 1, limit)).max().unwrap_or(d + 1),
            Value::Object(m) => m.values().map(|x| go(x, d + 1, limit)).max().unwrap_or(d + 1),
            _ => d,
        }
    }
    go(v, 0, limit)
}

/// Normalises an object's JSON in place (see the module documentation).
pub fn prepare(ty: ObjectType, v: &mut Value) {
    let defs = schema::definitions();
    let Some(root) = defs.get(&schema::def_name(ty)) else {
        sort_keys(v, 0);
        return;
    };
    walk(defs, &[root], v, 0);
}

/// Sorts object keys recursively (opaque values).
pub fn sort_keys(v: &mut Value, d: usize) {
    if d > crate::MAX_JSON_DEPTH {
        return;
    }
    match v {
        Value::Object(m) => {
            m.sort_keys();
            for x in m.values_mut() {
                sort_keys(x, d + 1);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| sort_keys(x, d + 1)),
        _ => {}
    }
}

fn deref<'a>(defs: &'a Map<String, Value>, mut s: &'a Value) -> &'a Value {
    for _ in 0..32 {
        match s.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/")).and_then(|n| defs.get(n)) {
            Some(t) => s = t,
            None => break,
        }
    }
    s
}

/// Evaluates the generated `if` conditions: only `required` and `properties.<p>.const` occur. Anything else is
/// undecided (`None`): both branches are then followed.
fn condition(cond: &Value, v: &Value) -> Option<bool> {
    let Value::Object(c) = cond else { return None };
    let Value::Object(m) = v else { return Some(false) };
    let mut result = true;
    for (k, x) in c {
        match k.as_str() {
            "required" => {
                result &= x.as_array()?.iter().all(|p| p.as_str().is_some_and(|p| m.contains_key(p)));
            }
            "properties" => {
                for (p, ps) in x.as_object()? {
                    let Value::Object(ps) = ps else { return None };
                    if ps.len() != 1 {
                        return None;
                    }
                    let expected = ps.get("const")?;
                    if let Some(actual) = m.get(p) {
                        result &= actual == expected;
                    }
                }
            }
            _ => return None,
        }
    }
    Some(result)
}

/// The schemas that apply to `v` at a position described by `s`: `$ref` targets, every `allOf` member, every
/// `anyOf`/`oneOf` alternative (all of them; validation picks), and the `if` branch selected by the value.
fn expand<'a>(defs: &'a Map<String, Value>, s: &'a Value, v: &Value, out: &mut Vec<&'a Value>, d: usize) {
    if d > 16 {
        return;
    }
    let s = deref(defs, s);
    out.push(s);
    for k in ["allOf", "anyOf", "oneOf"] {
        if let Some(Value::Array(alts)) = s.get(k) {
            for a in alts {
                expand(defs, a, v, out, d + 1);
            }
        }
    }
    if let Some(cond) = s.get("if") {
        let branch = condition(cond, v);
        if branch != Some(false)
            && let Some(t) = s.get("then")
        {
            expand(defs, t, v, out, d + 1);
        }
        if branch != Some(true)
            && let Some(e) = s.get("else")
        {
            expand(defs, e, v, out, d + 1);
        }
    }
}

/// Whether a schema declares structure below it (anything but an opaque `true`/`{}`/free-form object).
fn structural(s: &Value) -> bool {
    match s {
        Value::Object(m) => {
            m.contains_key("properties")
                || m.contains_key("items")
                || m.contains_key("prefixItems")
                || m.get("additionalProperties").is_some_and(Value::is_object)
                || m.get("type")
                    .is_some_and(|t| t != "object" && t.as_array().is_none_or(|a| !a.contains(&"object".into())))
                || m.contains_key("const")
                || m.contains_key("enum")
                || m.contains_key("anyOf")
                || m.contains_key("oneOf")
                || m.contains_key("allOf")
        }
        _ => false,
    }
}

fn child_schemas<'a>(cands: &[&'a Value], key: &str, index: Option<usize>) -> Vec<&'a Value> {
    let mut out = Vec::new();
    for c in cands {
        let found = match index {
            Some(i) => c.get("prefixItems").and_then(|p| p.get(i)).or_else(|| c.get("items")),
            None => c.get("properties").and_then(|p| p.get(key)).or_else(|| c.get("additionalProperties")),
        };
        if let Some(s) = found
            && !s.is_boolean()
        {
            out.push(s);
        }
    }
    out
}

/// Whether a value can change in the walk: non-NFC strings, objects, and arrays holding anything but numbers,
/// booleans, `null` and NFC strings (stroke points and other tuples are skipped without consulting the schema).
fn needs_walk(v: &Value) -> bool {
    match v {
        // An NFC string (every ASCII string is) cannot change: names are trimmed by their object.
        Value::String(s) => !is_nfc(s),
        Value::Object(_) => true,
        Value::Array(a) => a.iter().any(|x| match x {
            Value::String(s) => !is_nfc(s),
            Value::Array(_) | Value::Object(_) => true,
            _ => false,
        }),
        _ => false,
    }
}

fn nfc(s: &mut String) {
    if !is_nfc(s) {
        *s = s.nfc().collect();
    }
}

fn walk(defs: &Map<String, Value>, schemas: &[&Value], v: &mut Value, d: usize) {
    if d > MAX_MODEL_DEPTH {
        return; // rejected by the depth check
    }
    let mut cands = Vec::new();
    for s in schemas {
        expand(defs, s, v, &mut cands, 0);
    }
    if !cands.iter().any(|c| structural(c)) {
        sort_keys(v, d);
        return;
    }
    match v {
        Value::String(s) => nfc(s),
        Value::Array(items) => {
            for (i, item) in items.iter_mut().enumerate() {
                if needs_walk(item) {
                    let sub = child_schemas(&cands, "", Some(i));
                    walk(defs, &sub, item, d + 1);
                }
            }
        }
        Value::Object(m) => {
            let object_type = cands.iter().find_map(|c| c.get("x-objectType")).and_then(Value::as_str);
            if object_type == Some(ObjectType::Swatch.as_str()) && m.get("type") == Some(&Value::from("swatch")) {
                swatch_alias(m);
            }
            for (k, x) in m.iter_mut() {
                if needs_walk(x) {
                    let sub = child_schemas(&cands, k, None);
                    walk(defs, &sub, x, d + 1);
                }
            }
            if object_type.is_some()
                && object_type == m.get("type").and_then(Value::as_str)
                && let Some(Value::String(n)) = m.get_mut("name")
            {
                let trimmed = n.trim();
                if trimmed.len() != n.len() {
                    *n = trimmed.to_owned();
                }
            }
            // Rank = position of the key's first appearance in the candidates' `properties`, in candidate order
            // (schema order); undeclared keys after all declared ones, lexicographically.
            let props: Vec<&Map<String, Value>> =
                cands.iter().filter_map(|c| c.get("properties").and_then(Value::as_object)).collect();
            let rank = |k: &str| {
                let mut offset = 0;
                for p in &props {
                    if let Some(i) = p.keys().position(|x| x == k) {
                        return offset + i;
                    }
                    offset += p.len();
                }
                usize::MAX
            };
            let mut sorted = true;
            let mut prev: Option<(usize, &str)> = None;
            for k in m.keys() {
                let r = (rank(k), k.as_str());
                if prev.is_some_and(|p| p > r) {
                    sorted = false;
                    break;
                }
                prev = Some(r);
            }
            if !sorted {
                let mut entries: Vec<(String, Value)> = std::mem::take(m).into_iter().collect();
                entries.sort_by_cached_key(|(k, _)| (rank(k), k.clone()));
                m.extend(entries);
            }
        }
        _ => {}
    }
}

fn swatch_alias(m: &mut Map<String, Value>) {
    let Some(id) = m.get("id").cloned() else { return };
    match m.get("colourId") {
        None => {
            m.shift_remove("id");
            m.insert("colourId".into(), id);
        }
        Some(c) if *c == id => {
            m.shift_remove("id");
        }
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn depth_counts_containers() {
        assert_eq!(depth(&json!(1), 10), 0);
        assert_eq!(depth(&json!([]), 10), 1);
        assert_eq!(depth(&json!({"a": [[1]]}), 10), 3);
    }

    #[test]
    fn opaque_values_are_untouched_but_ordered() {
        let mut v = json!({"b": {"y": " e\u{301} ", "x": 1}, "a": [{"d": 1, "c": 2}]});
        sort_keys(&mut v, 0);
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            "{\"a\":[{\"c\":2,\"d\":1}],\"b\":{\"x\":1,\"y\":\" e\u{301} \"}}"
        );
    }
}
