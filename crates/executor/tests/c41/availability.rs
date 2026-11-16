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

/// A project of `batches` × 99 command-created peg elements (and the project's own), with its element ids.
fn project_of(batches: usize) -> (ExecutorHandle, Vec<Value>) {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&h, "project.create", json!({ "name": "Availability" }));
    for _ in 0..batches {
        ok(&h, "element.create", json!({ "type": "peg", "count": 99 }));
    }
    let elements = h.invoke(Invocation::user("element.list", json!({}))).result.unwrap_or_default();
    let ids = elements["elements"].as_array().unwrap().iter().filter_map(|x| x.get("id").cloned()).collect();
    (h, ids)
}

/// One case of ruling O-24 measured on a project: the longest request of the first availability pass after each real
/// repetition of the change, and the best of five steady passes after them.
struct Case {
    what: &'static str,
    holds: Vec<std::time::Duration>,
    steady: std::time::Duration,
}

/// The four cases of ruling O-24 on `h` (whose elements are `ids`): the selections of every element, a copy of them, a
/// current exposure column of `runs` runs, and a scene of `cameras` cameras with one current; each change is made
/// `reps` times, each repetition a real change (ruling D-34: the selections cleared and set again, another copy, the
/// column's runs in another phase, one more camera and another current one).
fn o24_cases(h: &ExecutorHandle, ids: &[Value], runs: u32, cameras: usize, reps: usize) -> Vec<Case> {
    use std::time::{Duration, Instant};
    let refs = gui_refs();
    let best = || {
        (0..5)
            .map(|_| {
                let t = Instant::now();
                h.availability(refs.clone()).unwrap();
                t.elapsed()
            })
            .min()
            .unwrap()
    };
    let first = || h.availability_holds(refs.clone()).unwrap().into_iter().max().unwrap_or_default();
    let measure = |what: &'static str, change: &dyn Fn(usize)| -> Case {
        let holds: Vec<Duration> = (0..reps)
            .map(|k| {
                change(k);
                first()
            })
            .collect();
        Case { what, holds, steady: best() }
    };
    let mut out = Vec::new();
    let select = |_: usize| {
        for context in ["timeline", "staging"] {
            ok(h, "edit.selection.set", json!({ "context": context, "ids": [] }));
            ok(h, "edit.selection.set", json!({ "context": context, "ids": ids }));
        }
    };
    out.push(measure("selected elements", &select));
    let copy = |_: usize| ok(h, "edit.copy", json!({ "context": "staging" }));
    out.push(measure("copied elements", &copy));
    ok(h, "edit.selection.set", json!({ "context": "timeline", "ids": [] }));
    ok(h, "edit.selection.set", json!({ "context": "staging", "ids": [] }));
    let made =
        h.invoke(Invocation::user("element.create", json!({ "type": "drawing", "name": "Long" }))).result.unwrap();
    let long = made["elementIds"][0].as_str().unwrap().to_owned();
    let column = |k: usize| {
        let list: Vec<Value> = (1..=runs)
            .map(|f| json!({ "drawing": if (f as usize + k).is_multiple_of(2) { "2" } else { "1" }, "from": f, "to": f }))
            .collect();
        ok(
            h,
            "xsheet.exposures.set",
            json!({ "columns": [{ "elementId": long, "from": 1, "to": runs, "exposures": list }], "createMissingDrawings": true }),
        );
        ok(h, "app.context.set", json!({ "elementId": long, "frame": runs / 2 }));
    };
    out.push(measure("a current exposure column", &column));
    let mut left = cameras;
    while left > 0 {
        let n = left.min(99);
        ok(h, "element.create", json!({ "type": "camera", "count": n }));
        left -= n;
    }
    let cams = h.invoke(Invocation::user("anim.camera.list", json!({}))).result.unwrap();
    let current = |k: usize| {
        if k > 0 {
            ok(h, "element.create", json!({ "type": "camera", "count": 1 }));
        }
        let camera = cams["cameras"][k]["id"].as_str().unwrap().to_owned();
        ok(h, "app.context.set", json!({ "elementId": camera, "frame": 1 }));
    };
    out.push(measure("cameras, one current", &current));
    out
}

/// The longest request any chunk of a pass holds the document thread, the worst of the chunks.
fn worst_chunk(h: &ExecutorHandle) -> std::time::Duration {
    gui_refs().chunks(32).map(|c| h.availability_chunk_hold(c.to_vec()).unwrap()).max().unwrap_or_default()
}

/// The CI form of a large project's figure against the same figure on a small project, measured in the same process
/// (ruling D-41): at most `RATIO` times the small project's, the small one taken at least `FLOOR` (a figure of a few
/// tenths of a millisecond is the timer's, not the work's), and never above the absolute `CEILING`, which only catches a
/// gross regression. The figures of ruling O-23's 20 ms budget are a workstation receipt
/// (`o23_budget_receipt`, release and debug, 30 samples).
/// Measured on the workstation (debug, the minimum of three): 9.0 for the selection case (the first pass reads the
/// 10,101-id selection once after the change, ruling O-24's memo, against 100 ids), 1.1 to 2.6 for the others, about 1
/// for the steady passes and the worst chunk.
const RATIO: u32 = 20;
const FLOOR: std::time::Duration = std::time::Duration::from_millis(1);
/// Ruling D-45: four times the slowest runner's figure before D-41 (22.6 ms, run 43822938737), so the ceiling only
/// catches a gross regression.
const CEILING: std::time::Duration = std::time::Duration::from_millis(100);

fn within(large: std::time::Duration, small: std::time::Duration) -> bool {
    large <= small.max(FLOOR) * RATIO && large <= CEILING
}

