#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod support;
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use support::*;
use tt_persist::lock::{Boundary, hooks};

#[test]
fn e3_real_client_gets_hint_without_shell_notices() {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            hooks::set(None);
        }
    }
    let _reset = Reset;
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    c.request("resources/subscribe", json!({"uri":"app://events"}));
    c.request("logging/setLevel", json!({"level":"debug"}));
    hooks::set(Some(Arc::new(|b, _| {
        if b == Boundary::Committed {
            panic!("review: commit panic");
        }
    })));
    let out = h.invoke(tt_executor::Invocation::user("element.create", json!({"type":"peg","name":"Panic"})));
    assert_eq!(out.error.unwrap().code, "internal");
    hooks::set(None);
    let deadline = Instant::now() + Duration::from_secs(60);
    while !h.degraded() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let out = c.run("timeline.rows.list", json!({}));
    assert_eq!(out["error"]["code"], "internal", "{out}");
    assert_eq!(out["error"]["details"]["reason"], "engineStopped", "{out}");
    assert!(out["error"]["hint"].as_str().unwrap().contains("project.recovery.list"));
    c.drain(Duration::from_secs(3));
    let notes = serde_json::to_string(&c.notes_of("notifications/message")).unwrap();
    assert!(!notes.contains("shell."), "{notes}");
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
