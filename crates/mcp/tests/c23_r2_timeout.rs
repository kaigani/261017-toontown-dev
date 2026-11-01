//! A typed watchdog timeout must reach the MCP caller without waiting for the audit entry.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod support;
use serde_json::json;
use std::time::{Duration, Instant};
use support::*;
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation};
#[test]
fn smooth_watchdog_reaches_mcp_and_next_read_answers() {
    let dir = tmp();
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_millis(600) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let points: Vec<_> = (0..20_000).map(|i| json!({"x":2000+i,"y":2000})).collect();
    let add=h.invoke(Invocation::user("draw.stroke.add",json!({"drawingId":"dr_01K742SG0KKMCAP9X1E6Z5P4SW","kind":"invisible","fit":false,"layer":"new","points":points})));
    assert!(add.is_ok(), "{add:?}");
    let ids = add.result.unwrap()["strokeIds"].clone();
    let start = Instant::now();
    let r = c.run("draw.smooth.apply", json!({"drawingId":"dr_01K742SG0KKMCAP9X1E6Z5P4SW","ids":ids,"tolerance":1}));
    let elapsed = start.elapsed();
    println!("MCP timeout {r}; elapsed {elapsed:?}");
    assert_eq!(r["error"]["code"], "command.timeout");
    assert!(elapsed < Duration::from_secs(2));
    let start = Instant::now();
    let r = c.run("doc.state.get", json!({}));
    assert_eq!(r["status"], "ok", "{r}");
    assert!(start.elapsed() < Duration::from_secs(1));
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
