#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::path::Path;
use std::sync::Arc;

use common::*;
use serde_json::json;
use tt_executor::{ExecutorHandle, Status};
use tt_persist::lock::{Boundary, hooks};

/// A panic at the commit (outside the handler's boundary): the engine cannot vouch for its session, so it is degraded:
/// the call in flight answers `internal`, every later call answers `internal` with reason `engineStopped` and the hint
/// that names the way out, and nothing restarts inside the executor (recovery is a new executor, the host's restart).
/// Before ruling E-29 the same input ended the document thread without the reason or the hint.
#[test]
fn e1_commit_boundary_fallback_degrades() {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            hooks::set(None);
        }
    }
    let _reset = Reset;
    let dir = std::env::temp_dir().join(format!("tt-r2-degraded-{}", std::process::id()));
    let options = || tt_executor::ExecutorOptions {
        config: tt_executor::ExecutorConfig { budget: std::time::Duration::from_secs(60) },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_executor::UserDirs::under(&dir)),
            maintenance: tt_executor::Maintenance {
                tick: std::time::Duration::from_millis(100),
                autosave: Some(std::time::Duration::ZERO),
                ..Default::default()
            },
            ..Default::default()
        },
    };
    let h = ExecutorHandle::spawn_with_options(options()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(&h, "element.create", json!({"type": "peg", "name": "Before", "sceneId": "scene:#1"}));
    h.run_maintenance(true).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !files(&dir).keys().any(|x| x.ends_with("recovery.json")) {
        assert!(std::time::Instant::now() < deadline, "autosave not produced");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // A panic on the document thread inside an action (the persist boundary hook stands for any handler panic).
    hooks::set(Some(Arc::new(|at: Boundary, _: &Path| {
        if at == Boundary::Committed {
            panic!("test: a handler panics");
        }
    })));
    let events = h.subscribe();
    let died = run(&h, "element.create", json!({"type": "peg", "name": "Dies", "sceneId": "scene:#1"}));
    hooks::set(None);
    // The caller is answered as soon as its reply is dropped by the unwinding; the engine marks itself degraded and
    // raises the notice right after, on the document thread: wait for the notice (bounded), then check the flag.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut notices = 0;
    while notices == 0 && std::time::Instant::now() < deadline {
        if let Ok(e) = events.recv_timeout(std::time::Duration::from_millis(100))
            && e.name == "shell.engineDegraded"
        {
            notices += 1;
        }
    }
    assert_eq!(notices, 1, "the degraded notice");
    assert!(h.degraded(), "a panic outside an invocation degrades the engine");
    assert_eq!(events.try_iter().filter(|e| e.name == "shell.engineDegraded").count(), 0, "raised once");
    assert_eq!(died.status, Status::Error);
    assert_eq!(died.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{died:?}");
    for (command, params) in [("timeline.rows.list", json!({})), ("doc.state.get", json!({}))] {
        let e = run(&h, command, params);
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{command}: {e:?}");
        let error = e.error.as_ref().unwrap();
        assert!(error.message.contains("document thread has stopped"), "{e:?}");
        assert_eq!(error.details["reason"], tt_executor::ENGINE_STOPPED);
        assert!(error.hint.as_deref().unwrap_or_default().contains("project.recovery.list"), "{e:?}");
    }
    let before = files(&dir);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < until {
        let _ = h.run_maintenance(true);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(files(&dir), before, "E-29 no snapshot after degradation");
    for (command, params) in [
        ("element.create", json!({"type":"peg","name":"Refused"})),
        ("timeline.currentFrame.set", json!({"frame":9})),
        ("project.recovery.list", json!({})),
        ("edit.undo", json!({})),
        ("playback.start", json!({})),
    ] {
        let e = run(&h, command, params).error.unwrap();
        assert_eq!(e.code, "internal");
        assert_eq!(e.details["reason"], "engineStopped");
        assert!(e.hint.unwrap().contains("project.recovery.list"));
    }
    for entry in tt_executor::registry().unwrap().entries() {
        let out = run(&h, entry.id(), json!({}));
        let e = out.error.unwrap();
        assert_eq!(e.code, "internal", "{}", entry.id());
        assert_eq!(e.details["reason"], "engineStopped", "{}", entry.id());
        assert!(e.hint.unwrap().contains("project.recovery.list"));
    }
    let replay = h
        .timing_replay(
            tt_executor::Origin::User,
            json!({"surface":"tracks","zoomLevel":6,"firstFrame":1,"scrollTop":0,"platform":"windows"}),
            json!([{"x":160,"y":16,"modifiers":[]},{"x":192,"y":16,"modifiers":[]}]),
            false,
        )
        .unwrap_err();
    assert_eq!(replay.code, "internal");
    assert_eq!(replay.details["reason"], "engineStopped");
    assert!(replay.hint.unwrap().contains("project.recovery.list"));
    let fresh = ExecutorHandle::spawn_with_options(options()).unwrap();
    let owner = h.abandon();
    assert!(fresh.release_recovery_of(&owner).unwrap() > 0);
    let listed = ok(&fresh, "project.recovery.list", json!({}));
    assert!(!listed.as_array().unwrap().is_empty(), "{listed}");
    drop(fresh);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
fn files(root: &Path) -> std::collections::BTreeMap<String, u64> {
    fn visit(root: &Path, at: &Path, out: &mut std::collections::BTreeMap<String, u64>) {
        if let Ok(rd) = std::fs::read_dir(at) {
            for entry in rd {
                let e = entry.unwrap();
                let p = e.path();
                let m = e.metadata().unwrap();
                if m.is_dir() {
                    visit(root, &p, out);
                } else {
                    out.insert(p.strip_prefix(root).unwrap().to_string_lossy().into_owned(), m.len());
                }
            }
        }
    }
    let mut out = Default::default();
    visit(root, root, &mut out);
    out
}
