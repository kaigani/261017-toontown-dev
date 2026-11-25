//! M5 C5.2a, ruling E-51: the undo history's memory bound through the commands. The bound is the product's 1 GiB; a
//! dev-only hook (`test-hooks`) sets a small one so steps of a known size cross it: 300 note changes of about 10 KB each
//! against a bound of about 230 of them. The oldest steps are discarded first, never below 200; undo through the whole
//! kept history arrives at the state before the oldest kept step, and no history command reaches a discarded step.
//! The bound's own mechanics (the sizes, the floor, the step-count limit, the clean marker and the floor of a repaired
//! open across a discard) are the unit tests of `crates/executor/src/history.rs`.
#![cfg(feature = "test-hooks")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::json;
use tt_executor::{ExecutorConfig, ExecutorHandle};

const CHANGES: usize = 300;

/// The note of change `i`: 5,000 characters, different for each change.
fn note(i: usize) -> String {
    let head = format!("note {i:04} ");
    let fill = char::from(b'a' + (i % 26) as u8);
    let mut s = head.clone();
    s.extend(std::iter::repeat_n(fill, 5_000 - head.len()));
    s
}

#[test]
fn history_commands_never_reach_a_discarded_step() {
    // About 230 changes' worth (each change: the new and the previous note, 10 KB, with its pointers).
    tt_executor::hooks::set_history_bound(230 * 10_100);
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: std::time::Duration::from_secs(60) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let element = ok(&h, "element.create", json!({ "type": "drawing", "name": "Noted" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    for i in 0..CHANGES {
        ok(&h, "element.note.set", json!({ "elementId": element, "text": note(i) }));
    }
    tt_executor::hooks::set_history_bound(0);

    let history = ok(&h, "edit.history.get", json!({ "limit": 1000 }));
    let total = history["total"].as_u64().unwrap() as usize;
    assert!((200..CHANGES).contains(&total), "the bound discarded the oldest steps, never below 200: {total}");
    assert_eq!(history["position"].as_u64().unwrap() as usize, total);
    let entries = history["entries"].as_array().unwrap();
    assert_eq!(entries.len(), total);
    // The kept steps are the most recent `total` changes (the element's creation and the first notes are gone).
    assert!(entries.iter().all(|e| e["label"] == entries[0]["label"]), "only note changes are kept");

    // Undo through the whole kept history: the note before the oldest kept change.
    let undone = ok(&h, "edit.undo", json!({ "steps": 1000 }));
    assert_eq!(undone["undone"].as_array().unwrap().len(), total);
    assert_eq!(undone["position"], 0);
    let oldest_kept = CHANGES - total;
    let shown = ok(&h, "element.get", json!({ "elementId": element }));
    assert_eq!(shown["note"], json!(note(oldest_kept - 1)), "the state before the oldest kept change");

    // Nothing below: undo is refused, `edit.history.move` to 0 changes nothing, and redo comes back through the kept
    // steps only, to the last note.
    let refused = err(&h, "edit.undo", json!({ "steps": 1 }));
    assert_eq!(refused.code, "edit.nothingToUndo", "{refused:?}");
    ok(&h, "edit.history.move", json!({ "position": 0 }));
    assert_eq!(ok(&h, "element.get", json!({ "elementId": element }))["note"], json!(note(oldest_kept - 1)));
    let redone = ok(&h, "edit.redo", json!({ "steps": 1000 }));
    assert_eq!(redone["position"].as_u64().unwrap() as usize, total);
    assert_eq!(ok(&h, "element.get", json!({ "elementId": element }))["note"], json!(note(CHANGES - 1)));
}
