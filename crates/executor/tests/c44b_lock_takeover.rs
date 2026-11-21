//! Rulings D-77 and D-78 (finding C44B-R1S-07; design note `reviews/M4-design-lock-takeover.md`): the lock takeover.
//!
//! Two defects removed files by name on the strength of an earlier decision. Zero owners: a releasing arbiter closed
//! its file and then removed it by name, so a reaper could remove it first, a third contender create its own, and the
//! delayed removal delete the third's (`c13_r1_persistence::concurrent_stale_lock_takeover_has_only_one_owner`).
//! Two owners (Unix): a contender that decided "stale" and paused lost its arbiter to a delayed reaper, another
//! contender acquired properly, and the paused one resumed and unlinked the new owner's protected lock. The fix: every
//! removal is the owner's own while it still protects the file, or a proof of removal that takes the file's own
//! protection and re-validates, after the pause point, that it is the very file decided about. Each test suspends
//! participants at the protocol's boundaries (the `test-hooks` feature) and checks, for any timing, that a protected
//! file is never removed by another contender and that the lock ends with exactly one owner whose handle stays valid.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Barrier, Mutex, PoisonError};
use std::time::Duration;

use tt_persist::lock::{self, Boundary, LockHandle, LockInfo, Locked};

/// The hook is process-global: tests that install one run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn project() -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tt-d77-lock-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("Takeover.animpkg");
    std::fs::write(&p, b"x").unwrap();
    (dir, p)
}

