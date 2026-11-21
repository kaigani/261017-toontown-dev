//! Rulings D-77, D-78 and D-83 (findings C44B-R1S-07, C44B-R2S-01 and C44B-R2S-02; design note
//! `reviews/M4-design-lock-takeover.md`): the lock takeover.
//!
//! Defects that removed or trusted files by name on the strength of an earlier decision. Zero owners: a releasing
//! arbiter closed its file and then removed it by name, so a reaper could remove it first, a third contender create its
//! own, and the delayed removal delete the third's. Two owners (Unix): a contender that decided "stale" and paused lost
//! its arbiter to a delayed reaper, another contender acquired properly, and the paused one resumed and unlinked the
//! new owner's protected lock. Zero owners by creation (Unix): a creator suspended between its `create_new` and its
//! `flock` locked a file a prover had already unlinked and took the arbiter role on it. The fix: every removal is the
//! owner's own while it still protects the file, or a proof of removal that takes the file's own protection and
//! re-validates the very file after the pause point; and a handle confers a role only while the path still names it.
//!
//! **How the tests order participants (ruling D-83 item 2).** Each participant is a named thread. The hook records
//! every boundary each one reaches and suspends exactly the participant the test armed at that boundary; the test
//! waits until that participant is held there and resumes it by name. Nothing is ordered by time: every wait is for a
//! state (a participant held at a boundary, a thread returned); the 60 s limits only turn a hang into a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use tt_persist::lock::{self, Boundary, LockHandle, LockInfo, Locked};

/// The hook is process-global: tests that install one run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn project() -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tt-d83-lock-{}", tt_persist::fsx::random_token(6)));
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

/// A stale project lock (an unprotected file, a heartbeat of 2000): any contender decides "stale".
fn stale_lock(lock_file: &Path, id: &str) {
    let old = LockInfo {
        user: "d83".into(),
        host: tt_persist::host::host(),
        process_id: std::process::id(),
        app_version: "0.0.1".into(),
        opened_at: "2000-01-01T00:00:00.000Z".into(),
        heartbeat_at: "2000-01-01T00:00:00.000Z".into(),
        lock_id: id.into(),
    };
    std::fs::write(lock_file, serde_json::to_vec(&old).unwrap()).unwrap();
}

/// Makes a file look older than `TAKEOVER_STALE` (standing for the wait a real contender would make).
fn age_past_stale(path: &Path) {
    let old = SystemTime::now() - lock::TAKEOVER_STALE * 2;
    if let Ok(f) = std::fs::File::options().write(true).open(path) {
        let _ = f.set_modified(old);
    }
}

type Key = (String, Boundary);

#[derive(Default)]
struct Sched {
    /// (participant, boundary) pairs to suspend once.
    armed: Vec<Key>,
    /// Participants suspended now.
    held: Vec<Key>,
    /// Every boundary every participant reached, in order.
    seen: Vec<Key>,
    /// Set when the test ends: nobody is suspended any more.
    open: bool,
}

/// The participants' schedule: installed as the lock hook for the given files; cleared, with every participant
/// released, when dropped (so a failing assertion cannot leave threads suspended).
struct Schedule {
    inner: Arc<(Mutex<Sched>, Condvar)>,
}

const LIMIT: Duration = Duration::from_secs(60);

impl Schedule {
    fn install(files: &[&Path]) -> Self {
        let inner = Arc::new((Mutex::new(Sched::default()), Condvar::new()));
        let hook_inner = inner.clone();
        let files: Vec<PathBuf> = files.iter().map(|p| p.to_path_buf()).collect();
        lock::hooks::set(Some(Arc::new(move |b, p| {
            if !files.iter().any(|f| f == p) {
                return;
            }
            let Some(name) = std::thread::current().name().map(str::to_owned) else { return };
            let (m, cv) = &*hook_inner;
            let mut g = m.lock().unwrap_or_else(PoisonError::into_inner);
            let key = (name, b);
            g.seen.push(key.clone());
            cv.notify_all();
            if g.open {
                return;
            }
            if let Some(i) = g.armed.iter().position(|k| *k == key) {
                g.armed.remove(i);
                g.held.push(key.clone());
                cv.notify_all();
                while g.held.contains(&key) && !g.open {
                    g = cv.wait(g).unwrap_or_else(PoisonError::into_inner);
                }
            }
        })));
        Schedule { inner }
    }

