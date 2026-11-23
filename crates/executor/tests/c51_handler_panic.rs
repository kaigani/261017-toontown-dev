//! M5 C5.1a rounds 1 and 2 (builder; rulings E-21, E-29): a panic outside the invocation boundary (here at the commit,
//! after the handler returned) degrades the engine. Its own test binary, since the persist boundary hook it sets is
//! process-wide (`reviews/M5-design-handler-panics.md`). A handler's own panic is contained: `src/panic_tests.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::path::Path;
use std::sync::Arc;

use common::*;
use serde_json::json;
use tt_executor::{ExecutorHandle, Status};
use tt_persist::lock::{Boundary, hooks};

/// A panic at the commit (outside the handler's boundary): the engine cannot vouch for its session, so it is degraded:
/// the call in flight answers `internal`, every later call answers `internal` with reason `engineStopped` and the hint
/// that names the way out, and nothing restarts inside the executor (recovery is a new executor, the host's restart).
/// Before ruling E-29 the same input ended the document thread without the reason or the hint.
#[test]
fn e29_a_panic_at_the_commit_degrades_the_engine() {
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
    let events = h.subscribe();
    let died = run(&h, "element.create", json!({"type": "peg", "name": "Dies", "sceneId": "scene:#1"}));
    hooks::set(None);
    // The caller is answered as soon as its reply is dropped by the unwinding; the engine marks itself degraded and
    // raises the notice right after, on the document thread: wait for the notice (bounded), then check the flag.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut notices = 0;
    while notices == 0 && std::time::Instant::now() < deadline {
        if let Ok(e) = events.recv_timeout(std::time::Duration::from_millis(100))
            && e.name == "shell.engineDegraded"
        {
            notices += 1;
        }
    }
    assert_eq!(notices, 1, "the degraded notice");
    assert!(h.degraded(), "a panic outside an invocation degrades the engine");
    assert_eq!(events.try_iter().filter(|e| e.name == "shell.engineDegraded").count(), 0, "raised once");
    assert_eq!(died.status, Status::Error);
    assert_eq!(died.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{died:?}");
    for (command, params) in [("timeline.rows.list", json!({})), ("doc.state.get", json!({}))] {
        let e = run(&h, command, params);
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{command}: {e:?}");
        let error = e.error.as_ref().unwrap();
        assert!(error.message.contains("document thread has stopped"), "{e:?}");
        assert_eq!(error.details["reason"], tt_executor::ENGINE_STOPPED);
        assert!(error.hint.as_deref().unwrap_or_default().contains("project.recovery.list"), "{e:?}");
    }
    // The open session is gone with the thread: a new executor (what the host's restart creates) has no project.
    let fresh = ExecutorHandle::spawn(Default::default()).unwrap();
    assert_eq!(run(&fresh, "timeline.rows.list", json!({})).error.map(|e| e.code), Some("project.none".to_owned()));
}
