//! M5 argument sweep (ruling E-26): every command this milestone adds, called with boundary arguments generated from
//! its own parameter schema (zero, negative, the largest integers, reversed pairs, empty arrays, missing optional
//! members, ids that do not exist), on the fixture, on a scene of length 1 and on an empty scene. The only assertion:
//! the call answers with a result or a structured error, and a read sent after it answers (the document thread is
//! alive). C5.1b and C5.2 extend [`COMMANDS`] with their commands.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Map, Value, json};
use tt_executor::{ExecutorHandle, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const MISSING_ELEMENT: &str = "scene:#99/element:#99";

/// The commands of M5 by checkpoint: every `timeline.*` and `playback.*` command (C5.1) and `xsheet.currentCell.set`.
/// C5.1b and C5.2 add theirs here.
const COMMANDS: &[&str] = &["xsheet.currentCell.set"];
const PREFIXES: &[&str] = &["timeline.", "playback."];

fn commands() -> Vec<&'static str> {
    let registry = tt_executor::registry().unwrap();
    let mut ids: Vec<&'static str> = registry
        .entries()
        .map(tt_registry::Entry::id)
        .filter(|id| PREFIXES.iter().any(|p| id.starts_with(p)) || COMMANDS.contains(id))
        .collect();
    ids.sort_unstable();
    assert!(ids.len() >= 29, "the 28 commands of C5.1 and xsheet.currentCell.set: {ids:?}");
    ids
}

/// A schema node with `$ref` resolved against the schema's definitions.
fn resolve<'a>(node: &'a Value, root: &'a Value) -> &'a Value {
    match node.get("$ref").and_then(Value::as_str) {
        Some(r) => {
            let name = r.rsplit('/').next().unwrap_or("");
            root.get("definitions").or_else(|| root.get("$defs")).and_then(|d| d.get(name)).unwrap_or(node)
        }
        None => node,
    }
}

