//! M5 argument sweep (rulings E-26, E-33): every command this milestone adds, called with boundary arguments generated
//! from its own parameter schema (each member's minimum, maximum and the values just outside them; zero, negative and
//! the largest integers; one valid value for a patterned string; reversed pairs; empty arrays; objects with every member
//! set; missing optional members; ids that do not exist), on the fixture, on a scene of length 1 and on an empty scene.
//! Each call answers with a result or a structured error other than `internal`, and a read sent after it answers (the
//! document thread is alive). A member is varied on a valid base call: the other required members take valid values,
//! each enumerated value in turn (ruling E-38), so a boundary value is refused only by its own member's schema. Per
//! command it counts the calls that passed validation and reached the handler, and per numeric member the boundary
//! values that reached it; it prints the table and fails when a callable command, or a numeric member of one, has none.
//! The gesture entry (`timing_gesture`, `timing_replay`) gets its own sweep of coordinates (ruling E-31). C5.1b and C5.2
//! extend [`COMMANDS`] with their commands.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tt_executor::{ExecutorHandle, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const MISSING_ELEMENT: &str = "scene:#99/element:#99";

/// The commands of M5 by checkpoint: every `timeline.*` and `playback.*` command (C5.1), `xsheet.currentCell.set`, and
/// the seven `xsheet.*` commands of C5.2a (ruling E-70, C52A-R1S-05). C5.2b adds theirs here.
const COMMANDS: &[&str] = &[
    "xsheet.currentCell.set",
    "xsheet.display.set",
    "xsheet.panel.set",
    "xsheet.columnWidth.set",
    "xsheet.lightTable.add",
    "xsheet.lightTable.move",
    "xsheet.lightTable.remove",
    "xsheet.lightTable.set",
];
const PREFIXES: &[&str] = &["timeline.", "playback."];

fn commands() -> Vec<&'static str> {
    let registry = tt_executor::registry().unwrap();
    let mut ids: Vec<&'static str> = registry
        .entries()
        .map(tt_registry::Entry::id)
        .filter(|id| PREFIXES.iter().any(|p| id.starts_with(p)) || COMMANDS.contains(id))
        // Implemented commands only: the prefixes also name later milestones' not-implemented stubs (`playback.player.*`).
        .filter(|id| {
            registry.get(id).is_some_and(|e| !matches!(e.status(), tt_registry::Status::NotImplemented { .. }))
        })
        .collect();
    ids.sort_unstable();
    assert!(ids.len() >= 36, "the 28 commands of C5.1, xsheet.currentCell.set and the seven of C5.2a: {ids:?}");
    for id in COMMANDS {
        assert!(ids.contains(id), "{id} is selected");
    }
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

/// One string matching a JSON-schema `pattern` of the subset the M5 schemas use: anchors, literals, character classes
/// (the first character of the class), `\\d`, `{n}` / `{n,m}` / `+` / `*` / `?` quantifiers and groups (an optional group
/// is left out).
fn sample(pattern: &str) -> String {
    fn atom(chars: &[char], i: &mut usize) -> String {
        match chars.get(*i) {
            Some('[') => {
                let start = *i + 1;
                while chars.get(*i).is_some_and(|c| *c != ']') {
                    *i += 1;
                }
                *i += 1;
                chars.get(start).map(|c| c.to_string()).unwrap_or_default()
            }
            Some('(') => {
                *i += 1;
                let mut inner = String::new();
                while chars.get(*i).is_some_and(|c| *c != ')') {
                    inner.push_str(&piece(chars, i));
                }
                *i += 1;
                inner
            }
            Some('\\') => {
                *i += 2;
                match chars.get(*i - 1) {
                    Some('d') => "0".to_owned(),
                    Some(c) => c.to_string(),
                    None => String::new(),
                }
            }
            Some('^' | '$') => {
                *i += 1;
                String::new()
            }
            Some(c) => {
                *i += 1;
                c.to_string()
            }
            None => String::new(),
        }
    }
    fn piece(chars: &[char], i: &mut usize) -> String {
        let a = atom(chars, i);
        match chars.get(*i) {
            Some('{') => {
                let start = *i + 1;
                while chars.get(*i).is_some_and(|c| *c != '}') {
                    *i += 1;
                }
                let spec: String = chars[start..*i].iter().collect();
                *i += 1;
                let n: usize = spec.split(',').next().and_then(|n| n.trim().parse().ok()).unwrap_or(1);
                a.repeat(n)
            }
            Some('+') => {
                *i += 1;
                a
            }
            Some('*' | '?') => {
                *i += 1;
                String::new()
            }
            _ => a,
        }
    }
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    let mut out = String::new();
    while i < chars.len() {
        out.push_str(&piece(&chars, &mut i));
    }
    out
}

