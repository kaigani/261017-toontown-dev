//! M2 C2.1 obligation 4: the 100 000-stroke edit, undo, redo and dry-run paths of the drawing commands, measured from
//! observations — wall time per call, bytes allocated during the call, and bytes still held after it (the growth of
//! history and snapshots) — through the full integrity gate. Every observation is kept in
//! `target/evidence/c21/strokes-100k-<os>.json` and recomputed by `scripts/analyse-measurements.py`.
//!
//! The counting allocator wraps the system allocator for this test binary only (`stats_alloc`, a dev-dependency); the
//! product graphs are unaffected.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "test")]
#![allow(clippy::cast_precision_loss, reason = "statistics")]

mod c14_evidence;
mod common;

use std::alloc::System;
use std::time::{Duration, Instant};

use c14_evidence::Recorder;
use serde_json::{Value, json};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, InvokeOptions, Origin, Status};
use tt_model::document::Document;
use tt_model::entities::drawing::Drawing;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const STROKES: usize = 100_000;
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const LAYER: &str = "nd_01K742SG0PC20XDQKASM9CGZHR";

/// The fixture whose first drawing's first layer holds 100 000 pencil strokes (distinct geometry).
fn large() -> Document {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let index = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut d = serde_json::to_value(&parts.drawings[index]).unwrap();
    let template = d["nodes"][0]["strokes"][1].clone();
    d["nodes"][0]["strokes"] = json!(
        (0..STROKES)
            .map(|n| {
                let mut s = template.clone();
                s["id"] = json!(format!("s{n}"));
                let x = (n % 1000) as f64;
                let y = (n / 1000) as f64;
                s["points"] = json!([[x, y, 0, 0, 0.3, 0.2], [x + 0.9, y + 0.5, -0.3, 0, 0, 0]]);
                s
            })
            .collect::<Vec<_>>()
    );
    parts.drawings[index] = tt_model::object::from_json::<Drawing>(&d).unwrap();
    Document::from_parts(parts).unwrap()
}

/// Bytes held: `stats_alloc` already counts a reallocation's growth as allocated and its shrinking as deallocated.
fn live(stats: stats_alloc::Stats) -> f64 {
    stats.bytes_allocated as f64 - stats.bytes_deallocated as f64
}

/// Runs one call, recording wall time, bytes allocated during it and bytes retained after it.
fn measure(rec: &mut Recorder, series: &str, h: &ExecutorHandle, inv: Invocation) -> Value {
    let region = Region::new(GLOBAL);
    let t = Instant::now();
    let env = h.invoke(inv);
    let wall = t.elapsed();
    let change = region.change();
    assert_eq!(env.status, Status::Ok, "{series}: {:?}", env.error);
    rec.sample(&format!("{series}.wallMs"), "ms", t, wall.as_secs_f64() * 1e3, None);
    rec.sample(&format!("{series}.allocatedBytes"), "bytes", t, change.bytes_allocated as f64, None);
    rec.sample(&format!("{series}.retainedBytes"), "bytes", t, live(change), None);
    // Everything the process holds after the call (bounded history: this must level off, not grow per edit).
    rec.sample("process.liveBytes", "bytes", t, live(GLOBAL.stats()), Some(series));
    env.result.unwrap_or(Value::Null)
}

fn user(id: &str, params: Value) -> Invocation {
    Invocation::user(id, params)
}

#[test]
#[ignore = "release-only measurement (about 5 minutes at 100 000 strokes); CI runs it in the headless job with             `cargo test --release -p tt-executor --test c21_measurements -- --ignored` on Windows and macOS and checks             the evidence with scripts/analyse-measurements.py (M2 C2.1 obligation 4)"]
fn hundred_thousand_stroke_edit_undo_redo_and_dry_run_are_measured() {
    let reps = 10;
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(600) }).unwrap();
    let doc = large();
    let t = Instant::now();
    assert!(h.open_document(doc, None, false).is_ok());
    let open_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut rec = Recorder::new();
    for i in 0..reps {
        let add = json!({ "drawingId": D1, "kind": "pencil", "layer": LAYER, "thickness": 1,
                          "bezier": [{"x": i, "y": -5}, {"x": i, "y": -6}, {"x": i + 1, "y": -6}, {"x": i + 1, "y": -5}] });
        measure(&mut rec, "strokeAdd", &h, user("draw.stroke.add", add.clone()));
        measure(&mut rec, "undo", &h, user("edit.undo", json!({})));
        measure(&mut rec, "redo", &h, user("edit.redo", json!({})));
        measure(
            &mut rec,
            "strokeAddDryRun",
            &h,
            Invocation {
                command: "draw.stroke.add".into(),
                params: add,
                options: InvokeOptions { dry_run: true, ..Default::default() },
                origin: Origin::User,
            },
        );
        measure(
            &mut rec,
            "strokeSet",
            &h,
            user("draw.stroke.set", json!({ "drawingId": D1, "strokeId": format!("s{i}"), "thickness": 2 + i })),
        );
        // The M1 property-edit path on the same drawing, for comparison (ADR-0013 consequences: 2.4–2.8 s at C1.2).
        measure(
            &mut rec,
            "patchProperty",
            &h,
            user("doc.object.patch", json!({ "targets": [format!("{D1}/s{i}")], "props": { "meta": { "n": i } } })),
        );
        measure(
            &mut rec,
            "brushAdd",
            &h,
            user(
                "draw.stroke.add",
                json!({ "drawingId": D1, "kind": "brush", "layer": "new",
            "points": [{"x": 0, "y": -20, "pressure": 0.2}, {"x": 30, "y": -10, "pressure": 1}, {"x": 60, "y": -25, "pressure": 0.5}] }),
            ),
        );
    }
    let strokes = h.snapshot().unwrap().unwrap().drawing(&tt_model::Id::parse(D1).unwrap()).unwrap().nodes()[0]
        .strokes()
        .as_ref()
        .unwrap()
        .len();
    assert_eq!(strokes, STROKES + reps, "every add of the layer committed and every undo was redone");
    let report = rec.report(
        "M2 C2.1 obligation 4: 100k-stroke edit/undo/redo/dry-run (draw.* through the full integrity gate)",
        "fixture seed 1, drawing 1, layer 1 holding 100000 pencil strokes",
        json!({ "strokes": STROKES, "repetitions": reps, "openMs": open_ms }),
    );
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c21");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join(format!("strokes-100k-{}.json", std::env::consts::OS)),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    let lives = rec.values("process.liveBytes");
    let n = lives.len();
    // After the snapshot budget is reached, a further round of edits holds no more memory than the round before
    // (allowing 25 % for allocator slack and the growing undo list of small patches).
    if n >= 21 {
        let (a, b) = (lives[n - 8], lives[n - 1]);
        assert!(b <= a * 1.25, "live bytes keep growing: {a} -> {b}");
    }
    println!("process live bytes: first {} last {}", lives[0], lives[n - 1]);
    for s in ["strokeAdd", "undo", "redo", "strokeAddDryRun", "strokeSet", "patchProperty", "brushAdd"] {
        println!(
            "{s}: {} ms (p50), {} bytes retained (p50)",
            c14_evidence::summary(&rec.values(&format!("{s}.wallMs")))["p50"],
            c14_evidence::summary(&rec.values(&format!("{s}.retainedBytes")))["p50"]
        );
    }
}
