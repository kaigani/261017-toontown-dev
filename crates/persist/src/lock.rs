//! Lock files (10 §8.6, LOCK-01…LOCK-03; D13): opening a project for writing creates `<package>.lock` next to a package
//! or `.lock` inside a folder-form project, holding `{user, host, processId, appVersion, openedAt, heartbeatAt,
//! lockId}`. The holder refreshes `heartbeatAt` (every 60 s) and removes the file on close. A live lock held by someone
//! else makes the open read-only (`project.lockedByOther`); a lock whose heartbeat is older than 5 minutes, or whose
//! process is known dead on the same host, is stale and is taken over with a warning.
//!
//! `lockId` is a random token: two executors in one process (tests, embedding hosts) are still "someone else" to each
//! other. Every change of a lock file (publishing, taking over, heartbeat, release) happens under the lock's arbiter
//! `<lock>.takeover`, created exclusively; an opener reads its publication back before it reports ownership, and a
//! heartbeat fails closed (ADR-0014 §11; C13-R1-02, C13-R3-01).
//!
//! **Operating-system ownership (C1.4 obligations 1–2, ADR-0014 §11.4).** The holder keeps its lock file, and every
//! arbiter keeps its arbitration file, open for its whole life: on Windows a handle with write access (shared for
//! reading, writing and deletion, so other sessions can still read the lock, and a foreign replacement is still caught
//! by the token checks), elsewhere an exclusive `flock`. The kernel releases it when the process ends, however it
//! ends. A [`probe`] (Windows: an open that refuses to share writing, which fails while any write handle is open;
//! elsewhere: a shared `flock`) therefore proves a live holder without trusting process ids (reuse), clocks (jumps,
//! future modification times) or heartbeats (a paused holder): a protected lock whose probe answers "held" is never
//! stale and never removed by a taker. A protected lock on this host whose probe
//! answers "free" was left by a dead process and is taken over at once. Heartbeat age and process ids remain the
//! rule only where the probe cannot answer (another host on a file system that does not carry locks, or a lock written
//! without protection). The lock file is written in place through the protected handle (a rename would replace the
//! protected file); a reader that catches it mid-write sees an unreadable lock, which counts as held.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::ModelError;

use crate::{codes, fsx, host, time};

/// Heartbeat interval (LOCK-01).
pub const HEARTBEAT_SECS: u64 = 60;
/// Staleness (LOCK-02, D13).
pub const STALE_SECS: u64 = 300;
/// The key that marks a lock file as protected by its holder's open handle (an addition to 10 §8.6's fields; readers
/// that do not know it ignore it).
pub const PROTECTION_KEY: &str = "osProtection";

/// The content of a lock file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockInfo {
    /// User name.
    pub user: String,
    /// Host name.
    pub host: String,
    /// Process ID.
    pub process_id: u32,
    /// Application version.
    pub app_version: String,
    /// When the lock was taken.
    pub opened_at: String,
    /// Last refresh.
    pub heartbeat_at: String,
    /// The holder's token.
    #[serde(default)]
    pub lock_id: String,
}

impl LockInfo {
    fn new() -> Self {
        let now = time::now_iso();
        Self {
            user: host::user(),
            host: host::host(),
            process_id: host::pid(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            opened_at: now.clone(),
            heartbeat_at: now,
            lock_id: fsx::random_token(16),
        }
    }

    /// Whether the lock is stale by the heartbeat rule: heartbeat older than [`STALE_SECS`], or its process known
    /// dead on the same host. Openers ask [`probe`] first: a lock its holder still protects is never stale.
    #[must_use]
    pub fn stale(&self) -> bool {
        let age = time::ms_from_iso(&self.heartbeat_at).map_or(i64::MAX, |t| time::now_ms().saturating_sub(t));
        #[allow(clippy::cast_possible_wrap, reason = "constant")]
        if age > (STALE_SECS as i64) * 1000 {
            return true;
        }
        self.host == host::host() && host::process_alive(self.process_id) == Some(false)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, ModelError> {
        let mut v = serde_json::to_value(self).map_err(|e| ModelError::new(codes::INTERNAL, e.to_string()))?;
        if let Some(m) = v.as_object_mut() {
            m.insert(PROTECTION_KEY.into(), json!(PROTECTION));
        }
        serde_json::to_vec_pretty(&v).map_err(|e| ModelError::new(codes::INTERNAL, e.to_string()))
    }
}

/// How this build protects its lock files.
const PROTECTION: &str = if cfg!(windows) { "shareMode" } else { "flock" };

/// Boundaries of the lock protocol where a test can suspend a publisher, a heartbeat, a release or a takeover
/// (C1.4 obligation 1). Hooks exist only with the `test-hooks` feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// The arbiter was taken (any operation).
    ArbiterTaken,
    /// An arbiter verified its file is still its own, before removing it with its protection still held (ruling D-77).
    ArbiterReleasing,
    /// A contender proved an arbitration file abandoned under the file's own protection, before removing it (ruling
    /// D-77).
    ArbiterAbandonedProven,
    /// A file was created exclusively and is not yet protected (ruling D-83 item 1; the path is the created file's:
    /// the arbitration file or the lock file).
    CreatedUnprotected,
    /// One attempt at the arbiter ended without it (the arbitration file existed, was being removed, or the created
    /// file was lost before it was protected; ruling D-83 item 2: a contender's attempt is observable).
    ArbiterNotTaken,
    /// An opener created its lock file, before writing its content.
    PublishCreated,
    /// An opener wrote its lock file, before reading it back.
    PublishWritten,
    /// A taker decided the lock is stale, before proving it may remove it (the decision, not yet an act).
    TakeoverDecided,
    /// A taker holds the stale lock file's own protection, before re-validating it and removing it (ruling D-78).
    TakeoverProven,
    /// A heartbeat verified ownership, before writing.
    HeartbeatChecked,
    /// A heartbeat wrote, before releasing the arbiter.
    HeartbeatWritten,
    /// A release verified ownership, before removing the file.
    ReleaseChecked,
    /// A save of the project at this lock's location, before it writes (C14-R1-08; the save checks ownership after).
    SaveBeforeWrite,
    /// A save wrote the project, before it is applied to the session (the finish checks ownership again).
    SaveWritten,
    /// A range export's rollback or recovery, before it puts an original back from its backup (the path is the
    /// target; M3 C3.3 round 2).
    RangeRestore,
    /// A range export's publication, before it renames a staged file onto its target (the path is the target; M3
    /// C3.3 round 2).
    RangePublish,
    /// An invocation or batch step committed on the document thread, before the next boundary (the path is empty; M3
    /// C3.4 round 2: request cancellation between steps).
    Committed,
    /// An engine's recovery write checked its fence under the ownership lock, before it writes (the path is the
    /// recovery entry's folder; M4 C4.1 round 2, ruling O-18).
    RecoveryWriteChecked,
    /// An MCP server-side work thread (`list_commands`, `describe_command`, `get_docs`) started, before its work (the
    /// path is empty; M4 C4.1 closure, ruling O-20: holding such threads at their cap).
    ServerWork,
}

#[cfg(feature = "test-hooks")]
#[doc(hidden)]
pub mod hooks {
    //! Test-only suspension points (`test-hooks` feature).
    use std::path::Path;
    use std::sync::{Arc, Mutex, PoisonError};