    fn with<R>(&self, f: impl FnOnce(&mut Sched) -> R) -> R {
        let (m, cv) = &*self.inner;
        let r = f(&mut m.lock().unwrap_or_else(PoisonError::into_inner));
        cv.notify_all();
        r
    }

    /// Suspends `who` the next time it reaches `b`.
    fn arm(&self, who: &str, b: Boundary) {
        self.with(|s| s.armed.push((who.to_owned(), b)));
    }

    /// Waits until `who` is suspended at `b`.
    fn wait_held(&self, who: &str, b: Boundary) {
        let key = (who.to_owned(), b);
        let (m, cv) = &*self.inner;
        let start = Instant::now();
        let mut g = m.lock().unwrap_or_else(PoisonError::into_inner);
        while !g.held.contains(&key) {
            assert!(start.elapsed() < LIMIT, "{who} never reached {b:?}; it reached {:?}", g.seen);
            g = cv.wait_timeout(g, Duration::from_millis(100)).unwrap_or_else(PoisonError::into_inner).0;
        }
    }

    /// Resumes `who`, suspended at `b`.
    fn resume(&self, who: &str, b: Boundary) {
        let key = (who.to_owned(), b);
        self.with(|s| {
            assert!(s.held.contains(&key), "{who} is not suspended at {b:?}");
            s.held.retain(|k| *k != key);
        });
    }

    /// Whether `who` has reached `b` so far.
    fn reached(&self, who: &str, b: Boundary) -> bool {
        self.with(|s| s.seen.iter().any(|(n, x)| n == who && *x == b))
    }
}

impl Drop for Schedule {
    fn drop(&mut self) {
        self.with(|s| {
            s.open = true;
            s.held.clear();
        });
        lock::hooks::set(None);
    }
}

