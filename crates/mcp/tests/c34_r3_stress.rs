//! Sustained concurrent cancellation wire stress, independent of builder tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod support;
use serde_json::json;
use std::time::{Duration, Instant};
#[test]
fn concurrent_wire_cancellation_sixty_seconds() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut options = support::opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = support::Client::connect(&h, options, json!({}));
    let start = Instant::now();
    let mut count = 0;
    let mut seed = 731u64;
    while start.elapsed() < Duration::from_secs(60) {
        let mut ids = Vec::new();
        let mut cancellations = Vec::new();
        for _ in 0..24 {
            let id = c.send("tools/call", json!({"name":"run_command","arguments":{"id":"edit.selection.set","params":{"context":"drawing","ids":[]}}}));
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            if !seed.is_multiple_of(3) {
                cancellations.push((id, (seed >> 16) % 300));
            } else {
                ids.push(id);
            }
            count += 1;
        }
        for (id, delay) in cancellations {
            std::thread::sleep(Duration::from_micros(delay));
            c.notify("notifications/cancelled", json!({"requestId":id}));
        }
        for id in ids {
            assert!(c.wait(id, Duration::from_secs(10)).is_some(), "server stalled");
        }
    }
    assert_eq!(c.run("doc.state.get", json!({}))["status"], "ok");
    eprintln!("STRESS requests={count} seconds={}", start.elapsed().as_secs_f64());
    c.close().unwrap();
}
