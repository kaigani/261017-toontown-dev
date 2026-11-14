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
        // The best of five passes, so a scheduling hiccup of a loaded runner does not decide it.
        let mut took = std::time::Duration::MAX;
        for _ in 0..5 {
            let started = std::time::Instant::now();
            let a = h.availability(refs.clone()).unwrap();
            took = took.min(started.elapsed());
            assert_eq!(a.len(), refs.len());
        }
        eprintln!("{state}: {} references in {took:?}", refs.len());
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
    // Ruling O-24: the large selections and clipboard, a long exposure column and a scene with many cameras. A
    // precheck reads such a value once after it changes (`memo`); the passes after that read nothing of it.
    // The first pass after a change: its total and the longest any of its requests held the document thread.
    let first = |h: &ExecutorHandle| {
        let holds = h.availability_holds(refs.clone()).unwrap();
        (holds.iter().sum::<Duration>(), holds.iter().copied().max().unwrap_or_default())
    };
    let ids: Vec<Value> =
        elements["elements"].as_array().unwrap().iter().filter_map(|x| x.get("id").cloned()).collect();
    let mut steady = Vec::new();
    ok(&large, "edit.selection.set", json!({ "context": "timeline", "ids": ids }));
    ok(&large, "edit.selection.set", json!({ "context": "staging", "ids": ids }));
    steady.push(("10,101 selected elements", first(&large), best(&large)));
    ok(&large, "edit.copy", json!({ "context": "staging" }));
    steady.push(("10,101 copied elements", first(&large), best(&large)));
    ok(&large, "edit.selection.set", json!({ "context": "timeline", "ids": [] }));
    ok(&large, "edit.selection.set", json!({ "context": "staging", "ids": [] }));
    let made =
        large.invoke(Invocation::user("element.create", json!({ "type": "drawing", "name": "Long" }))).result.unwrap();
    let long = made["elementIds"][0].as_str().unwrap().to_owned();
    let runs: Vec<Value> =
        (1..=10_000u32).map(|f| json!({ "drawing": if f % 2 == 0 { "2" } else { "1" }, "from": f, "to": f })).collect();
    ok(
        &large,
        "xsheet.exposures.set",
        json!({ "columns": [{ "elementId": long, "from": 1, "to": 10_000, "exposures": runs }], "createMissingDrawings": true }),
    );
    ok(&large, "app.context.set", json!({ "elementId": long, "frame": 5_000 }));
    steady.push(("a current column of 10,000 runs", first(&large), best(&large)));
    for _ in 0..10 {
        ok(&large, "element.create", json!({ "type": "camera", "count": 99 }));
    }
    let cams = large.invoke(Invocation::user("anim.camera.list", json!({}))).result.unwrap();
    let camera = cams["cameras"][0]["id"].as_str().unwrap().to_owned();
    ok(&large, "app.context.set", json!({ "elementId": camera, "frame": 1 }));
    steady.push(("991 cameras, one current", first(&large), best(&large)));
    for (what, (after_change, hold), pass) in &steady {
        eprintln!("{what}: first pass after the change {after_change:?} (longest request {hold:?}), then {pass:?}");
        assert!(
            *pass <= e + Duration::from_millis(20) && *pass < Duration::from_millis(50),
            "{what}: {pass:?} vs empty {e:?}"
        );
        assert!(*hold <= Duration::from_millis(20), "{what}: a request of the first pass held the thread {hold:?}");
    }
    // Each request on the document thread is one chunk of at most 32 references.
    let mut worst = Duration::ZERO;
    for chunk in refs.chunks(32) {
        worst = worst.max(large.availability_chunk_hold(chunk.to_vec()).unwrap());
    }
    eprintln!("longest chunk on the document thread: {worst:?}");
    assert!(worst <= Duration::from_millis(20), "{worst:?}");
    // A user command during continuous availability passes. The probe is a command that only changes session state
    // (the current frame): a command that writes a file (a preference) measures the disk, not the document thread.
    let user = |_: u64| {
        let t = Instant::now();
        ok(&large, "app.frame.set", json!({ "frame": 1 }));
        t.elapsed()
    };
    let quiet = (0..20).map(user).max().unwrap_or_default();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker = {
        let (h, r, stop) = (large.clone(), refs.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                h.availability(r.clone()).unwrap();
            }
        })
    };
    let slowest = (0..20).map(user).max().unwrap_or_default();
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    worker.join().unwrap();
    eprintln!("slowest user command: {quiet:?} without passes, {slowest:?} during continuous passes");
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

