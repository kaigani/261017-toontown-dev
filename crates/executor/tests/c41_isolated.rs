//! M4 C4.1 builder tests that install a process-wide hook (`tt_persist::lock::hooks`), in their own binary so no other
//! test meets the hook (orchestrator ruling D-2). Builder-owned.
//!
//! Ruling O-18: an abandoned engine's recovery write and the release of its entries are one step each with respect to
//! the other. A writer paused after its fence check (the `RecoveryWriteChecked` boundary, under the ownership lock)
//! finishes before the release, which then leaves the entry released (offered), never overwritten by the late write;
//! a writer that comes after the release writes nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Invocation, Status};
use tt_persist::lock::{self, Boundary};

fn over(dir: &Path) -> ExecutorHandle {
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

fn entry(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir.join("Recovery"))
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.join("recovery.json").exists())
}

fn heartbeat_of(entry: &Path) -> String {
    let v: Value = serde_json::from_slice(&std::fs::read(entry.join("recovery.json")).unwrap()).unwrap();
    v["heartbeatAt"].as_str().unwrap_or_default().to_owned()
}

/// The tests of this binary install the process-wide hook one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

#[test]
fn a_paused_recovery_write_finishes_before_the_release_and_a_later_one_writes_nothing() {
    let _serial = serial();
    let dir = std::env::temp_dir().join(format!("tt-c41-o18-{}", tt_executor::random_token(6)));
    let first = over(&dir);
    let created = ok(&first, "project.create", json!({ "name": "Paused writer" }));
    let scene = created["sceneId"].as_str().unwrap().to_owned();
    ok(&first, "doc.object.patch", json!({ "targets": [scene.clone()], "props": { "meta": { "n": 1 } } }));
    first.run_maintenance(true).unwrap();
    let started = Instant::now();
    let folder = loop {
        if let Some(e) = entry(&dir) {
            break e;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "no first autosave");
        std::thread::yield_now();
    };
    // Pause the next recovery write of this entry after its fence check (it holds the ownership lock).
    let (reached_tx, reached_rx) = mpsc::channel::<()>();
    let (resume_tx, resume_rx) = mpsc::channel::<()>();
    let armed = Arc::new(Mutex::new(Some((reached_tx, resume_rx))));
    let target = folder.clone();
    lock::hooks::set(Some(Arc::new(move |b, p| {
        if b != Boundary::RecoveryWriteChecked || p != target {
            return;
        }
        let taken = armed.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some((reached, resume)) = taken {
            let _ = reached.send(());
            let _ = resume.recv_timeout(Duration::from_secs(60));
        }
    })));
    ok(&first, "doc.object.patch", json!({ "targets": [scene.clone()], "props": { "meta": { "n": 2 } } }));
    first.run_maintenance(true).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(30)).expect("the writer reaches its checked boundary");
    // The engine is abandoned while its write is inside the step; the release is asked meanwhile.
    let second = over(&dir);
    let owner = first.abandon();
    let releaser = {
        let second = second.clone();
        std::thread::spawn(move || second.release_recovery_of(&owner).unwrap())
    };
    // The release cannot interleave: it waits for the paused write (held well past any scheduling delay).
    let held = Instant::now();
    while held.elapsed() < Duration::from_millis(300) {
        assert!(!releaser.is_finished(), "the release ran inside the writer's step");
        std::thread::yield_now();
    }
    resume_tx.send(()).unwrap();
    assert_eq!(releaser.join().unwrap(), 1, "the entry is released after the write finished");
    lock::hooks::set(None);
    assert_eq!(heartbeat_of(&folder), "1970-01-01T00:00:00Z", "the late write did not overwrite the release");
    let released = std::fs::read(folder.join("recovery.json")).unwrap();
    // A write after the release sees the fence and writes nothing.
    ok(&first, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 3 } } }));
    first.run_maintenance(true).unwrap();
    let waited = Instant::now();
    while waited.elapsed() < Duration::from_millis(300) {
        std::thread::yield_now();
    }
    assert_eq!(std::fs::read(folder.join("recovery.json")).unwrap(), released);
    let list = ok(&second, "project.recovery.list", json!({}));
    assert_eq!(list.as_array().map(Vec::len), Some(1), "the released entry is offered: {list}");
    drop((first, second));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Closure round 5: the recovery heartbeat runs on the document thread and never waits for the ownership lock. While
