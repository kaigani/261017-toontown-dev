//! M4 C4.1 round 3 (C41-R3-03, ruling O-10): every command reference the GUI offers is enabled exactly when it runs.
//! The sweep takes every reference of the native menu, the default keymap and every implemented command with a GUI
//! location (11 §14.1: not agent-only), and compares availability with a dry run of the same reference, in both
//! directions, in states that construct each state rule C4.1 can construct: no project, an empty project, a project
//! with nothing selected and an empty clipboard, a selection, a filled clipboard, an undoable step, a redoable step,
//! a cell selection. References whose required parameters a dialog collects are out of scope (their refusals depend
//! on what the user types). Builder-owned.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use tt_executor::{ExecutorHandle, Invocation};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

/// Refusals that judge parameters a dialog supplies even when none is missing (a save without a path asks for one;
/// discarding unsaved changes asks first): the GUI's dialog flow, not a state rule.
const FORM_REFUSALS: &[&str] = &["project.pathRequired", "project.unsavedChanges"];

fn gui_refs() -> Vec<(String, Map<String, Value>)> {
    let mut refs: BTreeSet<(String, String)> = BTreeSet::new();
    let registry = tt_executor::registry().unwrap();
    for e in registry.entries() {
        if let Some(c) = e.command
            && c.decl.agent_only.is_none()
            && matches!(e.status(), tt_executor::DeclarationStatus::Implemented)
        {
            refs.insert((e.id().to_owned(), "{}".to_owned()));
        }
    }
    fn walk(v: &Value, out: &mut BTreeSet<(String, String)>) {
        match v {
            Value::Object(m) => {
                if let Some(id) = m.get("command").and_then(Value::as_str) {
                    let p = m.get("params").cloned().unwrap_or_else(|| json!({}));
                    out.insert((id.to_owned(), p.to_string()));
                }
                m.values().for_each(|x| walk(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let menu: Value = serde_json::from_str(include_str!("../../../../desktop/src/ui-bindings/menu.json")).unwrap();
    walk(&menu, &mut refs);
    let keymap: Value = serde_json::from_str(include_str!("../../src/keymap-default.json")).unwrap();
    for b in keymap["bindings"].as_array().unwrap() {
        let p = b.get("params").cloned().unwrap_or_else(|| json!({}));
        refs.insert((b["commandId"].as_str().unwrap().to_owned(), p.to_string()));
    }
    refs.into_iter()
        .filter(|(id, _)| {
            registry.get(id).is_some_and(|e| matches!(e.status(), tt_executor::DeclarationStatus::Implemented))
        })
        .map(|(id, p)| (id, serde_json::from_str::<Value>(&p).unwrap().as_object().cloned().unwrap_or_default()))
        .collect()
}

fn ok(h: &ExecutorHandle, id: &str, p: Value) {
    let e = h.invoke(Invocation::user(id, p.clone()));
    assert!(e.error.is_none(), "{id} {p}: {e:?}");
}

/// The states, each built from the previous one where it says so.
fn states() -> Vec<(&'static str, ExecutorHandle)> {
    let none = ExecutorHandle::spawn(Default::default()).unwrap();
    let empty = ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&empty, "project.create", json!({ "name": "Empty" }));
    let fixture = || {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        h
    };
    let nothing = fixture();
    let selected = fixture();
    ok(&selected, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
    let clipboard = fixture();
    ok(&clipboard, "edit.copy", json!({ "context": "staging", "objectIds": [BOB] }));
    let undo = fixture();
    ok(&undo, "doc.object.patch", json!({ "targets": [BOB], "props": { "meta": { "n": 1 } } }));
    let redo = fixture();
    ok(&redo, "doc.object.patch", json!({ "targets": [BOB], "props": { "meta": { "n": 1 } } }));
    ok(&redo, "edit.undo", json!({}));
    let cells = fixture();
    ok(&cells, "edit.selection.set", json!({ "context": "xsheet", "ids": [format!("{BOB}/cell:#1")] }));
    vec![
        ("no project", none),
        ("empty project", empty),
        ("nothing selected, empty clipboard", nothing),
        ("a staging selection", selected),
        ("a filled clipboard", clipboard),
        ("an undoable step", undo),
        ("a redoable step", redo),
        ("a cell selection", cells),
    ]
}

#[test]
fn every_gui_reference_is_enabled_exactly_when_it_runs() {
    let refs = gui_refs();
    assert!(refs.len() > 150, "{}", refs.len());
    let mut mismatches = Vec::new();
    // Per reference: the state refusals met, by state (the inventory of the round-3 response).
    let mut refusals: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut runnable = 0;
    for (state, h) in states() {
        for (id, params) in &refs {
            let a = h.availability(vec![(id.clone(), params.clone())]).unwrap().remove(0);
            if a.dialog {
                continue;
            }
            runnable += 1;
            let mut inv = Invocation::user(id, a.params.clone());
            inv.options.dry_run = true;
            let e = h.invoke(inv);
            if let Some(err) = &e.error {
                refusals
                    .entry(format!("{id} {}", Value::Object(params.clone())))
                    .or_default()
                    .insert(format!("{}: {state}", err.code));
                if FORM_REFUSALS.contains(&err.code.as_str()) {
                    continue;
                }
            }
            if a.enabled != e.is_ok() {
                mismatches.push(format!(
                    "{state}: {id} {}: available {} ({:?}) but the dry run {}",
                    Value::Object(params.clone()),
                    a.enabled,
                    a.reason,
                    e.error
                        .as_ref()
                        .map_or_else(|| "succeeds".to_owned(), |x| format!("refuses {}: {}", x.code, x.message))
                ));
            }
        }
    }
    if std::env::var_os("TT_PRINT_REFUSALS").is_some() {
        for (r, s) in &refusals {
            eprintln!("{r}\t{}", s.iter().cloned().collect::<Vec<_>>().join("; "));
        }
    }
    assert!(runnable > 200, "{runnable}");
    assert!(mismatches.is_empty(), "{} mismatches:\n{}", mismatches.len(), mismatches.join("\n"));
}

/// The cost of asking handlers: one availability call over every GUI reference (more than the native menu ever asks
/// at once) stays far below the executor's call budget in every state.
#[test]
fn asking_every_handler_stays_cheap() {
    let refs = gui_refs();
    for (state, h) in states() {
        let started = std::time::Instant::now();
        let a = h.availability(refs.clone()).unwrap();
        let took = started.elapsed();
        eprintln!("{state}: {} references in {took:?}", a.len());
        assert_eq!(a.len(), refs.len());
        assert!(took < std::time::Duration::from_millis(50), "{state}: {took:?}");
    }
}

/// Ruling O-23 budgets on a project of 10,000 command-created elements: a full pass costs no more than on an empty
/// project plus a small constant and stays under 50 ms; no availability request holds the document thread for more
/// than 20 ms; a user command submitted during continuous passes completes within 100 ms.
#[test]
fn passes_stay_constant_on_a_large_project() {
    use std::time::{Duration, Instant};
    let refs = gui_refs();
    let empty = ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&empty, "project.create", json!({ "name": "Empty" }));
    let large = ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&large, "project.create", json!({ "name": "Large" }));
    let built = Instant::now();
    for _ in 0..102 {
        ok(&large, "element.create", json!({ "type": "peg", "count": 99 }));
    }
    let elements = large.invoke(Invocation::user("element.list", json!({}))).result.unwrap_or_default();
    let n = elements["elements"].as_array().map_or(0, Vec::len);
    eprintln!("fixture: {n} elements built through commands in {:?}", built.elapsed());
    assert!(n >= 10_000, "{n}");
    // The best of five passes of each, so a scheduling hiccup of the runner does not decide the comparison.
    let best = |h: &ExecutorHandle| {
        (0..5)
            .map(|_| {
                let t = Instant::now();
                h.availability(refs.clone()).unwrap();
                t.elapsed()
            })
            .min()
            .unwrap()
    };
    let (e, l) = (best(&empty), best(&large));
    eprintln!("full pass over {} references: empty project {e:?}, 10,000 elements {l:?}", refs.len());
    assert!(l <= e + Duration::from_millis(20), "large {l:?} vs empty {e:?}");
    assert!(l < Duration::from_millis(50), "{l:?}");
    // Each request on the document thread is one chunk of at most 32 references.
    let mut worst = Duration::ZERO;
    for chunk in refs.chunks(32) {
        worst = worst.max(large.availability_chunk_hold(chunk.to_vec()).unwrap());
    }
    eprintln!("longest chunk on the document thread: {worst:?}");
    assert!(worst <= Duration::from_millis(20), "{worst:?}");
    // A user command during continuous availability passes.
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker = {
        let (h, r, stop) = (large.clone(), refs.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                h.availability(r.clone()).unwrap();
            }
        })
    };
    let mut slowest = Duration::ZERO;
    for i in 0..20 {
        let t = Instant::now();
        ok(&large, "ui.statusBar.set", json!({ "visible": i % 2 == 0 }));
        slowest = slowest.max(t.elapsed());
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    worker.join().unwrap();
    eprintln!("slowest user command during continuous passes: {slowest:?}");
    assert!(slowest < Duration::from_millis(100), "{slowest:?}");
}

fn boom_precheck(_cx: &tt_executor::Cx, _v: &Value) -> Result<(), tt_registry::CommandError> {
    panic!("a precheck that panics (test-only command)")
}

/// A test-only command whose precheck panics (ruling O-23 isolation).
#[tt_registry::command(
    id = "app.state.get",
    kind = query,
    label = "Test-only command with a panicking precheck",
    scopes = [read],
    precheck = boom_precheck,
    example(title = "Test only", params = "{}")
)]
fn boom(_cx: &mut tt_executor::Cx, _p: Value) -> Result<Value, tt_registry::CommandError> {
    Ok(json!({ "ran": true }))
}

/// Ruling O-23: a precheck that panics is contained: availability reports the reference unavailable with an internal
/// reason, an invocation answers `internal` without running the handler, and the session and the document thread
/// survive (both answer again afterwards).
#[test]
fn a_panicking_precheck_is_contained() {
    let registry = Box::leak(Box::new(tt_registry::Registry::assemble(&[&BOOM]).unwrap()));
    let h = ExecutorHandle::spawn_with_registry(tt_executor::ExecutorConfig::default(), registry).unwrap();
    for _ in 0..2 {
        let a = h.availability(vec![("app.state.get".into(), serde_json::Map::new())]).unwrap().remove(0);
        assert!(!a.enabled && a.reason.as_deref().is_some_and(|r| r.starts_with("internal")), "{a:?}");
        let e = h.invoke(Invocation::user("app.state.get", json!({})));
        let err = e.error.expect("refused");
        assert_eq!(err.code, "internal", "{err:?}");
        assert!(e.result.is_none(), "the handler did not run");
    }
}