    use super::Boundary;

    type Hook = Arc<dyn Fn(Boundary, &Path) + Send + Sync>;
    static HOOK: Mutex<Option<Hook>> = Mutex::new(None);

    /// Installs (or clears) the hook called at every [`Boundary`] with the lock file's path.
    pub fn set(hook: Option<Hook>) {
        *HOOK.lock().unwrap_or_else(PoisonError::into_inner) = hook;
    }

    pub(super) fn at(b: Boundary, path: &Path) {
        let hook = HOOK.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(h) = hook {
            h(b, path);
        }
    }
}

/// A suspension point of the protocol, also used by the executor's save (C14-R1-08). It does nothing unless the
/// `test-hooks` feature is enabled, which only test builds do (C14-R1-10).
#[inline]
pub fn boundary(b: Boundary, path: &Path) {
    #[cfg(feature = "test-hooks")]
    hooks::at(b, path);
    #[cfg(not(feature = "test-hooks"))]
    let _ = (b, path);
}

/// Creates a file exclusively and protects it for as long as the returned handle lives.
fn create_protected(path: &Path) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE: the open write handle is the protection (see
        // `probe`); others may still read, replace or remove the file, which the token checks catch.
        o.share_mode(0x0000_0007);
    }
    let f = o.open(path)?;
    boundary(Boundary::CreatedUnprotected, path);
    // Elsewhere than Windows the protection is a `flock` taken after the creation (design note
    // `reviews/M4-design-lock-takeover.md`, step C1). A conflicting lock is a `probe`'s momentary shared lock, waited
    // out, or a prover's: a contender that found this file older than TAKEOVER_STALE while its creator was suspended.
    #[cfg(not(windows))]
    {
        let mut attempt = 0;
        loop {
            match f.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) if attempt < 200 => {
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    // Another opener holds a lock on it (a prover). Removing the file by name now, without its
                    // protection, could land on a successor's file once the prover has removed this one (ruling D-83
                    // item 1): it is left, unprotected and empty, for a proof to remove once it is stale.
                    drop(f);
                    return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "locked by another holder"));
                }
                Err(std::fs::TryLockError::Error(e)) => {
                    // A file system that cannot lock: nobody can hold this file's protection, so the M1 rule stays (its
                    // own just-created file, removed while the path still names it).
                    remove_own(&f, path);
                    drop(f);
                    return Err(e);
                }
            }
        }
        // C2 (ruling D-83 item 1): under the protection, the path must still name the very file opened. A prover may
        // have proven it abandoned and removed it between its creation and the lock; this handle then protects a file
        // no path names and confers nothing. That is contention: the caller decides again from the start.
        if !names(&f, path) {
            drop(f);
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "the created file was removed before it was protected",
            ));
        }
    }
    Ok(f)
}

/// Whether `path` still names the very file `held` is open on (ruling D-83 item 1: a handle is trusted only while
/// the path still names it). Elsewhere than Windows: the same device and inode. On Windows a protected file is
/// protected from its creation (the creating handle can write, and a proof's open is refused while any handle can
/// write), so no contender can remove or replace it while the handle is open: the handle names it.
fn names(held: &File, path: &Path) -> bool {
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::MetadataExt;
        match (held.metadata(), fs::metadata(path)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(windows)]
    {
        let _ = (held, path);
        true
    }
}

