//! C1.4 obligations 4 and 5: the folder fingerprint's cost at the 2 s watch interval and what callers wait for it,
//! the size+mtime blind spot of the watch and its detection at save (RO-02, C14-R1-09), the content check's cost at
//! save, and the autosave snapshot's latency during a long atomic batch, under queued requests and with large preserved
//! media. Every observation is kept (C14-R1-07): `target/evidence/c14/{fingerprint,autosave}-<os>.json`, checked by
//! `scripts/analyse-measurements.py`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "statistics")]

mod c13_support;
mod c14_evidence;
mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use c13_support::*;
use c14_evidence::{Recorder, ms};
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

/// Calls a cheap query every 2 ms until `stop`, returning `(tMs, latencyMs)` on the recorder's origin.
fn probe_callers(
    h: &ExecutorHandle,
    stop: &Arc<AtomicBool>,
    origin: Instant,
) -> std::thread::JoinHandle<Vec<(f64, f64)>> {
    let (h, stop) = (h.clone(), stop.clone());
    std::thread::spawn(move || {
        let mut v = Vec::new();
        while !stop.load(Ordering::Relaxed) {
            let t = Instant::now();
            let _ = run(&h, "doc.state.get", json!({}));
            v.push((ms(t.saturating_duration_since(origin)), ms(t.elapsed())));
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
    let mut rec = Recorder::new();
    // Direct cost: every maintenance pass runs the external-change check (watch interval zero).
    let h = spawn_m(
        &dir.join("direct"),
        Maintenance { tick: Duration::ZERO, watch: Duration::ZERO, ..Maintenance::default() },
    );
    let opened = Instant::now();
    job_ok(&h, "project.open", json!({ "path": path(&folder) }));
    rec.sample("openMs", "ms", opened, ms(opened.elapsed()), Some("includes the content hash recorded at open"));
    for _ in 0..20 {
        let at = Instant::now();
        let w = h.run_maintenance_timed(false).unwrap().watch.unwrap();
        rec.sample("watchPassMs", "ms", at, ms(w), None);
    }
    // The content check of a save (C14-R1-09): a clean save verifies the content before it would replace anything.
    for _ in 0..5 {
        let at = Instant::now();
        let started = ok(&h, "project.save", json!({}));
        rec.sample("saveContentCheckMs", "ms", at, ms(at.elapsed()), Some("project.save handler incl. content check"));
        wait(&h, &started);
    }
    // Callers waiting while passes run back to back (worst case).
    let stop = Arc::new(AtomicBool::new(false));
    let callers = probe_callers(&h, &stop, rec.origin());
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(3) {
        h.run_maintenance_timed(false).unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    rec.extend("callerLatencyBackToBackMs", "ms", callers.join().unwrap());
    // The size+mtime blind spot of the watch: same size, modification time restored.
    let events = h.subscribe();
    let victim = extras.join("f00042.bin");
    let mtime = std::fs::metadata(&victim).unwrap().modified().unwrap();
    std::fs::write(&victim, "XXXXXXXXXXXXXXXX").unwrap();
    std::fs::File::options().write(true).open(&victim).unwrap().set_modified(mtime).unwrap();
    h.run_maintenance_timed(false).unwrap();
    let watch_blind = !events.try_iter().any(|e| e.name == "doc.package.changedOnDisk");
    // …which the save sees (the content recorded at open).
    let at_save = err(&h, "project.save", json!({})).code;
    // A size-changing edit is detected by the watch (control).
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
    let callers = probe_callers(&h, &stop, rec.origin());
    std::thread::sleep(Duration::from_secs(10));
    stop.store(true, Ordering::Relaxed);
    rec.extend("callerLatencyScheduled2sWatchMs", "ms", callers.join().unwrap());
    drop(h);
    let report = rec.report(
        "C1.4 obligation 4 (RO-02 folder fingerprint) and C14-R1-09 (content check at save)",
        &format!("folder-form project + {FILES} files of 16 bytes under extras/"),
        json!({
            "sizeMtimePreservingEditSeenByWatch": !watch_blind,
            "sizeMtimePreservingEditAtSave": at_save,
            "sizeChangingEditSeenByWatch": control,
            "note": "the 2 s watch compares names, sizes and modification times; every save verifies content hashes \
                     recorded at open or the last save (SI-047 interim ruling)",
        }),
    );
    c14_evidence::write("fingerprint", &report);
    assert!(control, "{report}");
    assert_eq!(at_save, "project.changedOnDisk", "{report}");
    assert!(watch_blind, "the watch tick stays size+mtime by ruling; if that changes, update SI-047: {report}");
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
    // Waits for the copy a snapshot started (only when one was taken).
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
    let mut rec = Recorder::new();
    // Typical snapshot (AUTO-02 budget: <= 50 ms typical).
    for i in 0..15 {
        ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "i": i } } }));
        let at = Instant::now();
        let t = h.run_maintenance_timed(true).unwrap();
        let snap = t.autosave_snapshot.expect("a dirty revision is snapshotted");
        rec.sample("snapshotMs", "ms", at, ms(snap), None);
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
    rec.sample("waitBehindLongBatchMs", "ms", asked, ms(asked.elapsed()), Some("1000-step atomic batch ahead"));
    let (env, batch_took) = long.join().unwrap();
    assert!(env.is_ok(), "{env:?}");
    rec.sample("longBatchMs", "ms", batch_started, ms(batch_took), None);
    if let Some(s) = during.autosave_snapshot {
        rec.sample("snapshotAfterLongBatchMs", "ms", asked, ms(s), None);
        wait_autosaved();
    }
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
    for _ in 0..10 {
        let asked = Instant::now();
        let t = h.run_maintenance_timed(true).unwrap();
        let total = asked.elapsed();
        let held =
            t.heartbeat.unwrap_or_default() + t.watch.unwrap_or_default() + t.autosave_snapshot.unwrap_or_default();
        rec.sample("queuedSchedulingDelayMs", "ms", asked, ms(total.saturating_sub(held)), None);
        // A snapshot is skipped while the previous copy is still being written (no new copy to wait for).
        if let Some(s) = t.autosave_snapshot {
            rec.sample("queuedSnapshotMs", "ms", asked, ms(s), None);
            wait_autosaved();
        }
    }
    stop.store(true, Ordering::Relaxed);
    for l in load {
        l.join().unwrap();
    }
    let report = rec.report(
        "C1.4 obligation 5 (AUTO-02 autosave latency)",
        &format!(
            "sample_document(901) ({objects} entries, embedded media 60 KiB each) + a 64 MiB preserved entry \
             (extras/media.bin)"
        ),
        json!({
            "longBatchSteps": 1000, "committedRev": committed, "recoveryRevs": revs, "queueThreads": 8,
            "maximumSchedulingDelay": "one maintenance tick (1 s) plus the document-thread work queued ahead of it; \
                                       an atomic batch is one request, so the autosave waits for the whole batch \
                                       (each request is bounded by the executor's never-block budget)",
        }),
    );
    c14_evidence::write("autosave", &report);
    assert!(revs.iter().all(|r| *r <= committed), "recovery holds only committed revisions: {report}");
    assert!(revs.contains(&committed), "{report}");
    let typical = report["series"]["snapshotMs"]["summary"]["p50"].as_f64().unwrap();
    assert!(typical <= 50.0, "AUTO-02: typical snapshot {typical} ms: {report}");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}