/// Boundary values for a schema node.
fn candidates(node: &Value, root: &Value, depth: usize) -> Vec<Value> {
    let node = resolve(node, root);
    let mut out = Vec::new();
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = node.get(key).and_then(Value::as_array) {
            for b in branches {
                out.extend(candidates(b, root, depth));
            }
        }
    }
    if let Some(values) = node.get("enum").and_then(Value::as_array) {
        out.extend(values.iter().cloned());
        out.push(json!("nope"));
    }
    if let Some(c) = node.get("const") {
        out.push(c.clone());
    }
    let types: Vec<&str> = match node.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    for t in types {
        match t {
            "integer" => out.extend([0, -1, 1, 2, 9_999, 100_000, i64::MAX, i64::MIN].map(|v| json!(v))),
            "number" => out.extend([json!(0), json!(-1.5), json!(1), json!(120), json!(1e300), json!(-1e300)]),
            "string" => out.extend(["", "nope", MISSING_ELEMENT, "scene:#99", BOB, "scene:#1"].map(|v| json!(v))),
            "boolean" => out.extend([json!(true), json!(false)]),
            "null" => out.push(Value::Null),
            "array" => {
                out.push(json!([]));
                out.push(json!([BOB, MISSING_ELEMENT]));
                if depth < 2
                    && let Some(items) = node.get("items")
                {
                    for c in candidates(items, root, depth + 1).into_iter().take(4) {
                        out.push(json!([c]));
                    }
                }
            }
            "object" => {
                out.push(json!({}));
                if depth < 2
                    && let Some(props) = node.get("properties").and_then(Value::as_object)
                {
                    for (k, v) in props {
                        for c in candidates(v, root, depth + 1).into_iter().take(3) {
                            out.push(json!({ k: c }));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out.dedup();
    out
}

/// The values of an enumerated member (`enum`, or `oneOf`/`anyOf` of `const`s), or none.
fn enum_values(node: &Value, root: &Value) -> Vec<Value> {
    let node = resolve(node, root);
    let mut out: Vec<Value> = node.get("enum").and_then(Value::as_array).cloned().unwrap_or_default();
    for key in ["anyOf", "oneOf", "allOf"] {
        for b in node.get(key).and_then(Value::as_array).into_iter().flatten() {
            let b = resolve(b, root);
            if let Some(c) = b.get("const") {
                out.push(c.clone());
            }
            out.extend(enum_values(b, root));
        }
    }
    out.dedup();
    out
}

/// A plausible value for a required member (the first candidate a handler is likely to accept).
fn plausible(node: &Value, root: &Value) -> Value {
    let c = candidates(node, root, 0);
    c.iter()
        .find(|v| v.as_i64() == Some(1) || v.as_str() == Some(BOB) || v.as_bool() == Some(true))
        .or_else(|| c.iter().find(|v| v.is_string() && v.as_str() != Some("") && v.as_str() != Some("nope")))
        .or_else(|| c.first())
        .cloned()
        .unwrap_or(Value::Null)
}

/// The boundary calls of one command.
fn calls(schema: &Value) -> Vec<Value> {
    let props = schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
    let required: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let mut base = Map::new();
    for r in &required {
        if let Some(p) = props.get(r) {
            base.insert(r.clone(), plausible(p, schema));
        }
    }
    let mut out = vec![json!({}), Value::Object(base.clone())];
    for (k, p) in &props {
        for c in candidates(p, schema, 0) {
            let mut m = base.clone();
            m.insert(k.clone(), c);
            out.push(Value::Object(m));
        }
    }
    // Every pair of enumerated members, value by value (Next Drawing is `direction: next` with `unit: drawing`).
    let enums: Vec<(&String, Vec<Value>)> =
        props.iter().map(|(k, p)| (k, enum_values(p, schema))).filter(|(_, v)| !v.is_empty()).collect();
    for (i, (ka, va)) in enums.iter().enumerate() {
        for (kb, vb) in enums.iter().skip(i + 1) {
            for a in va {
                for b in vb {
                    let mut m = base.clone();
                    m.insert((*ka).clone(), a.clone());
                    m.insert((*kb).clone(), b.clone());
                    out.push(Value::Object(m));
                }
            }
        }
    }
    for (a, b) in [("from", "to"), ("start", "end"), ("from", "toFrame"), ("fromFrame", "toFrame")] {
        if props.contains_key(a) && props.contains_key(b) {
            for (x, y) in [(20, 3), (1, 0), (0, -5), (i64::MAX, 1), (5, 5)] {
                let mut m = base.clone();
                m.insert(a.to_owned(), json!(x));
                m.insert(b.to_owned(), json!(y));
                out.push(Value::Object(m));
            }
        }
    }
    out
}

/// The sample fixture with its second scene rebuilt by `edit` (the elements of that scene), then made current.
fn with_second_scene(edit: &dyn Fn(&mut Value, &mut Vec<Value>)) -> ExecutorHandle {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let mut scene = serde_json::to_value(&parts.scenes[1]).unwrap();
    let ids: Vec<String> =
        scene["elements"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().to_owned()).collect();
    let mut elements: Vec<Value> = parts
        .elements
        .iter()
        .filter(|e| ids.contains(&e.id().as_str().to_owned()))
        .map(|e| serde_json::to_value(e).unwrap())
        .collect();
    edit(&mut scene, &mut elements);
    let kept: Vec<String> = elements.iter().map(|e| e["id"].as_str().unwrap().to_owned()).collect();
    let scene_id = scene["id"].as_str().unwrap().to_owned();
    parts.scenes[1] = tt_model::object::from_json(&scene).unwrap();
    parts.elements.retain(|e| !ids.contains(&e.id().as_str().to_owned()));
    for e in &elements {
        parts.elements.push(tt_model::object::from_json(e).unwrap());
    }
    assert!(kept.iter().all(|k| ids.contains(k)));
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let h = ExecutorHandle::spawn(tt_executor::ExecutorConfig::default()).unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    ok(&h, "app.context.set", json!({"sceneId": scene_id}));
    h
}

fn empty_scene() -> ExecutorHandle {
    with_second_scene(&|scene, elements| {
        scene["tree"] = json!([]);
        scene["columnOrder"] = json!([]);
        scene["elements"] = json!([]);
        scene["currentCameraId"] = Value::Null;
        elements.clear();
    })
}

/// The second scene with one drawing exposed at frame 1 only (drawn through the kernel), current: the drawing steps
/// have a change to visit, and frame 1 is the scene's last.
fn one_frame_scene() -> ExecutorHandle {
    let h = with_second_scene(&|_, elements| {
        for e in elements.iter_mut() {
            if e.get("duration").is_some_and(|d| !d.is_null()) {
                e["duration"] = json!(1);
                e["start"] = json!(1);
            }
        }
    });
    let rows = ok(&h, "timeline.rows.list", json!({}))["rows"].as_array().cloned().unwrap_or_default();
    let drawing = rows.iter().find(|r| r["type"] == "drawing").unwrap()["elementId"].clone();
    ok(
        &h,
        "draw.rectangle.add",
        json!({"elementId": drawing, "frame": 1, "from": {"x": -10, "y": -10}, "to": {"x": 10, "y": 10}, "layer": "new"}),
    );
    ok(&h, "xsheet.currentCell.set", json!({"elementId": drawing, "frame": 1}));
    h
}

#[test]
fn every_m5_command_answers_every_boundary_call_and_the_engine_stays_alive() {
    let registry = tt_executor::registry().unwrap();
    let ids = commands();
    let fixtures: Vec<(&str, ExecutorHandle)> =
        vec![("fixture", fixture()), ("one-frame scene", one_frame_scene()), ("empty scene", empty_scene())];
    for (_, h) in &fixtures {
        assert_eq!(run(h, "timeline.rows.list", json!({})).status, Status::Ok);
    }
    let one_frame_len = ok(&fixtures[1].1, "timeline.rows.list", json!({}))["sceneLength"].clone();
    assert_eq!(one_frame_len, 1, "the one-frame scene");
    let mut total = 0;
    for id in &ids {
        let schema = registry.get(id).unwrap().params_schema();
        let list = calls(&schema);
        for (name, h) in &fixtures {
            for params in &list {
                total += 1;
                let env = run(h, id, params.clone());
                match env.status {
                    Status::Ok => {}
                    _ => {
                        let e = env.error.as_ref().unwrap_or_else(|| panic!("{name}: {id} {params}: no error"));
                        assert!(!e.code.is_empty(), "{name}: {id} {params}");
                        // A panic the engine contained (ruling E-29) is still a defect here: no call may answer it.
                        assert_ne!(
                            e.details["reason"], "handlerPanicked",
                            "{name}: {id} {params}: contained panic {e:?}"
                        );
                        assert_ne!(e.code, "internal", "{name}: {id} {params}: {e:?}");
                    }
                }
                let read = run(h, "timeline.rows.list", json!({}));
                assert!(
                    read.error.as_ref().is_none_or(|e| e.code != "internal"),
                    "{name}: after {id} {params} the read answers {read:?}"
                );
            }
            // Leave the clock still for the next command.
            run(h, "playback.stop", json!({}));
        }
    }
    // Ruling E-29 item 5: containment is the last line, not the fix: no handler panicked, contained or not.
    for (name, h) in &fixtures {
        assert_eq!(h.caught_panics().unwrap(), 0, "{name}: a handler panicked and was contained");
    }
    println!("{} commands, {total} boundary calls", ids.len());
}
