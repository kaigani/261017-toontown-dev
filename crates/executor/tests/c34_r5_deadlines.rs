//! Independent enqueue and deadline coverage.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{BatchRequest, ExecutorConfig, Invocation, Origin};
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn queue_consumes_budget_for_batches_and_plain_actions() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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
            let blocker = s.spawn(|| {
                h.invoke(Invocation::user("element.create", json!({"type":"peg","name":"block","sceneId":"scene:#1"})))
            });
            rx.recv_timeout(Duration::from_secs(10)).unwrap();
            // Only the first commit is held.
            tt_persist::lock::hooks::set(None);
            let timer = s.spawn(|| {
                std::thread::sleep(Duration::from_millis(delay));
                release.send(()).unwrap();
            });
            let at = Instant::now();
            let env = if batch {
                let req: BatchRequest = serde_json::from_value(json!({"label":"queued","atomic":false,"steps":[{"id":"element.create","params":{"type":"peg","name":"queued","sceneId":"scene:#1"}}]})).unwrap();
                h.batch(req, Origin::User)
            } else {
                h.invoke(Invocation::user("element.create", json!({"type":"peg","name":"queued","sceneId":"scene:#1"})))
            };
            if delay < 300 {
                assert!(env.is_ok(), "{env:?}");
                assert!(at.elapsed() < Duration::from_millis(300));
            } else {
                assert_eq!(env.error.unwrap().code, "command.timeout");
            }
            timer.join().unwrap();
            blocker.join().unwrap();
        });
        assert_eq!(rev(&h), before + if delay < 300 { 2 } else { 1 });
    }
}

#[test]
fn held_probe_timeout_keeps_one_remembered_commit() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(200) });
    let before = rev(&h);
    let count = std::sync::atomic::AtomicUsize::new(0);
    let mut inv = Invocation::user("element.create", json!({"type":"peg","name":"remember","sceneId":"scene:#1"}));
    inv.options.request_id = Some("r5-held".into());
    let env = h
        .invoke_served_until(inv.clone(), &|| {
            if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                std::thread::sleep(Duration::from_millis(400));
            }
            false
        })
        .0;
    assert_eq!(env.error.unwrap().code, "command.timeout");
    assert_eq!(rev(&h), before + 1);
    let (retry, cached) = h.invoke_served(inv);
    assert!(retry.is_ok());
    assert!(cached);
    assert_eq!(rev(&h), before + 1);
}