/// Whether a live process protects the file: `Some(true)` held, `Some(false)` free (nobody alive protects it, as seen
/// from this host), `None` unknown (absent, or the file system cannot tell).
#[must_use]
pub fn probe(path: &Path) -> Option<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Read access, sharing reading and deletion but not writing: refused while any handle can write the file.
        match OpenOptions::new().read(true).share_mode(0x0000_0001 | 0x0000_0004).open(path) {
            Ok(_) => Some(false),
            // ERROR_SHARING_VIOLATION
            Err(e) if e.raw_os_error() == Some(32) => Some(true),
            Err(_) => None,
        }
    }
    #[cfg(not(windows))]
    {
        let f = File::open(path).ok()?;
        match f.try_lock_shared() {
            Ok(()) => {
                let _ = f.unlock();
                Some(false)
            }
            Err(std::fs::TryLockError::WouldBlock) => Some(true),
            Err(std::fs::TryLockError::Error(_)) => None,
        }
    }
}

/// The age of a file's modification time, in either direction (a modification time in the future counts by its
/// distance from now, so a clock jump cannot make a file look young forever).
fn age(path: &Path) -> Option<Duration> {
    let m = fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(match m.elapsed() {
        Ok(d) => d,
        Err(e) => e.duration(),
    })
}

/// The lock file of a project path.
#[must_use]
pub fn lock_path(project: &Path) -> PathBuf {
    if project.is_dir() {
        project.join(crate::layout::FOLDER_LOCK)
    } else {
        let mut s = project.as_os_str().to_owned();
        s.push(".lock");
        PathBuf::from(s)
    }
}

/// A held lock; dropping it removes the file (when it is still ours).
#[derive(Debug)]
pub struct LockHandle {
    path: PathBuf,
    info: Mutex<LockInfo>,
    /// The protected handle (released on drop, or when ownership is found lost).
    file: Mutex<Option<File>>,
}

impl LockHandle {
    /// The lock file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The lock's content.
    #[must_use]
    pub fn info(&self) -> LockInfo {
        self.info.lock().map(|i| i.clone()).unwrap_or_else(|p| p.into_inner().clone())
    }

    /// Whether the lock file still holds this lock (not removed, not taken over).
    #[must_use]
    pub fn held(&self) -> bool {
        let ours = self.info.lock().map(|i| i.lock_id.clone()).unwrap_or_default();
        self.file.lock().is_ok_and(|f| f.is_some()) && read(&self.path).is_some_and(|i| i.lock_id == ours)
    }

    /// Refreshes the heartbeat (LOCK-01), under the lock's arbiter (C13-R3-01). It fails closed: when the lock file is
    /// missing, unreadable or holds another token, nothing is written and the error tells the caller this session no
    /// longer owns the project (maintenance then continues read-only). A heartbeat never recreates or replaces
    /// another claimant's publication. When a takeover holds the arbiter, this heartbeat is skipped (the next one
    /// sees the outcome). After writing, the publication is read back: a heartbeat that resumed after losing the
    /// arbiter and the lock (a holder paused past every bound on a file system without protection) finds another
    /// token and fails closed too.
    ///
    /// # Errors
    /// `project.lockedByOther` when the lock is no longer this session's; `fs.ioError` when it cannot be written.
    pub fn refresh(&self) -> Result<(), ModelError> {
        let mut info = self.info.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(arbiter) = Arbiter::try_take(&self.path) else { return Ok(()) };
        let lost = |current: Option<LockInfo>| {
            ModelError::with_details(
                codes::LOCKED_BY_OTHER,
                "this session no longer holds the project's lock (taken over, removed or unreadable)",
                json!({ "lock": current }),
            )
        };
        match read(&self.path) {
            Some(current) if current.lock_id == info.lock_id => {}
            current => {
                self.release_handle();
                return Err(lost(current));
            }
        }
        boundary(Boundary::HeartbeatChecked, &self.path);
        let mut next = info.clone();
        next.heartbeat_at = time::now_iso();
        let text = next.to_bytes()?;
        let mut guard = self.file.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(f) = guard.as_mut() else {
            return Err(lost(read(&self.path)));
        };
        write_in_place(f, &text).map_err(|e| fsx::fs_error(&e, &self.path, "cannot refresh the lock"))?;
        drop(guard);
        boundary(Boundary::HeartbeatWritten, &self.path);
        let still = arbiter.still_ours();
        drop(arbiter);
        match read(&self.path) {
            Some(back) if back.lock_id == info.lock_id && still => {
                *info = next;
                Ok(())
            }
            back => {
                self.release_handle();
                Err(lost(back))
            }
        }
    }

    fn release_handle(&self) {
        let _ = self.file.lock().map(|mut f| f.take());
    }
}

fn write_in_place(f: &mut File, text: &[u8]) -> std::io::Result<()> {
    f.seek(SeekFrom::Start(0))?;
    f.write_all(text)?;
    f.set_len(text.len() as u64)?;
    f.sync_all()
}

impl Drop for LockHandle {
    fn drop(&mut self) {
        let ours = self.info.lock().map(|i| i.lock_id.clone()).unwrap_or_default();
        // Removed only while it is still ours, under the arbiter (never someone else's publication), and while its
        // protected handle is still open (ruling D-78: an own removal; the handle shares deletion on Windows), which is
        // closed only after.
        let arbiter = Arbiter::take(&self.path, 200);
        let mine = arbiter.is_some() && read(&self.path).is_some_and(|i| i.lock_id == ours);
        if mine {
            boundary(Boundary::ReleaseChecked, &self.path);
            let guard = self.file.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(f) = guard.as_ref() {
                remove_own(f, &self.path);
            }
        }
        self.release_handle();
        drop(arbiter);
    }
}