/// another engine of the process holds it (its recovery write paused after its fence check), the heartbeat is skipped;
/// it is retried at the next maintenance pass, not at the next heartbeat interval (a minute later).
#[test]
fn a_skipped_heartbeat_is_retried_at_the_next_maintenance_pass() {
    let _serial = serial();
    let dir = std::env::temp_dir().join(format!("tt-c41-beat-{}", tt_executor::random_token(6)));
    let (a_dir, b_dir) = (dir.join("a"), dir.join("b"));
    let a = over(&a_dir);
    let created = ok(&a, "project.create", json!({ "name": "Beating" }));
    ok(&a, "doc.object.patch", json!({ "targets": [created["sceneId"]], "props": { "meta": { "n": 1 } } }));
    a.run_maintenance(true).unwrap();
    let started = Instant::now();
    let folder = loop {
        if let Some(e) = entry(&a_dir) {
            break e;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "no first autosave");
        std::thread::yield_now();
    };
    // The document thread takes the write's completion (its autosaved revision) with its next request.
    ok(&a, "app.state.get", json!({}));
    // Another engine's recovery write pauses inside the ownership lock.
    let (reached_tx, reached_rx) = mpsc::channel::<()>();
    let (resume_tx, resume_rx) = mpsc::channel::<()>();
    let armed = Arc::new(Mutex::new(Some((reached_tx, resume_rx))));
    let target = b_dir.clone();
    lock::hooks::set(Some(Arc::new(move |b, p| {
        if b != Boundary::RecoveryWriteChecked || !p.starts_with(&target) {
            return;
        }
        let taken = armed.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some((reached, resume)) = taken {
            let _ = reached.send(());
            let _ = resume.recv_timeout(Duration::from_secs(60));
        }
    })));
    let b = over(&b_dir);
    let other = ok(&b, "project.create", json!({ "name": "Writing" }));
    ok(&b, "doc.object.patch", json!({ "targets": [other["sceneId"]], "props": { "meta": { "n": 1 } } }));
    b.run_maintenance(true).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(30)).expect("the other engine's write holds the ownership lock");
    let before = heartbeat_of(&folder);
    std::thread::sleep(Duration::from_millis(1100));
    // A's heartbeat comes due (forced) while the lock is held: skipped, and A writes no copy (nothing changed).
    a.run_maintenance(true).unwrap();
    assert_eq!(heartbeat_of(&folder), before, "the heartbeat waited for the lock or wrote through it");
    resume_tx.send(()).unwrap();
    let waited = Instant::now();
    while entry(&b_dir).is_none() {
        assert!(waited.elapsed() < Duration::from_secs(30), "the other write did not finish");
        std::thread::yield_now();
    }
    lock::hooks::set(None);
    // The next passes, long before the next interval (a minute): the skipped heartbeat is written as soon as the lock is
    // free (the other write may still hold it for its last step when its file appears).
    let retried = Instant::now();
    let mut passes = 0;
    while heartbeat_of(&folder) == before {
        assert!(retried.elapsed() < Duration::from_secs(10), "the skipped heartbeat was not retried ({passes} passes)");
        a.run_maintenance(false).unwrap();
        passes += 1;
        std::thread::sleep(Duration::from_millis(50));
    }
    eprintln!("the skipped heartbeat was written at pass {passes} after the lock was released");
    drop(a);
    drop(b);
    let _ = std::fs::remove_dir_all(&dir);
}
