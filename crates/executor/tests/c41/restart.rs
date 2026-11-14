//! M4 C4.1: restarting an engine that stopped answering (reviews/M4-design-engine-restart.md; C1.4 gap 3).
//!
//! A first executor autosaves an unsaved project into the user folder and is then abandoned while alive (as a stuck
//! document thread would be). A second executor over the same folder, in the same process, releases the abandoned
//! recovery data and offers it; `project.recovery.open` brings the work back. The heartbeat side (a never-returning
//! handler answers `command.timeout` within the budget) is `c12_never_block`'s; the host's three-strikes state machine
//! is unit-tested in `tt-desktop`. Builder-owned.

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
    // Nothing is released before the first engine is named as abandoned (C41-R1-15).
    assert_eq!(second.release_abandoned_recovery().unwrap(), 0);
    let owner = first.abandon();
    // The fenced engine writes nothing more, even when its maintenance runs (it "wakes up").
    let info = |d: &std::path::Path| {
        std::fs::read_dir(d.join("Recovery"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| std::fs::read(e.path().join("recovery.json")).unwrap_or_default())
            .collect::<Vec<_>>()
    };
    let before_wake = info(&dir);
    ok(&first, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "unsaved": 43 } } }));
    first.run_maintenance(true).unwrap();
    assert_eq!(info(&dir), before_wake, "an abandoned engine writes no autosave or heartbeat");
    // The restart releases exactly the named engine's data.
    assert_eq!(second.release_recovery_of(&owner).unwrap(), 1);
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

/// Ruling O-18: at start, an owner record a dead process left without a completed recovery is removed (the lock
/// liveness rule); a completed recovery of a dead process is offered, never removed; a live process's record is kept.
#[test]
fn owner_records_without_a_completed_recovery_are_scavenged_at_start() {
    let dir = std::env::temp_dir().join(format!("tt-c41-scavenge-{}", tt_executor::random_token(6)));
    let recovery = dir.join("Recovery");
    let mut exited = if cfg!(windows) {
        std::process::Command::new("cmd").args(["/C", "exit"]).spawn().unwrap()
    } else {
        std::process::Command::new("true").spawn().unwrap()
    };
    let dead = exited.id();
    exited.wait().unwrap();
    let host = tt_persist::host::host();
    let owner = |id: &str, pid: u32| {
        std::fs::create_dir_all(recovery.join(id).join("project")).unwrap();
        std::fs::write(recovery.join(id).join("engine"), format!("en_test\n{pid}\n{host}")).unwrap();
    };
    owner("rc_dead0001", dead);
    owner("rc_dead0002", dead);
    let info = json!({ "recoveryId": "rc_dead0002", "projectId": "pr_x", "projectName": "Completed", "originalPath": null,
        "savedAt": "2026-01-01T00:00:00Z", "documentRev": 1, "appVersion": "0", "processId": dead, "host": host,
        "heartbeatAt": "2026-01-01T00:00:00Z" });
    std::fs::write(recovery.join("rc_dead0002").join("recovery.json"), info.to_string()).unwrap();
    owner("rc_live0003", std::process::id());
    let h = over(&dir);
    assert!(!recovery.join("rc_dead0001").exists(), "the dead owner's incomplete record is removed");
    assert!(recovery.join("rc_dead0002").join("recovery.json").exists(), "a completed recovery is never removed");
    assert!(recovery.join("rc_live0003").exists(), "a live process's record is kept");
    let list = ok(&h, "project.recovery.list", json!({}));
    assert!(list.as_array().unwrap().iter().any(|e| e["recoveryId"] == "rc_dead0002"), "{list}");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Closure round 5: a recovery copy whose owner is a live process of this host is not offered as orphaned, however old
/// its heartbeat (heartbeats can be skipped while another engine writes); a copy a restart released is.
#[test]
fn a_live_owner_is_never_orphaned_and_a_released_copy_is() {
    let dir = std::env::temp_dir().join(format!("tt-c41-live-{}", tt_executor::random_token(6)));
    let recovery = dir.join("Recovery");
    let host = tt_persist::host::host();
    let write = |id: &str, beat: &str| {
        std::fs::create_dir_all(recovery.join(id).join("project")).unwrap();
        let info = json!({ "recoveryId": id, "projectId": "pr_x", "projectName": id, "originalPath": null,
            "savedAt": "2026-01-01T00:00:00Z", "documentRev": 1, "appVersion": "0", "processId": std::process::id(),
            "host": host, "heartbeatAt": beat });
        std::fs::write(recovery.join(id).join("recovery.json"), info.to_string()).unwrap();
    };
    write("rc_a1170001", "2026-01-01T00:00:00Z");
    write("rc_a1170002", "1970-01-01T00:00:00Z");
    let h = over(&dir);
    let list = ok(&h, "project.recovery.list", json!({}));
    let ids: Vec<&str> = list.as_array().unwrap().iter().filter_map(|e| e["recoveryId"].as_str()).collect();
    assert!(!ids.contains(&"rc_a1170001"), "a live owner's copy with an old heartbeat was offered: {list}");
    assert!(ids.contains(&"rc_a1170002"), "a released copy was not offered: {list}");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}
