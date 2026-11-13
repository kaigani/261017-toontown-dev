//! M4 C4.1: restarting an engine that stopped answering (reviews/M4-design-engine-restart.md; C1.4 gap 3).
//!
//! A first executor autosaves an unsaved project into the user folder and is then abandoned while alive (as a stuck
//! document thread would be). A second executor over the same folder, in the same process, releases the abandoned
//! recovery data and offers it; `project.recovery.open` brings the work back. The heartbeat side (a never-returning
//! handler answers `command.timeout` within the budget) is `c12_never_block`'s; the host's three-strikes state machine
//! is unit-tested in `tt-desktop`. Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::time::Duration;

use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Invocation, Status};

fn over(dir: &std::path::Path) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(60) },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_executor::UserDirs::under(dir)),
            maintenance: tt_executor::Maintenance {
                tick: Duration::ZERO,
                autosave: Some(Duration::ZERO),
                ..Default::default()
            },
            ..Default::default()
        },
    })
    .unwrap()
}

fn ok(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    let env = h.invoke(Invocation::user(id, params.clone()));
    assert_eq!(env.status, Status::Ok, "{id} {params}: {:?}", env.error);
    env.result.unwrap_or(Value::Null)
}

#[test]
fn a_restarted_executor_offers_the_abandoned_autosave() {
    let dir = std::env::temp_dir().join(format!("tt-c41-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let first = over(&dir);
    let created = ok(&first, "project.create", json!({ "name": "Lost work" }));
    let scene = created["sceneId"].as_str().unwrap().to_owned();
    ok(&first, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "unsaved": 42 } } }));
    first.run_maintenance(true).unwrap();
    // The recovery copy is written on its own thread (AUTO-02): wait for it, bounded (30 s, far above a local write).
    let started = std::time::Instant::now();
    let written = || {
        std::fs::read_dir(dir.join("Recovery"))
            .map(|rd| rd.filter_map(Result::ok).any(|e| e.path().join("recovery.json").exists()))
            .unwrap_or(false)
    };
    while !written() {
        assert!(started.elapsed() < Duration::from_secs(30), "no autosave was written");
        std::thread::yield_now();
    }
    // The first executor is abandoned alive: its recovery data belongs to a live process and is not offered yet.
    let second = over(&dir);
    assert!(ok(&second, "project.recovery.list", json!({})).as_array().unwrap().is_empty());
    // The restart releases it.
    assert_eq!(second.release_abandoned_recovery().unwrap(), 1);
    let list = ok(&second, "project.recovery.list", json!({}));
    let entry = &list.as_array().unwrap()[0];
    assert_eq!(entry["projectName"], "Lost work");
    let started = ok(&second, "project.recovery.open", json!({ "recoveryId": entry["recoveryId"] }));
    let done = ok(&second, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60_000 }));
    let rec = if done.get("record").is_some() { done["record"].clone() } else { done };
    assert_eq!(rec["state"], "succeeded", "{rec}");
    let info = ok(&second, "project.info.get", json!({}));
    assert_eq!(info["name"], "Lost work");
    assert_eq!(info["dirty"], true, "a recovered project is modified (01 §3.7)");
    let scene_obj = ok(&second, "doc.object.get", json!({ "ref": scene }));
    assert_eq!(scene_obj["object"]["meta"]["unsaved"].as_i64().or(scene_obj["meta"]["unsaved"].as_i64()), Some(42));
    drop(second);
    drop(first);
    let _ = std::fs::remove_dir_all(&dir);
}