/// Ruling O-23's passes stay constant on a large project (ruling D-41's CI form): a project of 10,000 command-created
/// elements against a project of 100, in the same process. Each figure of the large project (a full pass; for each
/// case of ruling O-24, the first pass's longest request after the change as the minimum of three real repetitions, and
/// the steady pass after it; the worst chunk) is within a stated multiple of the same figure on the small project and
/// under an absolute ceiling of 100 ms (ruling D-45). A user command submitted during continuous passes completes within 100 ms.
#[test]
fn passes_stay_constant_on_a_large_project() {
    use std::time::{Duration, Instant};
    let refs = gui_refs();
    let built = Instant::now();
    let (large, ids) = project_of(102);
    let (small, small_ids) = project_of(1);
    eprintln!(
        "fixture: {} and {} elements built through commands in {:?}",
        ids.len(),
        small_ids.len(),
        built.elapsed()
    );
    assert!(ids.len() >= 10_000, "{}", ids.len());
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
    let (s, l) = (best(&small), best(&large));
    eprintln!("full pass over {} references: 100 elements {s:?}, 10,000 elements {l:?}", refs.len());
    assert!(within(l, s), "full pass: large {l:?} vs small {s:?}");
    let cases_l = o24_cases(&large, &ids, 10_000, 991, 3);
    let cases_s = o24_cases(&small, &small_ids, 100, 10, 3);
    for (cl, cs) in cases_l.iter().zip(&cases_s) {
        let (hl, hs) = (cl.holds.iter().min().copied().unwrap(), cs.holds.iter().min().copied().unwrap());
        eprintln!(
            "{}: first pass's longest request {hl:?} (samples {:?}) vs small {hs:?} (samples {:?}), ratio {:.1}; steady {:?} vs {:?}",
            cl.what,
            cl.holds,
            cs.holds,
            hl.as_secs_f64() / hs.max(FLOOR).as_secs_f64(),
            cl.steady,
            cs.steady
        );
        assert!(within(hl, hs), "{}: the first pass's longest request {hl:?} vs small {hs:?}", cl.what);
        assert!(within(cl.steady, cs.steady), "{}: steady pass {:?} vs small {:?}", cl.what, cl.steady, cs.steady);
    }
    let (wl, ws) = (worst_chunk(&large), worst_chunk(&small));
    eprintln!("longest chunk on the document thread: {wl:?} vs small {ws:?}");
    assert!(within(wl, ws), "worst chunk: large {wl:?} vs small {ws:?}");
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

/// Ruling O-23's 20 ms budget where the figure means something (ruling D-41): a workstation receipt, run in release
/// and debug at each handoff that touches availability, committed under `reviews/evidence/`. For each case of ruling
/// O-24 on the 10,000-element project, 30 real repetitions of the change, the first pass's longest request each time;
/// and 30 samples of the worst chunk. Every sample is at most 20 ms. The receipt (median, p95, max and the samples) is
/// written to `TT_AVAILABILITY_RECEIPT` when set.
///
/// `cargo test -p tt-executor --test c41 o23_budget_receipt -- --ignored` (and with `--release`).
#[test]
#[ignore = "a workstation receipt (ruling D-41): 30 samples, release and debug; not a CI assertion"]
fn o23_budget_receipt() {
    use std::time::Duration;
    let (large, ids) = project_of(102);
    let cases = o24_cases(&large, &ids, 10_000, 991, 30);
    let chunks: Vec<Duration> = (0..30).map(|_| worst_chunk(&large)).collect();
    let stats = |samples: &[Duration]| {
        let mut ms: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
        ms.sort_by(f64::total_cmp);
        let at = |q: f64| ms[((q * ms.len() as f64).ceil() as usize).saturating_sub(1).min(ms.len() - 1)];
        let r2 = |v: f64| (v * 100.0).round() / 100.0;
        json!({ "n": ms.len(), "median": r2(at(0.5)), "p95": r2(at(0.95)), "max": r2(at(1.0)),
                "samples": ms.iter().map(|v| r2(*v)).collect::<Vec<_>>() })
    };
    let mut rows = Map::new();
    for c in &cases {
        rows.insert(format!("10,000 elements: {}", c.what), stats(&c.holds));
    }
    rows.insert("worst chunk".into(), stats(&chunks));
    let receipt = json!({
        "kind": "toontown.c41.o23-budget",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "budgetMs": 20,
        "figures": rows,
    });
    eprintln!("{}", serde_json::to_string_pretty(&receipt).unwrap());
    if let Ok(path) = std::env::var("TT_AVAILABILITY_RECEIPT") {
        std::fs::write(&path, serde_json::to_string_pretty(&receipt).unwrap() + "\n").unwrap();
    }
    let budget = Duration::from_millis(20);
    for c in &cases {
        let worst = c.holds.iter().max().copied().unwrap();
        assert!(worst <= budget, "10,000 elements, {}: a first pass's request held the thread {worst:?}", c.what);
    }
    let worst = chunks.iter().max().copied().unwrap();
    assert!(worst <= budget, "worst chunk {worst:?}");
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
        // C42A-R1-08: the Drawing view window switched to the Camera view type.
        "drawing-view-as-camera" => ok(h, "view.type.set", json!({ "viewId": "drawingView", "type": "camera" })),
        // M4 C4.3a: the Top view (M6 C6.1's) active, and the Drawing view window switched to the Top view type.
        "top-view" => ok(h, "view.active.set", json!({ "viewId": "topView" })),
        "drawing-view-as-top" => ok(h, "view.type.set", json!({ "viewId": "drawingView", "type": "top" })),
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
