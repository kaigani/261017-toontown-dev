//! Selection work must observe request cancellation before blocking later calls.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, Status};
#[test]
fn cancelled_large_selection_releases_worker_for_following_read() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(200) });
    let start = Instant::now();
    let selected =
        run(&h, "cell.select", json!({"elementIds":["scene:\"Intro\"/element:\"Bob\""],"from":1,"to":100000}));
    assert!(start.elapsed() < Duration::from_secs(2), "caller watchdog must answer");
    if selected.status == Status::Error {
        assert!(
            matches!(selected.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout" | "limit.exceeded")),
            "{selected:?}"
        );
    }
    let read = run(&h, "doc.state.get", json!({}));
    assert_eq!(read.status, Status::Ok, "cancelled selection must stop its work, freeing the queue: {read:?}");
}
