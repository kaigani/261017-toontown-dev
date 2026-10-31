//! Measure autosave queueing and snapshot cost during a real 100k-stroke edit.
#![allow(clippy::unwrap_used, missing_docs, reason = "review measurements")]
mod c13_support;
mod c14_evidence;
mod common;
use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Maintenance, PersistOptions, UserDirs};
use tt_model::{document::Document, entities::drawing::Drawing};

#[test]
fn autosave_takes_a_committed_snapshot_after_large_geometry_work() {
    let dir = tmp();
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let at = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let template = d["nodes"][0]["strokes"][1].clone();
    d["nodes"][0]["strokes"] = Value::Array(
        (0..100_000)
            .map(|i| {
                let mut s = template.clone();
                s["id"] = json!(format!("autosave{i}"));
                s
            })
            .collect(),
    );
    parts.drawings[at] = tt_model::object::from_json::<Drawing>(&d).unwrap();
    let drawing = parts.drawings[at].id().as_str().to_owned();
    let h = ExecutorHandle::spawn_with_options(ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(120) },
        persist: PersistOptions {
            user_dirs: Some(UserDirs::under(&dir.join("user"))),
            maintenance: Maintenance { tick: Duration::ZERO, autosave: Some(Duration::ZERO), ..Maintenance::default() },
            migrations: tt_persist::migrate::CHAIN,
        },
    })
    .unwrap();
    assert!(h.open_document(Document::from_parts(parts).unwrap(), None, false).is_ok());
    job_ok(&h, "project.saveAs", json!({"path":dir.join("Large.animpkg")}));
    let events = h.subscribe();
    let edit_h = h.clone();
    let mut rec = c14_evidence::Recorder::new();
    let began = Instant::now();
    let worker = std::thread::spawn(move || {
        let e = ok(
            &edit_h,
            "draw.stroke.add",
            json!({"drawingId":drawing,"kind":"pencil","points":[{"x":0,"y":0},{"x":50,"y":0}]}),
        );
        (e, began.elapsed())
    });
    std::thread::sleep(Duration::from_millis(50));
    let asked = Instant::now();
    let timings = h.run_maintenance_timed(true).unwrap();
    let wait = asked.elapsed();
    let (_, edit) = worker.join().unwrap();
    let snap = timings.autosave_snapshot.unwrap();
    rec.sample("waitBehindGeometryMs", "ms", asked, wait.as_secs_f64() * 1000.0, None);
    rec.sample("geometryEditMs", "ms", began, edit.as_secs_f64() * 1000.0, None);
    rec.sample("snapshotMs", "ms", asked, snap.as_secs_f64() * 1000.0, None);
    let until = Instant::now() + Duration::from_secs(120);
    loop {
        if events.try_iter().any(|e| e.name == "project.autosaved") {
            break;
        }
        assert!(Instant::now() < until, "autosave did not complete");
        std::thread::sleep(Duration::from_millis(10));
    }
    let report = rec.report(
        "C2.1 obligation 11",
        "100000 existing pencil strokes; real stroke add then forced autosave",
        json!({"strokes":100000,"repetitions":1,"documentRev":rev(&h)}),
    );
    c14_evidence::write("c21-r1-autosave", &report);
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