/// The arbitration file of a lock file (`<lock>.takeover`).
fn takeover_of(lock_file: &Path) -> PathBuf {
    let mut s = lock_file.as_os_str().to_owned();
    s.push(".takeover");
    PathBuf::from(s)
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

/// How long a contender is left to try while a participant is suspended. Not a bound: the assertions hold for any
/// duration; a contender retries every 5 ms, so this gives it dozens of attempts.
const CONTENDING: Duration = Duration::from_millis(300);

#[test]
fn a_releasing_arbiter_removes_its_file_while_protected_and_no_contender_removes_it() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("the first open takes the lock") };
    // The holder closes the project: its release takes the arbiter, removes the lock file and then releases the
    // arbiter, suspended where the arbiter has checked its file is its own and is about to remove it.
    let (reached, resume) = suspend(Boundary::ArbiterReleasing, &lock_file);
    let releaser = std::thread::spawn(move || drop(handle));
    reached.recv_timeout(Duration::from_secs(30)).expect("the release reaches its arbiter's removal");
    assert!(!lock_file.exists(), "the release removed the lock file before releasing its arbiter");
    assert!(takeover.exists(), "the arbitration file is still there");
    assert_eq!(lock::probe(&takeover), Some(true), "the releasing arbiter still protects its file");
    // A contender opens the project meanwhile: it finds the arbitration file, cannot prove it abandoned, removes
    // nothing and keeps trying.
    let contender = {
        let p = project.clone();
        std::thread::spawn(move || lock::acquire(&p))
    };
    std::thread::sleep(CONTENDING);
    assert!(takeover.exists(), "a contender removed an arbitration file its holder still protects");
    assert_eq!(lock::probe(&takeover), Some(true));
    assert!(!lock_file.exists(), "no lock is published while another contender holds the arbiter");
    resume.send(()).unwrap();
    releaser.join().unwrap();
    // The released arbiter is gone, and the contender owns the lock.
    let outcome = contender.join().unwrap();
    assert!(matches!(outcome, Locked::Acquired { .. }), "the contender owns the lock once released: {outcome:?}");
    drop(outcome);
    lock::hooks::set(None);
    assert!(!takeover.exists(), "every arbiter removed its file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_abandoned_arbitration_file_is_removed_once_under_its_own_protection_and_one_contender_owns_the_lock() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    // An arbitration file left by a holder that died after writing its token: nobody protects it.
    std::fs::write(&takeover, b"deadbeef").unwrap();
    assert_eq!(lock::probe(&takeover), Some(false));
    // The first contender proves it abandoned and is suspended holding that proof, before removing it.
    let (reached, resume) = suspend(Boundary::ArbiterAbandonedProven, &lock_file);
    let first = {
        let p = project.clone();
        std::thread::spawn(move || lock::acquire(&p))
    };
    reached.recv_timeout(Duration::from_secs(30)).expect("the first contender proves the file abandoned");
    // A second contender meanwhile: no arbiter can be created while the file exists, so nobody publishes a lock.
    let second = {
        let p = project.clone();
        std::thread::spawn(move || lock::acquire(&p))
    };
    std::thread::sleep(CONTENDING);
    assert!(!lock_file.exists(), "nobody publishes a lock while the abandoned arbitration file is being removed");
    assert!(!second.is_finished(), "the second contender is still waiting for the arbiter");
    resume.send(()).unwrap();
    let outcomes = [first.join().unwrap(), second.join().unwrap()];
    let owners = outcomes.iter().filter(|o| matches!(o, Locked::Acquired { .. })).count();
    let held = outcomes.iter().filter(|o| matches!(o, Locked::HeldByOther(_))).count();
    assert_eq!((owners, held), (1, 1), "exactly one owner, the other told it is held: {outcomes:?}");
    drop(outcomes);
    lock::hooks::set(None);
    assert!(!takeover.exists(), "the abandoned arbitration file and every later one are gone");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A stale project lock (an unprotected file, a heartbeat of 2000): any contender decides "stale".
fn stale_lock(lock_file: &Path, id: &str) {
    let old = LockInfo {
        user: "d78".into(),
        host: tt_persist::host::host(),
        process_id: std::process::id(),
        app_version: "0.0.1".into(),
        opened_at: "2000-01-01T00:00:00.000Z".into(),
        heartbeat_at: "2000-01-01T00:00:00.000Z".into(),
        lock_id: id.into(),
    };
    std::fs::write(lock_file, serde_json::to_vec(&old).unwrap()).unwrap();
}

type Pause = (Boundary, Receiver<()>, Sender<()>);

/// Suspends, once each, the first participant that reaches each boundary of `at` for `lock_file`.
fn pauses(lock_file: &Path, at: &[Boundary]) -> Vec<Pause> {
    let target = lock_file.to_owned();
    let mut armed = Vec::new();
    let mut out = Vec::new();
    for &b in at {
        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel::<()>();
        armed.push((b, Some((reached_tx, resume_rx))));
        out.push((b, reached_rx, resume_tx));
    }
    let armed = Arc::new(Mutex::new(armed));
    lock::hooks::set(Some(Arc::new(move |b, p| {
        if p != target {
            return;
        }
        let taken = {
            let mut g = armed.lock().unwrap_or_else(PoisonError::into_inner);
            g.iter_mut().find(|(x, _)| *x == b).and_then(|(_, slot)| slot.take())
        };
        if let Some((reached, resume)) = taken {
            let _ = reached.send(());
            let _ = resume.recv_timeout(Duration::from_secs(60));
        }
    })));
    out
}

fn reached(p: &[Pause], b: Boundary) {
    let (_, r, _) = p.iter().find(|x| x.0 == b).unwrap();
    r.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|_| panic!("{b:?} was reached"));
}

fn resume(p: &[Pause], b: Boundary) {
    let (_, _, s) = p.iter().find(|x| x.0 == b).unwrap();
    s.send(()).unwrap();
}

fn spawn_acquire(project: &Path) -> std::thread::JoinHandle<Locked> {
    let p = project.to_owned();
    std::thread::spawn(move || lock::acquire(&p))
}

fn owners(outcomes: &[Locked]) -> Vec<&LockHandle> {
    outcomes
        .iter()
        .filter_map(|o| match o {
            Locked::Acquired { handle, .. } => Some(handle),
            _ => None,
        })
        .collect()
}