/// `lock::acquire` on a thread named `who`.
fn participant(who: &str, project: &Path) -> std::thread::JoinHandle<Locked> {
    let p = project.to_owned();
    std::thread::Builder::new().name(who.to_owned()).spawn(move || lock::acquire(&p)).unwrap()
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
fn a_releasing_arbiter_removes_its_file_while_protected_and_no_contender_removes_it() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("the first open takes the lock") };
    let sched = Schedule::install(&[&lock_file, &takeover]);
    // H closes the project: its release takes the arbiter, removes the lock file and releases the arbiter, suspended
    // where the arbiter has checked its file is its own and is about to remove it.
    sched.arm("H", Boundary::ArbiterReleasing);
    let h = std::thread::Builder::new().name("H".into()).spawn(move || drop(handle)).unwrap();
    sched.wait_held("H", Boundary::ArbiterReleasing);
    assert!(!lock_file.exists(), "the release removed the lock file before releasing its arbiter");
    assert_eq!(lock::probe(&takeover), Some(true), "the releasing arbiter still protects its file");
    // C opens the project meanwhile: its attempt at the arbiter ends without it, and it removed nothing.
    sched.arm("C", Boundary::ArbiterNotTaken);
    let c = participant("C", &project);
    sched.wait_held("C", Boundary::ArbiterNotTaken);
    assert_eq!(lock::probe(&takeover), Some(true), "a contender removed an arbitration file its holder protects");
    assert!(!sched.reached("C", Boundary::ArbiterTaken));
    assert!(!lock_file.exists(), "no lock is published while another contender holds the arbiter");
    sched.resume("H", Boundary::ArbiterReleasing);
    h.join().unwrap();
    sched.resume("C", Boundary::ArbiterNotTaken);
    let outcome = c.join().unwrap();
    assert!(matches!(outcome, Locked::Acquired { .. }), "the contender owns the lock once released: {outcome:?}");
    drop(outcome);
    drop(sched);
    assert!(!takeover.exists() && !lock_file.exists(), "every file was removed by its owner");
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
    let sched = Schedule::install(&[&lock_file, &takeover]);
    // F proves it abandoned and is suspended holding that proof, before removing it.
    sched.arm("F", Boundary::ArbiterAbandonedProven);
    let f = participant("F", &project);
    sched.wait_held("F", Boundary::ArbiterAbandonedProven);
    // S meanwhile: its attempt ends without the arbiter (the file exists while F's proof holds it open).
    sched.arm("S", Boundary::ArbiterNotTaken);
    let s = participant("S", &project);
    sched.wait_held("S", Boundary::ArbiterNotTaken);
    assert!(!sched.reached("S", Boundary::ArbiterTaken), "no arbiter while the abandoned file is being removed");
    assert!(!lock_file.exists(), "nobody publishes a lock meanwhile");
    sched.resume("F", Boundary::ArbiterAbandonedProven);
    sched.resume("S", Boundary::ArbiterNotTaken);
    let outcomes = [f.join().unwrap(), s.join().unwrap()];
    let owned = owners(&outcomes);
    assert_eq!(owned.len(), 1, "exactly one owner: {outcomes:?}");
    assert!(owned[0].held());
    assert_eq!(outcomes.iter().filter(|o| matches!(o, Locked::HeldByOther(_))).count(), 1, "{outcomes:?}");
    drop(outcomes);
    drop(sched);
    assert!(!takeover.exists(), "the abandoned arbitration file and every later one are gone");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The zero-owner schedule (Unix: on Windows a removed file stays under its name while a handle that shares deletion
/// is open, so the successor cannot even be created: `windows_a_delete_pending_arbiter_blocks_every_successor`).
#[cfg(unix)]
#[test]
fn the_zero_owner_schedule_a_delayed_reaper_never_removes_a_successors_arbiter() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    stale_lock(&lock_file, "stale");
    std::fs::write(&takeover, b"left-behind").unwrap();
    let sched = Schedule::install(&[&lock_file, &takeover]);
    // Reaper R proves the abandoned arbitration file and is suspended holding that proof.
    sched.arm("R", Boundary::ArbiterAbandonedProven);
    let r = participant("R", &project);
    sched.wait_held("R", Boundary::ArbiterAbandonedProven);
    // The other reaper's removal, behind R's back.
    std::fs::remove_file(&takeover).unwrap();
    // D takes a fresh arbiter and decides the lock stale: suspended there, holding its arbiter.
    sched.arm("D", Boundary::TakeoverDecided);
    let d = participant("D", &project);
    sched.wait_held("D", Boundary::TakeoverDecided);
    assert_eq!(lock::probe(&takeover), Some(true), "D protects its fresh arbitration file");
    // R resumes: its re-validation finds another file, it removes nothing, and its attempt ends without the arbiter.
    sched.arm("R", Boundary::ArbiterNotTaken);
    sched.resume("R", Boundary::ArbiterAbandonedProven);
    sched.wait_held("R", Boundary::ArbiterNotTaken);
    assert_eq!(lock::probe(&takeover), Some(true), "a delayed reaper removed the successor's arbitration file");
    // D completes and owns the lock; then R is told it is held.
    sched.resume("D", Boundary::TakeoverDecided);
    let d_outcome = d.join().unwrap();
    let Locked::Acquired { handle, .. } = &d_outcome else { panic!("D owns the lock: {d_outcome:?}") };
    assert!(handle.held());
    sched.resume("R", Boundary::ArbiterNotTaken);
    let r_outcome = r.join().unwrap();
    assert!(matches!(r_outcome, Locked::HeldByOther(_)), "R is told the lock is held: {r_outcome:?}");
    let Locked::Acquired { handle, .. } = &d_outcome else { unreachable!() };
    assert!(handle.held(), "D's handle stays valid after R returned");
    drop((d_outcome, r_outcome));
    drop(sched);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The two-owner schedule (Unix; see the zero-owner test for Windows). W's `Acquired` is required before D resumes.
#[cfg(unix)]
#[test]
fn the_two_owner_schedule_a_resumed_stale_decision_never_removes_a_live_owners_lock() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    stale_lock(&lock_file, "stale");
    let sched = Schedule::install(&[&lock_file, &takeover]);
    // D decides the lock stale under its arbiter and is suspended before acting on it.
    sched.arm("D", Boundary::TakeoverDecided);
    let d = participant("D", &project);
    sched.wait_held("D", Boundary::TakeoverDecided);
    // D's arbitration file removed behind its back (the reviewer's delayed removal; the protocol can no longer do it,
    // so the test does): W takes an arbiter and acquires properly, completely, while D is suspended.
    std::fs::remove_file(&takeover).unwrap();
    let w_outcome = participant("W", &project).join().unwrap();
    let Locked::Acquired { handle: w_handle, .. } = &w_outcome else { panic!("W acquires first: {w_outcome:?}") };
    assert!(w_handle.held());
    // D resumes: its decision is acted on only through a proof of removal, refused while W protects its lock.
    sched.resume("D", Boundary::TakeoverDecided);
    let d_outcome = d.join().unwrap();
    assert!(matches!(d_outcome, Locked::HeldByOther(_)), "D is told the lock is held: {d_outcome:?}");
    assert!(w_handle.held(), "D's resumed decision removed W's protected lock");
    drop((w_outcome, d_outcome));
    drop(sched);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Windows (ruling D-83 item 2): a removed arbitration file whose holder's handle (which shares deletion) is open stays
/// under its name, delete pending; no contender creates a successor until the handle closes, and the paused holder,
/// whose arbiter is no longer its own, does not announce ownership.
#[cfg(windows)]
#[test]
fn windows_a_delete_pending_arbiter_blocks_every_successor() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    stale_lock(&lock_file, "stale");
    let sched = Schedule::install(&[&lock_file, &takeover]);
    sched.arm("D", Boundary::TakeoverDecided);
    let d = participant("D", &project);
    sched.wait_held("D", Boundary::TakeoverDecided);
    // Removed behind D's back: delete pending while D's handle is open.
    std::fs::remove_file(&takeover).unwrap();
    sched.arm("W", Boundary::ArbiterNotTaken);
    let w = participant("W", &project);
    sched.wait_held("W", Boundary::ArbiterNotTaken);
    assert!(!sched.reached("W", Boundary::ArbiterTaken), "no successor arbiter while the old one is delete pending");
    assert_eq!(lock::read(&lock_file).map(|i| i.lock_id).as_deref(), Some("stale"), "nothing published meanwhile");
    sched.resume("D", Boundary::TakeoverDecided);
    let d_outcome = d.join().unwrap();
    assert!(!matches!(d_outcome, Locked::Acquired { .. }), "D's arbiter is not its own: {d_outcome:?}");
    sched.resume("W", Boundary::ArbiterNotTaken);
    let w_outcome = w.join().unwrap();
    let Locked::Acquired { handle, .. } = &w_outcome else { panic!("W owns the lock after D: {w_outcome:?}") };
    assert!(handle.held());
    drop((d_outcome, w_outcome));
    drop(sched);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Finding C44B-R2S-01's schedule: A's `create_new` of the arbitration file succeeds and A is suspended before its
/// protection; R, past `TAKEOVER_STALE`, proves the empty file abandoned. Unix: R removes it, and A, resuming, finds
/// that the path no longer names the file it locked: its attempt ends without the arbiter (C2). Windows: A's creating
/// handle protects the file, R's proof is refused. Either way, exactly one owner.
#[test]
fn a_file_removed_between_its_creation_and_its_protection_confers_nothing() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    let sched = Schedule::install(&[&lock_file, &takeover]);
    sched.arm("A", Boundary::CreatedUnprotected);
    let a = participant("A", &project);
    sched.wait_held("A", Boundary::CreatedUnprotected);
    assert!(takeover.exists(), "A created the arbitration file");
    age_past_stale(&takeover);
    sched.arm("R", Boundary::ArbiterNotTaken);
    let r = participant("R", &project);
    sched.wait_held("R", Boundary::ArbiterNotTaken);
    if cfg!(windows) {
        assert!(takeover.exists(), "Windows: the creating handle protects the file; R's proof is refused");
    } else {
        assert!(!takeover.exists(), "Unix: R proved the aged, empty, unlocked file abandoned and removed it");
        sched.arm("A", Boundary::ArbiterNotTaken);
    }
    sched.resume("A", Boundary::CreatedUnprotected);
    if !cfg!(windows) {
        sched.wait_held("A", Boundary::ArbiterNotTaken);
        assert!(!sched.reached("A", Boundary::ArbiterTaken), "A took the arbiter role on a file no path names");
        assert!(!lock_file.exists(), "nothing was published on the lost file");
        sched.resume("A", Boundary::ArbiterNotTaken);
    }
    sched.resume("R", Boundary::ArbiterNotTaken);
    let outcomes = [a.join().unwrap(), r.join().unwrap()];
    let owned = owners(&outcomes);
    assert_eq!(owned.len(), 1, "exactly one owner: {outcomes:?}");
    assert!(owned[0].held(), "the owner's handle is valid after both returned");
    drop(outcomes);
    drop(sched);
    assert!(!takeover.exists() && !lock_file.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Unix: a creator that cannot take its new file's `flock` because a prover holds it gives up and removes nothing (a
/// removal by name without the protection could land on a successor's file once the prover removed this one). Windows
/// has no such window: a prover's open is refused from the file's creation.
#[cfg(unix)]
#[test]
fn a_creators_give_up_never_removes_a_file_a_prover_holds() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    let sched = Schedule::install(&[&lock_file, &takeover]);
    // C created the arbitration file and is suspended before its protection.
    sched.arm("C", Boundary::CreatedUnprotected);
    let c = participant("C", &project);
    sched.wait_held("C", Boundary::CreatedUnprotected);
    // P takes that file's `flock` for a proof and is suspended before re-validating it.
    sched.arm("P", Boundary::ArbiterAbandonedProven);
    let p = participant("P", &project);
    sched.wait_held("P", Boundary::ArbiterAbandonedProven);
    // C resumes: its `flock` is refused for its whole wait, it gives up, and its attempt ends without the arbiter.
    sched.arm("C", Boundary::ArbiterNotTaken);
    sched.resume("C", Boundary::CreatedUnprotected);
    sched.wait_held("C", Boundary::ArbiterNotTaken);
    assert!(takeover.exists(), "the creator's give-up removed a file whose protection a prover holds");
    assert!(!sched.reached("C", Boundary::ArbiterTaken));
    // The empty file is P's to judge. Standing for the wait past TAKEOVER_STALE, it is aged: P's proof removes exactly
    // the file it holds, and the contenders then settle on one owner.
    age_past_stale(&takeover);
    sched.resume("P", Boundary::ArbiterAbandonedProven);
    sched.resume("C", Boundary::ArbiterNotTaken);
    let outcomes = [c.join().unwrap(), p.join().unwrap()];
    let owned = owners(&outcomes);
    assert_eq!(owned.len(), 1, "exactly one owner: {outcomes:?}");
    assert!(owned[0].held());
    drop(outcomes);
    drop(sched);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The boundaries a participant can end at, with the file each reports.
const ENDINGS: &[(&str, &str)] = &[
    ("ArbiterAbandonedProven", "lock"),
    ("ArbiterNotTaken", "lock"),
    ("CreatedUnprotected", "takeover"),
    ("ArbiterTaken", "lock"),
    ("TakeoverDecided", "lock"),
    ("TakeoverProven", "lock"),
    ("CreatedUnprotected", "lock"),
    ("PublishCreated", "lock"),
    ("PublishWritten", "lock"),
    ("ArbiterReleasing", "lock"),
    ("ReleaseChecked", "lock"),
];

/// Child process: opens the project (and closes it) and ends the process at the boundary named by the environment, so
/// the system closes its handles and none of its own cleanup runs.
#[test]
#[ignore = "run by a_participant_ending_at_any_pause_point_leaves_a_lock_the_next_open_owns as a child process"]
fn child_ends_at_a_boundary() {
    let Ok(project) = std::env::var("TT_D83_CHILD_PROJECT") else { return };
    let at = std::env::var("TT_D83_CHILD_AT").unwrap();
    let file = std::env::var("TT_D83_CHILD_FILE").unwrap();
    let lock_file = lock::lock_path(Path::new(&project));
    let target = if file == "takeover" { takeover_of(&lock_file) } else { lock_file };
    lock::hooks::set(Some(Arc::new(move |b, p| {
        if format!("{b:?}") == at && p == target {
            std::process::exit(0);
        }
    })));
    if let Locked::Acquired { handle, .. } = lock::acquire(Path::new(&project)) {
        drop(handle);
    }
    // Not reached when the boundary was: the parent checks that the child ended there.
    std::process::exit(3);
}

#[test]
fn a_participant_ending_at_any_pause_point_leaves_a_lock_the_next_open_owns() {
    let _s = serial();
    lock::hooks::set(None);
    for (at, file) in ENDINGS {
        let (dir, project) = project();
        let lock_file = lock::lock_path(&project);
        let takeover = takeover_of(&lock_file);
        // A stale lock and a left-behind arbitration file: one open passes every boundary in ENDINGS, in this order.
        stale_lock(&lock_file, "stale");
        std::fs::write(&takeover, b"left-behind").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "child_ends_at_a_boundary", "--ignored", "--nocapture", "--test-threads=1"])
            .env("TT_D83_CHILD_PROJECT", &project)
            .env("TT_D83_CHILD_AT", at)
            .env("TT_D83_CHILD_FILE", file)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(0), "{at} ({file}): the child ended at the boundary");
        // What the ended participant left is unprotected; standing for the wait past TAKEOVER_STALE, it is aged.
        age_past_stale(&takeover);
        age_past_stale(&lock_file);
        let outcome = lock::acquire(&project);
        let Locked::Acquired { handle, .. } = &outcome else { panic!("{at} ({file}): next open owns: {outcome:?}") };
        assert!(handle.held(), "{at} ({file})");
        drop(outcome);
        assert!(!lock_file.exists() && !takeover.exists(), "{at} ({file}): nothing left behind after the release");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Opens a file for reading as another program would: on Windows sharing reading, and deletion when `share_delete`.
fn foreign_reader(path: &Path, share_delete: bool) -> std::fs::File {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(if share_delete { 0x1 | 0x4 } else { 0x1 });
    }
    #[cfg(not(windows))]
    let _ = share_delete;
    o.open(path).unwrap()
}

