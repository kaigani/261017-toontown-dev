//! Obligation 5 measured with actual pipelined MCP calls and a background export, with raw samples.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review measurement")]
mod support;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;
use tt_executor::{ExecutorHandle, ExecutorOptions, Maintenance, PersistOptions, UserDirs};

#[test]
fn autosave_under_queued_protocol_calls_and_background_export() {
    let dir = tmp();
    let pkg = dir.join("Large.animpkg");
    let doc = tt_model::fixture::sample_document(901).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    for (name, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(name, tt_persist::container::BlobData::Bytes(Arc::new(vec![9; 60 * 1024])));
    }
    tt_persist::save::save(
        &pkg,
        tt_persist::container::Form::Package,
        &tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-30T00:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    let container =
        tt_persist::Container::open(&pkg, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
    let mut names = container.names();
    names.sort_by_key(|n| n != "mimetype");
    let mut zip = tt_persist::zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in names {
        zip.add_bytes(&name, &container.read(&name, 1 << 30).unwrap(), tt_persist::zip::Method::for_name(&name))
            .unwrap();
    }
    zip.add_bytes("extras/media.bin", &vec![5; 64 << 20], tt_persist::zip::Method::Stored).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    drop(container);
    std::fs::write(&pkg, bytes).unwrap();
    let h = ExecutorHandle::spawn_with_options(ExecutorOptions {
        persist: PersistOptions {
            user_dirs: Some(UserDirs::under(&dir.join("user"))),
            maintenance: Maintenance { tick: Duration::ZERO, autosave: Some(Duration::ZERO), ..Default::default() },
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let opened = c.run("project.open", json!({ "path": path(&pkg) }));
    let waited = c.call("job_wait", json!({ "jobId": opened["result"]["jobId"], "timeoutMs": 30000 }));
    assert_eq!(waited["structuredContent"]["result"]["state"], "succeeded", "{waited}");
    let scene = doc.project().scenes().ids()[0].as_str();
    let events = h.subscribe();
    let mut samples = Vec::new();
    for round in 0..10 {
        let exported =
            c.run("doc.package.export", json!({ "path": path(&dir.join(format!("export-{round}.animpkg"))) }));
        assert_eq!(exported["status"], "ok");
        let mut pending = Vec::new();
        for i in 0..50 {
            pending.push(c.send(
                "tools/call",
                json!({ "name": "set_properties", "arguments": {
                "targets": [scene], "props": { "meta": { "round": round, "queued": i } }
            } }),
            ));
        }
        // Ensure a committed dirty revision exists before asking for the snapshot, while more calls remain queued.
        assert_eq!(c.wait(pending[0], Duration::from_secs(30)).unwrap()["result"]["structuredContent"]["status"], "ok");
        let active = h.invoke(tt_executor::Invocation::user(
            "job.list",
            json!({ "jobId": exported["result"]["jobId"], "all": true }),
        ));
        let begin = Instant::now();
        let timing = h.run_maintenance_timed(true).unwrap();
        let elapsed = begin.elapsed();
        for rid in &pending[1..] {
            assert_eq!(c.wait(*rid, Duration::from_secs(30)).unwrap()["result"]["structuredContent"]["status"], "ok");
        }
        let final_export = c.call("job_wait", json!({ "jobId": exported["result"]["jobId"], "timeoutMs": 30000 }));
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            if events.try_iter().any(|e| e.name == "project.autosaved") {
                break;
            }
            assert!(Instant::now() < until, "autosave did not complete");
            std::thread::sleep(Duration::from_millis(5));
        }
        samples.push(json!({ "round": round, "queuedProtocolCalls": 50,
            "backgroundExportAtSnapshot": active.result,
            "maintenanceElapsedMs": elapsed.as_secs_f64() * 1000.0,
            "snapshotMs": timing.autosave_snapshot.map(|t| t.as_secs_f64() * 1000.0),
            "heartbeatMs": timing.heartbeat.map(|t| t.as_secs_f64() * 1000.0),
            "watchMs": timing.watch.map(|t| t.as_secs_f64() * 1000.0),
            "export": final_export["structuredContent"],
            "committedRevAfterQueue": h.snapshot().unwrap().unwrap().document_rev(),
        }));
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c14-r1");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("autosave-protocol-raw.json"), serde_json::to_vec_pretty(&samples).unwrap()).unwrap();
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    assert!(samples.iter().all(|s| s["snapshotMs"].as_f64().is_some_and(|v| v < 50.0)), "{samples:?}");
    assert!(
        samples
            .iter()
            .any(|s| matches!(s["backgroundExportAtSnapshot"][0]["state"].as_str(), Some("queued" | "running"))),
        "no background export overlapped the measurement"
    );
}
