//! Cache eviction plus cold-cache undo/redo; allocator observations are process-live bytes, not RSS.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA")]
mod common;
use common::*;
use serde_json::{Value, json};
use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};
use std::alloc::System;
#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
fn live() -> i64 {
    let s = GLOBAL.stats();
    s.bytes_allocated as i64 - s.bytes_deallocated as i64
}
fn read(h: &tt_executor::ExecutorHandle) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":D}))
}
#[test]
fn six_hundred_layers_evict_expensive_layer_before_undo_redo_without_changing_deltas() {
    let mut p = tt_model::fixture::sample_parts(1).unwrap();
    let i = p.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut d = serde_json::to_value(&p.drawings[i]).unwrap();
    let node = d["nodes"][0].clone();
    let stroke = node["strokes"][1].clone();
    let mut nodes = Vec::new();
    for k in 0..600 {
        let mut n = node.clone();
        n["id"] = json!(format!("nd_r3_cache{k}"));
        let mut s = stroke.clone();
        s["id"] = json!(format!("st_r3_cache{k}"));
        s["closed"] = json!(true);
        s["points"] = json!([[0, 0, 0, 0, 0, 0], [100, 0, 0, 0, 0, 0], [100, 100, 0, 0, 0, 0], [0, 100, 0, 0, 0, 0]]);
        let mut strokes = vec![s];
        if k == 0 {
            for cut in 1..16 {
                for horizontal in [false, true] {
                    let v = cut as f64 * 6.;
                    let mut s = stroke.clone();
                    s["id"] = json!(format!("st_r3_grid{cut}_{horizontal}"));
                    s["points"] = if horizontal {
                        json!([[0, v, 0, 0, 0, 0], [100, v, 0, 0, 0, 0]])
                    } else {
                        json!([[v, 0, 0, 0, 0, 0], [v, 100, 0, 0, 0, 0]])
                    };
                    strokes.push(s);
                }
            }
        }
        n["strokes"] = json!(strokes);
        nodes.push(n);
    }
    d["nodes"] = json!(nodes);
    p.drawings[i] = tt_model::object::from_json(&d).unwrap();
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::document::Document::from_parts(p).unwrap(), None, false).is_ok());
    let initial = read(&h)["nodes"][0]["zones"].clone();
    assert_eq!(initial.as_array().unwrap().len(), 256);
    let add = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"invisible","layer":"nd_r3_cache0","fit":false,"points":[{"x":200,"y":200},{"x":210,"y":200}]}),
    );
    let after = read(&h)["nodes"][0]["zones"].clone();
    let rx = h.subscribe();
    let mut samples = Vec::new();
    let mut query_samples = Vec::new();
    let mut deltas = Vec::new();
    for _ in 0..6 {
        // Each full read visits 600 distinct maps in order and evicts the first map (capacity 512).
        ok(&h, "edit.undo", json!({}));
        assert_eq!(read(&h)["nodes"][0]["zones"], initial);
        let undo = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").map(|e| e.payload).collect::<Vec<_>>();
        assert_eq!(undo.len(), 1);
        ok(&h, "edit.redo", json!({}));
        assert_eq!(read(&h)["nodes"][0]["zones"], after);
        let redo = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").map(|e| e.payload).collect::<Vec<_>>();
        assert_eq!(redo.len(), 1);
        deltas.push(json!({"undoCreated":undo[0]["createdIds"],"undoRemoved":undo[0]["removedIds"],"redoCreated":redo[0]["createdIds"],"redoRemoved":redo[0]["removedIds"]}));
        samples.push(live());
        let mut reads = vec![live()];
        for _ in 0..2 {
            drop(read(&h));
            reads.push(live());
        }
        assert!((reads[2] - reads[1]).abs() < 1_048_576, "cache churn at fixed history must plateau: {reads:?}");
        query_samples.push(reads);
    }
    assert!(deltas.windows(2).all(|w| w[0] == w[1]), "cold eviction must not alter deltas");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/review-c22-r3");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("cache.json"),serde_json::to_vec_pretty(&json!({"layers":600,"firstLayerZones":256,"cacheCapacity":512,"liveBytes":samples,"sameHistoryQueryLiveBytes":query_samples,"meaning":"Across-transaction process live bytes include retained revision/diff history; cache plateau compares reads at fixed history.","deltas":deltas,"add":add})).unwrap()).unwrap();
    eprintln!("600-layer cold-cache undo/redo process-live bytes: {samples:?}");
}
