//! C1.4 obligations 1 and 2 (ADR-0014 §11.4): cross-session write exclusivity under deterministic suspension at every
//! ownership boundary of the lock protocol, and liveness that does not depend on process ids or clocks.
//!
//! Each test suspends one participant (publisher, heartbeat, release, takeover) at a [`Boundary`] through the
//! `test-hooks` feature, races other openers against it, resumes it, and checks: at most one `Acquired` at any time,
//! a resumed participant never reports ownership it lost, and a live holder is never taken over. Measurements go to
//! `target/evidence/c14/lock-liveness.json`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Barrier, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use c14_evidence::{Recorder, ms};
use serde_json::{Value, json};
use tt_persist::lock::{self, Boundary, LockInfo, Locked};

mod c14_evidence;

/// The hook is process-global: tests that install one run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn project() -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tt-c14-lock-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("Race.animpkg");
    std::fs::write(&p, b"x").unwrap();
    (dir, p)
}

/// Suspends the first participant that reaches `at` for `lock_file`: returns (reached, resume).
fn suspend(at: Boundary, lock_file: &Path) -> (Receiver<()>, Sender<()>) {
    let (reached_tx, reached_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel::<()>();
    let target = lock_file.to_owned();
    let armed = Arc::new(Mutex::new(Some((reached_tx, resume_rx))));
    lock::hooks::set(Some(Arc::new(move |b, p| {
        if b != at || p != target {
            return;
        }
        let taken = armed.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some((reached, resume)) = taken {
            let _ = reached.send(());
            let _ = resume.recv_timeout(Duration::from_secs(60));
        }
    })));
    (reached_rx, resume_tx)
}

fn race(project: &Path, n: usize) -> Vec<Locked> {
    let barrier = Arc::new(Barrier::new(n));
    let threads: Vec<_> = (0..n)
        .map(|_| {
            let (p, b) = (project.to_owned(), barrier.clone());
            std::thread::spawn(move || {
                b.wait();
                lock::acquire(&p)
            })
        })
        .collect();
    threads.into_iter().map(|t| t.join().unwrap()).collect()
}

fn acquired(v: &[Locked]) -> usize {
    v.iter().filter(|l| matches!(l, Locked::Acquired { .. })).count()
}

fn stale_unprotected(path: &Path, id: &str) {
    let old = LockInfo {
        user: "c14".into(),
        host: tt_persist::host::host(),
        process_id: std::process::id(),
        app_version: "0.0.1".into(),
        opened_at: "2000-01-01T00:00:00.000Z".into(),
        heartbeat_at: "2000-01-01T00:00:00.000Z".into(),
        lock_id: id.into(),
    };
    std::fs::write(path, serde_json::to_vec(&old).unwrap()).unwrap();
}

#[test]
fn a_publisher_suspended_at_each_publication_boundary_is_never_raced_into_two_owners() {
    let _g = serial();
    for at in [Boundary::ArbiterTaken, Boundary::PublishCreated, Boundary::PublishWritten] {
        let (dir, project) = project();
        let file = lock::lock_path(&project);
        let (reached, resume) = suspend(at, &file);
        let p = project.clone();
        let publisher = std::thread::spawn(move || lock::acquire(&p));
        reached.recv_timeout(Duration::from_secs(30)).unwrap();
        let racers = race(&project, 8);
        assert_eq!(acquired(&racers), 0, "{at:?}: a racer took the lock from a suspended publisher");
        resume.send(()).unwrap();
        let outcome = publisher.join().unwrap();
        lock::hooks::set(None);
        let Locked::Acquired { handle, took_over: None } = outcome else { panic!("{at:?}: the publisher lost") };
        assert!(handle.held());
        drop(racers);
        drop(handle);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_suspended_heartbeat_is_not_taken_over_and_resumes_as_the_owner() {
    let _g = serial();
    for at in [Boundary::HeartbeatChecked, Boundary::HeartbeatWritten] {
        let (dir, project) = project();
        let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("acquire") };
        let handle = Arc::new(handle);
        let (reached, resume) = suspend(at, handle.path());
        let h = handle.clone();
        let beat = std::thread::spawn(move || h.refresh());
        reached.recv_timeout(Duration::from_secs(30)).unwrap();
        let racers = race(&project, 8);
        assert_eq!(acquired(&racers), 0, "{at:?}");
        resume.send(()).unwrap();
        beat.join().unwrap().unwrap();
        lock::hooks::set(None);
        assert!(handle.held());
        drop(racers);
        drop(handle);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_suspended_release_hands_over_cleanly() {
    let _g = serial();
    let (dir, project) = project();
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("acquire") };
    let (reached, resume) = suspend(Boundary::ReleaseChecked, handle.path());
    let release = std::thread::spawn(move || drop(handle));
    reached.recv_timeout(Duration::from_secs(30)).unwrap();
    let racers = race(&project, 4);
    assert_eq!(acquired(&racers), 0, "the lock is still protected while its release is suspended");
    resume.send(()).unwrap();
    release.join().unwrap();
    lock::hooks::set(None);
    let after = race(&project, 8);
    assert_eq!(acquired(&after), 1, "exactly one opener gets the released lock");
    drop(after);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_suspended_takeover_leaves_exactly_one_owner() {
    let _g = serial();
    let (dir, project) = project();
    let file = lock::lock_path(&project);
    stale_unprotected(&file, "stale");
    let (reached, resume) = suspend(Boundary::TakeoverDecided, &file);
    let p = project.clone();
    let taker = std::thread::spawn(move || lock::acquire(&p));
    reached.recv_timeout(Duration::from_secs(30)).unwrap();
    let racers = race(&project, 8);
    assert_eq!(acquired(&racers), 0, "nobody else takes over while the arbiter's holder is suspended");
    resume.send(()).unwrap();
    let outcome = taker.join().unwrap();
    lock::hooks::set(None);
    let Locked::Acquired { handle, took_over: Some(old) } = outcome else { panic!("the taker lost") };
    assert_eq!(old.lock_id, "stale");
    assert!(handle.held());
    drop(racers);
    drop(handle);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_paused_arbiter_past_its_abandonment_age_is_not_removed_and_resumes() {
    let _g = serial();
    let (dir, project) = project();
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("acquire") };
    let handle = Arc::new(handle);
    let (reached, resume) = suspend(Boundary::HeartbeatChecked, handle.path());
    let h = handle.clone();
    let beat = std::thread::spawn(move || h.refresh());
    reached.recv_timeout(Duration::from_secs(30)).unwrap();
    // Paused longer than the abandonment age of an arbitration file.
    std::thread::sleep(lock::TAKEOVER_STALE + Duration::from_secs(1));
    let mut takeover = handle.path().as_os_str().to_owned();
    takeover.push(".takeover");
    let takeover = PathBuf::from(takeover);
    assert_eq!(lock::probe(&takeover), Some(true));
    let racers = race(&project, 8);
    assert_eq!(acquired(&racers), 0);
    assert!(takeover.exists(), "a protected arbiter is never removed as abandoned");
    resume.send(()).unwrap();
    beat.join().unwrap().unwrap();
    lock::hooks::set(None);
    assert!(handle.held());
    assert!(!takeover.exists());
    drop(racers);
    drop(handle);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_resumed_heartbeat_whose_lock_was_replaced_fails_closed() {
    // Where the file system does not carry the holder's protection (another host on a share without locks), a lock
    // can be replaced under a paused holder. Simulated by replacing the file while the heartbeat waits for the
    // arbiter: the heartbeat never writes, reports the loss, and gives up its handle.
    let _g = serial();
    let (dir, project) = project();
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("acquire") };
    let mut forged = handle.info();
    forged.lock_id = "someone-else".into();
    std::fs::write(handle.path(), serde_json::to_vec(&forged).unwrap()).unwrap();
    assert!(handle.refresh().is_err());
    assert!(!handle.held());
    assert_eq!(lock::read(handle.path()).unwrap().lock_id, "someone-else", "nothing written over the new owner");
    drop(handle);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn liveness_does_not_depend_on_process_ids_or_clocks() {
    let _g = serial();
    let (dir, project) = project();
    let file = lock::lock_path(&project);
    // A dead holder's protected lock whose process id now belongs to a live process (PID reuse: our own id).
    let mut reused = serde_json::to_value(LockInfo {
        user: "c14".into(),
        host: tt_persist::host::host(),
        process_id: std::process::id(),
        app_version: "0.0.1".into(),
        opened_at: tt_persist::time::now_iso(),
        heartbeat_at: tt_persist::time::now_iso(),
        lock_id: "dead".into(),
    })
    .unwrap();
    reused[lock::PROTECTION_KEY] = json!("test");
    std::fs::write(&file, serde_json::to_vec(&reused).unwrap()).unwrap();
    let Locked::Acquired { handle, took_over: Some(old) } = lock::acquire(&project) else {
        panic!("a protected lock nobody protects is stale whatever its process id says")
    };
    assert_eq!(old.lock_id, "dead");
    // A live holder whose heartbeat is far in the past or the future (clock jumps both ways) stays held: the unit
    // test `a_live_lock_is_protected_and_never_stale` rewrites a live lock's heartbeat both ways.
    assert!(matches!(lock::acquire(&project), Locked::HeldByOther(_)));
    drop(handle);
    // An unprotected lock with a heartbeat in the future: held while its file is fresh, stale once the file has not
    // changed for STALE_SECS (bounded).
    let mut future = reused.clone();
    future.as_object_mut().unwrap().remove(lock::PROTECTION_KEY);
    future["heartbeatAt"] = json!("2100-01-01T00:00:00.000Z");
    std::fs::write(&file, serde_json::to_vec(&future).unwrap()).unwrap();
    assert!(matches!(lock::acquire(&project), Locked::HeldByOther(_)));
    let old_mtime = SystemTime::now() - Duration::from_secs(lock::STALE_SECS + 5);
    std::fs::File::options().write(true).open(&file).unwrap().set_modified(old_mtime).unwrap();
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("a future heartbeat is bounded") };
    drop(handle);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Child process for the crash test: holds a lock until killed.
#[test]
#[ignore = "run by crashed_holder_is_detected_at_once as a child process"]
fn child_lock_holder() {
    let Ok(p) = std::env::var("TT_C14_CHILD_PROJECT") else { return };
    let Locked::Acquired { handle, .. } = lock::acquire(Path::new(&p)) else { panic!("child acquire") };
    println!("child-holds {}", handle.info().lock_id);
    std::thread::sleep(Duration::from_secs(120));
    drop(handle);
}

/// Starts a child process holding the project's lock and waits until it holds it.
fn child_holding(project: &Path) -> (std::process::Child, LockInfo) {
    let file = lock::lock_path(project);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_lock_holder", "--ignored", "--nocapture", "--test-threads=1"])
        .env("TT_C14_CHILD_PROJECT", project)
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !(lock::probe(&file) == Some(true) && lock::read(&file).is_some()) {
        assert!(start.elapsed() < Duration::from_secs(60), "the child never took the lock");
        std::thread::sleep(Duration::from_millis(10));
    }
    let info = lock::read(&file).unwrap();
    (child, info)
}

#[test]
fn crashed_holder_is_detected_at_once_and_liveness_is_measured() {
    let _g = serial();
    let (dir, project) = project();
    let file = lock::lock_path(&project);
    let mut rec = Recorder::new();
    // Five crashed holders (C14-R1-07: every observation kept).
    for round in 0..5 {
        let (mut child, child_info) = child_holding(&project);
        // While the child lives: held, from another process.
        assert!(matches!(lock::acquire(&project), Locked::HeldByOther(_)));
        if round == 0 {
            for _ in 0..500 {
                let t = Instant::now();
                assert_eq!(lock::probe(&file), Some(true));
                rec.sample("probeHeldMicros", "us", t, t.elapsed().as_secs_f64() * 1e6, None);
            }
            for _ in 0..50 {
                let t = Instant::now();
                assert!(matches!(lock::acquire(&project), Locked::HeldByOther(_)));
                rec.sample("acquireOnHeldMicros", "us", t, t.elapsed().as_secs_f64() * 1e6, None);
            }
        }
        child.kill().unwrap();
        let _ = child.wait();
        let killed = Instant::now();
        let (handle, took) = loop {
            match lock::acquire(&project) {
                Locked::Acquired { handle, took_over } => break (handle, took_over),
                _ => {
                    assert!(killed.elapsed() < Duration::from_secs(30), "the dead holder's lock was not taken over");
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        };
        rec.sample("crashedHolderTakeoverMs", "ms", killed, ms(killed.elapsed()), None);
        assert_eq!(took.unwrap().lock_id, child_info.lock_id);
        drop(handle);
        // process_alive (the fallback) is bounded.
        let t = Instant::now();
        let _ = tt_persist::host::process_alive(4_000_000_000);
        rec.sample("processAliveFallbackMs", "ms", t, ms(t.elapsed()), None);
        assert!(t.elapsed() <= tt_persist::host::LIVENESS_TIMEOUT + Duration::from_millis(500));
    }
    let report = rec.report(
        "C1.4 obligation 2 (ADR-0014 §11.4)",
        "a package's lock held by a child process (this test binary), killed five times",
        json!({
            "protection": if cfg!(windows) { "write handle shared read/write/delete; probe refuses to share writing" } else { "flock" },
            "bounds": {
                "protectedLiveHolder": "never stale (probe), whatever heartbeat, clock or process id say",
                "protectedDeadHolderSameHost": "at once (first open after the process ends)",
                "protectedOtherHostProbeFree": format!("heartbeat rule: {} s", lock::STALE_SECS),
                "unprotectedLock": format!("heartbeat older than {} s, or process known dead on this host", lock::STALE_SECS),
                "unprotectedFutureHeartbeat": format!("file unchanged for {} s", lock::STALE_SECS),
                "unreadableUnprotected": format!("{} s", lock::TAKEOVER_STALE.as_secs()),
                "unreadableUnknown": format!("{} s", lock::STALE_SECS),
                "protectedArbiter": "never removed while its holder lives",
                "unprotectedArbiter": format!("{} s (either clock direction)", lock::TAKEOVER_STALE.as_secs()),
                "processAliveTimeoutMs": tt_persist::host::LIVENESS_TIMEOUT.as_millis(),
            },
        }),
    );
    c14_evidence::write("lock-liveness", &report);
    assert!(rec.values("crashedHolderTakeoverMs").iter().all(|t| *t < 5_000.0), "{report}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C14-R1-08 (obligation 1): a save suspended before it writes, or after it wrote and before the session takes it,
/// loses the project's lock to another session (its publication replaced, as on a file system that does not carry
/// the holder's protection). The save fails and says why; the project stays modified, its saved revision and its
/// on-disk identity (stamp) do not advance, it continues read-only, and its recovery data stays.
#[test]
fn ownership_lost_during_a_suspended_save_never_advances_the_save() {
    let _g = serial();
    for at in [Boundary::SaveBeforeWrite, Boundary::SaveWritten] {
        let (dir, _) = project();
        let pkg = dir.join("Owned.animpkg");
        let h = tt_executor::ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
            config: tt_executor::ExecutorConfig { budget: Duration::from_secs(60) },
            persist: tt_executor::PersistOptions {
                user_dirs: Some(tt_executor::UserDirs::under(&dir.join("user"))),
                maintenance: tt_executor::Maintenance { tick: Duration::ZERO, ..tt_executor::Maintenance::default() },
                migrations: tt_persist::migrate::CHAIN,
            },
        })
        .unwrap();
        let ok = |id: &str, params: Value| {
            let env = h.invoke(tt_executor::Invocation::user(id, params));
            assert!(env.is_ok(), "{id}: {env:?}");
            env.result.unwrap_or(Value::Null)
        };
        ok("project.create", json!({ "name": "Owned", "path": pkg.to_string_lossy() }));
        let scene = ok("doc.object.find", json!({ "type": "scene", "limit": 1 }))["items"][0]["id"].clone();
        ok("doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 1 } } }));
        // Recovery data exists before the save.
        let events = h.subscribe();
        h.run_maintenance(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while !events.try_iter().any(|e| e.name == "project.autosaved") {
            assert!(Instant::now() < deadline, "no autosave");
            std::thread::sleep(Duration::from_millis(5));
        }
        let recovery = dir.join("user").join("Recovery");
        let recovery_entries = || std::fs::read_dir(&recovery).map_or(0, Iterator::count);
        assert!(recovery_entries() > 0);
        let before = ok("doc.state.get", json!({}));
        let bytes_before = std::fs::read(&pkg).unwrap();
        let lockfile = lock::lock_path(&pkg);
        let (reached, resume) = suspend(at, &lockfile);
        let started = ok("project.save", json!({}));
        reached.recv_timeout(Duration::from_secs(30)).unwrap();
        let mut taken: Value = serde_json::from_slice(&std::fs::read(&lockfile).unwrap()).unwrap();
        taken["lockId"] = json!("another-session");
        std::fs::write(&lockfile, serde_json::to_vec(&taken).unwrap()).unwrap();
        resume.send(()).unwrap();
        let waited = ok("job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60_000 }));
        lock::hooks::set(None);
        let rec = waited.get("record").cloned().unwrap_or(waited);
        assert_eq!(rec["state"], "failed", "{at:?}: {rec}");
        assert_eq!(rec["error"]["code"], "project.saveFailed", "{at:?}: {rec}");
        let reason = if at == Boundary::SaveBeforeWrite { "fileInUse" } else { "ownershipLost" };
        assert_eq!(rec["error"]["details"]["reason"], reason, "{at:?}: {rec}");
        let after = ok("doc.state.get", json!({}));
        assert_eq!(after["dirty"], true, "{at:?}: the project stays modified");
        assert_eq!(after["savedRev"], before["savedRev"], "{at:?}: the saved revision does not advance");
        assert_eq!(after["readOnly"], true, "{at:?}: the session continues read-only (LOCK-02)");
        assert!(recovery_entries() > 0, "{at:?}: the recovery data stays");
        if at == Boundary::SaveBeforeWrite {
            assert_eq!(std::fs::read(&pkg).unwrap(), bytes_before, "nothing written after the loss");
        } else {
            // The stamp did not advance: the save's own write is an external change to this session (forced watch).
            h.run_maintenance(true).unwrap();
            assert!(
                events.try_iter().any(|e| e.name == "doc.package.changedOnDisk"),
                "{at:?}: the stamp advanced to the file the session did not take"
            );
        }
        drop(h);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
