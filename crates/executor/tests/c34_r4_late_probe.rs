//! 200 normal-load trials of an answer positively later than the 250 ms question bound.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tt_executor::Invocation;
use tt_vector::work::hooks as work;
#[test]
fn two_hundred_answers_after_the_bound_must_latch_after_one_question() {
    let mut bad = Vec::new();
    let mut latencies = Vec::new();
    let mut histogram = std::collections::BTreeMap::new();
    for trial in 0..200 {
        let h = fixture();
        let n = AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel();
        work::reset_asks();
        let latency = std::thread::scope(|scope| {
            let call = scope.spawn(|| {
                h.invoke_served_until(
                    Invocation::user("element.create", json!({"type":"peg","name":"Late","sceneId":"scene:#1"})),
                    &|| {
                        let asks_at_start = work::asks();
                        if asks_at_start == 0 {
                            return false;
                        }
                        let index = n.fetch_add(1, Ordering::SeqCst);
                        let start = Instant::now();
                        if index == 0 {
                            tx.send(()).unwrap();
                        }
                        let hold = Duration::from_millis(260);
                        while start.elapsed() < hold {
                            std::thread::sleep(hold.saturating_sub(start.elapsed()));
                        }
                        eprintln!(
                            "LATE_CALLBACK trial={trial} index={index} asks_at_start={asks_at_start} ms={}",
                            start.elapsed().as_secs_f64() * 1000.0
                        );
                        false
                    },
                )
                .0
            });
            rx.recv_timeout(Duration::from_secs(10)).unwrap();
            let start = Instant::now();
            assert!(run(&h, "doc.state.get", json!({})).is_ok());
            let latency = start.elapsed();
            assert!(call.join().unwrap().is_ok());
            latency
        });
        let asks = work::asks();
        *histogram.entry(asks).or_insert(0usize) += 1;
        eprintln!("LATE_TRIAL trial={trial} asks={asks} unrelated_ms={}", latency.as_secs_f64() * 1000.0);
        if asks != 1 {
            bad.push((trial, asks));
        }
        latencies.push(latency);
    }
    latencies.sort();
    eprintln!(
        "LATE_SUMMARY trials=200 histogram={histogram:?} min={:?} median={:?} p95={:?} p99={:?} max={:?}",
        latencies[0], latencies[99], latencies[189], latencies[197], latencies[199]
    );
    assert!(bad.is_empty(), "C34-R4-02: answers after the bound did not latch: {bad:?}");
}
