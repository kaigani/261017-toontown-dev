//! TypeScript publication of JSON Schemas (C1.1 readiness bullet 1; ADR-0012).
//!
//! Emits a TypeScript module holding the schema definitions as a typed object literal (for the front end's
//! validator, `desktop/src/ipc/schema.ts`) and one TypeScript type per definition. Integer bounds beyond
//! `Number.MAX_SAFE_INTEGER` are written as `bigint` literals, and integer types whose range exceeds the safe range
//! are typed `number | bigint`: the front end's lossless JSON parser returns `bigint` exactly for those values.

use std::fmt::Write as _;

use serde_json::{Map, Value};

/// Largest integer a JavaScript number holds exactly.
pub const MAX_SAFE: i64 = 9_007_199_254_740_991;

fn ts_string(s: &str) -> String {
    // A JSON string literal is a valid TypeScript string literal.
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned())
}

fn key(k: &str) -> String {
    let ident = k.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if ident { k.to_owned() } else { ts_string(k) }
}

fn number_literal(n: &serde_json::Number, bigint_ok: bool) -> String {
    if let Some(i) = n.as_i64() {
        if i.abs() > MAX_SAFE && bigint_ok {
            return format!("{i}n");
        }
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return if bigint_ok { format!("{u}n") } else { u.to_string() };
    }
    crate::canonical::format_number(n).unwrap_or_else(|_| "0".to_owned())
}

/// Writes a JSON value as a TypeScript literal; numbers under `minimum`/`maximum`/`exclusive*` keys that are beyond
/// the safe range become `bigint` literals.
fn literal(v: &Value, out: &mut String, indent: usize, bound: bool) {
    let pad = "  ".repeat(indent + 1);
    let end = "  ".repeat(indent);
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number_literal(n, bound)),
        Value::String(s) => out.push_str(&ts_string(s)),
        Value::Array(a) if a.is_empty() => out.push_str("[]"),
        Value::Array(a) => {
            out.push_str("[\n");
            for item in a {
                out.push_str(&pad);
                literal(item, out, indent + 1, false);
                out.push_str(",\n");
            }
            out.push_str(&end);
            out.push(']');
        }
        Value::Object(m) if m.is_empty() => out.push_str("{}"),
        Value::Object(m) => {
            out.push_str("{\n");
            for (k, item) in m {
                let is_bound = matches!(k.as_str(), "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum");
                let _ = write!(out, "{pad}{}: ", key(k));
                literal(item, out, indent + 1, is_bound);
                out.push_str(",\n");
            }
            out.push_str(&end);
            out.push('}');
        }
    }
}

fn type_name(def: &str) -> String {
    def.replace(['-', '.'], "_")
}

fn unsafe_integer(s: &Map<String, Value>) -> bool {
    let beyond = |k: &str| {
        s.get(k).and_then(Value::as_number).is_some_and(|n| {
            n.as_i64().map_or_else(|| n.as_u64().is_some_and(|u| u > MAX_SAFE as u64), |i| i.abs() > MAX_SAFE)
        })
    };
    beyond("minimum") || beyond("maximum") || (!s.contains_key("minimum") || !s.contains_key("maximum"))
}

