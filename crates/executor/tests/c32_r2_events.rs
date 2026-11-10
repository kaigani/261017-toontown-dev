//! Independent interpolation-scope and history event oracle.
//! History assertions intentionally fail on C32-R2-02.
mod common;
use common::*;
use serde_json::{Value, json};
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
fn events(h: &tt_executor::ExecutorHandle, command: &str, params: Value) -> Vec<Value> {
    let rx = h.subscribe();
    if command == "batch" {
        assert_eq!(batch(h, params).status, tt_executor::Status::Ok);
    } else {
        ok(h, command, params);
    }
    rx.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload).collect()
}
#[test]
fn interpolation_interval_survives_undo_redo() {
    let h = fixture();
    let changed = events(&h, "anim.transform.set", json!({"nodeIds":[B],"frame":18,"rotation":37}));
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["frames"], json!({"from":12.,"to":24.}));
    let history: Vec<_> = ["edit.undo", "edit.redo"].into_iter().map(|cmd| events(&h, cmd, json!({}))).collect();
    eprintln!("undo/redo event counts: {:?}", history.iter().map(Vec::len).collect::<Vec<_>>());
    for ev in history {
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0]["frames"], changed[0]["frames"]);
    }
}
#[test]
fn all_frame_scope_both_orders_and_separate_transactions() {
    for reverse in [false, true] {
        let h = fixture();
        let mut steps = vec![
            json!({"id":"anim.transform.set","params":{"nodeIds":[B],"target":"static","skew":9}}),
            json!({"id":"anim.transform.set","params":{"nodeIds":[B],"frame":18,"rotation":37}}),
        ];
        if reverse {
            steps.reverse();
        }
        let ev = events(&h, "batch", json!({"label":"R2","atomic":true,"steps":steps}));
        assert_eq!(ev.len(), 1);
        assert!(ev[0].get("frames").is_none());
    }
    let h = fixture();
    let rx = h.subscribe();
    for frame in [17, 18] {
        ok(&h, "anim.transform.set", json!({"nodeIds":[B],"frame":frame,"rotation":37}));
    }
    assert_eq!(rx.try_iter().filter(|e| e.name == "anim.changed").count(), 2);
}

#[test]
fn key_move_remove_and_segment_intervals() {
    for (cmd, params) in [
        ("anim.key.move", json!({"nodeId":B,"fromFrame":18,"toFrame":19,"channels":["rotation"]})),
        ("anim.key.remove", json!({"nodeIds":[B],"frame":18,"channels":["rotation"]})),
        ("anim.segment.set", json!({"nodeIds":[B],"frame":18,"channels":["rotation"],"constant":true})),
        ("anim.segment.uniform.apply", json!({"nodeId":B,"frame":18,"side":"right","channels":["rotation"]})),
    ] {
        let h = fixture();
        ok(&h, "anim.transform.set", json!({"nodeIds":[B],"frame":18,"rotation":37}));
        let ev = events(&h, cmd, params);
        assert_eq!(ev.len(), 1, "{cmd}");
        assert_eq!(ev[0]["frames"], json!({"from":12.,"to":24.}), "{cmd}: {ev:?}");
    }
}

#[test]
fn duplicated_function_key_reports_its_neighbour_interval() {
    let h = fixture();
    ok(&h, "anim.transform.set", json!({"nodeIds":[B],"frame":18,"rotation":37}));
    let ev =
        events(&h, "anim.function.key.duplicate", json!({"nodeId":B,"channel":"rotation","frame":18,"toFrame":19}));
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0]["frames"], json!({"from":18.,"to":24.}));
    let replays: Vec<_> = ["edit.undo", "edit.redo"].into_iter().map(|cmd| events(&h, cmd, json!({}))).collect();
    eprintln!("duplicate undo/redo event counts: {:?}", replays.iter().map(Vec::len).collect::<Vec<_>>());
    for history in replays {
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["frames"], ev[0]["frames"]);
    }
}
