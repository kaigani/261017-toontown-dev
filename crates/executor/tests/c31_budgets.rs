//! M3 C3.1: calibration of the timing work account (`TIMING_WORK_BUDGET`, `commands/timing.rs`) — raw observations of
//! the accepted worst cases and of the refusal, kept as evidence (never a compliance claim).
//!
//! The scene grows to `TT_C31_COLUMNS` drawing columns (default 1000) of 100 000 frames each (the 10 §5 limits:
//! elements per scene ≥ 1 000 handled, scene length ≤ 100 000 frames). Each case records its outcome (`ok` or the
//! refusal code), wall time and the work it was allowed; the assertions are only that every case answers well inside
//! the 30-s watchdog and that an over-budget case is the typed `limit.exceeded` with nothing staged.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, Status};

const INTRO: &str = "scene:\"Intro\"";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

fn case(h: &tt_executor::ExecutorHandle, name: &str, id: &str, params: Value, out: &mut Vec<Value>) -> (Status, f64) {
    let r0 = rev(h);
    let t = Instant::now();
    let env = run(h, id, params);
    let secs = t.elapsed().as_secs_f64();
    let outcome = match &env.error {
        None => "ok".to_owned(),
        Some(e) => e.code.clone(),
    };
    println!("c31-budget {name}: {outcome} in {secs:.3} s");
    out.push(json!({ "case": name, "command": id, "outcome": outcome, "seconds": secs, "revBefore": r0, "revAfter": rev(h) }));
    if env.status != Status::Ok {
        assert_eq!(rev(h), r0, "{name}: a refusal stages nothing");
    }
    (env.status, secs)
}

#[test]
#[ignore = "measurement; run with TT_MEASURE=1 and --ignored in release"]
fn timing_budget_raw_observations() {
    let columns: usize = std::env::var("TT_C31_COLUMNS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
    let h = fixture_with(ExecutorConfig { budget: Duration::from_secs(120) });
    let mut out = Vec::new();
    // Columns of 100 000 frames (two runs each, written as runs: cheap to build).
    let mut made = 0;
    while made < columns {
        let n = (columns - made).min(99);
        ok(&h, "element.create", json!({ "type": "drawing", "count": n, "sceneId": INTRO }));
        made += n;
    }
    let ids: Vec<String> = ok(&h, "element.list", json!({ "sceneId": INTRO, "order": "xsheet", "types": ["drawing"] }))
        ["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    let t = Instant::now();
    for chunk in ids.chunks(50) {
        let cols: Vec<Value> = chunk
            .iter()
            .map(|e| json!({ "elementId": e, "from": 1, "to": 100_000, "exposures": [{ "drawing": "1", "from": 1, "to": 50_000 }, { "drawing": "2", "from": 50_001, "to": 100_000 }] }))
            .collect();
        ok(&h, "xsheet.exposures.set", json!({ "createMissingDrawings": true, "columns": cols }));
    }
    println!("c31-budget setup: {} columns of 100000 frames in {:.3} s", ids.len(), t.elapsed().as_secs_f64());
    let all: Vec<&String> = ids.iter().collect();
    let tenth: Vec<&String> = ids.iter().take(ids.len() / 10).collect();
    // Accepted cases.
    case(&h, "read one 100k column", "xsheet.exposures.get", json!({ "elementIds": [ids[0]] }), &mut out);
    case(
        &h,
        "cycle 999 x 100 frames",
        "cell.cycle.create",
        json!({ "elementId": BOB, "from": 1, "to": 100, "cycles": 999 }),
        &mut out,
    );
    case(
        &h,
        "set exposure on a tenth of the columns",
        "cell.exposure.set",
        json!({ "elementIds": tenth, "from": 1, "to": 100_000, "length": 2 }),
        &mut out,
    );
    case(&h, "read the whole sheet", "xsheet.exposures.get", json!({ "sceneId": INTRO }), &mut out);
    case(&h, "element list", "element.list", json!({ "sceneId": INTRO }), &mut out);
    case(&h, "unused drawings of the scene", "element.unusedDrawing.list", json!({ "sceneId": INTRO }), &mut out);
    // The largest single calls: every column, 100 000 frames each (may be refused at the budget).
    let (_, s1) = case(
        &h,
        "remove exposure on every column",
        "cell.exposure.remove",
        json!({ "elementIds": all, "from": 1, "to": 1 }),
        &mut out,
    );
    let (st, s2) = case(
        &h,
        "insert a cell in all columns",
        "cell.range.add",
        json!({ "allElements": true, "elementIds": [BOB], "frame": 1 }),
        &mut out,
    );
    for s in [s1, s2] {
        assert!(s < 30.0, "every case answers inside the watchdog: {s} s");
    }
    if st != Status::Ok {
        assert_eq!(out.last().unwrap()["outcome"], "limit.exceeded");
    }
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c31");
    std::fs::create_dir_all(&dir).unwrap();
    let report = json!({ "columns": ids.len(), "framesPerColumn": 100_000, "budget": 300_000_000u64, "os": std::env::consts::OS, "cases": out });
    std::fs::write(dir.join("timing-budget.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
