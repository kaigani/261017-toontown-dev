//! M2 C2.4 readiness item 8 (R1-12): raw baselines for the proposed budgets — measured, not promised; the binding
//! targets stay M11 C11.1. Edit, undo, redo and dry-run wall time and allocation are `c21_measurements.rs` (same CI
//! job); this test adds, on the same 100 000-stroke document:
//!
//! - retained bytes (counting allocator) and resident set size (the OS's own report) before and after the series of
//!   5 rounds of stroke add, undo, redo and dry run;
//! - queued-read latency: a cheap read sent 50 ms after a long command (a stroke add to the 100 000-stroke layer),
//!   from another thread, and when it is answered relative to the long command;
//! - maintenance/autosave delay: a forced maintenance pass (autosave snapshot) requested 50 ms into the same long
//!   command, and an idle pass for comparison.
//!
//! Ignored by default: `TT_MEASURE=1 cargo test --release -p tt-executor --test c24_budgets -- --ignored`, in the CI
//! headless job on both OSes. Output: `target/evidence/c24/budgets-<os>.json`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::alloc::System;
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};
use tt_executor::{
    ExecutorConfig, ExecutorHandle, ExecutorOptions, Invocation, InvokeOptions, Maintenance, Origin, PersistOptions,
    UserDirs,
};
use tt_model::document::Document;
use tt_model::entities::drawing::Drawing;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const STROKES: usize = 100_000;
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const REPETITIONS: usize = 5;

fn large() -> (Document, String) {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let at = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let template = d["nodes"][0]["strokes"][1].clone();
    let layer = d["nodes"][0]["id"].as_str().unwrap().to_owned();
    d["nodes"][0]["strokes"] = json!(
        (0..STROKES)
            .map(|n| {
                let mut s = template.clone();
                let (x, y) = (
                    f64::from(u32::try_from(n % 400).unwrap()) * 6.0,
                    f64::from(u32::try_from(n / 400).unwrap()) * 6.0,
                );
                s["id"] = json!(format!("b{n}"));
                s["points"] = json!([[x, y, 0, 0, 0, 0], [x + 4.0, y + 1.0, 0, 0, 0, 0]]);
                s
            })
            .collect::<Vec<_>>()
    );
    parts.drawings[at] = tt_model::object::from_json::<Drawing>(&d).unwrap();
    (Document::from_parts(parts).unwrap(), layer)
}

fn live() -> f64 {
    let s = GLOBAL.stats();
    s.bytes_allocated as f64 - s.bytes_deallocated as f64
}

/// The resident set size of this process in bytes, as the OS reports it (`tasklist` on Windows, `ps` elsewhere).
fn rss() -> Option<f64> {
    let pid = std::process::id().to_string();
    if cfg!(windows) {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let field = text.split("\",\"").last()?;
        let kb: String = field.chars().filter(char::is_ascii_digit).collect();
        kb.parse::<f64>().ok().map(|k| k * 1024.0)
    } else {
        let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().ok().map(|k| k * 1024.0)
    }
}

fn add(layer: &str, i: usize) -> Value {
    let x = f64::from(u32::try_from(i).unwrap());
    json!({ "drawingId": D1, "kind": "pencil", "layer": layer, "thickness": 1,
            "bezier": [{"x": x, "y": -5}, {"x": x, "y": -6}, {"x": x + 1.0, "y": -6}, {"x": x + 1.0, "y": -5}] })
}

