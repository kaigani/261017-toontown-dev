//! All work-account heavy mutation routes observe the generic request watchdog atomically.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::ExecutorConfig;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
#[test]
fn heavy_command_watchdogs_leave_no_partial_state_and_release_queued_reads() {
    for id in [
        "draw.flatten.apply",
        "draw.optimize.apply",
        "draw.smooth.apply",
        "draw.centreLine.apply",
        "draw.linesToBrush.apply",
        "draw.breakApart.apply",
    ] {
        let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(800) });
        let a = ok(
            &h,
            "draw.rectangle.add",
            json!({"drawingId":D,"layer":"new","from":{"x":2000,"y":2000},"to":{"x":2100,"y":2100}}),
        );
        let b = ok(
            &h,
            "draw.rectangle.add",
            json!({"drawingId":D,"layer":"new","from":{"x":2200,"y":2000},"to":{"x":2300,"y":2100}}),
        );
        let params = match id {
            "draw.flatten.apply" | "draw.optimize.apply" => json!({"drawingId":D,"ids":[a["layerId"],b["layerId"]]}),
            "draw.smooth.apply" => {
                let points: Vec<_> = (0..400).map(|i| json!({"x":3000+i,"y":3000})).collect();
                let s = ok(
                    &h,
                    "draw.stroke.add",
                    json!({"drawingId":D,"kind":"invisible","fit":false,"layer":"new","points":points}),
                );
                json!({"drawingId":D,"ids":s["strokeIds"]})
            }
            "draw.centreLine.apply" => json!({"drawingId":D,"ids":["st_01K742SG0QD1F2HFZSX8R4V7WP"]}),
            "draw.linesToBrush.apply" => json!({"drawingId":D,"ids":a["strokeIds"]}),
            _ => {
                ok(
                    &h,
                    "draw.text.set",
                    json!({"drawingId":D,"textId":"nd_01K742SG0WFAQJFKAZKV4CD56C","text":"O".repeat(1000)}),
                );
                json!({"drawingId":D,"textIds":["nd_01K742SG0WFAQJFKAZKV4CD56C"]})
            }
        };
        let before = get(&h, D);
        let v = rev(&h);
        tt_vector::work::hooks::set_spin_micros(1_000_000);
        let t = Instant::now();
        let e = run(&h, id, params);
        let elapsed = t.elapsed().as_secs_f64();
        tt_vector::work::hooks::set_spin_micros(0);
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{id}: {e:?}");
        assert!(elapsed < 1.2, "{id}: {elapsed}");
        let t = Instant::now();
        let read = run(&h, "doc.state.get", json!({}));
        assert!(read.is_ok(), "{id}: {read:?}");
        println!("{id}: callerSeconds={elapsed} queuedReadSeconds={}", t.elapsed().as_secs_f64());
        assert_eq!(rev(&h), v, "{id}");
        assert_eq!(get(&h, D), before, "{id}");
    }
}