fn union(parts: Vec<String>) -> String {
    let mut seen: Vec<String> = Vec::new();
    for p in parts {
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    if seen.iter().any(|p| p == "unknown") {
        return "unknown".to_owned();
    }
    // `"scene" | string` is `string` in TypeScript (the schema keeps the distinction, e.g. a pattern on the string).
    if seen.iter().any(|p| p == "string") {
        seen.retain(|p| !p.split(" | ").all(|x| x.len() >= 2 && x.starts_with('\'') && x.ends_with('\'')));
    }
    match seen.len() {
        0 => "never".to_owned(),
        1 => seen.remove(0),
        _ => seen.join(" | "),
    }
}

/// The TypeScript type of a schema.
fn ts_type(s: &Value, defs: &Map<String, Value>, indent: usize) -> String {
    let Value::Object(m) = s else {
        return if s == &Value::Bool(false) { "never".to_owned() } else { "unknown".to_owned() };
    };
    if let Some(r) = m.get("$ref").and_then(Value::as_str) {
        let name = r.trim_start_matches("#/$defs/");
        // A model object position holds the full object or its stub (10 §2.2).
        if defs.get(name).is_some_and(|d| d.get("x-objectType").is_some()) {
            return format!("{} | {}Stub", type_name(name), type_name(name));
        }
        return type_name(name);
    }
    if let Some(c) = m.get("const") {
        return literal_type(c);
    }
    if let Some(Value::Array(e)) = m.get("enum") {
        return union(e.iter().map(literal_type).collect());
    }
    let has_base = m.contains_key("type") || m.contains_key("properties");
    for k in ["anyOf", "oneOf"] {
        if let Some(Value::Array(alts)) = m.get(k) {
            let branches: Vec<String> = alts.iter().map(|a| ts_type(a, defs, indent)).collect();
            // Constraint-only branches (e.g. `{required: [id]}` alternatives of a stub) do not change the type.
            if has_base && branches.iter().all(|b| b == "unknown") {
                break;
            }
            return union(branches);
        }
    }
    let types: Vec<String> = match m.get("type") {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        _ => {
            if m.contains_key("properties") {
                vec!["object".to_owned()]
            } else {
                return "unknown".to_owned();
            }
        }
    };
    let mut parts = Vec::new();
    for t in types {
        parts.push(match t.as_str() {
            "string" => "string".to_owned(),
            "boolean" => "boolean".to_owned(),
            "null" => "null".to_owned(),
            "number" => "number".to_owned(),
            "integer" => {
                if unsafe_integer(m) {
                    "number | bigint".to_owned()
                } else {
                    "number".to_owned()
                }
            }
            "array" => {
                if let Some(Value::Array(prefix)) = m.get("prefixItems") {
                    let items: Vec<String> = prefix.iter().map(|p| ts_type(p, defs, indent)).collect();
                    format!("[{}]", items.join(", "))
                } else {
                    let item = m.get("items").map_or_else(|| "unknown".to_owned(), |i| ts_type(i, defs, indent));
                    if item.contains(' ') { format!("({item})[]") } else { format!("{item}[]") }
                }
            }
            "object" => object_type(m, defs, indent),
            _ => "unknown".to_owned(),
        });
    }
    union(parts)
}

fn literal_type(v: &Value) -> String {
    match v {
        Value::String(s) => ts_string(s).replace('"', "'"),
        Value::Null => "null".to_owned(),
        other => other.to_string(),
    }
}

fn object_type(m: &Map<String, Value>, defs: &Map<String, Value>, indent: usize) -> String {
    let props = m.get("properties").and_then(Value::as_object);
    let additional = m.get("additionalProperties");
    match props {
        Some(p) if !p.is_empty() => {
            // A model object schema accepts the full object or its stub: `if: {required: [rev]}` selects the full
            // object, whose requirements are under `then` (the stub is its own `…Stub` type).
            let full = m.get("if") == Some(&serde_json::json!({ "required": ["rev"] }));
            let required_of = if full { m.get("then").unwrap_or(&Value::Null) } else { &Value::Null };
            let required: Vec<&str> = required_of
                .get("required")
                .or_else(|| m.get("required"))
                .and_then(Value::as_array)
                .map(|r| r.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let pad = "  ".repeat(indent + 1);
            let mut out = String::from("{\n");
            for (k, v) in p {
                let opt = if required.contains(&k.as_str()) { "" } else { "?" };
                if let Some(d) = v.get("description").and_then(Value::as_str) {
                    let _ = writeln!(out, "{pad}/** {} */", d.replace("*/", "* /").replace('\n', " "));
                }
                let _ = writeln!(out, "{pad}{}{opt}: {};", key(k), ts_type(v, defs, indent + 1));
            }
            out.push_str(&"  ".repeat(indent));
            out.push('}');
            out
        }
        _ => match additional {
            Some(Value::Object(_)) => {
                format!("Record<string, {}>", ts_type(additional.unwrap_or(&Value::Null), defs, indent))
            }
            _ => {
                if let Some(Value::Object(pp)) = m.get("patternProperties") {
                    let inner = union(pp.values().map(|v| ts_type(v, defs, indent)).collect());
                    format!("Record<string, {inner}>")
                } else {
                    "Record<string, unknown>".to_owned()
                }
            }
        },
    }
}

/// The stub interface of a model object definition: its ID field, `type` and (for named types) an optional `name`.
fn stub_type(def: &Value) -> Option<String> {
    def.get("x-objectType")?;
    let props = def.get("properties")?.as_object()?;
    let id_field = def.get("required")?.as_array()?.first()?.as_str()?;
    let ty = props.get("type")?.get("const")?;
    let mut out = format!("{{\n  {}: Id;\n  type: {};\n", key(id_field), literal_type(ty));
    if props.contains_key("name") {
        out.push_str("  name?: string;\n");
    }
    out.push('}');
    Some(out)
}

/// Whether a definition is a single object type (it can be an `interface`): it has `properties`, and any
/// `anyOf`/`oneOf` it carries only adds constraints (branches without `type`, `properties`, `$ref`, `enum`, `const`).
fn is_object_def(def: &Value) -> bool {
    let constraint_only = |alts: &Value| {
        alts.as_array().is_none_or(|a| {
            a.iter().all(|b| ["type", "properties", "$ref", "enum", "const"].iter().all(|k| b.get(k).is_none()))
        })
    };
    def.get("properties").is_some()
        && def.get("anyOf").is_none_or(constraint_only)
        && def.get("oneOf").is_none_or(constraint_only)
}

/// A complete TypeScript module.
///
/// `consts` are emitted as `export const NAME = <literal>;`, `defs_name` names the exported definitions object,
/// and one exported type per definition follows.
#[must_use]
pub fn module(header: &str, consts: &[(&str, Value)], defs_name: &str, defs: &Map<String, Value>) -> String {
    let mut out = String::new();
    for line in header.lines() {
        let _ = writeln!(out, "// {line}");
    }
    out.push_str("import type { Schema } from '../schema';\n\n");
    for (name, v) in consts {
        let mut lit = String::new();
        literal(v, &mut lit, 0, false);
        let _ = writeln!(out, "export const {name} = {lit};\n");
    }
    let _ = write!(out, "export const {defs_name}: Readonly<Record<string, Schema>> = ");
    literal(&Value::Object(defs.clone()), &mut out, 0, false);
    out.push_str(";\n");
    for (name, def) in defs {
        let t = ts_type(def, defs, 0);
        let doc = def
            .get("description")
            .and_then(Value::as_str)
            .map(|d| format!("/** {} */\n", d.replace("*/", "* /").replace('\n', " ")))
            .unwrap_or_default();
        if t.starts_with('{') && is_object_def(def) {
            let _ = write!(out, "\n{doc}export interface {} {t}\n", type_name(name));
        } else {
            let _ = write!(out, "\n{doc}export type {} = {t};\n", type_name(name));
        }
        if let Some(stub) = stub_type(def) {
            let n = type_name(name);
            let _ = write!(out, "\n/** A `{n}` reduced to its stub (10 §2.2). */\nexport interface {n}Stub {stub}\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn types_and_bigint_bounds() {
        let defs: Map<String, Value> = serde_json::from_value(json!({
            "A": {"type": "object", "properties": {"n": {"type": "integer", "minimum": 0, "maximum": u64::MAX},
                  "s": {"type": "integer", "minimum": 0, "maximum": 100}, "o": {"anyOf": [{"$ref": "#/$defs/B"}, {"type": "null"}]},
                  "t": {"type": "array", "prefixItems": [{"type": "string"}, {"type": "integer", "minimum": 1, "maximum": 5}]}},
                  "required": ["n"], "additionalProperties": false},
            "B": {"enum": ["x", "y"]}
        }))
        .unwrap();
        let m = module("generated", &[("V", json!("1.0"))], "DEFS", &defs);
        assert!(m.contains("maximum: 18446744073709551615n"), "{m}");
        assert!(m.contains("n: number | bigint;"), "{m}");
        assert!(m.contains("s?: number;"), "{m}");
        assert!(m.contains("o?: B | null;"), "{m}");
        assert!(m.contains("t?: [string, number];"), "{m}");
        assert!(m.contains("export type B = 'x' | 'y';"), "{m}");
        assert!(m.contains("export const V = \"1.0\";"), "{m}");
    }
}
