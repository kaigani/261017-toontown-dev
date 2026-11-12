//! Independent enqueue and deadline coverage.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{BatchRequest, ExecutorConfig, Invocation, Origin};
#[test]
fn queue_consumes_budget_for_batches_and_plain_actions() {
    for (delay, batch) in [(120, true), (450, true), (120, false), (450, false)] {
        let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(300) });
        let (tx, rx) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let wait = std::sync::Mutex::new(wait);
        tt_persist::lock::hooks::set(Some(std::sync::Arc::new(move |at, _| {
            if at == tt_persist::lock::Boundary::Committed {
                tx.send(()).unwrap();
                wait.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
            }
        })));
        let before = rev(&h);
        std::thread::scope(|s| {
            let blocker = s.spawn(|| h.invoke(Invocation::user("element.create",json!({"type":"peg","name":"block","sceneId":"scene:#1"}))));
            rx.recv_timeout(Duration::from_secs(10)).unwrap();
            // Only the first commit is held.
            tt_persist::lock::hooks::set(None);
            let timer = s.spawn(|| { std::thread::sleep(Duration::from_millis(delay)); release.send(()).unwrap(); });
            let at = Instant::now();
            let env = if batch {
                let req: BatchRequest = serde_json::from_value(json!({"label":"queued","atomic":false,"steps":[{"id":"element.create","params":{"type":"peg","name":"queued","sceneId":"scene:#1"}}]})).unwrap();
                h.batch(req,Origin::User)
            } else { h.invoke(Invocation::user("element.create",json!({"type":"peg","name":"queued","sceneId":"scene:#1"}))) };
            if delay < 300 { assert!(env.is_ok(),"{env:?}"); assert!(at.elapsed()<Duration::from_millis(300)); }
            else { assert_eq!(env.error.unwrap().code,"command.timeout"); }
            timer.join().unwrap(); blocker.join().unwrap();
        });
        assert_eq!(rev(&h),before+if delay<300 {2} else {1});
    }
}
