//! Selection work must observe request cancellation before blocking later calls.
//!
//! Restated as a shape under M5 ruling E-46 (D-45): the follow-up read used to answer within the executor's 200 ms
//! budget counted from its enqueue, which includes the cancelled selection's tail, and the 3-core macOS runner missed it
//! once (run 45243854755, `doc.state.get` timed out). Now: the time until the worker answers again after the cancelled
//! selection must be a small part of the same selection run to completion in the same process (the cancellation saved
//! the work), under a ceiling that only catches a gross regression; every phase is printed on failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, Status};

const ELEMENT: &str = "scene:\"Intro\"/element:\"Bob\"";

#[test]
fn cancelled_large_selection_releases_worker_for_following_read() {
    // The same selection run to completion (a budget it never reaches): what the cancellation must save.
    let full = fixture_with(ExecutorConfig { budget: Duration::from_secs(120) });
    let t = Instant::now();
    let whole = run(&full, "cell.select", json!({"elementIds":[ELEMENT],"from":1,"to":100000}));
    let full_ms = t.elapsed().as_secs_f64() * 1000.0;
    drop(full);

    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(200) });
    let start = Instant::now();
    let selected = run(&h, "cell.select", json!({"elementIds":[ELEMENT],"from":1,"to":100000}));
    let answered_ms = start.elapsed().as_secs_f64() * 1000.0;
    // Reads until one answers: a read whose caller timed out is discarded when it reaches the worker, so the first
    // answer comes once the cancelled selection has stopped.
    let freed = Instant::now();
    let mut timed_out = 0;
    let mut first_error = None;
    let read = loop {
        let r = run(&h, "doc.state.get", json!({}));
        if r.status == Status::Ok || timed_out >= 50 {
            break r;
        }
        timed_out += 1;
        if first_error.is_none() {
            first_error = r.error.clone();
        }
    };
    let free_ms = freed.elapsed().as_secs_f64() * 1000.0;
    let phases = format!(
        "full selection {full_ms:.0} ms ({:?}); cancelled selection answered after {answered_ms:.0} ms ({:?} {:?}); \
         worker answered again after {free_ms:.0} ms, {timed_out} reads timed out first (first: {first_error:?})",
        whole.status,
        selected.status,
        selected.error.as_ref().map(|e| e.code.as_str()),
    );

    assert!(answered_ms < 2000.0, "caller watchdog must answer: {phases}");
    if selected.status == Status::Error {
        assert!(
            matches!(selected.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout" | "limit.exceeded")),
            "{selected:?}; {phases}"
        );
    }
    assert_eq!(
        read.status,
        Status::Ok,
        "cancelled selection must stop its work, freeing the queue: {read:?}; {phases}"
    );
    // The shape: the cancellation saves most of the work; the ceiling catches a worker that never stops.
    assert!(free_ms < full_ms / 2.0, "the worker stayed busy for most of the uncancelled selection: {phases}");
    assert!(free_ms < 5000.0, "the worker stayed busy: {phases}");
}
