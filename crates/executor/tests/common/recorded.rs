//! Executed, retained request/result records (M2 C2.1 obligation 1; 11 §13 "examples everywhere").
//!
//! A record is the JSON of a real execution. What legitimately differs between two executions (generated ids,
//! times, durations, tokens) is found by executing twice when recording, and is listed as JSON Pointers under
//! `varies`; everything else must reproduce exactly when the records are checked again (on every OS in CI). Paths
//! under the run's temporary folder are written as `/tmp/…` with `/` separators, so records do not depend on the
//! machine.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "test helpers")]

use std::path::Path;

use serde_json::{Map, Value};

/// Whether this run records (`TT_RECORD_EXAMPLES=1`) instead of checking.
pub fn recording() -> bool {
    std::env::var("TT_RECORD_EXAMPLES").is_ok_and(|v| v == "1")
}

/// Rewrites strings under `dir` (as given, or with `/` separators) to `/tmp/<rest>` with `/` separators.
pub fn portable_paths(v: &mut Value, dir: &Path) {
    let raw = dir.to_string_lossy().into_owned();
    let alt = raw.replace('\\', "/");
    portable(v, &raw, &alt);
}

fn portable(v: &mut Value, raw: &str, alt: &str) {
    match v {
        Value::String(s) => {
            for prefix in [raw, alt] {
                if !prefix.is_empty() && s.contains(prefix) {
                    let replaced = s.replace(prefix, "/tmp");
                    *s = replaced.replace('\\', "/");
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| portable(x, raw, alt)),
        Value::Object(m) => m.values_mut().for_each(|x| portable(x, raw, alt)),
        _ => {}
    }
}

fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// The pointers at which `a` and `b` differ (a differing structure counts at the shallowest differing node).
pub fn differences(a: &Value, b: &Value, at: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) if keys(x) == keys(y) => {
            for (k, v) in x {
                differences(v, &y[k], &format!("{at}/{}", escape(k)), out);
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                differences(p, q, &format!("{at}/{i}"), out);
            }
        }
        _ if a == b => {}
        _ => out.push(at.to_owned()),
    }
}

fn keys(m: &Map<String, Value>) -> Vec<&String> {
    m.keys().collect()
}

/// Checks `live` against the `recorded` value, ignoring the pointers in `varies` (and everything below them).
pub fn matches(recorded: &Value, live: &Value, varies: &[String]) -> Result<(), String> {
    let mut diffs = Vec::new();
    differences(recorded, live, "", &mut diffs);
    let unexpected: Vec<&String> = diffs
        .iter()
        .filter(|d| {
            !varies.iter().any(|v| *d == v || d.starts_with(&format!("{v}/")) || v.starts_with(&format!("{d}/")))
        })
        .collect();
    if unexpected.is_empty() { Ok(()) } else { Err(format!("differs at {unexpected:?}")) }
}

/// Validates `value` against a JSON Schema (draft 2020-12); the error messages.
pub fn schema_errors(schema: &Value, value: &Value) -> Vec<String> {
    match jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(schema) {
        Ok(v) => v.iter_errors(value).map(|e| format!("{} at {}", e, e.instance_path())).collect(),
        Err(e) => vec![format!("schema does not compile: {e}")],
    }
}

/// Writes JSON in the repository's text form (two spaces, `\n`, final newline).
pub fn write_json(path: &Path, v: &Value) {
    let mut text = serde_json::to_string_pretty(v).unwrap();
    text.push('\n');
    std::fs::write(path, text).unwrap();
}

pub fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}
