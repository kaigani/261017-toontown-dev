//! M3 C3.1: the M2 → M3 handoff obligations that C3.1 can meet (reviews/M2-C2.4-r3.md § M3 handoff list).
//!
//! * Item 7 — timeline and exposure operations run on the common staged transaction / cancellation path: one undo
//!   step each; request-owned cancellation of a running and of a queued call; no late document or UI mutation after a
//!   timeout; guarded replay (`requestId`, `expectRev`, the sheet's own `expectRev`) after an intervening edit.
//! * Item 5 — drawing selections and transient zone IDs are untouched by unrelated element edits; only an own-layer
//!   change prunes the invalid part.
//! * Item 6 — element duplication and drawing duplication keep content verbatim (C24-T1 owner resolution, stretch
//!   fills, opaque `meta`/`ext`) through undo/redo and save/open, rendering identically.
//! * Item 1 — `tool.symmetry.set` stays session-only around the new commands.
//!
//! Plus a save/open round trip of the C3.1 edits and same-sequence determinism. The spin hook is process-global, so
//! the tests of this binary take turns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::time::{Duration, Instant};

use c13_support::job_ok;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, InvokeOptions, Status};

const INTRO: &str = "scene:\"Intro\"";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
const BOB_ID: &str = "el_01K742SG11W3SGB8Q1H50W74VB";
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

static HOOK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    HOOK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The scene and its elements as JSON without revision stamps.
fn scene_state(h: &ExecutorHandle) -> Value {
    let mut out = vec![strip(get(h, INTRO))];
    for e in ok(h, "element.list", json!({ "sceneId": INTRO }))["elements"].as_array().unwrap() {
        out.push(strip(get(h, e["id"].as_str().unwrap())));
    }
    Value::Array(out)
}

/// The first JSON Pointer where two values differ.
fn first_diff(a: &Value, b: &Value, at: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let (p, q) = (x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null));
                if let Some(d) = first_diff(p, q, &format!("{at}/{k}")) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let (p, q) = (x.get(i).unwrap_or(&Value::Null), y.get(i).unwrap_or(&Value::Null));
                if let Some(d) = first_diff(p, q, &format!("{at}/{i}")) {
                    return Some(d);
                }
            }
            None
        }
        _ => (a != b).then(|| format!("{at}: {a} vs {b}")),
    }
}

fn without_times(mut v: Value) -> Value {
    match &mut v {
        Value::Object(m) => {
            m.remove("createdAt");
            for x in m.values_mut() {
                *x = without_times(x.take());
            }
        }
        Value::Array(a) => {
            for x in a.iter_mut() {
                *x = without_times(x.take());
            }
        }
        _ => {}
    }
    v
}

fn strip(mut v: Value) -> Value {
    if let Value::Object(m) = &mut v {
        m.remove("rev");
    }
    v
}

