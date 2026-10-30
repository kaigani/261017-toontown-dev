//! Never block (11 P5, §14.6; ADR-0013 §3; register row R58): a registered handler that does not return cannot block
//! its caller or later callers. Every call answers within the watchdog budget with `command.timeout`, the executor
//! keeps answering, and it serves requests again as soon as the handler returns. Preempting a handler that never
//! returns is not possible in-process (no safe thread termination); the host restarts the engine (C1.4), see
//! reviews/M1-C1.2-r1-response.md.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tt_executor::{Cx, ExecutorConfig, ExecutorHandle, Invocation};
use tt_registry::{CommandError, Registry, command};

static RELEASE: AtomicBool = AtomicBool::new(false);

/// A defective handler: it loops until the test releases it (in production nothing would).
#[command(
    id = "app.state.get",
    kind = query,
    label = "Loops",
    scopes = [read],
    example(title = "Loops", params = "{}")
)]
fn looping(_cx: &mut Cx, _p: Value) -> Result<Value, CommandError> {
    while !RELEASE.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(json!({ "released": true }))
}

#[test]
fn a_handler_that_does_not_return_never_blocks_callers() {
    let registry: &'static Registry = Box::leak(Box::new(Registry::assemble(&[&LOOPING]).unwrap()));
    let budget = Duration::from_millis(200);
    let h = ExecutorHandle::spawn_with_registry(ExecutorConfig { budget }, registry).unwrap();
    for _ in 0..3 {
        let t = Instant::now();
        let env = h.invoke(Invocation::user("app.state.get", json!({})));
        assert!(t.elapsed() < budget * 5, "a call answered after {:?}", t.elapsed());
        let e = env.error.unwrap();
        assert_eq!(e.code, "command.timeout");
        assert_eq!(e.details["budgetMs"], 200);
    }
    // Other requests (here a snapshot) also answer within the budget while the document thread is busy.
    let t = Instant::now();
    assert_eq!(h.snapshot().unwrap_err().code, "command.timeout");
    assert!(t.elapsed() < budget * 5);
    // Once the handler returns, the queued requests drain (their callers are gone) and the executor serves again.
    RELEASE.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let env = h.invoke(Invocation::user("app.state.get", json!({})));
        if env.is_ok() {
            assert_eq!(env.result.unwrap()["released"], true);
            break;
        }
        assert!(Instant::now() < deadline, "the executor did not recover: {env:?}");
    }
}
