//! Observe the same 20k-anchor staging operation under configurable dev model optimization.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review measurement")]
mod common;
use common::*;
use serde_json::json;
#[test]
fn observe_twenty_thousand_anchor_staging() {
    let mut raw = vec![];
    for _ in 0..5 {
        let h = fixture();
        let points: Vec<_> = (0..20000).map(|i| json!({"x":2000+i,"y":2000})).collect();
        let t = std::time::Instant::now();
        ok(
            &h,
            "draw.stroke.add",
            json!({"drawingId":"dr_01K742SG0KKMCAP9X1E6Z5P4SW","kind":"invisible","fit":false,"layer":"new","points":points}),
        );
        raw.push(t.elapsed().as_secs_f64());
    }
    println!("staging raw seconds={raw:?}");
    if let Some(out) = std::env::var_os("TT_REVIEW_OUT") {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            std::path::PathBuf::from(out).join("model-staging.json"),
            serde_json::to_vec_pretty(&raw).unwrap(),
        )
        .unwrap();
    }
}
