//! M2 C2.4 readiness item 4 (C22-T2 gate, orchestrator ruling on the C2.4 plan): raw observations of Flatten on the
//! 100 000-stroke drawing (two layers of 50 000 strokes), five repetitions each, in release in the CI headless job on
//! both OSes:
//!
//! - **budgeted** — the product: refused atomically with `limit.exceeded` before the watchdog while the merge exceeds
//!   `MERGE_WORK_BUDGET`; never labelled completion;
//! - **unbudgeted** — the budget lifted by the dev-only hook (`tt-executor/test-hooks`), timing the merge to completion.
//!
//! The gate compares the unbudgeted p50 with the same run's 100k `strokeAdd` p50 (`c21_measurements.rs`) and the 30 s
//! watchdog: `scripts/c22-t2-gate.py`. Ignored by default; `TT_MEASURE=1 cargo test --release -p tt-executor --features
//! test-hooks --test c24_measure -- --ignored`. Output: `target/evidence/c24/flatten-100k-<os>.json`.
#![cfg(feature = "test-hooks")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle};
use tt_model::entities::drawing::Drawing;
use tt_model::object::from_json;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const REPETITIONS: usize = 5;

/// The sample with drawing "1" replaced by two layers of 50 000 short pencil strokes each (as `c23_measure.rs`).
fn large() -> tt_model::document::Document {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let index = parts.drawings.iter().position(|d| d.id().as_str() == D1).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[index]).unwrap();
    let template = d["nodes"][0]["strokes"][1].clone();
    let layer = d["nodes"][0].clone();
    let make = |k: usize, dy: f64| -> Value {
        let mut l = layer.clone();
        l["id"] = json!(format!("nd_01K9MEASURE0000000000000{k}"));
        l["strokes"] = json!(
            (0..50_000)
                .map(|n| {
                    let mut s = template.clone();
                    let (x, y) = (f64::from(n % 250) * 10.0, f64::from(n / 250) * 10.0 + dy);
                    s["id"] = json!(format!("st_m{k}_{n}"));
                    s["points"] = json!([[x, y, 0, 0, 0, 0], [x + 5.0, y + 2.0, 0, 0, 0, 0]]);
                    s
                })
                .collect::<Vec<_>>()
        );
        l
    };
    d["nodes"] = json!([make(1, 0.0), make(2, 5.0)]);
    parts.drawings[index] = from_json::<Drawing>(&d).unwrap();
    tt_model::document::Document::from_parts(parts).unwrap()
}

fn flatten_once(budget: Option<u64>) -> Value {
    tt_executor::hooks::set_merge_work_budget(budget.unwrap_or(0));
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(600) }).unwrap();
    let t = Instant::now();
    assert!(h.open_document(large(), None, false).is_ok());
    let open = t.elapsed().as_secs_f64();
    let ids = json!(["nd_01K9MEASURE00000000000001", "nd_01K9MEASURE00000000000002"]);
    let v = rev(&h);
    let t = Instant::now();
    let env = run(&h, "draw.flatten.apply", json!({ "drawingId": D1, "ids": ids }));
    let seconds = t.elapsed().as_secs_f64();
    tt_executor::hooks::set_merge_work_budget(0);
    json!({
        "seconds": seconds, "openSeconds": open, "status": format!("{:?}", env.status),
        "error": env.error.as_ref().map(|e| json!({ "code": e.code })),
        "committed": rev(&h) == v + 1, "unchangedOnRefusal": env.error.is_none() || rev(&h) == v,
    })
}

fn p50(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[(s.len() - 1) / 2]
}

#[test]
#[ignore = "measurement; run with TT_MEASURE=1 and --ignored in release with --features test-hooks"]
fn flatten_100k_raw_observations() {
    let budgeted: Vec<Value> = (0..REPETITIONS).map(|_| flatten_once(None)).collect();
    for b in &budgeted {
        // The product never holds the worker past the watchdog: a refusal is atomic and quick, never a completion.
        if b["error"].is_object() {
            assert_eq!(b["error"]["code"], "limit.exceeded", "{b}");
            assert_eq!(b["unchangedOnRefusal"], true, "{b}");
            assert!(b["seconds"].as_f64().unwrap() < 30.0, "{b}");
        }
    }
    let unbudgeted: Vec<Value> = (0..REPETITIONS).map(|_| flatten_once(Some(u64::MAX))).collect();
    let secs = |v: &[Value]| v.iter().map(|x| x["seconds"].as_f64().unwrap()).collect::<Vec<_>>();
    let out = json!({
        "strokes": 100_000, "repetitions": REPETITIONS,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "budgeted": budgeted, "budgetedP50Seconds": p50(&secs(&budgeted)),
        "unbudgeted": unbudgeted, "unbudgetedP50Seconds": p50(&secs(&unbudgeted)),
        "unbudgetedAllCompleted": unbudgeted.iter().all(|u| u["committed"] == true),
    });
    if std::env::var("TT_MEASURE").as_deref() == Ok("1") {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c24");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("flatten-100k-{}.json", std::env::consts::OS)),
            serde_json::to_string_pretty(&out).unwrap() + "\n",
        )
        .unwrap();
    }
    println!("{out}");
}
