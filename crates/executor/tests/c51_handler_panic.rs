//! M5 C5.1a round 1 (builder; ruling E-21): what the executor does today when a handler panics. Its own test binary,
//! since the persist boundary hook it sets is process-wide (`reviews/M5-design-handler-panics.md`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::path::Path;
use std::sync::Arc;

use common::*;
use serde_json::json;
use tt_executor::{ExecutorHandle, Status};
use tt_persist::lock::{Boundary, hooks};

/// What the executor does today when a handler panics (the response's E-21 section; the handler-panic design note):
/// the call in flight answers `internal`, the document thread is gone with the session it held, every later call
/// answers `internal`, and nothing restarts inside the executor (recovery is a new executor, the host's restart).
#[test]
fn e21_a_handler_panic_today_stops_the_document_thread() {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            hooks::set(None);
        }
    }
    let _reset = Reset;
    let h = fixture();
    ok(&h, "element.create", json!({"type": "peg", "name": "Before", "sceneId": "scene:#1"}));
    // A panic on the document thread inside an action (the persist boundary hook stands for any handler panic).
    hooks::set(Some(Arc::new(|at: Boundary, _: &Path| {
        if at == Boundary::Committed {
            panic!("test: a handler panics");
        }
    })));
    let died = run(&h, "element.create", json!({"type": "peg", "name": "Dies", "sceneId": "scene:#1"}));
    hooks::set(None);
    assert_eq!(died.status, Status::Error);
    assert_eq!(died.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{died:?}");
    for (command, params) in [("timeline.rows.list", json!({})), ("doc.state.get", json!({}))] {
        let e = run(&h, command, params);
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{command}: {e:?}");
        assert!(e.error.as_ref().unwrap().message.contains("document thread has stopped"), "{e:?}");
    }
    // The open session is gone with the thread: a new executor (what the host's restart creates) has no project.
    let fresh = ExecutorHandle::spawn(Default::default()).unwrap();
    assert_eq!(run(&fresh, "timeline.rows.list", json!({})).error.map(|e| e.code), Some("project.none".to_owned()));
}