#[test]
#[ignore = "measurement; run with TT_MEASURE=1 and --ignored in release"]
fn budget_baselines_raw_observations() {
    let dir = c13_support::tmp();
    let h = ExecutorHandle::spawn_with_options(ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(600) },
        persist: PersistOptions {
            user_dirs: Some(UserDirs::under(&dir.join("user"))),
            // Maintenance runs only when asked (the delays below are measured from the request).
            maintenance: Maintenance {
                tick: Duration::from_secs(3600),
                autosave: Some(Duration::from_secs(3600)),
                ..Maintenance::default()
            },
            migrations: tt_persist::migrate::CHAIN,
        },
    })
    .unwrap();
    let (doc, layer) = large();
    let rss_start = rss();
    let live_start = live();
    assert!(h.open_document(doc, None, false).is_ok());
    c13_support::job_ok(&h, "project.saveAs", json!({ "path": dir.join("Budgets.animpkg") }));
    let (rss_open, live_open) = (rss(), live());
    // The series: stroke add, undo, redo, dry run.
    let mut series = Vec::new();
    for i in 0..5 {
        let t = Instant::now();
        ok(&h, "draw.stroke.add", add(&layer, i));
        let a = t.elapsed().as_secs_f64();
        let t = Instant::now();
        ok(&h, "edit.undo", json!({}));
        let u = t.elapsed().as_secs_f64();
        let t = Instant::now();
        ok(&h, "edit.redo", json!({}));
        let r = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let env = h.invoke(Invocation {
            command: "draw.stroke.add".into(),
            params: add(&layer, 1000 + i),
            options: InvokeOptions { dry_run: true, ..Default::default() },
            origin: Origin::User,
        });
        assert!(env.error.is_none(), "{:?}", env.error);
        let dr = t.elapsed().as_secs_f64();
        series.push(json!({ "strokeAddSeconds": a, "undoSeconds": u, "redoSeconds": r, "dryRunSeconds": dr, "liveBytes": live(), "rssBytes": rss() }));
    }
    let (rss_series, live_series) = (rss(), live());
    // Queued reads and maintenance behind a long command.
    let mut queued = Vec::new();
    let mut maintenance = Vec::new();
    for i in 0..REPETITIONS {
        let worker = h.clone();
        let params = add(&layer, 2000 + i);
        let started = Instant::now();
        let long = std::thread::spawn(move || {
            ok(&worker, "draw.stroke.add", params);
            started.elapsed().as_secs_f64()
        });
        std::thread::sleep(Duration::from_millis(50));
        let asked = started.elapsed().as_secs_f64();
        let t = Instant::now();
        let read = run(&h, "edit.history.get", json!({}));
        let read_s = t.elapsed().as_secs_f64();
        let long_s = long.join().unwrap();
        queued.push(json!({ "longCommandSeconds": long_s, "readSentAtSeconds": asked, "readSeconds": read_s,
            "readAnsweredAfterLongSeconds": asked + read_s - long_s, "readOk": read.error.is_none() }));
        let worker = h.clone();
        let params = add(&layer, 3000 + i);
        let started = Instant::now();
        let long = std::thread::spawn(move || {
            ok(&worker, "draw.stroke.add", params);
            started.elapsed().as_secs_f64()
        });
        std::thread::sleep(Duration::from_millis(50));
        let asked = started.elapsed().as_secs_f64();
        let t = Instant::now();
        let timings = h.run_maintenance_timed(true).unwrap();
        let wait = t.elapsed().as_secs_f64();
        let long_s = long.join().unwrap();
        let t = Instant::now();
        let idle = h.run_maintenance_timed(true).unwrap();
        let idle_s = t.elapsed().as_secs_f64();
        maintenance.push(json!({
            "longCommandSeconds": long_s, "requestedAtSeconds": asked, "behindLongSeconds": wait,
            "autosaveSnapshotSeconds": timings.autosave_snapshot.map(|d| d.as_secs_f64()),
            "heartbeatSeconds": timings.heartbeat.map(|d| d.as_secs_f64()),
            "idleSeconds": idle_s, "idleAutosaveSnapshotSeconds": idle.autosave_snapshot.map(|d| d.as_secs_f64()),
        }));
    }
    let out = json!({
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "strokes": STROKES, "repetitions": REPETITIONS,
        "memory": {
            "rssBytes": { "start": rss_start, "afterOpen": rss_open, "afterSeries": rss_series },
            "liveBytes": { "start": live_start, "afterOpen": live_open, "afterSeries": live_series },
        },
        "series": series, "queuedRead": queued, "maintenance": maintenance,
    });
    if std::env::var("TT_MEASURE").as_deref() == Ok("1") {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c24");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join(format!("budgets-{}.json", std::env::consts::OS)),
            serde_json::to_string_pretty(&out).unwrap() + "\n",
        )
        .unwrap();
    }
    println!("{out}");
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
