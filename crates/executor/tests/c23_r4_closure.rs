//! Independent round-4 closure variants; M2 C2.3 round 3 (C23-R3-01): a call whose caller timed out is refused at the acceptance boundary whatever it
//! changes, and the session state it had changed is restored in full. Controls beside the reviewer's proving test
//! (`c23_r3_selection_cancel`): a previous non-empty selection, the point selection, the current frame and the mode
//! survive a cancelled selection, alone and inside an atomic batch; the same work is accepted afterwards.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::Duration;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle};

const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

/// The seed-1 fixture with `n` simple disjoint pencil strokes in one layer of `D` (selecting them all crosses a
/// 200 ms budget through real work), under `budget`.
fn crowded(n: usize, budget: Duration) -> ExecutorHandle {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let index = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut drawing = serde_json::to_value(&parts.drawings[index]).unwrap();
    let template = drawing["nodes"][0]["strokes"][1].clone();
    let mut layer = drawing["nodes"][0].clone();
    layer["strokes"] = json!(
        (0..n)
            .map(|i| {
                let mut s = template.clone();
                s["id"] = json!(format!("st_round3_{i}"));
                s["points"] = json!([[i * 3, 2000, 0, 0, 0, 0], [i * 3 + 1, 2001, 0, 0, 0, 0]]);
                s
            })
            .collect::<Vec<_>>()
    );
    drawing["nodes"] = json!([layer]);
    parts.drawings[index] = tt_model::object::from_json(&drawing).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let h = ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
        config: ExecutorConfig { budget },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_executor::UserDirs::under(&user_dir())),
            ..Default::default()
        },
    })
    .unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    h
}

/// The session state a cancelled call must not change: the app state (selection, frame, mode, element), the drawing
/// selection with the point selection, and the revision. Retried until the worker answers.
fn session(h: &ExecutorHandle) -> Value {
    for _ in 0..100 {
        let a = run(h, "app.state.get", json!({}));
        let s = run(h, "draw.selection.get", json!({ "drawingId": D }));
        if let (Some(a), Some(s)) = (a.result, s.result) {
            return json!({ "app": a, "points": s["selectedPoints"], "ids": s["selectedIds"], "rev": rev(h) });
        }
    }
    panic!("the worker did not answer");
}

fn prepare(h: &ExecutorHandle) {
    ok(h, "draw.selection.set", json!({ "drawingId": D, "ids": ["st_round3_7", "st_round3_8"] }));
    ok(h, "draw.pointSelection.set", json!({ "drawingId": D, "points": [{ "strokeId": "st_round3_7", "anchor": 1 }] }));
    ok(h, "app.frame.set", json!({ "frame": 2 }));
    ok(h, "app.mode.set", json!({ "mode": "drawing" }));
}

#[test]
fn cancelled_session_document_and_prefs_batch_exports_nothing() {
    let _guard = SERIAL.lock().unwrap();
    let h = crowded(3000, Duration::from_millis(200));
    prepare(&h);
    let drawing = get(&h, D);
    ok(&h, "draw.inspectorPreview.set", json!({"visible":true}));
    let pref_path = user_dir().join("Settings/preferences.json");
    let prefs = std::fs::read(&pref_path).unwrap();
    let a = h
        .connect_agent(tt_executor::agents::AgentConnect {
            policy: Some(tt_executor::agents::Policy::Autonomous),
            ..Default::default()
        })
        .unwrap();
    let origin = tt_executor::Origin::Agent { session: a.session_id, name: a.client, scopes: a.scopes };
    let before = session(&h);
    let rx = h.subscribe();
    let e = h.batch_audited(
        serde_json::from_value(json!({"label":"r4 rollback", "atomic":true,"steps":[
            {"id":"draw.drawing.rename","params":{"drawingId":D,"name":"must rollback"}},
            {"id":"draw.pointSelection.set","params":{"drawingId":D,"points":[]}},
            {"id":"draw.inspectorPreview.set","params":{"visible":false}},
            {"id":"draw.selection.set","params":{"drawingId":D,"by":"all"}}
        ]}))
        .unwrap(),
        origin,
        "batch",
    );
    assert_eq!(e.error.unwrap().code, "command.timeout");
    assert_eq!(session(&h), before);
    assert_eq!(get(&h, D), drawing);
    assert_eq!(std::fs::read(&pref_path).unwrap(), prefs);
    assert_eq!(rx.try_iter().filter(|e| e.name == "draw.selection.changed").count(), 0);
    let path = std::env::temp_dir().join(format!("tt-r4-replay-{}.json", std::process::id()));
    ok(&h, "agent.activity.export", json!({"path":path.to_string_lossy(),"format":"batch","onExists":"overwrite"}));
    let replay: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(replay["steps"], json!([]));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn nonempty_selection_is_preserved_and_success_emits_once() {
    let _guard = SERIAL.lock().unwrap();
    let h = crowded(3000, Duration::from_millis(200));
    prepare(&h);
    let before = session(&h);
    let e = run(&h, "draw.selection.set", json!({"drawingId":D,"by":"all"}));
    assert_eq!(e.error.unwrap().code, "command.timeout");
    assert_eq!(session(&h), before);
    let rx = h.subscribe();
    ok(&h, "draw.selection.set", json!({"drawingId":D,"ids":["st_round3_9"]}));
    assert_eq!(session(&h)["ids"], json!(["st_round3_9"]));
    assert_eq!(rx.try_iter().filter(|e| e.name == "draw.selection.changed").count(), 1);
}

fn user_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "tt-r4-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn queued_point_and_prefs_calls_timeout_without_late_changes() {
    let _guard = SERIAL.lock().unwrap();
    let h = crowded(3000, Duration::from_millis(200));
    prepare(&h);
    ok(&h, "draw.inspectorPreview.set", json!({"visible":true}));
    let before = session(&h);
    let path = user_dir().join("Settings/preferences.json");
    let prefs = std::fs::read(&path).unwrap();
    tt_vector::work::hooks::set_spin_micros(1_000_000);
    let worker = h.clone();
    let slow = std::thread::spawn(move || run(&worker, "draw.selection.set", json!({"drawingId":D,"by":"all"})));
    std::thread::sleep(Duration::from_millis(50));
    let point = run(&h, "draw.pointSelection.set", json!({"drawingId":D,"points":[]}));
    let preview = run(&h, "draw.inspectorPreview.set", json!({"visible":false}));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(slow.join().unwrap().error.unwrap().code, "command.timeout");
    assert_eq!(point.error.unwrap().code, "command.timeout");
    assert_eq!(preview.error.unwrap().code, "command.timeout");
    assert_eq!(session(&h), before);
    assert_eq!(std::fs::read(path).unwrap(), prefs);
}