#[test]
fn the_zero_owner_schedule_a_delayed_reaper_never_removes_a_successors_arbiter() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    stale_lock(&lock_file, "stale");
    std::fs::write(&takeover, b"left-behind").unwrap();
    let p = pauses(&lock_file, &[Boundary::ArbiterAbandonedProven, Boundary::TakeoverDecided]);
    // Reaper R proves the abandoned arbitration file and is suspended holding that proof.
    let r = spawn_acquire(&project);
    reached(&p, Boundary::ArbiterAbandonedProven);
    // The other reaper's removal, behind R's back (the file R decided about goes; Unix unlinks it, Windows marks it).
    let _ = std::fs::remove_file(&takeover);
    // Contender D takes a fresh arbiter (on Windows, once the old file is gone) and decides the lock stale.
    let d = spawn_acquire(&project);
    std::thread::sleep(CONTENDING);
    // R resumes: its re-validation finds another file (Unix) or its handle deletes only the old one (Windows).
    resume(&p, Boundary::ArbiterAbandonedProven);
    reached(&p, Boundary::TakeoverDecided);
    assert!(takeover.exists(), "a delayed reaper removed the successor's arbitration file");
    assert_eq!(lock::probe(&takeover), Some(true), "D still protects its arbitration file");
    resume(&p, Boundary::TakeoverDecided);
    let outcomes = [d.join().unwrap(), r.join().unwrap()];
    lock::hooks::set(None);
    let owned = owners(&outcomes);
    assert_eq!(owned.len(), 1, "exactly one owner: {outcomes:?}");
    assert!(owned[0].held(), "the owner's handle is valid");
    drop(outcomes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_two_owner_schedule_a_resumed_stale_decision_never_removes_a_live_owners_lock() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    stale_lock(&lock_file, "stale");
    let p = pauses(&lock_file, &[Boundary::TakeoverDecided]);
    // D decides the lock stale under its arbiter and is suspended before acting on it.
    let d = spawn_acquire(&project);
    reached(&p, Boundary::TakeoverDecided);
    // D's arbitration file removed behind its back (the reviewer's delayed removal; the protocol can no longer do it,
    // so the test does): W can take an arbiter and acquire properly (on Windows, once D's file is really gone).
    let _ = std::fs::remove_file(&takeover);
    let w = spawn_acquire(&project);
    std::thread::sleep(CONTENDING);
    let w_first = w.is_finished();
    // D resumes: its decision is acted on only through a proof of removal, refused while W protects its lock.
    resume(&p, Boundary::TakeoverDecided);
    let d_outcome = d.join().unwrap();
    let w_outcome = w.join().unwrap();
    lock::hooks::set(None);
    if w_first {
        let Locked::Acquired { handle, .. } = &w_outcome else { panic!("W acquired first: {w_outcome:?}") };
        assert!(handle.held(), "D's resumed decision removed W's protected lock");
        assert!(matches!(d_outcome, Locked::HeldByOther(_)), "D is told the lock is held: {d_outcome:?}");
    }
    let outcomes = [d_outcome, w_outcome];
    let owned = owners(&outcomes);
    assert_eq!(owned.len(), 1, "exactly one owner: {outcomes:?}");
    assert!(owned[0].held(), "the owner's handle is valid");
    drop(outcomes);
    let _ = std::fs::remove_dir_all(&dir);
}

/// xorshift64: a fixed, reproducible schedule of contenders and delays.
fn next(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

#[test]
fn many_contenders_with_jitter_leave_exactly_one_owner_whose_handle_stays_valid() {
    let _s = serial();
    lock::hooks::set(None);
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    for trial in 0..40 {
        // A stale lock, and every third trial a left-behind arbitration file as well.
        stale_lock(&lock_file, &format!("stale-{trial}"));
        if trial % 3 == 0 {
            std::fs::write(takeover_of(&lock_file), b"left-behind").unwrap();
        }
        let n = 6 + usize::try_from(next(&mut seed) % 7).unwrap();
        let barrier = Arc::new(Barrier::new(n));
        let threads: Vec<_> = (0..n)
            .map(|_| {
                let delay = next(&mut seed) % 4_000;
                let (p, b) = (project.clone(), barrier.clone());
                std::thread::spawn(move || {
                    b.wait();
                    std::thread::sleep(Duration::from_micros(delay));
                    lock::acquire(&p)
                })
            })
            .collect();
        let outcomes: Vec<Locked> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        let owned = owners(&outcomes);
        assert_eq!(owned.len(), 1, "trial {trial}: exactly one owner: {outcomes:?}");
        assert!(owned[0].held(), "trial {trial}: the owner's handle stays valid after every other contender returned");
        drop(outcomes);
        assert!(!lock_file.exists(), "trial {trial}: the owner's release removed its lock");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
