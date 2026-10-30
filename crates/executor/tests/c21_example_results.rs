//! M2 C2.1 obligation 1 (C14-R1-16, 11 §13 "describe_command includes at least one example request and its
//! result"): every declared example of every implemented command is executed on the fixture project and its result
//! is retained in `crates/registry/src/example-results.json`, which `describe_command` returns with the example.
//!
//! Checking (the default): each example runs again, its successful result must satisfy the command's result schema
//! and the envelope schema, and must reproduce the retained result exactly except at the pointers recorded as
//! varying (generated ids, times, tokens; found by executing twice when recording). CI runs this on Windows and
//! macOS, so the retained results are reproduced on both. Recording: `TT_RECORD_EXAMPLES=1 cargo test -p
//! tt-executor --test c21_example_results`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;
#[path = "common/examples.rs"]
mod examples;
#[path = "common/recorded.rs"]
mod recorded;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use examples::*;
use serde_json::{Value, json};
use tt_executor::{Invocation, Status};

/// Results longer than this (compact JSON bytes) are retained as `{bytes, sha256}` of their compact JSON: the embedded
/// sound of `doc.blob.get` and the element schema of `doc.schema.get` (both reproducible, both readable by running
/// the example).
const DIGEST_ABOVE: usize = 32 * 1024;

fn digest(v: &Value) -> Value {
    let text = serde_json::to_string(v).unwrap();
    json!({ "bytes": text.len(), "sha256": tt_persist::fsx::sha256(text.as_bytes()) })
}

fn results_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../registry/src/example-results.json")
}

/// One execution of every example: command id → `[{title, result}]` (portable paths).
fn execute_all() -> (BTreeMap<String, Vec<(String, Value)>>, Vec<String>) {
    let registry = tt_executor::registry().unwrap();
    let envelope_schema = tt_registry::schema_of::<tt_executor::Envelope>();
    let mut out: BTreeMap<String, Vec<(String, Value)>> = BTreeMap::new();
    let mut failures = Vec::new();
    for e in registry.entries() {
        let Some(c) = e.command else { continue };
        if e.spec.kind == tt_registry::Kind::Event || !matches!(e.status(), tt_registry::Status::Implemented) {
            continue;
        }
        for ex in c.decl.examples {
            let dir = std::env::temp_dir().join(format!("tt-c21-ex-{}", tt_persist::fsx::random_token(6)));
            std::fs::create_dir_all(&dir).unwrap();
            let files = with_files(e.id());
            let h = if files { prepared_with_files(e.id(), &dir) } else { prepared() };
            clipboard_for(&h, e.id());
            let mut params: Value = serde_json::from_str(ex.params).unwrap();
            if files {
                map_tmp(&mut params, &dir);
            }
            if e.id().starts_with("agent.") {
                agent_placeholders(&h, &dir, &mut params);
            }
            let env = h.invoke(Invocation::user(e.id(), params));
            if env.status != Status::Ok {
                failures.push(format!("{} `{}`: {:?}", e.id(), ex.title, env.error));
            } else {
                let mut result = env.result.clone().unwrap_or(Value::Null);
                for (what, schema, value) in [
                    ("result", e.result_schema(), result.clone()),
                    ("envelope", envelope_schema.clone(), serde_json::to_value(&env).unwrap()),
                ] {
                    let errors = recorded::schema_errors(&schema, &value);
                    if !errors.is_empty() {
                        failures.push(format!("{} `{}`: {what} schema: {errors:?}", e.id(), ex.title));
                    }
                }
                recorded::portable_paths(&mut result, &dir);
                out.entry(e.id().to_owned()).or_default().push((ex.title.to_owned(), result));
            }
            drop(h);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
    (out, failures)
}

#[test]
fn every_implemented_example_has_a_retained_executed_result() {
    let (first, failures) = execute_all();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    if recorded::recording() {
        let (second, failures) = execute_all();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        let mut file = serde_json::Map::new();
        for (id, list) in &first {
            let items: Vec<Value> = list
                .iter()
                .zip(&second[id])
                .map(|((title, a), (_, b))| {
                    let mut varies = Vec::new();
                    recorded::differences(a, b, "", &mut varies);
                    let text = serde_json::to_string(a).unwrap();
                    if text.len() > DIGEST_ABOVE {
                        assert!(varies.is_empty(), "{id} `{title}`: a digest needs a reproducible result");
                        json!({ "title": title, "resultDigest": digest(a), "varies": varies })
                    } else {
                        json!({ "title": title, "result": a, "varies": varies })
                    }
                })
                .collect();
            file.insert(id.clone(), Value::Array(items));
        }
        recorded::write_json(&results_file(), &Value::Object(file));
        return;
    }
    let stored = recorded::read_json(&results_file());
    let mut problems = Vec::new();
    let mut checked = 0;
    for (id, list) in &first {
        let Some(rec) = stored.get(id).and_then(Value::as_array) else {
            problems.push(format!("{id}: no retained result (re-record with TT_RECORD_EXAMPLES=1)"));
            continue;
        };
        for (title, live) in list {
            let Some(r) = rec.iter().find(|r| r["title"] == title.as_str()) else {
                problems.push(format!("{id} `{title}`: no retained result"));
                continue;
            };
            let varies: Vec<String> =
                r["varies"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
            if let Some(d) = r.get("resultDigest") {
                if *d != digest(live) {
                    problems.push(format!("{id} `{title}`: result digest differs"));
                }
            } else if let Err(e) = recorded::matches(&r["result"], live, &varies) {
                problems.push(format!("{id} `{title}`: {e}"));
            }
            checked += 1;
        }
    }
    for id in stored.as_object().unwrap().keys() {
        if !first.contains_key(id) {
            problems.push(format!("{id}: retained but no longer an implemented example"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    println!("retained example results reproduced: {checked}");
    assert!(checked >= 110, "{checked}");
}

/// The retained results are what `describe_command` shows (`examples[i].result`), and each validates against the
/// command's result schema as stored.
#[test]
fn describe_command_carries_the_retained_results() {
    let registry = tt_executor::registry().unwrap();
    let mut with_result = 0;
    for e in registry.entries() {
        if !matches!(e.status(), tt_registry::Status::Implemented) || e.spec.kind == tt_registry::Kind::Event {
            continue;
        }
        let d = e.describe();
        let examples = d["examples"].as_array().unwrap();
        assert!(!examples.is_empty(), "{}", e.id());
        assert!(
            examples[0].get("result").is_some() || examples[0].get("resultDigest").is_some(),
            "{}: first example has no executed result",
            e.id()
        );
        for ex in examples {
            if let Some(r) = ex.get("result") {
                let errors = recorded::schema_errors(&e.result_schema(), r);
                assert!(errors.is_empty(), "{} `{}`: {errors:?}", e.id(), ex["title"]);
                with_result += 1;
            }
        }
    }
    assert!(with_result >= 100, "{with_result}");
}