/// Reads a lock file (`None` when absent or unreadable).
#[must_use]
pub fn read(path: &Path) -> Option<LockInfo> {
    read_raw(path).map(|(i, _)| i)
}

/// A lock file and whether its holder protects it.
fn read_raw(path: &Path) -> Option<(LockInfo, bool)> {
    let b = fsx::read_limited(path, 64 * 1024).ok()?;
    let v: Value = serde_json::from_slice(&b).ok()?;
    let protected = v.get(PROTECTION_KEY).and_then(Value::as_str).is_some();
    Some((serde_json::from_value(v).ok()?, protected))
}

/// The outcome of trying to lock a project.
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "one value per project open; boxing the handle would change the API callers and tests match on"
)]
pub enum Locked {
    /// We hold the lock; `took_over` names a stale lock that was replaced.
    Acquired {
        /// The handle.
        handle: LockHandle,
        /// A stale lock that was taken over.
        took_over: Option<LockInfo>,
    },
    /// Someone else holds a live lock: open read-only.
    HeldByOther(LockInfo),
    /// The location is not writable: open read-only (RO-01).
    NotWritable(String),
}

/// How long an unprotected, empty arbitration file (or an unreadable, unprotected lock) may live before it counts as
/// left behind by a crashed process. A protected one is never removed while its holder lives; an unprotected one whose
/// holder provably died goes at once (ADR-0014 §11.4).
pub const TAKEOVER_STALE: Duration = Duration::from_secs(10);

fn takeover_path(lock: &Path) -> PathBuf {
    let mut s = lock.as_os_str().to_owned();
    s.push(".takeover");
    PathBuf::from(s)
}

/// The lock's arbitration file (`<lock>.takeover`, created exclusively and protected while held): every change of the
/// lock file — publishing, taking over, heartbeat, release — happens while holding it (C13-R1-02, C13-R3-01).
struct Arbiter {
    /// The lock file this arbiter serves (the path the boundaries report).
    lock: PathBuf,
    path: PathBuf,
    token: String,
    file: Option<File>,
}

impl Arbiter {
    /// One attempt. An arbitration file its holder left behind is removed only by [`remove_if_abandoned`], which proves
    /// it abandoned while holding the file's own protection; a protected one is left alone however old it looks.
    fn try_take(lock: &Path) -> Option<Self> {
        Self::attempt(lock).ok()
    }