// ----- ruling O-24: the sweep grows with the inventory ---------------------------------------------------------------

/// The inventory (docs/availability-inventory.md): its `precheck` rows name a state in which the refusal occurs and one
/// in which it does not.
const INVENTORY: &str = include_str!("../../../../docs/availability-inventory.md");

/// A row's state: its name and the reference's parameters in it.
fn state_of(text: &str) -> (String, Map<String, Value>) {
    let text = text.trim();
    match text.split_once(' ') {
        Some((name, params)) => {
            (name.to_owned(), serde_json::from_str::<Value>(params).unwrap().as_object().cloned().unwrap())
        }
        None => (text.to_owned(), Map::new()),
    }
}

fn created_id(h: &ExecutorHandle, ty: &str) -> String {
    let e = h.invoke(Invocation::user("element.create", json!({ "type": ty })));
    assert!(e.error.is_none(), "{ty}: {e:?}");
    e.result.unwrap()["elementIds"][0].as_str().unwrap().to_owned()
}

fn select_rows(h: &ExecutorHandle, ids: &[&str]) {
    for context in ["timeline", "staging", "xsheet"] {
        ok(h, "edit.selection.set", json!({ "context": context, "ids": ids }));
    }
}

fn stroke(h: &ExecutorHandle, extra: Value) -> String {
    let mut p = json!({ "points": [{ "x": 0, "y": 0 }, { "x": 100, "y": 100 }] });
    if let (Some(m), Value::Object(x)) = (p.as_object_mut(), extra) {
        m.extend(x);
    }
    let e = h.invoke(Invocation::user("draw.stroke.add", p));
    assert!(e.error.is_none(), "{e:?}");
    e.result.unwrap()["strokeIds"][0].as_str().unwrap().to_owned()
}

fn current_drawing(h: &ExecutorHandle) -> String {
    let st = h.invoke(Invocation::user("app.state.get", json!({}))).result.unwrap();
    st["drawing"]["id"].as_str().unwrap().to_owned()
}

