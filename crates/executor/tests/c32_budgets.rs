//! M3 C3.2 calibration (ADR-0019, never-block): long-range evaluation against the 30-second watchdog, measured on the CI
//! runners in release. `TT_MEASURE=1 cargo test --release -p tt-executor --test c32_budgets -- --ignored --nocapture`
//! writes `target/evidence/c32/evaluator-budget.json`. These are single observations on shared runners, not bounds; the
//! criterion is that every call answers, within the watchdog, a result or a typed refusal, and changes no revision.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorConfig;

const INTRO: &str = "scene:\"Intro\"";

#[test]
#[ignore = "release-only measurement; CI runs it in `measurements release`"]
fn long_range_evaluation_answers_within_the_watchdog() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_secs(120) });
    // A deep rig: 24 nested pegs, each with a 1 000-point path and three 1-D channels of 1 000 keys.
    let mut parent: Option<String> = None;
    let mut nodes = Vec::new();
    for i in 0..24 {
        let mut p = json!({ "type": "peg", "name": format!("Deep{i}"), "sceneId": INTRO });
        if let Some(q) = &parent {
            p["parentId"] = json!(q);
        }
        let id = ok(&h, "element.create", p)["elementIds"][0].as_str().unwrap().to_owned();
        ok(&h, "element.timing.set", json!({ "elementIds": [id], "duration": 100_000 }));
        let points: Vec<Value> = (0..1_000)
            .map(|k| json!({ "kind": "key", "frame": 1 + k * 100, "x": f64::from(k % 17), "y": f64::from(k % 13), "z": 0 }))
            .collect();
        ok(&h, "anim.path.set", json!({ "nodeId": id, "points": points }));
        for channel in ["rotation", "scale.x", "skew"] {
            let keys: Vec<Value> = (0..1_000)
                .map(|k| json!({ "frame": 1 + k * 100, "value": 1.0 + f64::from(k % 7), "preset": "easeInOut" }))
                .collect();
            ok(&h, "anim.function.set", json!({ "nodeId": id, "channel": channel, "keys": keys }));
        }
        parent = Some(id.clone());
        nodes.push(id);
    }
    let start = rev(&h);
    let deepest = nodes.last().unwrap().clone();
    let frames: Vec<f64> = (0..10_000).map(|i| 1.0 + f64::from(i) * 9.999).collect();
    let every: Vec<f64> = (0..100_000).map(|i| 1.0 + f64::from(i)).collect();
    let cases = [
        (
            "deepest node, 10 000 frames, world only",
            "anim.evaluate",
            json!({ "nodeIds": [deepest], "frames": frames, "include": ["world"] }),
        ),
        (
            "deepest node, 100 000 frames, world only",
            "anim.evaluate",
            json!({ "nodeIds": [deepest], "frames": every, "include": ["world"] }),
        ),
        ("all 24 nodes, 10 000 frames, everything", "anim.evaluate", json!({ "nodeIds": nodes, "frames": frames })),
        ("path sample over the bar", "anim.path.sample", json!({ "nodeId": deepest, "from": 1, "to": 100_000 })),
        (
            "function sample over the bar",
            "anim.function.sample",
            json!({ "nodeId": deepest, "channel": "rotation", "from": 1, "to": 100_000 }),
        ),
        ("path read back with passage frames", "anim.path.get", json!({ "nodeId": deepest })),
    ];
    let mut records = Vec::new();
    for (name, id, params) in cases {
        let t = Instant::now();
        let env = run(&h, id, params);
        let seconds = t.elapsed().as_secs_f64();
        let outcome = env.error.as_ref().map_or("completed".to_owned(), |e| e.code.clone());
        println!("{name}: {seconds:.3} s, {outcome}");
        assert!(seconds < 30.0, "{name} took {seconds} s: past the watchdog");
        assert_eq!(rev(&h), start, "{name}");
        records.push(json!({ "case": name, "seconds": seconds, "outcome": outcome }));
    }
    if std::env::var("TT_MEASURE").is_ok_and(|v| v == "1") {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c32");
        std::fs::create_dir_all(&dir).unwrap();
        let out = json!({ "records": records, "os": std::env::consts::OS, "nodes": 24, "pointsPerPath": 1000, "keysPerChannel": 1000 });
        std::fs::write(dir.join("evaluator-budget.json"), serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }
}
