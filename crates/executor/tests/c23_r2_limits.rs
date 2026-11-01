//! Smooth 20k: typed budget refusal, atomic drawing state, next read and raw timing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::ExecutorConfig;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
#[test]
fn smooth_twenty_thousand_anchors_refuses_within_budget_and_commits_nothing() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_secs(30) });
    let pts: Vec<_> = (0..20_000).map(|i| json!({"x":2000+i,"y":2000})).collect();
    let s = ok(&h, "draw.stroke.add", json!({"drawingId":D,"kind":"invisible","fit":false,"layer":"new","points":pts}));
    let before = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
    let v = rev(&h);
    let start = Instant::now();
    let e = run(&h, "draw.smooth.apply", json!({"drawingId":D,"ids":s["strokeIds"],"tolerance":1}));
    let seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let state = run(&h, "doc.state.get", json!({}));
    let read_seconds = start.elapsed().as_secs_f64();
    let record = json!({"anchors":20_000,"seconds":seconds,"queuedReadSeconds":read_seconds,"error":e.error,"revisionUnchanged":rev(&h)==v,"profile":if cfg!(debug_assertions){"debug"}else{"release"}});
    println!("{record}");
    let out = std::env::var_os("TT_REVIEW_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("tt-c23-r2-limits"));
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("smooth-20k.json"), serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    assert_eq!(record["error"]["code"], "limit.exceeded");
    assert!(seconds < 20.0, "{seconds}");
    assert!(state.is_ok());
    assert!(read_seconds < 1.0);
    assert_eq!(rev(&h), v);
    assert_eq!(ok(&h, "draw.drawing.get", json!({"drawingId":D})), before);
}
