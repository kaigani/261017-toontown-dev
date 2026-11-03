#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review cache retention probe")]
mod common;
use serde_json::json;
use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};
use std::alloc::System;
#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;
fn live() -> i64 {
    let s = GLOBAL.stats();
    s.bytes_allocated as i64 - s.bytes_deallocated as i64
}
#[test]
fn repeated_reads_past_the_512_map_capacity_have_stable_retained_memory() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let i = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
    let node = d["nodes"][0].clone();
    let stroke = node["strokes"][1].clone();
    d["nodes"] = json!(
        (0..600)
            .map(|k| {
                let mut n = node.clone();
                n["id"] = json!(format!("nd_review_cache{k}"));
                let mut s = stroke.clone();
                s["id"] = json!(format!("st_review_cache{k}"));
                s["closed"] = json!(true);
                s["points"] =
                    json!([[0, 0, 0, 0, 0, 0], [10, 0, 0, 0, 0, 0], [10, 10, 0, 0, 0, 0], [0, 10, 0, 0, 0, 0]]);
                n["strokes"] = json!([s]);
                n
            })
            .collect::<Vec<_>>()
    );
    parts.drawings[i] = tt_model::object::from_json(&d).unwrap();
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::document::Document::from_parts(parts).unwrap(), None, false).is_ok());
    let mut observations = vec![live()];
    for _ in 0..4 {
        let r = common::ok(&h, "draw.drawing.get", json!({"drawingId":"dr_01K742SG0KKMCAP9X1E6Z5P4SW"}));
        assert_eq!(r["nodes"].as_array().unwrap().len(), 600);
        drop(r);
        observations.push(live());
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/review-c22");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("cache-retention.json"),serde_json::to_vec_pretty(&json!({"layers":600,"capacity":512,"liveBytes":observations,"meaning":"counting allocator process live bytes after each dropped query result; includes model and session; not RSS or a cache byte ceiling"})).unwrap()).unwrap();
    assert!(observations[4] - observations[2] < 1_048_576, "repeated cache churn should plateau: {observations:?}");
}