    /// One attempt that says why it failed: `Err(None)` contention (the arbitration file exists, is being removed, or
    /// the created file was lost before it was protected), `Err(Some(e))` another error creating it, which the caller
    /// classifies ([`contention`]: a pending deletion is contention, ruling D-84).
    fn attempt(lock: &Path) -> Result<Self, Option<std::io::Error>> {
        let path = takeover_path(lock);
        match create_protected(&path) {
            Ok(mut f) => {
                let token = fsx::random_token(8);
                let _ = f.write_all(token.as_bytes()).and_then(|()| f.flush());
                boundary(Boundary::ArbiterTaken, lock);
                Ok(Self { lock: lock.to_owned(), path, token, file: Some(f) })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists || e.raw_os_error() == Some(32) => {
                // A2 (design note `reviews/M4-design-lock-takeover.md`): a left-behind arbitration file is removed only
                // under its own protection, re-validated after the pause point: its token is written (a holder writes
                // it only after taking its protection) or it is older than TAKEOVER_STALE (a publisher between
                // creating and protecting it is younger). On Windows any file no live holder protects is abandoned.
                let valid = |b: Option<&[u8]>| {
                    cfg!(windows) || b.is_some_and(|b| !b.is_empty()) || age(&path).is_some_and(|a| a > TAKEOVER_STALE)
                };
                // Not renamed aside (ruling D-84): several contenders can hold proofs of one arbitration file at once,
                // and a rename is by name, so a late one could move a successor's file. Its pending deletion is
                // contention instead, re-decided by the caller within its bound.
                let proof = remove_proven(&path, lock, Boundary::ArbiterAbandonedProven, false, false, valid);
                if proof == Removal::Unsupported && age(&path).is_some_and(|a| a > TAKEOVER_STALE) {
                    // A file system that cannot lock: protection cannot be proven, the age rule of M1 stays.
                    let _ = fs::remove_file(&path);
                }
                boundary(Boundary::ArbiterNotTaken, lock);
                Err(None)
            }
            Err(e) => {
                boundary(Boundary::ArbiterNotTaken, lock);
                Err(Some(e))
            }
        }
    }

    /// Up to `attempts` attempts, 5 ms apart.
    fn take(lock: &Path, attempts: u32) -> Option<Self> {
        for _ in 0..attempts {
            if let Some(a) = Self::try_take(lock) {
                return Some(a);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    /// Whether the arbitration file is still this arbiter's (it was not removed as abandoned meanwhile): the path still
    /// names the protected file (ruling D-83 item 1) and holds this arbiter's token.
    fn still_ours(&self) -> bool {
        self.file.as_ref().is_some_and(|f| names(f, &self.path))
            && fsx::read_limited(&self.path, 64).is_ok_and(|b| b == self.token.as_bytes())
    }
}

impl Drop for Arbiter {
    fn drop(&mut self) {
        // Ruling D-77: the file is removed while its protection is still held (unlinked before the `flock` is released;
        // on Windows deleted while the handle, which shares deletion, is still open), and only then closed. No other
        // contender can therefore see a live arbiter's file unprotected, judge it abandoned and remove it, and no
        // removal by name of this file can land on a successor's file (handoff C4.4b section 6: the "0 owners"
        // interleaving).
        if self.still_ours() {
            boundary(Boundary::ArbiterReleasing, &self.lock);
            if let Some(f) = self.file.as_ref() {
                remove_own(f, &self.path);
            }
        }
        drop(self.file.take());
    }
}

/// What a removal by a non-owner found (ruling D-78; design note `reviews/M4-design-lock-takeover.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Removal {
    /// The file was the one decided about, and it is removed.
    Removed,
    /// A live owner (or a probe, or another prover) protects it: nothing is removed.
    Protected,
    /// The path names another file now, or its content no longer matches the decision: the caller re-decides.
    Changed,
    /// No file at the path.
    Gone,
    /// The file system cannot lock (Unix `flock` refused with an error; Windows refusing the sharing semantics):
    /// protection cannot be proven.
    Unsupported,
}

/// A proof of removal (ruling D-78): the contender takes the file's own protection, which a live owner refuses; after
/// the pause point `at`, it re-validates under that protection that the file is the very one its decision was about
/// (`valid` reads the bytes read through the opened file, `None` when unreadable), and removes exactly that file.
///
/// - **Windows:** an open with read and delete access that shares reading and deletion but not writing fails while any
///   handle can write the file (a live owner protects its file from the moment it exists); a delete-pending file
///   refuses the open too (contention). `FILE_FLAG_DELETE_ON_CLOSE` deletes the file this handle opened when it
///   closes, never a file created at the path later (none can be while it exists). A file that fails `valid` is still
///   one no live owner holds, so it is dead either way; the caller re-decides.
///   With `aside` (ruling D-84; only for a file one prover alone can hold: the lock file, proven under the arbiter),
///   the proven file is first renamed to a unique name beside it while this handle protects it, so its name is free at
///   once even when another program's handle that shares deletion (another opener's probe, a scanner, an indexer, a
///   backup) defers the deletion; the delete-on-close removes it under its new name when the last handle closes.
/// - **Elsewhere:** an exclusive, non-blocking `flock` on the opened file, refused while its owner, a probe or another
///   prover holds a lock on it (with `patient`, a probe's momentary shared lock is waited out for up to 200 ms). After
///   the pause point, the opened file's device and inode must equal the path's and its bytes satisfy `valid`; the path
///   is then unlinked while the `flock` is held, and closed after. Nobody can replace the path in between: that needs
///   its removal, and every removal needs this `flock` or is its owner's own. An unlink frees the name at once.
fn remove_proven(
    path: &Path,
    lock: &Path,
    at: Boundary,
    patient: bool,
    aside: bool,
    valid: impl Fn(Option<&[u8]>) -> bool,
) -> Removal {
    proven_removal(path, lock, at, patient, aside, &valid)
}

/// The bytes of an opened file (at most 64 KiB), read through that handle.
fn bytes_of(f: &File) -> Option<Vec<u8>> {
    let mut b = Vec::new();
    f.take(64 * 1024).read_to_end(&mut b).ok().map(|_| b)
}

#[cfg(windows)]
fn proven_removal(
    path: &Path,
    lock: &Path,
    at: Boundary,
    _patient: bool,
    aside: bool,
    valid: &dyn Fn(Option<&[u8]>) -> bool,
) -> Removal {
    use std::os::windows::fs::OpenOptionsExt;
    const GENERIC_READ: u32 = 0x8000_0000;
    const DELETE: u32 = 0x0001_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
    let f = match OpenOptions::new()
        .access_mode(GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Removal::Gone,
        // ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED: a file system without these sharing semantics.
        Err(e) if matches!(e.raw_os_error(), Some(1 | 50)) => return Removal::Unsupported,
        // ERROR_SHARING_VIOLATION (a live owner), ERROR_ACCESS_DENIED (delete pending): contention.
        Err(_) => return Removal::Protected,
    };
    boundary(at, lock);
    let ok = valid(bytes_of(&f).as_deref());
    if ok && aside {
        // The name is freed at once (ruling D-84). A failed rename leaves the name pending, which the caller treats as
        // contention.
        let mut s = path.as_os_str().to_owned();
        s.push(format!(".removed-{}", fsx::random_token(6)));
        let _ = fs::rename(path, PathBuf::from(s));
    }
    // Closing the handle deletes the file it opened (wherever it now is, once the last handle closes).
    drop(f);
    if ok { Removal::Removed } else { Removal::Changed }
}

#[cfg(not(windows))]
fn proven_removal(
    path: &Path,
    lock: &Path,
    at: Boundary,
    patient: bool,
    _aside: bool,
    valid: &dyn Fn(Option<&[u8]>) -> bool,
) -> Removal {
    use std::os::unix::fs::MetadataExt;
    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Removal::Gone,
        Err(_) => return Removal::Protected,
    };
    // A `probe`'s shared lock is momentary: with `patient`, it is waited out (up to 200 ms, as `create_protected`
    // does); a live owner's lock outlasts that.
    let mut attempt = 0;
    loop {
        match f.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if patient && attempt < 200 => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(std::fs::TryLockError::WouldBlock) => return Removal::Protected,
            Err(std::fs::TryLockError::Error(_)) => return Removal::Unsupported,
        }
    }
    boundary(at, lock);
    // Re-validated after the pause point, under the `flock`: the path still names this file, and what it holds is what
    // the decision was about.
    let same = match (f.metadata(), fs::metadata(path)) {
        (Ok(held), Ok(named)) => held.dev() == named.dev() && held.ino() == named.ino(),
        _ => false,
    };
    if !same || !valid(bytes_of(&f).as_deref()) {
        return Removal::Changed;
    }
    let _ = fs::remove_file(path);
    // Closing the file releases the `flock`, after the unlink.
    drop(f);
    Removal::Removed
}

/// An own removal (ruling D-78): the owner removes its own file while `held`, the handle that protects it, is still
/// open (Unix: unlinked while the `flock` is held, and only if the path still names that inode; Windows: deleted while
/// the handle, which shares deletion, is open). The caller has verified its nonce (the arbiter's token, the lock's
/// `lockId`) and closes the handle after.
fn remove_own(held: &File, path: &Path) {
    if names(held, path) {
        let _ = fs::remove_file(path);
    }
}

/// What the lock file says to an opener.
enum State {
    /// No lock file.
    Absent,
    /// A live owner (or a lock that cannot be read yet and is not old enough to be abandoned).
    Held(LockInfo),
    /// A stale lock (LOCK-02), or an unreadable one abandoned; `Some` when it was readable.
    Stale(Option<LockInfo>),
}

fn state(path: &Path) -> State {
    if !path.exists() {
        return State::Absent;
    }
    let probed = probe(path);
    match read_raw(path) {
        // Its holder is alive: whatever the heartbeat and the clocks say (obligation 2).
        Some((c, _)) if probed == Some(true) => State::Held(c),
        // Protected by a holder on this host that no longer protects it: that process ended (no PID reuse question).
        Some((c, true)) if probed == Some(false) && c.host == host::host() => State::Stale(Some(c)),
        Some((c, _)) if c.stale() => State::Stale(Some(c)),
        // A heartbeat in the future (the writer's clock is ahead, or ours jumped back) cannot keep an unprotected lock
        // alive forever: it is stale once the file itself has not changed for STALE_SECS.
        Some((c, _))
            if time::ms_from_iso(&c.heartbeat_at).is_some_and(|t| t > time::now_ms())
                && age(path).is_some_and(|a| a > Duration::from_secs(STALE_SECS)) =>
        {
            State::Stale(Some(c))
        }
        Some((c, _)) => State::Held(c),
        None if probed == Some(true) => State::Held(unreadable_holder(path)),
        // Unreadable and unprotected: a publisher that crashed before writing, once TAKEOVER_STALE has passed (a live
        // publisher protects its file from the moment it exists on Windows, and before writing elsewhere).
        None if probed == Some(false) && age(path).is_some_and(|a| a > TAKEOVER_STALE) => State::Stale(None),
        // Unreadable and unknown: like a lock whose heartbeat is its modification time (C13-R3-01).
        None if age(path).is_some_and(|a| a > Duration::from_secs(STALE_SECS)) => State::Stale(None),
        None => State::Held(unreadable_holder(path)),
    }
}

/// The holder reported for a lock that cannot be read.
fn unreadable_holder(path: &Path) -> LockInfo {
    let at = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .map(time::iso_from_ms)
        .unwrap_or_default();
    LockInfo {
        user: String::new(),
        host: String::new(),
        process_id: 0,
        app_version: String::new(),
        opened_at: at.clone(),
        heartbeat_at: at,
        lock_id: String::new(),
    }
}

/// Whether an error creating a file at `lock`'s location is contention rather than a folder that cannot be written
/// (ruling D-84): an existing file; on Windows a sharing violation or a name whose deletion is pending (reported as
/// access denied); and any permission error while the folder itself can be written.
fn contention(e: &std::io::Error, lock: &Path) -> bool {
    e.kind() == std::io::ErrorKind::AlreadyExists
        || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 303)))
        || (e.kind() == std::io::ErrorKind::PermissionDenied && folder_writable(lock))
}

