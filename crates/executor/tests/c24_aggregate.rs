//! M2 C2.4 readiness item 9 (reviews/M2-C2.3-r3.md): large aggregates in the queue/cancellation corpus. Break Text
//! Apart pre-charges glyph preparation and result staging, so a text above its bound — up to the schema-limit
//! 100 000 characters — is refused with `limit.exceeded` before anything is prepared or staged, and the size-limit
//! render (8192 × 8192) completes. Raw observations, separately for each: the caller's answer time (refusal or result),
//! the following read's time, and a read queued behind the call from another thread.
//!
//! `an_over_bound_text_is_refused_before_any_glyph_is_prepared` runs in `check`; the measurement is ignored by default:
//! `TT_MEASURE=1 cargo test --release -p tt-executor --test c24_aggregate -- --ignored`, in the CI headless job on both
//! OSes. Output: `target/evidence/c24/aggregate-<os>.json`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const REPETITIONS: usize = 5;

/// A text of `n` characters (words and spaces).
fn text(n: usize) -> String {
    "Toontown aggregate ".chars().cycle().take(n).collect()
}

fn rev_of(h: &ExecutorHandle) -> u64 {
    rev(h)
}

#[test]
fn an_over_bound_text_is_refused_before_any_glyph_is_prepared() {
    let h = fixture();
    let t = ok(&h, "draw.text.add", json!({ "drawingId": D1, "position": {"x": 0, "y": 0}, "text": text(30_000) }));
    let before = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
    let v = rev_of(&h);
    let started = Instant::now();
    let e = err(&h, "draw.breakApart.apply", json!({ "drawingId": D1, "textIds": [t["textId"]] }));
    let refused = started.elapsed();
    assert_eq!(e.code, "limit.exceeded", "{e:?}");
    assert_eq!(rev_of(&h), v, "nothing staged");
    assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": D1 })), before);
    // Refused by the pre-charge: no glyph was prepared (preparing 25 000 glyphs takes seconds even in release).
    assert!(refused < Duration::from_secs(5), "refused in {refused:?}");
}

/// One call answered on this thread, then a read; and the same call again with a read queued behind it on another
/// thread (sent 20 ms after the call).
fn observe(h: &ExecutorHandle, command: &str, params: &Value) -> Value {
    let v = rev_of(h);
    let t = Instant::now();
    let env = run(h, command, params.clone());
    let answered = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let read = run(h, "draw.drawing.get", json!({ "drawingId": D1, "include": ["zones"] }));
    let following = t.elapsed().as_secs_f64();
    let committed = rev_of(h) != v;
    if committed {
        ok(h, "edit.undo", json!({}));
    }
    let worker = h.clone();
    let (cmd, p) = (command.to_owned(), params.clone());
    let started = Instant::now();
    let call = std::thread::spawn(move || {
        let env = run(&worker, &cmd, p);
        (started.elapsed().as_secs_f64(), env.error.map(|e| e.code))
    });
    std::thread::sleep(Duration::from_millis(20));
    let t = Instant::now();
    let queued = run(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    let queued_read = t.elapsed().as_secs_f64();
    let (queued_call, queued_error) = call.join().unwrap();
    if rev_of(h) != v {
        ok(h, "edit.undo", json!({}));
    }
    json!({
        "answeredSeconds": answered, "status": format!("{:?}", env.status),
        "error": env.error.as_ref().map(|e| e.code.clone()), "committed": committed,
        "followingReadSeconds": following, "followingReadOk": read.error.is_none(),
        "queuedCallSeconds": queued_call, "queuedCallError": queued_error,
        "queuedReadSeconds": queued_read, "queuedReadOk": queued.error.is_none(),
    })
}

#[test]
#[ignore = "measurement; run with TT_MEASURE=1 and --ignored in release"]
fn aggregate_break_apart_and_size_limit_render_raw_observations() {
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(30) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let t = Instant::now();
    let added =
        ok(&h, "draw.text.add", json!({ "drawingId": D1, "position": {"x": 0, "y": 0}, "text": text(100_000) }));
    let text_add = t.elapsed().as_secs_f64();
    let apart = json!({ "drawingId": D1, "textIds": [added["textId"]] });
    let break_apart: Vec<Value> = (0..REPETITIONS).map(|_| observe(&h, "draw.breakApart.apply", &apart)).collect();
    for b in &break_apart {
        assert_eq!(b["error"], "limit.exceeded", "{b}");
        assert_eq!(b["committed"], false, "{b}");
    }
    // The size-limit render on the fixture's drawing as it is (a fresh executor: the 100 000-character text is not
    // part of the render case).
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(30) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let render = json!({ "drawingId": D1, "width": 8192, "height": 8192, "background": "#ffffff" });
    let renders: Vec<Value> = (0..REPETITIONS).map(|_| observe(&h, "draw.drawing.render", &render)).collect();
    let out = json!({
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "watchdogSeconds": 30, "repetitions": REPETITIONS,
        "breakApart": { "characters": 100_000, "textAddSeconds": text_add, "observations": break_apart },
        "render": { "width": 8192, "height": 8192, "observations": renders },
    });
    if std::env::var("TT_MEASURE").as_deref() == Ok("1") {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c24");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("aggregate-{}.json", std::env::consts::OS)),
            serde_json::to_string_pretty(&out).unwrap() + "\n",
        )
        .unwrap();
    }
    println!("{out}");
}