fn only_camera(h: &ExecutorHandle) -> String {
    let list = h.invoke(Invocation::user("anim.camera.list", json!({}))).result.unwrap();
    let cameras: Vec<&str> = list["cameras"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
    assert_eq!(cameras.len(), 1, "{list}");
    cameras[0].to_owned()
}

/// The states the inventory names (its header describes them); `a+b` applies `b` after `a`. Returns the objects the
/// state created, by the placeholder a row's parameters name them with (`$stroke`, `$text`).
fn build(name: &str) -> (ExecutorHandle, BTreeMap<&'static str, String>) {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    let mut vars = BTreeMap::new();
    let parts: Vec<&str> = name.split('+').collect();
    match parts[0] {
        "no-project" => return (h, vars),
        "no-drawing" => {
            ok(&h, "project.create", json!({ "name": "Empty" }));
            for part in &parts[1..] {
                apply_state(&h, part, &mut vars);
            }
            return (h, vars);
        }
        _ => {}
    }
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    for part in parts {
        apply_state(&h, part, &mut vars);
    }
    (h, vars)
}

fn apply_state(h: &ExecutorHandle, part: &str, vars: &mut BTreeMap<&'static str, String>) {
    match part {
        "stroke" | "locked-stroke" => {
            let s = stroke(h, json!({ "kind": "pencil" }));
            vars.insert("$stroke", format!("{}/{s}", current_drawing(h)));
            if part == "locked-stroke" {
                ok(h, "element.drawingLock.set", json!({ "locked": true }));
            }
        }
        "text" | "locked-text" => {
            let t =
                h.invoke(Invocation::user("draw.text.add", json!({ "text": "Ab", "position": { "x": 0, "y": 0 } })));
            assert!(t.error.is_none(), "{t:?}");
            let text = t.result.unwrap()["textId"].as_str().unwrap().to_owned();
            vars.insert("$text", format!("{}/{text}", current_drawing(h)));
            if part == "locked-text" {
                ok(h, "element.drawingLock.set", json!({ "locked": true }));
            }
        }
        "drawing" | "element" | "clipboard-empty" => {}
        "frame-3" => ok(h, "app.context.set", json!({ "elementId": BOB, "frame": 3 })),
        "locked-drawing" => ok(h, "element.drawingLock.set", json!({ "locked": true })),
        "drawing-selected" => {
            let s = stroke(h, json!({}));
            ok(h, "draw.selection.set", json!({ "ids": [s] }));
        }
        // M4 C4.2 (plan §8): a painted zone selected, no brush stroke (closed strokes and a paint, through commands).
        "painted-zone-selected" => {
            ok(
                h,
                "draw.rectangle.add",
                json!({ "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}, "layer": "new" }),
            );
            ok(h, "colour.paint.apply", json!({ "points": [{"x": 350, "y": 330}] }));
            ok(h, "draw.selection.set", json!({ "by": "point", "point": {"x": 350, "y": 330} }));
        }
        "locked-drawing-selected" => {
            apply_state(h, "drawing-selected", vars);
            ok(h, "element.drawingLock.set", json!({ "locked": true }));
        }
        "line-selected" => {
            let s = stroke(h, json!({ "kind": "pencil" }));
            ok(h, "draw.selection.set", json!({ "ids": [s] }));
        }
        "group-selected" => {
            let (a, b) = (stroke(h, json!({})), stroke(h, json!({})));
            let g = h.invoke(Invocation::user("draw.group.create", json!({ "ids": [a, b] })));
            assert!(g.error.is_none(), "{g:?}");
            let group = g.result.unwrap()["groupId"].as_str().unwrap().to_owned();
            ok(h, "draw.selection.set", json!({ "ids": [group] }));
        }
        "text-selected" => {
            let t =
                h.invoke(Invocation::user("draw.text.add", json!({ "text": "Ab", "position": { "x": 0, "y": 0 } })));
            assert!(t.error.is_none(), "{t:?}");
            let text = t.result.unwrap()["textId"].as_str().unwrap().to_owned();
            ok(h, "draw.selection.set", json!({ "ids": [text] }));
        }
        "anchors-selected" => {
            let s = stroke(h, json!({}));
            ok(h, "draw.pointSelection.set", json!({ "points": [{ "strokeId": s, "anchor": 0 }] }));
        }
        "peg-current" => {
            let peg = created_id(h, "peg");
            ok(h, "app.context.set", json!({ "elementId": peg, "frame": 1 }));
        }
        "sound-current" => {
            let sound = created_id(h, "sound");
            ok(h, "app.context.set", json!({ "elementId": sound, "frame": 1 }));
        }
        "empty-drawing-element" => {
            let d = created_id(h, "drawing");
            ok(h, "app.context.set", json!({ "elementId": d, "frame": 1 }));
        }
        "camera-current" => {
            let c = only_camera(h);
            ok(h, "app.context.set", json!({ "elementId": c, "frame": 1 }));
        }
        "locked-element" => ok(h, "element.lock.set", json!({ "elementIds": [BOB], "locked": true })),
        "blank-cell" => {
            ok(h, "cell.blank.add", json!({ "elementIds": [BOB], "frame": 1 }));
            ok(h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
        }
        "element-selected" => select_rows(h, &[BOB]),
        // M4 C4.2: the Drawing view window active (the `activeView` composition renders it, SI-107).
        "drawing-view" => ok(h, "view.active.set", json!({ "viewId": "drawingView" })),
        "peg-selected" => {
            let peg = created_id(h, "peg");
            select_rows(h, &[&peg]);
        }
        "locked-element-selected" => {
            apply_state(h, "locked-element", vars);
            select_rows(h, &[BOB]);
        }
        "camera-selected" => {
            let c = only_camera(h);
            select_rows(h, &[&c]);
        }
        "clipboard-cells" => ok(h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 1 })),
        "clipboard-objects" => {
            let s = stroke(h, json!({}));
            let d = current_drawing(h);
            ok(h, "edit.copy", json!({ "context": "drawing", "objectIds": [format!("{d}/{s}")] }));
        }
        "clipboard-elements" => ok(h, "edit.copy", json!({ "context": "staging", "objectIds": [BOB] })),
        "clipboard-elements-gone" => {
            let peg = created_id(h, "peg");
            ok(h, "edit.copy", json!({ "context": "staging", "objectIds": [peg] }));
            ok(h, "element.delete", json!({ "elementIds": [peg] }));
        }
        other => panic!("the inventory names a state the sweep does not build: {other}"),
    }
}

/// The parameters of a reference in a sweep state: the row's, completed for a dialog reference with the required
/// parameters of the command's first example that do not name a target (the row's state supplies the target).
fn completed(id: &str, params: Map<String, Value>) -> Map<String, Value> {
    const TARGETS: &[&str] = &[
        "drawingId",
        "elementId",
        "elementIds",
        "nodeIds",
        "objectIds",
        "ids",
        "frame",
        "sceneId",
        "strokeId",
        "textId",
    ];
    let registry = tt_executor::registry().unwrap();
    let Some(entry) = registry.get(id) else { return params };
    let schema = entry.params_schema();
    let required: Vec<String> =
        schema["required"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(str::to_owned)).collect();
    if required.iter().all(|r| params.contains_key(r)) {
        return params;
    }
    let example: Map<String, Value> = entry
        .command
        .and_then(|c| c.decl.examples.first())
        .and_then(|e| serde_json::from_str::<Value>(e.params).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let mut out = params;
    for r in required {
        if !out.contains_key(&r)
            && !TARGETS.contains(&r.as_str())
            && let Some(v) = example.get(&r)
        {
            out.insert(r, v.clone());
        }
    }
    out
}

/// Ruling O-24 item 3: for each `precheck` pair of the inventory, availability and a dry run agree in a state in
/// which the refusal occurs (both refuse, the dry run with that code) and in one in which it does not (the code does
/// not occur; availability equals the dry run).
#[test]
fn every_precheck_pair_refuses_and_passes_as_the_table_says() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for line in INVENTORY.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 7 || cells[3] != "precheck" || !cells[5].starts_with("refuse: ") {
            continue;
        }
        let (id, code) = (cells[1].trim_matches('`'), cells[2].trim_matches('`'));
        let (refuse, pass) = cells[5].trim_start_matches("refuse: ").split_once(" · pass: ").unwrap();
        for (which, text) in [("refuse", refuse), ("pass", pass)] {
            let (state, params) = state_of(text);
            let (h, vars) = build(&state);
            let mut raw = Value::Object(completed(id, params)).to_string();
            for (k, v) in &vars {
                raw = raw.replace(k, v);
            }
            let params = serde_json::from_str::<Value>(&raw).unwrap().as_object().cloned().unwrap();
            let a = h.availability(vec![(id.to_owned(), params.clone())]).unwrap().remove(0);
            let mut inv = Invocation::user(id, a.params.clone());
            inv.options.dry_run = true;
            let e = h.invoke(inv);
            let got = e.error.as_ref().map(|x| x.code.clone());
            let ok = match which {
                "refuse" => !a.dialog && !a.enabled && got.as_deref() == Some(code),
                _ => !a.dialog && got.as_deref() != Some(code) && a.enabled == e.is_ok(),
            };
            if !ok {
                failures.push(format!(
                    "{id} {code} [{which}: {state} {}]: dialog {} enabled {} ({:?}); dry run {:?}",
                    Value::Object(params),
                    a.dialog,
                    a.enabled,
                    a.reason,
                    e.error.map(|x| format!("{}: {}", x.code, x.message))
                ));
            }
        }
        checked += 1;
    }
    eprintln!("{checked} precheck pairs, both states each");
    assert!(checked > 150, "{checked}");
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}
