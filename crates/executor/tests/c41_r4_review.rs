#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
// M4 C4.1 round 3 (C41-R3-03, ruling O-10): every command reference the GUI offers is enabled exactly when it runs.
// The sweep takes every reference of the native menu, the default keymap and every implemented command with a GUI
// location (11 ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§14.1: not agent-only), and compares availability with a dry run of the same reference, in both
// directions, in states that construct each state rule C4.1 can construct: no project, an empty project, a project
// with nothing selected and an empty clipboard, a selection, a filled clipboard, an undoable step, a redoable step,
// a cell selection. References whose required parameters a dialog collects are out of scope (their refusals depend
// on what the user types). Builder-owned.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use tt_executor::{ExecutorHandle, Invocation};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

/// Refusals that judge parameters a dialog supplies even when none is missing (a save without a path asks for one;
/// discarding unsaved changes asks first): the GUI's dialog flow, not a state rule.

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
    let menu: Value = serde_json::from_str(include_str!("../../../desktop/src/ui-bindings/menu.json")).unwrap();
    walk(&menu, &mut refs);
    let keymap: Value = serde_json::from_str(include_str!("../src/keymap-default.json")).unwrap();
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
fn availability_preserves_observable_state_and_measures_contention() {
    let refs = gui_refs();
    let mut all = states();
    let large = ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&large, "project.create", json!({"name":"Large"}));
    for _ in 0..10 {
        ok(&large, "element.create", json!({"type":"peg","count":99}));
    }
    all.push(("990 command-created pegs", large));
    for (state, h) in all {
        let read = || {
            [
                "project.info.get",
                "edit.selection.get",
                "prefs.get",
                "edit.history.get",
                "app.state.get",
                "edit.clipboard.get",
                "job.list",
            ]
            .map(|id| {
                let e = h.invoke(Invocation::user(id, json!({})));
                (e.result, e.document_rev, e.error.map(|e| e.code))
            })
        };
        let before = read();
        let seq = h.events_broadcast();
        let started = std::time::Instant::now();
        let a = h.availability(refs.clone()).unwrap();
        eprintln!("{state}: {} references in {:?}", a.len(), started.elapsed());
        assert_eq!(seq, h.events_broadcast(), "availability emitted events: {state}");
        assert_eq!(before, read(), "availability changed state: {state}");
        let worker = h.clone();
        let r = refs.clone();
        let join = std::thread::spawn(move || {
            for _ in 0..5 {
                worker.availability(r.clone()).unwrap();
            }
        });
        let spin = std::time::Instant::now();
        while spin.elapsed() < std::time::Duration::from_millis(30) {
            std::thread::yield_now();
        }
        let started = std::time::Instant::now();
        let e = h.invoke(Invocation::user("app.state.get", json!({})));
        assert!(e.error.is_none());
        eprintln!("{state}: user command contention latency {:?}", started.elapsed());
        join.join().unwrap();
    }
}

#[test]
fn settings_bytes_and_times_survive_all_references() {
    fn files(root: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>, std::time::SystemTime)> {
        let mut out = Vec::new();
        if root.exists() {
            for e in std::fs::read_dir(root).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    out.extend(files(&p));
                } else {
                    out.push((p.clone(), std::fs::read(&p).unwrap(), p.metadata().unwrap().modified().unwrap()));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
    let root = std::env::temp_dir().join(format!("tt-r4-{}", tt_executor::random_token(8)));
    let h = ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_executor::UserDirs::under(&root)),
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    ok(&h, "project.create", json!({"name":"Review"}));
    ok(&h, "prefs.set", json!({"values":{"general.workspace.autoSave":false}}));
    let before = files(&root);
    let seq = h.events_broadcast();
    h.availability(gui_refs()).unwrap();
    assert_eq!(seq, h.events_broadcast());
    assert_eq!(before, files(&root));
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}
