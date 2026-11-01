//! M2 C2.3 readiness item 18: one raw measurement per heavy command (flatten of a 100 000-stroke drawing, render of
//! it, centre line of a large brush stroke). Ignored by default; `TT_MEASURE=1 cargo test -p tt-executor --test
//! c23_measure -- --ignored` writes the raw observations to `reviews/evidence/M2-C2.3/measurements/`. Performance
//! budgets stay with C22-T2 and C22-R1-12 (open); these files are observations, not pass/fail gates.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "measurement")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle};
use tt_model::entities::drawing::Drawing;
use tt_model::object::from_json;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

/// The sample with drawing "1" replaced by two layers of 50 000 short pencil strokes each, on a grid, no crossings.
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

fn measure(h: &ExecutorHandle, command: &str, params: Value) -> Value {
    let t = Instant::now();
    let env = run(h, command, params.clone());
    let seconds = t.elapsed().as_secs_f64();
    json!({
        "command": command, "params": params, "seconds": seconds, "status": format!("{:?}", env.status),
        "error": env.error.map(|e| json!({ "code": e.code, "message": e.message })),
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    })
}

#[test]
#[ignore = "measurement; run with TT_MEASURE=1 and --ignored"]
fn heavy_commands_raw_measurements() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reviews/evidence/M2-C2.3/measurements");
    let write = std::env::var("TT_MEASURE").as_deref() == Ok("1");
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(600) }).unwrap();
    let t = Instant::now();
    let env = h.open_document(large(), None, false);
    let open = t.elapsed().as_secs_f64();
    assert!(env.is_ok(), "{env:?}");
    let mut results = vec![
        ("render-100k", measure(&h, "draw.drawing.render", json!({ "drawingId": D1, "width": 1024 }))),
        (
            "flatten-100k",
            measure(&h, "draw.flatten.apply", json!({ "drawingId": D1, "ids": ["nd_01K9MEASURE00000000000001", "nd_01K9MEASURE00000000000002"] })),
        ),
    ];
    // Centre line of a large brush stroke: a 2 000-sample spine.
    let pts: Vec<Value> = (0..2000).map(|k| json!({ "x": f64::from(k), "y": 3000.0 + 40.0 * libm_free_sin(f64::from(k) / 50.0), "pressure": 0.7 })).collect();
    let b = run(&h, "draw.stroke.add", json!({ "drawingId": D1, "kind": "brush", "layer": "new", "points": pts, "pen": { "tipShape": "round", "minSize": 4, "maxSize": 12, "tipAngle": 0 } }));
    if let Some(id) = b.result.as_ref().and_then(|r| r["strokeIds"][0].as_str()) {
        results.push(("centre-line-large", measure(&h, "draw.centreLine.apply", json!({ "drawingId": D1, "ids": [id] }))));
    }
    for (name, mut v) in results {
        v["openSeconds"] = json!(open);
        v["strokes"] = json!(100_000);
        eprintln!("{name}: {v}");
        if write {
            std::fs::create_dir_all(&out).unwrap();
            std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
        }
    }
}

/// A bounded triangle wave (no trigonometry needed for a test shape).
fn libm_free_sin(x: f64) -> f64 {
    let t = x.rem_euclid(4.0);
    if t < 2.0 { t - 1.0 } else { 3.0 - t }
}
