//! C1.4 obligations 4 and 5: the folder fingerprint's cost at the 2 s watch interval and what callers wait for it,
//! the size+mtime blind spot (RO-02), and the autosave snapshot's latency during a long atomic batch, under queued
//! requests and with large preserved media. Results: `target/evidence/c14/{fingerprint,autosave}-<os>.json`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "statistics")]

mod c13_support;
mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use c13_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Maintenance, PersistOptions, UserDirs};

/// Files in the large-file-count fixture.
const FILES: usize = 10_000;

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn spawn_m(dirs: &Path, m: Maintenance) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(120) },
        persist: PersistOptions {
            user_dirs: Some(UserDirs::under(dirs)),
            maintenance: m,
            migrations: tt_persist::migrate::CHAIN,
        },
    })
    .unwrap()
}

fn stats(v: &mut [f64]) -> Value {
    v.sort_by(f64::total_cmp);
    let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    json!({ "n": v.len(), "p50": at(0.5), "p90": at(0.9), "p99": at(0.99), "max": v[v.len() - 1] })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn write_evidence(name: &str, v: &Value) {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c14");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join(format!("{name}-{}.json", std::env::consts::OS)), serde_json::to_vec_pretty(v).unwrap())
        .unwrap();
}

/// Calls a cheap query every 2 ms until `stop`, returning each call's latency in ms.
fn probe_callers(h: &ExecutorHandle, stop: &Arc<AtomicBool>) -> std::thread::JoinHandle<Vec<f64>> {
    let (h, stop) = (h.clone(), stop.clone());
    std::thread::spawn(move || {
        let mut v = Vec::new();
        while !stop.load(Ordering::Relaxed) {
            let t = Instant::now();
            let _ = run(&h, "doc.state.get", json!({}));
            v.push(ms(t.elapsed()));
            std::thread::sleep(Duration::from_millis(2));
        }
        v
    })
}

#[test]
fn folder_fingerprint_cost_at_the_watch_interval_and_the_size_mtime_blind_spot() {
    let dir = tmp();
    let folder = dir.join("Many");
    let h = spawn(&dir.join("setup"));
    ok(
        &h,
        "project.create",
        json!({ "name": "Many", "path": format!("{}{}", path(&folder), std::path::MAIN_SEPARATOR) }),
    );
    ok(&h, "project.close", json!({}));
    drop(h);
    let extras = folder.join("extras");
    std::fs::create_dir_all(&extras).unwrap();
    for i in 0..FILES {
        std::fs::write(extras.join(format!("f{i:05}.bin")), format!("{i:016}")).unwrap();
    }
    // Direct cost: every maintenance pass runs the external-change check (watch interval zero).
    let h = spawn_m(
        &dir.join("direct"),
        Maintenance { tick: Duration::ZERO, watch: Duration::ZERO, ..Maintenance::default() },
    );
    job_ok(&h, "project.open", json!({ "path": path(&folder) }));
    let mut watch: Vec<f64> = (0..20).map(|_| ms(h.run_maintenance_timed(false).unwrap().watch.unwrap())).collect();
    // Callers waiting while passes run back to back (worst case).
    let stop = Arc::new(AtomicBool::new(false));
    let callers = probe_callers(&h, &stop);
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(3) {
        h.run_maintenance_timed(false).unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    let mut back_to_back = callers.join().unwrap();
    // The size+mtime blind spot: same size, modification time restored.
    let events = h.subscribe();
    let victim = extras.join("f00042.bin");
    let mtime = std::fs::metadata(&victim).unwrap().modified().unwrap();
    std::fs::write(&victim, "XXXXXXXXXXXXXXXX").unwrap();
    std::fs::File::options().write(true).open(&victim).unwrap().set_modified(mtime).unwrap();
    h.run_maintenance_timed(false).unwrap();
    let blind = !events.try_iter().any(|e| e.name == "doc.package.changedOnDisk");
    // A size-changing edit is detected (control).
    std::fs::write(&victim, "changed size").unwrap();
    h.run_maintenance_timed(false).unwrap();
    let control = events.try_iter().any(|e| e.name == "doc.package.changedOnDisk");
    drop(h);
    // The real schedule: 1 s ticks, the 2 s watch interval, callers measured for 10 s.
    let h = spawn_m(
        &dir.join("scheduled"),
        Maintenance { tick: Duration::from_secs(1), watch: Duration::from_secs(2), ..Maintenance::default() },
    );
    job_ok(&h, "project.open", json!({ "path": path(&folder), "readOnly": true }));
    let stop = Arc::new(AtomicBool::new(false));
    let callers = probe_callers(&h, &stop);
    std::thread::sleep(Duration::from_secs(10));
    stop.store(true, Ordering::Relaxed);
    let mut scheduled = callers.join().unwrap();
    drop(h);
    let report = json!({
        "obligation": "C1.4 obligation 4 (RO-02 folder fingerprint)",
        "platform": std::env::consts::OS,
        "fixture": format!("folder-form project + {FILES} files of 16 bytes under extras/"),
        "watchPassMs": stats(&mut watch),
        "callerLatencyMs": {
            "backToBackPasses": stats(&mut back_to_back),
            "scheduled2sWatch10s": stats(&mut scheduled),
        },
        "sizeMtimePreservingEditDetected": !blind,
        "sizeChangingEditDetected": control,
        "note": "a same-size edit that restores the modification time is not detected (walk compares name, size, mtime): \
                 recorded as SI-046 for the spec owner",
    });
    write_evidence("fingerprint", &report);
    assert!(control, "{report}");
    assert!(blind, "the blind spot is documented as SI-046; if it is closed, update the handoff: {report}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn autosave_snapshot_latency_during_batches_queued_requests_and_large_media() {
    let dir = tmp();
    let pkg = dir.join("Autosave.animpkg");
    let doc = tt_model::fixture::sample_document(901).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    for (n, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(n, tt_persist::container::BlobData::Bytes(Arc::new(vec![9; 60 * 1024])));
    }
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &Default::default(),
        blobs: &blobs,
        session: None,
        saved_at: "2026-10-29T12:00:00.000Z".into(),
    };
    tt_persist::save::save(&pkg, tt_persist::container::Form::Package, &input, None, false).unwrap();
    let objects = doc.entry_keys().len();
    // Large preserved media: a 64 MiB entry outside the reserved names (ZIP-04: copied unchanged).
    {
        let c = tt_persist::Container::open(&pkg, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
        let mut names = c.names();
        names.sort_by_key(|n| n != "mimetype");
        let mut w = tt_persist::zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for n in &names {
            w.add_bytes(n, &c.read(n, 1 << 30).unwrap(), tt_persist::zip::Method::for_name(n)).unwrap();
        }
        w.add_bytes("extras/media.bin", &vec![5u8; 64 << 20], tt_persist::zip::Method::Stored).unwrap();
        let bytes = w.finish().unwrap().into_inner();
        drop(c);
        std::fs::write(&pkg, bytes).unwrap();
    }
    let h = spawn_m(
        &dir.join("user"),
        Maintenance { tick: Duration::ZERO, autosave: Some(Duration::ZERO), ..Maintenance::default() },
    );
    job_ok(&h, "project.open", json!({ "path": path(&pkg) }));
    let scene = first(&h, "scene");
    let events = h.subscribe();
    let wait_autosaved = || {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if events.try_iter().any(|e| e.name == "project.autosaved") {
                return;
            }
            assert!(Instant::now() < deadline, "no autosave");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    // Typical snapshot (AUTO-02 budget: <= 50 ms typical).
    let mut snapshot = Vec::new();
    for i in 0..15 {
        ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "i": i } } }));
        let t = h.run_maintenance_timed(true).unwrap();
        snapshot.push(ms(t.autosave_snapshot.unwrap()));
        wait_autosaved();
    }
    // During a long atomic batch: the snapshot waits for the batch (one thread); the copy holds the committed revision.
    let steps: Vec<Value> = (0..1000)
        .map(|i| json!({ "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "b": i } } } }))
        .collect();
    let hb = h.clone();
    let batch_started = Instant::now();
    let long = std::thread::spawn(move || {
        let env = batch(&hb, json!({ "label": "Long", "atomic": true, "steps": steps }));
        (env, batch_started.elapsed())
    });
    std::thread::sleep(Duration::from_millis(20));
    let asked = Instant::now();
    let during = h.run_maintenance_timed(true).unwrap();
    let wait_behind_batch = asked.elapsed();
    let (env, batch_took) = long.join().unwrap();
    assert!(env.is_ok(), "{env:?}");
    wait_autosaved();
    let committed = env.document_rev.unwrap();
    let recovery = dir.join("user").join("Recovery");
    let mut revs = Vec::new();
    for e in std::fs::read_dir(&recovery).unwrap().flatten() {
        for f in std::fs::read_dir(e.path()).into_iter().flatten().flatten() {
            if f.file_name().to_string_lossy().ends_with(".json")
                && let Ok(v) = serde_json::from_slice::<Value>(&std::fs::read(f.path()).unwrap())
                && let Some(r) = v.get("documentRev").and_then(Value::as_u64)
            {
                revs.push(r);
            }
        }
    }
    // Under queued requests: 8 threads keep the queue full while maintenance asks for its turn.
    let stop = Arc::new(AtomicBool::new(false));
    let load: Vec<_> = (0..8)
        .map(|k| {
            let (h, stop, scene) = (h.clone(), stop.clone(), scene.clone());
            std::thread::spawn(move || {
                let mut n = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    let _ = run(
                        &h,
                        "doc.object.patch",
                        json!({ "targets": [scene], "props": { "meta": { "q": k, "n": n } } }),
                    );
                    n += 1;
                }
            })
        })
        .collect();
    let mut queued_snapshot = Vec::new();
    let mut queued_delay = Vec::new();
    for _ in 0..10 {
        let asked = Instant::now();
        let t = h.run_maintenance_timed(true).unwrap();
        let total = asked.elapsed();
        if let Some(s) = t.autosave_snapshot {
            queued_snapshot.push(ms(s));
        }
        let held =
            t.heartbeat.unwrap_or_default() + t.watch.unwrap_or_default() + t.autosave_snapshot.unwrap_or_default();
        queued_delay.push(ms(total.saturating_sub(held)));
        wait_autosaved();
    }
    stop.store(true, Ordering::Relaxed);
    for l in load {
        l.join().unwrap();
    }
    let report = json!({
        "obligation": "C1.4 obligation 5 (AUTO-02 autosave latency)",
        "platform": std::env::consts::OS,
        "fixture": format!("sample_document(901) ({objects} entries, embedded media 60 KiB each) + a 64 MiB preserved entry (extras/media.bin)"),
        "snapshotMs": stats(&mut snapshot),
        "duringLongBatch": {
            "batchSteps": 1000, "batchMs": ms(batch_took), "maintenanceWaitedMs": ms(wait_behind_batch),
            "snapshotMs": during.autosave_snapshot.map(ms), "committedRev": committed, "recoveryRevs": revs,
        },
        "underQueuedRequests": { "threads": 8, "snapshotMs": stats(&mut queued_snapshot), "schedulingDelayMs": stats(&mut queued_delay) },
        "maximumSchedulingDelay": "one maintenance tick (1 s) plus the document-thread work queued ahead of it; \
                                   an atomic batch is one request, so the autosave waits for the whole batch",
    });
    write_evidence("autosave", &report);
    assert!(revs.iter().all(|r| *r <= committed), "recovery holds only committed revisions: {report}");
    assert!(revs.contains(&committed), "{report}");
    let typical = report["snapshotMs"]["p50"].as_f64().unwrap();
    assert!(typical <= 50.0, "AUTO-02: typical snapshot {typical} ms: {report}");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}
