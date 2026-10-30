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
use std::io::{Seek, SeekFrom, Write};
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
    /// An opener created its lock file, before writing its content.
    PublishCreated,
    /// An opener wrote its lock file, before reading it back.
    PublishWritten,
    /// A taker decided the lock is stale, before removing it.
    TakeoverDecided,
    /// A heartbeat verified ownership, before writing.
    HeartbeatChecked,
    /// A heartbeat wrote, before releasing the arbiter.
    HeartbeatWritten,
    /// A release verified ownership, before removing the file.
    ReleaseChecked,
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

#[inline]
fn boundary(b: Boundary, path: &Path) {
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
    #[cfg(not(windows))]
    match f.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "locked by another holder"));
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(e),
    }
    Ok(f)
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
        // Removed only while it is still ours, under the arbiter (never someone else's publication). The protected
        // handle is closed first (on Windows the file cannot be removed while it is open without delete sharing).
        let arbiter = Arbiter::take(&self.path, 200);
        let mine = arbiter.is_some() && read(&self.path).is_some_and(|i| i.lock_id == ours);
        if mine {
            boundary(Boundary::ReleaseChecked, &self.path);
        }
        self.release_handle();
        if mine {
            let _ = fs::remove_file(&self.path);
        }
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

/// How long an unprotected arbitration file (or an unreadable, unprotected lock) may live before it counts as left
/// behind by a crashed process. A protected one is never removed while its holder lives (ADR-0014 §11.4).
pub const TAKEOVER_STALE: Duration = Duration::from_secs(10);

fn takeover_path(lock: &Path) -> PathBuf {
    let mut s = lock.as_os_str().to_owned();
    s.push(".takeover");
    PathBuf::from(s)
}

/// The lock's arbitration file (`<lock>.takeover`, created exclusively and protected while held): every change of the
/// lock file — publishing, taking over, heartbeat, release — happens while holding it (C13-R1-02, C13-R3-01).
struct Arbiter {
    path: PathBuf,
    token: String,
    file: Option<File>,
}

impl Arbiter {
    /// One attempt. An arbitration file nobody protects any more and older than [`TAKEOVER_STALE`] is removed (its
    /// holder crashed); a protected one is left alone however old it looks.
    fn try_take(lock: &Path) -> Option<Self> {
        let path = takeover_path(lock);
        match create_protected(&path) {
            Ok(mut f) => {
                let token = fsx::random_token(8);
                let _ = f.write_all(token.as_bytes()).and_then(|()| f.flush());
                boundary(Boundary::ArbiterTaken, lock);
                Some(Self { path, token, file: Some(f) })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists || e.raw_os_error() == Some(32) => {
                if probe(&path) != Some(true) && age(&path).is_some_and(|a| a > TAKEOVER_STALE) {
                    let _ = fs::remove_file(&path);
                }
                None
            }
            Err(_) => None,
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

    /// Whether the arbitration file is still this arbiter's (it was not removed as abandoned meanwhile).
    fn still_ours(&self) -> bool {
        fsx::read_limited(&self.path, 64).is_ok_and(|b| b == self.token.as_bytes())
    }
}

impl Drop for Arbiter {
    fn drop(&mut self) {
        let ours = self.still_ours();
        drop(self.file.take());
        if ours {
            let _ = fs::remove_file(&self.path);
        }
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

/// Tries to lock a project path (LOCK-01, LOCK-02; ADR-0014 §11).
///
/// Publication and takeover happen only while holding the lock's arbiter (`<lock>.takeover`): under it, the lock's
/// state is read again; a stale lock is removed; the new lock is created exclusively and protected, written and
/// flushed; and the file is read back. `Acquired` is returned only when the file holds this session's token and the
/// arbiter was not lost meanwhile (C13-R3-01); otherwise the holder is reported. A lock that cannot be read counts as
/// held until it is abandoned.
#[must_use]
pub fn acquire(project: &Path) -> Locked {
    let path = lock_path(project);
    let info = LockInfo::new();
    let text = match info.to_bytes() {
        Ok(t) => t,
        Err(e) => return Locked::NotWritable(e.to_string()),
    };
    for _ in 0..400 {
        if let State::Held(c) = state(&path) {
            return Locked::HeldByOther(c);
        }
        let Some(arbiter) = Arbiter::try_take(&path) else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        let took_over = match state(&path) {
            State::Held(c) => return Locked::HeldByOther(c),
            State::Absent => None,
            State::Stale(c) => {
                boundary(Boundary::TakeoverDecided, &path);
                if let Err(e) = fs::remove_file(&path)
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    // Still protected (its holder came back between the probe and the removal), or not removable.
                    if probe(&path) == Some(true) {
                        return Locked::HeldByOther(read(&path).unwrap_or_else(|| unreadable_holder(&path)));
                    }
                    return Locked::NotWritable(format!("cannot replace the stale lock: {e}"));
                }
                c
            }
        };
        let mut file = match create_protected(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists || e.raw_os_error() == Some(32) => {
                drop(arbiter);
                continue;
            }
            Err(e) => return Locked::NotWritable(e.to_string()),
        };
        boundary(Boundary::PublishCreated, &path);
        if let Err(e) = write_in_place(&mut file, &text) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Locked::NotWritable(e.to_string());
        }
        boundary(Boundary::PublishWritten, &path);
        // Validate the publication before announcing ownership.
        let back = read(&path);
        let still = arbiter.still_ours();
        drop(arbiter);
        return match back {
            Some(b) if b.lock_id == info.lock_id && still => Locked::Acquired {
                handle: LockHandle { path, info: Mutex::new(info), file: Mutex::new(Some(file)) },
                took_over,
            },
            Some(b) => Locked::HeldByOther(b),
            None => Locked::HeldByOther(unreadable_holder(&path)),
        };
    }
    match state(&path) {
        State::Held(c) | State::Stale(Some(c)) => Locked::HeldByOther(c),
        _ => Locked::NotWritable("the lock could not be taken".to_owned()),
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
