//! Independent round-3 cancellation boundary probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, Invocation};
fn edit() -> Invocation {
    Invocation::user("element.create", json!({"type":"peg","name":"Review","sceneId":"scene:#1"}))
}
#[test]
fn blocked_probe_does_not_starve_other_callers_past_watchdog() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(800) });
    let (tx, rx) = std::sync::mpsc::channel();
    let n = AtomicUsize::new(0);
    let (other, elapsed) = std::thread::scope(|s| {
        let caller = s.spawn(|| h.invoke_served_until(edit(), &|| {
            if n.fetch_add(1, Ordering::SeqCst) == 1 {
                tx.send(()).unwrap();
                std::thread::sleep(Duration::from_secs(6));
            }
            false
        }));
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let start = Instant::now();
        let other = run(&h, "doc.state.get", json!({}));
        let elapsed = start.elapsed();
        caller.join().unwrap();
        (other, elapsed)
    });
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    eprintln!("blocked probe: unrelated query {elapsed:?}: {other:?}");
    assert!(other.is_ok(), "unrelated query lost its entire watchdog budget waiting for a caller probe");
}
#[test]
fn panicking_probe_leaves_worker_alive() {
    let h = fixture();
    let before = rev(&h);
    let n = AtomicUsize::new(0);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        h.invoke_served_until(edit(), &|| {
            assert_ne!(n.fetch_add(1, Ordering::SeqCst), 1, "probe panic");
            false
        })
    }));
    assert!(caught.is_err());
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    assert_eq!(rev(&h), before);
}
#[test]
fn cancellation_is_latched_even_if_probe_flips_back() {
    let h = fixture();
    let before = rev(&h);
    let n = AtomicUsize::new(0);
    let env = h.invoke_served_until(edit(), &|| n.fetch_add(1, Ordering::SeqCst) == 1).0;
    assert!(!env.is_ok());
    assert_eq!(rev(&h), before);
}
#[test]
fn overhead_ten_thousand_small_actions_and_thousand_step_batch() {
    let h = fixture();
    // app state actions avoid a growing document; invalid actions are not used for this measurement.
    let inv = || Invocation::user("edit.selection.set", json!({"context":"drawing","ids":[]}));
    let warm = h.invoke(inv());
    assert!(warm.is_ok(), "{warm:?}");
    for direct in [false, true] {
        let start = Instant::now();
        for _ in 0..10000 {
            let e = if direct { h.invoke_served_until(inv(), &|| false).0 } else { h.invoke(inv()) };
            assert!(e.is_ok(), "{e:?}");
        }
        eprintln!("PERF direct={direct} n=10000 ms={}", start.elapsed().as_secs_f64()*1000.0);
        let req = serde_json::from_value(json!({"label":"measure","atomic":false,"steps":(0..1000).map(|_| json!({"id":"edit.selection.set","params":{"context":"drawing","ids":[]}})).collect::<Vec<_>>()})).unwrap();
        let start = Instant::now();
        let e = if direct { h.batch_until(req, tt_executor::Origin::User, &|| false) } else {h.batch(req, tt_executor::Origin::User)};
        assert!(e.is_ok(), "{e:?}");
        eprintln!("PERF batch direct={direct} n=1000 ms={}", start.elapsed().as_secs_f64()*1000.0);
    }
}