#[test]
fn item7_every_timeline_action_is_exactly_one_undo_step() {
    let _t = turn();
    let h = fixture();
    let actions: Vec<(&str, Value)> = vec![
        ("element.create", json!({ "type": "peg", "count": 3, "sceneId": INTRO })),
        ("element.parentPeg.create", json!({ "elementIds": [BG] })),
        ("element.rename", json!({ "elementId": BOB, "name": "Robert" })),
        ("element.clone", json!({ "elementId": BOB })),
        ("element.duplicate", json!({ "elementIds": [BOB] })),
        ("element.delete", json!({ "elementIds": [PEG] })),
        ("element.order.move", json!({ "elementIds": [BG], "to": "front" })),
        ("element.parent.set", json!({ "elementIds": [BG], "parentId": PEG })),
        ("element.parent.remove", json!({ "elementIds": [BOB] })),
        ("element.colour.set", json!({ "elementIds": [BOB], "marker": "#123456" })),
        ("element.note.set", json!({ "elementId": BOB, "text": "x" })),
        ("element.properties.set", json!({ "elementId": BOB, "role": "background" })),
        ("element.timing.set", json!({ "elementIds": [PEG], "duration": 10 })),
        ("element.childExposure.apply", json!({ "pegId": PEG, "toFrame": 60 })),
        ("element.visibility.set", json!({ "scope": "scene", "visible": false, "apply": "all" })),
        ("element.lock.set", json!({ "locked": true, "apply": "all" })),
        ("element.annotation.set", json!({ "elementId": BOB, "annotation": true })),
        ("element.drawing.create", json!({ "elementId": BOB, "exposeFrom": 1, "exposeTo": 3 })),
        ("element.drawing.delete", json!({ "drawingIds": [D1], "ifInUse": "clearCells" })),
        ("cell.exposure.add", json!({ "elementIds": [BOB, BG], "from": 1, "to": 10 })),
        ("cell.exposure.set", json!({ "elementIds": [BOB], "from": 1, "to": 40, "length": 3 })),
        ("cell.range.add", json!({ "allElements": true, "elementIds": [BOB], "frame": 2, "count": 4 })),
        ("cell.range.move", json!({ "sourceElementIds": [BOB], "from": 1, "to": 6, "targetFrame": 30 })),
        ("cell.cycle.create", json!({ "elementId": BOB, "from": 1, "to": 8, "cycles": 4 })),
        (
            "cell.columnate.apply",
            json!({ "elementId": BOB, "frame": 1, "firstDrawing": "1", "lastDrawing": "10", "loopMode": "continuous", "loops": 3 }),
        ),
        ("cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 6, "cut": true })),
        (
            "xsheet.exposures.set",
            json!({ "createMissingDrawings": true, "columns": [{ "elementId": BOB, "exposures": [{ "drawing": "new", "from": 1, "to": 9 }] }] }),
        ),
    ];
    for (id, params) in actions {
        let before = scene_state(&h);
        let steps = ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap();
        ok(&h, id, params.clone());
        let after = ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap();
        assert_eq!(after, steps + 1, "{id}: exactly one undo step");
        assert_ne!(scene_state(&h), before, "{id} changed something");
        ok(&h, "edit.undo", json!({}));
        assert_eq!(first_diff(&scene_state(&h), &before, ""), None, "{id}: one undo restores the scene");
        ok(&h, "edit.redo", json!({}));
        ok(&h, "edit.undo", json!({}));
    }
}

/// A scene of 300 drawing columns: a call over all of them charges hundreds of work units.
fn big(budget: Duration) -> ExecutorHandle {
    let h = fixture_with(ExecutorConfig { budget });
    for _ in 0..3 {
        ok(&h, "element.create", json!({ "type": "drawing", "count": 99, "sceneId": INTRO }));
    }
    h
}

#[test]
fn item7_running_and_queued_cancellation_are_request_owned_and_commit_nothing() {
    let _t = turn();
    let budget = Duration::from_millis(800);
    let h = big(budget);
    let start = rev(&h);
    let state = scene_state(&h);
    tt_vector::work::hooks::set_spin_micros(4000);
    let a = h.clone();
    let running = std::thread::spawn(move || {
        run(&a, "cell.range.add", json!({ "allElements": true, "elementIds": [BOB], "frame": 1, "count": 2 }))
    });
    std::thread::sleep(Duration::from_millis(150));
    // Queued behind the running call: its own request, its own deadline.
    let t = Instant::now();
    let queued = run(&h, "cell.exposure.set", json!({ "elementIds": [BOB], "from": 1, "to": 40, "length": 2 }));
    let running = running.join().unwrap();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(running.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{running:?}");
    // The queued call either completed in its own window or timed out on its own deadline — never because of the
    // first caller's timeout, and never late.
    assert!(t.elapsed() < budget * 2, "queued call answered within its own budget: {:?}", t.elapsed());
    let read = run(&h, "doc.state.get", json!({}));
    assert!(read.is_ok());
    std::thread::sleep(Duration::from_millis(300));
    if queued.status == Status::Ok {
        assert_eq!(rev(&h), start + 1, "only the queued call committed");
    } else {
        assert_eq!(queued.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"));
        assert_eq!(rev(&h), start);
        assert_eq!(scene_state(&h), state, "no late mutation");
    }
}

#[test]
fn item7_no_late_ui_mutation_after_a_timeout() {
    let _t = turn();
    let budget = Duration::from_millis(600);
    let h = fixture_with(ExecutorConfig { budget });
    ok(&h, "element.select", json!({ "elementIds": [BOB] }));
    ok(&h, "element.current.set", json!({ "elementId": BOB }));
    let sel = ok(&h, "cell.selection.get", json!({}));
    let start = rev(&h);
    tt_vector::work::hooks::set_spin_micros(15_000);
    let e = run(&h, "element.create", json!({ "type": "drawing", "count": 99, "sceneId": INTRO }));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{e:?}");
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(rev(&h), start);
    assert_eq!(ok(&h, "cell.selection.get", json!({})), sel, "selection and current element untouched");
    // The worker is free and the next call commits normally.
    ok(&h, "element.create", json!({ "type": "drawing", "sceneId": INTRO }));
    assert_eq!(rev(&h), start + 1);
}

#[test]
fn item7_guarded_replay_after_an_intervening_edit() {
    let _t = turn();
    let h = fixture();
    // requestId: a repeated request returns its original result and applies nothing again.
    let opts = InvokeOptions { request_id: Some("c31-replay-1".into()), ..InvokeOptions::default() };
    let first = with(&h, "cell.exposure.add", json!({ "elementIds": [BOB_ID], "from": 1, "to": 1 }), opts.clone());
    assert!(first.is_ok());
    ok(&h, "element.rename", json!({ "elementId": BOB, "name": "Intervening" }));
    let r = rev(&h);
    let again = with(&h, "cell.exposure.add", json!({ "elementIds": [BOB_ID], "from": 1, "to": 1 }), opts);
    assert_eq!(again.result, first.result, "the original outcome, not a second application");
    assert_eq!(rev(&h), r);
    // expectRev: a call recorded against an earlier revision is refused after the intervening edit.
    let el = get(&h, "scene:\"Intro\"/element:\"Intervening\"");
    let stale_rev = el["rev"].as_u64().unwrap() - 1;
    let stale = InvokeOptions {
        expect_rev: [(el["id"].as_str().unwrap().to_owned(), stale_rev)].into_iter().collect(),
        ..InvokeOptions::default()
    };
    let env = with(
        &h,
        "cell.exposure.add",
        json!({ "elementIds": ["scene:\"Intro\"/element:\"Intervening\""], "from": 1, "to": 1 }),
        stale,
    );
    assert_eq!(env.status, Status::Error);
    assert_eq!(rev(&h), r);
    // The sheet's own guard.
    let sheet = ok(&h, "xsheet.exposures.get", json!({ "sceneId": INTRO }))["rev"].as_u64().unwrap();
    ok(
        &h,
        "cell.note.set",
        json!({ "elementId": "scene:\"Intro\"/element:\"Intervening\"", "frame": 1, "text": "edit" }),
    );
    let e = err(
        &h,
        "xsheet.exposures.set",
        json!({ "expectRev": sheet, "columns": [{ "elementId": "scene:\"Intro\"/element:\"Intervening\"", "exposures": [] }] }),
    );
    assert_eq!(e.code, "xsheet.revisionMismatch");
}

#[test]
fn item5_drawing_selection_and_zones_survive_unrelated_element_edits() {
    let _t = turn();
    let h = fixture();
    let r = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": { "x": 300, "y": 300 }, "to": { "x": 400, "y": 360 }, "layer": "new" }),
    );
    let zone = r["zoneIds"][0].as_str().unwrap().to_owned();
    let stroke = r["strokeIds"][0].as_str().unwrap().to_owned();
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "ids": [zone, stroke] }));
    let sel = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }));
    for (id, params) in [
        ("element.rename", json!({ "elementId": BG, "name": "Sky" })),
        ("element.order.move", json!({ "elementIds": [PEG], "to": "back" })),
        ("element.create", json!({ "type": "peg", "sceneId": INTRO })),
        ("cell.exposure.set", json!({ "elementIds": [BOB], "from": 1, "to": 40, "length": 2 })),
        ("element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": false })),
        ("element.drawing.rename", json!({ "drawingId": D1, "name": "one" })),
    ] {
        ok(&h, id, params);
        assert_eq!(
            ok(&h, "draw.selection.get", json!({ "drawingId": D1 }))["selectedIds"],
            sel["selectedIds"],
            "after {id}"
        );
    }
    // An own-layer change that removes the zone prunes only the zone.
    ok(&h, "draw.object.delete", json!({ "drawingId": D1, "ids": [stroke] }));
    let after = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }))["selectedIds"].clone();
    assert!(!after.to_string().contains(&zone), "the zone of the deleted rectangle is pruned: {after}");
}

