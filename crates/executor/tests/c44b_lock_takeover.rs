//! Ruling D-77 (the C4.4b handoff's section 6): the lock takeover's arbitration file `<lock>.takeover`.
//!
//! The defect: a releasing arbiter closed its file (releasing its protection) and then removed it by name; in between
//! a contender could judge the file abandoned and remove it, a third could create its own, and the first's delayed
//! removal deleted the third's file, so the third reported `HeldByOther` with its own publication and nobody owned the
//! lock (`c13_r1_persistence::concurrent_stale_lock_takeover_has_only_one_owner`, "0 owners"). The fix removes the
//! file while its protection is held, and a contender removes an arbitration file only after proving it abandoned
//! under that file's own protection. Each test suspends one participant at a boundary of the fix (the `test-hooks`
//! feature) and checks, for any timing, that the protected file is never removed by another contender and that the
//! lock ends with exactly one owner.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tt_persist::lock::{self, Boundary, Locked};

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