/// Whether the lock's folder can be written: a uniquely named file is created there and removed (ruling D-84: a
/// project is reported not writable only when its folder really cannot be written).
fn folder_writable(lock: &Path) -> bool {
    let mut s = lock.as_os_str().to_owned();
    s.push(format!(".probe-{}", fsx::random_token(6)));
    let p = PathBuf::from(s);
    match OpenOptions::new().write(true).create_new(true).open(&p) {
        Ok(f) => {
            drop(f);
            let _ = fs::remove_file(&p);
            true
        }
        Err(_) => false,
    }
}

/// How long `acquire` re-decides under contention before it answers from what is true at that moment (ruling D-84):
/// the lock held, or the arbiter held by another live opener, is answered `HeldByOther`; a folder that cannot be
/// written, `NotWritable`; when nothing is held (a removed file's name kept by another program's handle, an arbitration
/// file left empty by a creator that gave up, removable once older than `TAKEOVER_STALE`), it decides again for
/// another round.
const DECIDE_BOUND: Duration = Duration::from_secs(2);

/// Tries to lock a project path (LOCK-01, LOCK-02; ADR-0014 §11).
///
/// Publication and takeover happen only while holding the lock's arbiter (`<lock>.takeover`): under it, the lock's
/// state is read again; a stale lock is removed; the new lock is created exclusively and protected, written and
/// flushed; and the file is read back. `Acquired` is returned only when the path still names the protected file, it
/// holds this session's token and the arbiter was not lost meanwhile (C13-R3-01, D-83). Contention (an arbitration
/// file held or being removed, a name whose deletion is pending, a publication that could not be confirmed) is decided
/// again from the start (ruling D-84). `HeldByOther` is answered only when it is true at that moment (a live owner
/// protects the lock, or a lock the protocol counts as held, or another opener holds the arbiter past the bound), and
/// `NotWritable` only when the folder cannot be written or the lock cannot be written through its own handle.
#[must_use]
pub fn acquire(project: &Path) -> Locked {
    let path = lock_path(project);
    let info = LockInfo::new();
    let text = match info.to_bytes() {
        Ok(t) => t,
        Err(e) => return Locked::NotWritable(e.to_string()),
    };
    let pause = || std::thread::sleep(Duration::from_millis(5));
    loop {
        let deadline = std::time::Instant::now() + DECIDE_BOUND;
        while std::time::Instant::now() < deadline {
            if let State::Held(c) = state(&path) {
                return Locked::HeldByOther(c);
            }
            let arbiter = match Arbiter::attempt(&path) {
                Ok(a) => a,
                Err(Some(e)) if !contention(&e, &path) => return Locked::NotWritable(e.to_string()),
                Err(_) => {
                    pause();
                    continue;
                }
            };
            let took_over = match state(&path) {
                State::Held(c) => return Locked::HeldByOther(c),
                State::Absent => None,
                State::Stale(c) => {
                    boundary(Boundary::TakeoverDecided, &path);
                    // A4 (design note `reviews/M4-design-lock-takeover.md`): the decision above is acted on only
                    // through a proof of removal, re-validated after its pause point: the lock file is the one decided
                    // about (its `lockId`, or still unreadable for an abandoned unreadable lock) and no live owner
                    // protects it. Proven under the arbiter, it is renamed aside on Windows (D-84).
                    let decided = c.as_ref().map(|i| i.lock_id.clone());
                    let valid = |b: Option<&[u8]>| {
                        let now = b.and_then(|b| serde_json::from_slice::<LockInfo>(b).ok());
                        match &decided {
                            Some(id) => now.is_some_and(|i| &i.lock_id == id),
                            None => now.is_none(),
                        }
                    };
                    match remove_proven(&path, &path, Boundary::TakeoverProven, true, true, valid) {
                        Removal::Removed | Removal::Gone => {}
                        // Refused. Held only if a live owner protects it now (I2: it came back, or a lock was published
                        // meanwhile); a foreign handle that refuses sharing deletion is contention (D-84).
                        Removal::Protected => {
                            if probe(&path) == Some(true) {
                                return Locked::HeldByOther(read(&path).unwrap_or_else(|| unreadable_holder(&path)));
                            }
                            drop(arbiter);
                            pause();
                            continue;
                        }
                        // Another file, or another content: re-decide from the start.
                        Removal::Changed => {
                            drop(arbiter);
                            continue;
                        }
                        // A file system that cannot lock: the M1 rule, under the arbiter.
                        Removal::Unsupported => {
                            if let Err(e) = fs::remove_file(&path)
                                && e.kind() != std::io::ErrorKind::NotFound
                                && !contention(&e, &path)
                            {
                                return Locked::NotWritable(format!("cannot replace the stale lock: {e}"));
                            }
                        }
                    }
                    // This call's own unconfirmed publication (below) is not another's stale lock.
                    c.filter(|c| c.lock_id != info.lock_id)
                }
            };
            let mut file = match create_protected(&path) {
                Ok(f) => f,
                Err(e) if contention(&e, &path) => {
                    drop(arbiter);
                    pause();
                    continue;
                }
                Err(e) => return Locked::NotWritable(e.to_string()),
            };
            boundary(Boundary::PublishCreated, &path);
            if let Err(e) = write_in_place(&mut file, &text) {
                remove_own(&file, &path);
                drop(file);
                return Locked::NotWritable(e.to_string());
            }
            boundary(Boundary::PublishWritten, &path);
            // Validate the publication before announcing ownership: the path still names the protected file (ruling
            // D-83 item 1), it holds this session's token, and the arbiter was not lost meanwhile.
            let back = if names(&file, &path) { read(&path) } else { None };
            let still = arbiter.still_ours();
            drop(arbiter);
            if still && back.is_some_and(|b| b.lock_id == info.lock_id) {
                return Locked::Acquired {
                    handle: LockHandle { path, info: Mutex::new(info), file: Mutex::new(Some(file)) },
                    took_over,
                };
            }
            // Not confirmed (ruling D-84: never "held by another" for a lock nobody holds): the publication confers
            // nothing. It is closed, not removed (a removal needs a valid arbiter); a protected publication of this
            // host that nobody protects is stale at once, so the next round takes it over.
            drop(file);
        }
        // The bound passed without a decision: only what is true now is answered.
        match state(&path) {
            State::Held(c) => return Locked::HeldByOther(c),
            // Another opener holds the arbiter: the project is being opened by it.
            _ if probe(&takeover_path(&path)) == Some(true) => {
                return Locked::HeldByOther(read(&path).unwrap_or_else(|| unreadable_holder(&path)));
            }
            _ if !folder_writable(&path) => {
                return Locked::NotWritable("the project's folder cannot be written".to_owned());
            }
            // Nothing is held: decide again for another round.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test")]
    use super::*;

    fn project() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tt-lock-{}", fsx::random_token(4)));
        fs::create_dir_all(&dir).unwrap();
        let project = dir.join("A.animpkg");
        fs::write(&project, b"x").unwrap();
        (dir, project)
    }

    #[test]
    fn an_unreadable_lock_is_held_until_abandoned() {
        let (dir, project) = project();
        // A lock that cannot be read yet (its publisher is writing it, or paused) is held, never taken over.
        let path = lock_path(&project);
        fs::write(&path, b"").unwrap();
        assert!(matches!(acquire(&project), Locked::HeldByOther(_)));
        // Unprotected and unreadable for longer than TAKEOVER_STALE: a publisher that crashed before writing.
        let old = SystemTime::now() - TAKEOVER_STALE - Duration::from_secs(5);
        File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
        let Locked::Acquired { handle, took_over: None } = acquire(&project) else {
            panic!("an abandoned empty lock is stale")
        };
        assert!(handle.held());
        drop(handle);
        assert!(!path.exists());
        // A modification time in the future counts by its distance (a clock jump cannot keep it young).
        fs::write(&path, b"").unwrap();
        let future = SystemTime::now() + Duration::from_secs(3600);
        File::options().write(true).open(&path).unwrap().set_modified(future).unwrap();
        assert!(matches!(acquire(&project), Locked::Acquired { .. }));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_live_lock_is_protected_and_never_stale() {
        let (dir, project) = project();
        let Locked::Acquired { handle, took_over: None } = acquire(&project) else { panic!("first acquire") };
        assert_eq!(probe(handle.path()), Some(true));
        let Locked::HeldByOther(other) = acquire(&project) else { panic!("second acquire must see a live lock") };
        assert_eq!(other.lock_id, handle.info().lock_id);
        handle.refresh().unwrap();
        let (_, protected) = read_raw(handle.path()).unwrap();
        assert!(protected);
        // A heartbeat 26 years old or 74 years ahead (a clock jump, a paused holder) does not make a protected lock
        // stale.
        for at in ["2000-01-01T00:00:00.000Z", "2100-01-01T00:00:00.000Z"] {
            let mut f = handle.file.lock().unwrap();
            let mut jumped = handle.info();
            jumped.heartbeat_at = at.into();
            write_in_place(f.as_mut().unwrap(), &jumped.to_bytes().unwrap()).unwrap();
            drop(f);
            if at.starts_with("2000") {
                assert!(jumped.stale(), "by the heartbeat rule alone it would be stale");
            }
            assert!(matches!(acquire(&project), Locked::HeldByOther(_)), "{at}: the probe proves its holder alive");
        }
        drop(handle);
        assert!(!lock_path(&project).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_protected_lock_left_by_a_dead_holder_is_taken_over_and_its_heartbeat_fails_closed() {
        let (dir, project) = project();
        let Locked::Acquired { handle, .. } = acquire(&project) else { panic!("acquire") };
        let token = handle.info().lock_id;
        // The holder's process "ends": its handle closes, the file stays (a crash leaves it behind).
        handle.release_handle();
        // A `flock` belongs to the open file description, which a process spawned at that instant by another thread
        // (the parallel tests' liveness probes) shares until its close-on-exec runs: the lock is free once that
        // reference is gone, so wait for it (bounded; macOS CI run 41191607130).
        let freed = std::time::Instant::now();
        while probe(handle.path()) != Some(false) && freed.elapsed() < std::time::Duration::from_secs(2) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(probe(handle.path()), Some(false));
        let Locked::Acquired { handle: h2, took_over: Some(old) } = acquire(&project) else {
            panic!("a protected lock of this host that nobody protects is stale at once")
        };
        assert_eq!(old.lock_id, token);
        assert!(handle.refresh().is_err(), "the old holder sees the takeover");
        assert!(!handle.held());
        drop(handle);
        assert!(lock_path(&project).exists(), "the old holder does not remove the new lock");
        assert!(h2.held());
        drop(h2);
        assert!(!lock_path(&project).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unprotected_stale_lock_is_taken_over_and_a_live_unprotected_one_is_not() {
        let (dir, project) = project();
        let path = lock_path(&project);
        let mut info = LockInfo::new();
        info.lock_id = "legacy".into();
        fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();
        assert!(matches!(acquire(&project), Locked::HeldByOther(_)), "fresh heartbeat, same live process");
        info.heartbeat_at = "2000-01-01T00:00:00.000Z".into();
        fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();
        let Locked::Acquired { handle, took_over: Some(old) } = acquire(&project) else { panic!("stale") };
        assert_eq!(old.lock_id, "legacy");
        drop(handle);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_abandoned_arbiter_is_removed_only_when_unprotected() {
        let (dir, project) = project();
        let lock = lock_path(&project);
        let held = Arbiter::try_take(&lock).unwrap();
        let tk = takeover_path(&lock);
        // Protected (a paused arbiter, however old it looks): left alone.
        assert_eq!(probe(&tk), Some(true));
        assert!(Arbiter::try_take(&lock).is_none());
        assert!(tk.exists());
        drop(held);
        assert!(!tk.exists());
        // Unprotected with its token written: its holder died, removed at once (C1.4: no 10 s wait).
        fs::write(&tk, b"token").unwrap();
        assert!(Arbiter::try_take(&lock).is_none());
        assert!(!tk.exists(), "a dead holder's arbiter goes at once");
        // Unprotected and older than TAKEOVER_STALE (also with a future time): removed.
        for t in [SystemTime::now() - TAKEOVER_STALE * 2, SystemTime::now() + Duration::from_secs(3600)] {
            fs::write(&tk, b"x").unwrap();
            File::options().write(true).open(&tk).unwrap().set_modified(t).unwrap();
            assert!(Arbiter::try_take(&lock).is_none());
            assert!(!tk.exists());
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