/// The schema's own endpoints of a number: its minimum and maximum and the values just outside them.
fn endpoints(node: &Value, integer: bool) -> Vec<Value> {
    let step = if integer { 1.0 } else { 0.5 };
    let mut out = Vec::new();
    for (k, outside) in [("minimum", -step), ("maximum", step), ("exclusiveMinimum", 0.0), ("exclusiveMaximum", 0.0)] {
        if let Some(v) = node.get(k).and_then(Value::as_f64) {
            for x in [
                v,
                v + outside,
                if k.starts_with("exclusive") { v + if k == "exclusiveMinimum" { step } else { -step } } else { v },
            ] {
                out.push(if integer { json!(x as i64) } else { json!(x) });
            }
        }
    }
    out
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
            "integer" => {
                out.extend(endpoints(node, true));
                out.extend([0, -1, 1, 2, 9_999, 100_000, i64::MAX, i64::MIN].map(|v| json!(v)));
            }
            "number" => {
                out.extend(endpoints(node, false));
                out.extend([json!(0), json!(-1.5), json!(1), json!(120), json!(1e300), json!(-1e300)]);
            }
            "string" => {
                if let Some(pattern) = node.get("pattern").and_then(Value::as_str) {
                    out.push(json!(sample(pattern)));
                }
                out.extend(["", "nope", MISSING_ELEMENT, "scene:#99", BOB, "scene:#1"].map(|v| json!(v)));
            }
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
                    // Every member set at once, each to its plausible value (E-33: members combined).
                    let all: Map<String, Value> = props.iter().map(|(k, v)| (k.clone(), plausible(v, root))).collect();
                    out.push(Value::Object(all));
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

/// A plausible value for a member (the first candidate a handler is likely to accept): an enumerated member's first
/// value (ruling E-38), a patterned string's sample, else the likeliest candidate.
fn plausible(node: &Value, root: &Value) -> Value {
    if let Some(first) = enum_values(node, root).into_iter().next() {
        return first;
    }
    let resolved = resolve(node, root);
    if let Some(pattern) = resolved.get("pattern").and_then(Value::as_str) {
        return json!(sample(pattern));
    }
    let c = candidates(node, root, 2);
    c.iter()
        .find(|v| v.as_i64() == Some(1) || v.as_str() == Some(BOB) || v.as_bool() == Some(true))
        .or_else(|| c.iter().find(|v| v.is_string() && v.as_str() != Some("") && v.as_str() != Some("nope")))
        .or_else(|| c.first())
        .cloned()
        .unwrap_or(Value::Null)
}

/// The valid values of a required member: each enumerated value, or one plausible value (ruling E-38).
fn valid_values(node: &Value, root: &Value) -> Vec<Value> {
    let values = enum_values(node, root);
    if values.is_empty() { vec![plausible(node, root)] } else { values }
}

/// Whether a member is numeric (`integer` or `number`, directly or in a branch).
fn numeric(node: &Value, root: &Value) -> bool {
    let node = resolve(node, root);
    let typed = match node.get("type") {
        Some(Value::String(t)) => t == "integer" || t == "number",
        Some(Value::Array(a)) => a.iter().any(|t| t == "integer" || t == "number"),
        _ => false,
    };
    typed
        || ["anyOf", "oneOf", "allOf"]
            .iter()
            .any(|k| node.get(*k).and_then(Value::as_array).is_some_and(|b| b.iter().any(|b| numeric(b, root))))
}

/// The most valid base calls kept: the product of the required members' valid values when it is no larger, otherwise
/// one base per value index, each member taking its values in turn.
const BASE_CAP: usize = 12;

/// The valid base calls: the required members' valid values combined (ruling E-38).
fn bases(schema: &Value, props: &Map<String, Value>, required: &[String]) -> Vec<Map<String, Value>> {
    let lists: Vec<(String, Vec<Value>)> =
        required.iter().filter_map(|r| props.get(r).map(|p| (r.clone(), valid_values(p, schema)))).collect();
    let product: usize = lists.iter().map(|(_, v)| v.len().max(1)).product();
    let mut out: Vec<Map<String, Value>> = Vec::new();
    if product <= BASE_CAP {
        out.push(Map::new());
        for (k, values) in &lists {
            out = out
                .iter()
                .flat_map(|b| {
                    values.iter().map(move |v| {
                        let mut m = b.clone();
                        m.insert(k.clone(), v.clone());
                        m
                    })
                })
                .collect();
        }
    } else {
        let longest = lists.iter().map(|(_, v)| v.len()).max().unwrap_or(1);
        for i in 0..longest {
            out.push(lists.iter().map(|(k, v)| (k.clone(), v[i % v.len()].clone())).collect());
        }
    }
    out
}

/// One boundary call, and the member it varies with the value it gives it.
struct Call {
    params: Value,
    varied: Option<(String, Value)>,
}

/// The boundary calls of one command.
fn calls(schema: &Value) -> Vec<Call> {
    let props = schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
    let required: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let all_bases = bases(schema, &props, &required);
    let base = all_bases.first().cloned().unwrap_or_default();
    let plain = |v: Value| Call { params: v, varied: None };
    let mut out = vec![plain(json!({}))];
    out.extend(all_bases.iter().cloned().map(|b| plain(Value::Object(b))));
    for (k, p) in &props {
        // The bases that differ in the other members (a varied member's own base value is replaced).
        let mut others: Vec<Map<String, Value>> = Vec::new();
        for b in &all_bases {
            let mut m = b.clone();
            m.remove(k);
            if !others.contains(&m) {
                others.push(m);
            }
        }
        for c in candidates(p, schema, 0) {
            for b in &others {
                let mut m = b.clone();
                m.insert(k.clone(), c.clone());
                out.push(Call { params: Value::Object(m), varied: Some((k.clone(), c.clone())) });
            }
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
                    out.push(plain(Value::Object(m)));
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
                out.push(plain(Value::Object(m)));
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
    let mut table: Vec<(&str, usize, usize, String)> = Vec::new();
    // Per numeric member of a callable command: its boundary values, and those that reached the handler (ruling E-38).
    let mut numeric_unreached: Vec<String> = Vec::new();
    for id in &ids {
        let schema = registry.get(id).unwrap().params_schema();
        let list = calls(&schema);
        let props = schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
        let numeric_members: Vec<&String> = props.iter().filter(|(_, p)| numeric(p, &schema)).map(|(k, _)| k).collect();
        let mut tried: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut hit: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut reached = 0;
        for (name, h) in &fixtures {
            for call in &list {
                let params = &call.params;
                total += 1;
                let env = run(h, id, params.clone());
                // Reached the handler: answered, or refused by anything but parameter validation, the project check,
                // enablement or a precheck (`command.disabled`).
                let refused_before = env.error.as_ref().is_some_and(|e| {
                    matches!(
                        e.code.as_str(),
                        "arg.missing" | "arg.unknown" | "project.none" | "command.disabled" | "command.notImplemented"
                    ) || (e.code == "arg.invalid" && e.details.get("violations").is_some())
                });
                if !refused_before {
                    reached += 1;
                }
                if let Some((member, value)) = &call.varied
                    && value.is_number()
                    && let Some(m) = numeric_members.iter().find(|m| **m == member)
                {
                    tried.entry(m.as_str()).or_default().insert(value.to_string());
                    if !refused_before {
                        hit.entry(m.as_str()).or_default().insert(value.to_string());
                    } else if let Some(e) = env.error.as_ref().filter(|e| e.code.starts_with("arg.")) {
                        // On a valid base only the member's own schema may refuse its value (ruling E-38); enablement
                        // and the project check refuse the command, not the value.
                        let own = format!("/{m}");
                        let pointers: Vec<&str> = e.details["violations"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v["pointer"].as_str())
                            .collect();
                        assert!(
                            e.code == "arg.invalid"
                                && !pointers.is_empty()
                                && pointers.iter().all(|p| *p == own || p.starts_with(&format!("{own}/"))),
                            "{name}: {id} {params}: refused by more than `{m}`'s schema: {e:?}"
                        );
                    }
                }
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
        let callable = registry.get(id).is_some_and(|e| e.spec.kind != tt_registry::Kind::Event);
        let mut column = Vec::new();
        for m in &numeric_members {
            let (t, r) = (tried.get(m.as_str()).map_or(0, BTreeSet::len), hit.get(m.as_str()).map_or(0, BTreeSet::len));
            column.push(format!("{m} {r}/{t}"));
            if callable && r == 0 {
                numeric_unreached.push(format!("{id} {m}"));
            }
        }
        table.push((id, list.len() * fixtures.len(), reached, column.join(", ")));
    }
    println!(
        "{:<40} {:>6} {:>8}  numeric members: boundary values that reached the handler",
        "command", "calls", "reached"
    );
    for (id, calls, reached, numeric) in &table {
        println!("{id:<40} {calls:>6} {reached:>8}  {numeric}");
    }
    // Ruling E-33: a callable command (not an event) none of whose calls reached its handler is a sweep that proves
    // nothing about it.
    let unreached: Vec<&str> = table
        .iter()
        .filter(|(id, _, reached, _)| {
            *reached == 0 && registry.get(id).is_some_and(|e| e.spec.kind != tt_registry::Kind::Event)
        })
        .map(|(id, ..)| *id)
        .collect();
    assert!(unreached.is_empty(), "no call reached the handler of {unreached:?}");
    // Ruling E-38: a numeric member none of whose boundary values reached the handler was not swept.
    assert!(
        numeric_unreached.is_empty(),
        "no boundary value of these numeric members reached the handler: {numeric_unreached:?}"
    );
    // Ruling E-29 item 5: containment is the last line, not the fix: no handler panicked, contained or not.
    for (name, h) in &fixtures {
        assert_eq!(h.caught_panics().unwrap(), 0, "{name}: a handler panicked and was contained");
    }
    println!("{} commands, {total} boundary calls", ids.len());
}

/// Ruling E-31: the gesture entry (the window's `timing_gesture` and the headless `timing_replay`) refuses a coordinate
/// beyond its stated bound with `arg.invalid`, and accepts one at the bound; no coordinate panics or wraps, and the engine
/// answers after every call.
#[test]
fn the_gesture_entry_refuses_huge_coordinates_and_stays_alive() {
    let h = fixture();
    let bound = tt_executor::TIMING_COORDINATE_BOUND;
    let huge = [1e300, -1e300, f64::MAX, f64::MIN, 9_007_199_254_740_991.0, -9_007_199_254_740_991.0, bound + 1.0];
    let layout = |surface: &str, zoom: u32, scroll: f64| json!({"surface": surface, "zoomLevel": zoom, "firstFrame": 1, "scrollTop": scroll, "platform": "windows"});
    let mut refused = 0;
    for surface in ["ruler", "tracks", "layers", "xsheetGrid", "xsheetHeaders"] {
        for zoom in [0, 9] {
            for v in huge {
                for sample in [json!({"x": v, "y": 12}), json!({"x": 12, "y": v})] {
                    let replay =
                        h.timing_replay(tt_executor::Origin::User, layout(surface, zoom, 0.0), json!([sample]), false);
                    let e = replay.expect_err("refused");
                    assert_eq!(e.code, "arg.invalid", "{surface} {zoom} {sample}: {e:?}");
                    let press = h.timing_gesture(
                        tt_executor::Origin::User,
                        json!({"phase": "press", "layout": layout(surface, zoom, 0.0), "sample": sample}),
                    );
                    assert_eq!(press.expect_err("refused").code, "arg.invalid");
                    refused += 2;
                    assert!(run(&h, "timeline.rows.list", json!({})).is_ok(), "alive after {sample}");
                }
                // A huge scroll position in the layout.
                let e = h.timing_replay(
                    tt_executor::Origin::User,
                    layout(surface, zoom, v),
                    json!([{"x": 1, "y": 1}]),
                    false,
                );
                assert_eq!(e.expect_err("refused").code, "arg.invalid");
            }
            // At the bound: answered (a plan, possibly nothing), never a panic or a wrapped frame.
            for v in [bound, -bound] {
                let plan = h.timing_replay(
                    tt_executor::Origin::User,
                    layout(surface, zoom, 0.0),
                    json!([{"x": v, "y": 5}]),
                    false,
                );
                let plan = plan.expect("answered at the bound");
                if surface == "ruler" && v > 0.0 {
                    let frame = plan["plan"]["commands"][0][1]["frame"].as_u64().unwrap_or(0);
                    assert!(frame > 1, "a huge positive coordinate never wraps to frame 1: {plan}");
                }
            }
            // A release whose samples go beyond the bound.
            let press = h
                .timing_gesture(
                    tt_executor::Origin::User,
                    json!({"phase": "press", "layout": layout(surface, zoom, 0.0), "sample": {"x": 5, "y": 5}}),
                )
                .expect("press");
            let release = h.timing_gesture(
                tt_executor::Origin::User,
                json!({"phase": "release", "grabId": press["grabId"], "samples": [{"x": 5, "y": 5}, {"x": 1e300, "y": 5}]}),
            );
            assert_eq!(release.expect_err("refused").code, "arg.invalid");
        }
    }
    println!("{refused} huge coordinates refused");
    assert_eq!(h.caught_panics().unwrap(), 0);
}

/// Rulings E-31 and E-70 (C52A-R1S-04): every numeric member of the layout, for every surface, is bounded at the entry
/// before any arithmetic: the scroll offsets and the light-table rectangle beyond the coordinate bound (or a negative
/// size), a zoom level past the ladder and a first frame outside 1-100000 are `arg.invalid` at their pointer, at the
/// press and in the replay; at the bound they are answered; the engine answers after every call.
#[test]
fn the_gesture_entry_bounds_every_numeric_member_of_the_layout() {
    let h = fixture();
    let bound = tt_executor::TIMING_COORDINATE_BOUND;
    let base = |surface: &str| {
        json!({"surface": surface, "zoomLevel": 0, "firstFrame": 1, "scrollTop": 0, "scrollLeft": 0,
               "lightTable": {"x": 0, "y": 600, "width": 300, "height": 200}, "platform": "windows"})
    };
    let mut cases: Vec<(&str, Value)> = Vec::new();
    for v in [1e300, -1e300, f64::MAX, bound + 1.0, -(bound + 1.0)] {
        for member in
            ["/scrollTop", "/scrollLeft", "/lightTable/x", "/lightTable/y", "/lightTable/width", "/lightTable/height"]
        {
            cases.push((member, json!(v)));
        }
    }
    cases.push(("/lightTable/width", json!(-1.0)));
    cases.push(("/lightTable/height", json!(-0.5)));
    cases.push(("/zoomLevel", json!(10_000)));
    cases.push(("/firstFrame", json!(0)));
    cases.push(("/firstFrame", json!(100_001)));
    let mut refused = 0;
    for surface in ["ruler", "tracks", "layers", "xsheetGrid", "xsheetHeaders"] {
        for (member, v) in &cases {
            let mut layout = base(surface);
            *layout.pointer_mut(member).unwrap() = v.clone();
            let e = h.timing_replay(tt_executor::Origin::User, layout.clone(), json!([{"x": 5, "y": 5}]), false);
            let e = e.expect_err("refused");
            assert_eq!(
                (e.code.as_str(), e.details["pointer"].as_str()),
                ("arg.invalid", Some(format!("/layout{member}").as_str())),
                "{surface} {member} {v}: {e:?}"
            );
            let press = h.timing_gesture(
                tt_executor::Origin::User,
                json!({"phase": "press", "layout": layout, "sample": {"x": 5, "y": 5}}),
            );
            assert_eq!(press.expect_err("refused").code, "arg.invalid", "{surface} {member} {v}");
            refused += 2;
            assert!(run(&h, "timeline.rows.list", json!({})).is_ok(), "alive after {surface} {member} {v}");
        }
        // At the bound every member is answered.
        for member in
            ["/scrollTop", "/scrollLeft", "/lightTable/x", "/lightTable/y", "/lightTable/width", "/lightTable/height"]
        {
            let mut layout = base(surface);
            *layout.pointer_mut(member).unwrap() = json!(bound);
            assert!(
                h.timing_replay(tt_executor::Origin::User, layout, json!([{"x": 5, "y": 5}]), false).is_ok(),
                "{surface} {member} at the bound"
            );
        }
    }
    println!("{refused} out-of-bound layout members refused");
    assert_eq!(h.caught_panics().unwrap(), 0);
}