#[test]
fn item6_duplicates_keep_content_and_render_identically_through_undo_and_save() {
    let _t = turn();
    let h = fixture();
    // Opaque namespaces on the source drawing travel verbatim.
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [D1], "ops": [{ "op": "add", "path": "/meta", "value": { "x.vendor": { "keep": [1, 2] } } }] }),
    );
    let dup = ok(&h, "element.duplicate", json!({ "elementIds": [BOB] }));
    let dup_id = dup["elementIds"][0].as_str().unwrap().to_owned();
    let copied = ok(&h, "element.drawing.list", json!({ "elementId": dup_id }))["drawings"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let render = |h: &ExecutorHandle, d: &str| {
        ok(h, "draw.drawing.render", json!({ "drawingId": d, "width": 96 }))["pixelsSha256"].clone()
    };
    let content = |h: &ExecutorHandle, d: &str| ok(h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"].clone();
    let src_nodes = content(&h, D1);
    assert_eq!(content(&h, &copied), src_nodes, "nodes, stroke IDs, fills and owners copied verbatim, never re-noded");
    let src = get(&h, D1);
    assert_eq!(get(&h, &copied)["meta"], src["meta"]);
    assert_eq!(get(&h, &copied)["ext"], src["ext"]);
    let wa = ok(&h, "draw.drawing.render", json!({ "drawingId": D1, "width": 96 }));
    let wb = ok(&h, "draw.drawing.render", json!({ "drawingId": copied, "width": 96 }));
    assert_eq!(wb["warnings"], wa["warnings"], "same render warnings");
    assert_eq!(wb["drawingToPixel"], wa["drawingToPixel"]);
    assert_eq!(render(&h, &copied), render(&h, D1));
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(content(&h, &copied), src_nodes);
    // Save, reopen, compare.
    let dir = std::env::temp_dir().join(format!("tt-c31-item6-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Item6.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path.to_string_lossy() }));
    let h2 = tt_executor::ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(60) }).unwrap();
    job_ok(&h2, "project.open", json!({ "path": path.to_string_lossy() }));
    assert_eq!(content(&h2, &copied), src_nodes);
    assert_eq!(get(&h2, &copied)["meta"], src["meta"]);
    assert_eq!(render(&h2, &copied), render(&h, D1));
    drop(h2);
    std::fs::remove_file(&path).ok();
    std::fs::remove_dir(&dir).ok();
}

#[test]
fn item1_symmetry_stays_session_only() {
    let _t = turn();
    let h = fixture();
    let r = rev(&h);
    let steps = ok(&h, "edit.history.get", json!({}))["entries"].as_array().map_or(0, Vec::len);
    ok(&h, "tool.symmetry.set", json!({ "mode": "mirror" }));
    ok(&h, "element.create", json!({ "type": "drawing", "sceneId": INTRO }));
    ok(&h, "cell.exposure.add", json!({ "elementIds": [BOB], "from": 1, "to": 1 }));
    assert_eq!(rev(&h), r + 2, "only the two document commands made revisions");
    assert_eq!(ok(&h, "edit.history.get", json!({}))["entries"].as_array().map_or(0, Vec::len), steps + 2);
    let doc = h.snapshot().unwrap().unwrap();
    let json = serde_json::to_string(&doc.entry_raw_json(&tt_model::document::EntryKey::Project).unwrap()).unwrap();
    assert!(!json.contains("symmetry"), "no saved default");
}

#[test]
fn save_open_round_trip_and_determinism() {
    let _t = turn();
    let script = |h: &ExecutorHandle| -> Vec<Value> {
        vec![
            ok(h, "element.create", json!({ "type": "drawing", "name": "Walk", "sceneId": INTRO })),
            ok(
                h,
                "xsheet.exposures.set",
                json!({ "createMissingDrawings": true, "columns": [{ "elementId": "scene:\"Intro\"/element:\"Walk\"", "exposures": [{ "drawing": "1", "from": 1, "to": 2 }, { "drawing": "2", "from": 3, "to": 4 }] }] }),
            ),
            ok(
                h,
                "cell.cycle.create",
                json!({ "elementId": "scene:\"Intro\"/element:\"Walk\"", "from": 1, "to": 4, "cycles": 6 }),
            ),
            ok(h, "element.clone", json!({ "elementId": "scene:\"Intro\"/element:\"Walk\"" })),
            ok(h, "element.parentPeg.create", json!({ "elementIds": ["scene:\"Intro\"/element:\"Walk\""] })),
            ok(h, "element.timing.set", json!({ "elementIds": ["scene:\"Intro\"/element:\"Walk-P\""], "start": 3 })),
            ok(
                h,
                "cell.note.set",
                json!({ "elementId": "scene:\"Intro\"/element:\"Walk\"", "frame": 5, "text": "contact" }),
            ),
        ]
    };
    let a = fixture();
    let b = fixture();
    let ra = script(&a);
    let rb = script(&b);
    assert_eq!(ra, rb, "same sequence, same results and IDs (04 §17.1 item 8)");
    // Creation times are wall-clock stamps (read-only metadata); everything else is identical.
    assert_eq!(first_diff(&without_times(scene_state(&a)), &without_times(scene_state(&b)), ""), None);
    let dir = std::env::temp_dir().join(format!("tt-c31-rt-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&dir).unwrap();
    let p1 = dir.join("One.animpkg");
    job_ok(&a, "project.saveAs", json!({ "path": p1.to_string_lossy() }));
    let h2 = tt_executor::ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(60) }).unwrap();
    job_ok(&h2, "project.open", json!({ "path": p1.to_string_lossy() }));
    assert_eq!(first_diff(&scene_state(&h2), &scene_state(&a), ""), None);
    let s1 = ok(&a, "xsheet.exposures.get", json!({ "sceneId": INTRO }));
    let s2 = ok(&h2, "xsheet.exposures.get", json!({ "sceneId": INTRO }));
    assert_eq!(s1["columns"], s2["columns"]);
    drop(h2);
    std::fs::remove_file(&p1).ok();
    std::fs::remove_dir(&dir).ok();
}