/// Finding C44B-R2L-01 (ruling D-84), the reviewer's reproduction: an opener proves the stale lock and is suspended;
/// another program opens that file for reading, sharing deletion (another opener's probe, a scanner, an indexer, a
/// backup), so Windows defers its deletion. The opener renames the proven file aside under its protection, so the name
/// is free at once: it owns the lock, and a second opener is told it is held. Never "not writable", never zero owners.
#[test]
fn a_foreign_reader_of_the_stale_lock_never_leaves_it_without_an_owner() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    stale_lock(&lock_file, "stale");
    let sched = Schedule::install(&[&lock_file, &takeover]);
    sched.arm("A", Boundary::TakeoverProven);
    let a = participant("A", &project);
    sched.wait_held("A", Boundary::TakeoverProven);
    let reader = foreign_reader(&lock_file, true);
    sched.resume("A", Boundary::TakeoverProven);
    let a = a.join().unwrap();
    let b = lock::acquire(&project);
    drop(reader);
    let outcomes = [a, b];
    let owned = owners(&outcomes);
    assert_eq!(owned.len(), 1, "exactly one owner: {outcomes:?}");
    assert!(matches!(outcomes[0], Locked::Acquired { .. }), "the opener owns the lock: {outcomes:?}");
    assert!(!outcomes.iter().any(|o| matches!(o, Locked::NotWritable(_))), "contention is not unwritability");
    assert!(owned[0].held());
    drop(outcomes);
    drop(sched);
    // The renamed file went when the reader closed (Windows) or was unlinked (elsewhere): only the project is left.
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|e| e.file_name()).collect();
    assert_eq!(left.len(), 1, "nothing left beside the project: {left:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Windows (ruling D-84): a foreign reader that does not share deletion makes the proof's open fail. Nobody protects
/// the lock, so the opener is not told it is held: it decides again (it reaches its decision a second time) and owns
/// the lock once the reader closes.
#[cfg(windows)]
#[test]
fn a_reader_refusing_deletion_is_contention_not_a_holder() {
    let _s = serial();
    let (dir, project) = project();
    let lock_file = lock::lock_path(&project);
    let takeover = takeover_of(&lock_file);
    stale_lock(&lock_file, "stale");
    let reader = foreign_reader(&lock_file, false);
    let sched = Schedule::install(&[&lock_file, &takeover]);
    sched.arm("A", Boundary::TakeoverDecided);
    let a = participant("A", &project);
    sched.wait_held("A", Boundary::TakeoverDecided);
    sched.arm("A", Boundary::TakeoverDecided);
    sched.resume("A", Boundary::TakeoverDecided);
    sched.wait_held("A", Boundary::TakeoverDecided);
    assert!(!a.is_finished(), "the opener decides again instead of answering");
    drop(reader);
    sched.resume("A", Boundary::TakeoverDecided);
    let outcome = a.join().unwrap();
    let Locked::Acquired { handle, took_over } = &outcome else { panic!("the opener owns the lock: {outcome:?}") };
    assert!(handle.held());
    assert_eq!(took_over.as_ref().map(|i| i.lock_id.as_str()), Some("stale"));
    drop(outcome);
    drop(sched);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ruling D-84: a project is reported not writable only when its folder really cannot be written (Unix: a folder
/// without write permission; Windows has no such folder attribute, and its pending deletion is covered above).
#[cfg(unix)]
#[test]
fn only_a_folder_that_cannot_be_written_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;
    let _s = serial();
    lock::hooks::set(None);
    let (dir, project) = project();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(dir.join("root-check"), b"").is_ok() {
        // Permissions are not enforced for this user (root): nothing to observe.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let outcome = lock::acquire(&project);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(outcome, Locked::NotWritable(_)), "a read-only folder: {outcome:?}");
    let outcome = lock::acquire(&project);
    assert!(matches!(outcome, Locked::Acquired { .. }), "the same folder, writable again: {outcome:?}");
    drop(outcome);
    let _ = std::fs::remove_dir_all(&dir);
}

/// xorshift64: a fixed, reproducible set of contenders and start delays. The delays vary who meets whom; they order
/// nothing the assertions depend on (exactly one owner, its handle valid, for any interleaving).
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
    // The seed, printed in every assertion message; a loop of runs sets another per run (`TT_LOCK_SEED`).
    let start: u64 = std::env::var("TT_LOCK_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0x9E37_79B9_7F4A_7C15);
    let mut seed = start.max(1);
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
        assert_eq!(owned.len(), 1, "seed {start} trial {trial}: exactly one owner: {outcomes:?}");
        assert!(owned[0].held(), "seed {start} trial {trial}: the owner's handle stays valid after all returned");
        drop(outcomes);
        assert!(!lock_file.exists(), "seed {start} trial {trial}: the owner's release removed its lock");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
