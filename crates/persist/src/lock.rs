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

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_model::ModelError;

use crate::{codes, fsx, host, time};

/// Heartbeat interval (LOCK-01).
pub const HEARTBEAT_SECS: u64 = 60;
/// Staleness (LOCK-02, D13).
pub const STALE_SECS: u64 = 300;

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

    /// Whether the lock is stale: heartbeat older than [`STALE_SECS`], or its process known dead on this host.
    #[must_use]
    pub fn stale(&self) -> bool {
        let age = time::ms_from_iso(&self.heartbeat_at).map_or(i64::MAX, |t| time::now_ms().saturating_sub(t));
        #[allow(clippy::cast_possible_wrap, reason = "constant")]
        if age > (STALE_SECS as i64) * 1000 {
            return true;
        }
        self.host == host::host() && host::process_alive(self.process_id) == Some(false)
    }
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
        read(&self.path).is_some_and(|i| i.lock_id == ours)
    }

    /// Refreshes the heartbeat (LOCK-01), under the lock's arbiter (C13-R3-01). It fails closed: when the lock file is
    /// missing, unreadable or holds another token, nothing is written and the error tells the caller this session no
    /// longer owns the project (maintenance then continues read-only). A heartbeat never recreates or replaces
    /// another claimant's publication. When a takeover holds the arbiter, this heartbeat is skipped (the next one
    /// sees the outcome).
    ///
    /// # Errors
    /// `project.lockedByOther` when the lock is no longer this session's; `fs.ioError` when it cannot be written.
    pub fn refresh(&self) -> Result<(), ModelError> {
        let mut info = self.info.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(_arbiter) = Arbiter::try_take(&self.path) else { return Ok(()) };
        match read(&self.path) {
            Some(current) if current.lock_id == info.lock_id => {}
            current => {
                return Err(ModelError::with_details(
                    codes::LOCKED_BY_OTHER,
                    "this session no longer holds the project's lock (taken over, removed or unreadable)",
                    json!({ "lock": current }),
                ));
            }
        }
        info.heartbeat_at = time::now_iso();
        let text = serde_json::to_vec_pretty(&*info).map_err(|e| ModelError::new(codes::INTERNAL, e.to_string()))?;
        fsx::write_atomic(&self.path, &text, fsx::fs_error)
    }
}

impl Drop for LockHandle {
    fn drop(&mut self) {
        let ours = self.info.lock().map(|i| i.lock_id.clone()).unwrap_or_default();
        // Removed only while it is still ours, under the arbiter (never someone else's publication).
        if let Some(_arbiter) = Arbiter::take(&self.path, 200)
            && read(&self.path).is_some_and(|i| i.lock_id == ours)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Reads a lock file (`None` when absent or unreadable).
#[must_use]
pub fn read(path: &Path) -> Option<LockInfo> {
    let b = fsx::read_limited(path, 64 * 1024).ok()?;
    serde_json::from_slice(&b).ok()
}

/// The outcome of trying to lock a project.
#[derive(Debug)]
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

/// How long an arbitration file may live before it counts as left behind by a crashed process (a paused arbiter
/// beyond this is a C1.4 obligation, ADR-0014 §11).
const TAKEOVER_STALE: std::time::Duration = std::time::Duration::from_secs(10);

fn takeover_path(lock: &Path) -> PathBuf {
    let mut s = lock.as_os_str().to_owned();
    s.push(".takeover");
    PathBuf::from(s)
}

/// The lock's arbitration file (`<lock>.takeover`, created exclusively): every change of the lock file — publishing,
/// taking over, heartbeat, release — happens while holding it (C13-R1-02, C13-R3-01).
struct Arbiter {
    path: PathBuf,
}

impl Arbiter {
    /// One attempt. An arbitration file older than [`TAKEOVER_STALE`] is removed (its holder crashed).
    fn try_take(lock: &Path) -> Option<Self> {
        let path = takeover_path(lock);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Some(Self { path }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let abandoned = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > TAKEOVER_STALE);
                if abandoned {
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
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        None
    }
}

impl Drop for Arbiter {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// The modification age of a file.
fn age(path: &Path) -> Option<std::time::Duration> {
    fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok())
}

/// What the lock file says to an opener.
enum State {
    /// No lock file.
    Absent,
    /// A live owner (or a lock that cannot be read yet and is not old enough to be abandoned).
    Held(LockInfo),
    /// A stale lock (LOCK-02), or an unreadable one abandoned for [`STALE_SECS`]; `Some` when it was readable.
    Stale(Option<LockInfo>),
}

fn state(path: &Path) -> State {
    if !path.exists() {
        return State::Absent;
    }
    match read(path) {
        Some(c) if c.stale() => State::Stale(Some(c)),
        Some(c) => State::Held(c),
        // Unreadable (empty while its publisher writes, or damaged): like a lock whose heartbeat is its modification
        // time, stale only after STALE_SECS (C13-R3-01: a paused live writer is not taken over).
        None => {
            if age(path).is_some_and(|a| a > std::time::Duration::from_secs(STALE_SECS)) {
                State::Stale(None)
            } else {
                State::Held(unreadable_holder(path))
            }
        }
    }
}

/// The holder reported for a lock that cannot be read.
fn unreadable_holder(path: &Path) -> LockInfo {
    let at = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
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
/// state is read again; a stale lock is removed; the new lock is created exclusively, written and flushed; and the
/// file is read back. `Acquired` is returned only when the file holds this session's token (C13-R3-01); otherwise
/// the holder is reported. A lock that cannot be read counts as held until it is [`STALE_SECS`] old.
#[must_use]
pub fn acquire(project: &Path) -> Locked {
    let path = lock_path(project);
    let info = LockInfo::new();
    let text = match serde_json::to_vec_pretty(&info) {
        Ok(t) => t,
        Err(e) => return Locked::NotWritable(e.to_string()),
    };
    for _ in 0..400 {
        if let State::Held(c) = state(&path) {
            return Locked::HeldByOther(c);
        }
        let Some(arbiter) = Arbiter::try_take(&path) else {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        };
        let took_over = match state(&path) {
            State::Held(c) => return Locked::HeldByOther(c),
            State::Absent => None,
            State::Stale(c) => {
                if let Err(e) = fs::remove_file(&path)
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    return Locked::NotWritable(format!("cannot replace the stale lock: {e}"));
                }
                c
            }
        };
        let published = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut f| f.write_all(&text).and_then(|()| f.sync_all()));
        if let Err(e) = published {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                drop(arbiter);
                continue;
            }
            let _ = fs::remove_file(&path);
            return Locked::NotWritable(e.to_string());
        }
        // Validate the publication before announcing ownership.
        let back = read(&path);
        drop(arbiter);
        return match back {
            Some(b) if b.lock_id == info.lock_id => {
                Locked::Acquired { handle: LockHandle { path, info: Mutex::new(info) }, took_over }
            }
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

    #[test]
    fn an_unreadable_lock_is_held_until_abandoned_and_a_heartbeat_fails_closed() {
        let dir = std::env::temp_dir().join(format!("tt-lock-{}", fsx::random_token(4)));
        fs::create_dir_all(&dir).unwrap();
        let project = dir.join("A.animpkg");
        fs::write(&project, b"x").unwrap();
        // A lock that cannot be read yet (its publisher is writing it, or paused) is held, never taken over.
        let path = lock_path(&project);
        fs::write(&path, b"").unwrap();
        assert!(matches!(acquire(&project), Locked::HeldByOther(_)));
        // An unreadable lock untouched for longer than STALE_SECS is abandoned and taken over (LOCK-02 timing).
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(STALE_SECS + 60);
        fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
        let Locked::Acquired { handle, took_over: None } = acquire(&project) else {
            panic!("an abandoned lock is stale")
        };
        assert!(handle.held());
        // Its heartbeat refuses a missing lock and leaves it missing.
        fs::remove_file(&path).unwrap();
        assert!(handle.refresh().is_err());
        assert!(!path.exists(), "a heartbeat never recreates the lock");
        drop(handle);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn exclusive_live_and_stale() {
        let dir = std::env::temp_dir().join(format!("tt-lock-{}", fsx::random_token(4)));
        fs::create_dir_all(&dir).unwrap();
        let project = dir.join("A.animpkg");
        fs::write(&project, b"x").unwrap();
        let Locked::Acquired { handle, took_over: None } = acquire(&project) else { panic!("first acquire") };
        let Locked::HeldByOther(other) = acquire(&project) else { panic!("second acquire must see a live lock") };
        assert_eq!(other.lock_id, handle.info().lock_id);
        handle.refresh().unwrap();
        // Make it stale.
        let mut info = handle.info();
        info.heartbeat_at = "2000-01-01T00:00:00.000Z".into();
        fs::write(lock_path(&project), serde_json::to_vec(&info).unwrap()).unwrap();
        let Locked::Acquired { handle: h2, took_over: Some(old) } = acquire(&project) else { panic!("take over") };
        assert_eq!(old.lock_id, info.lock_id);
        assert!(handle.refresh().is_err(), "the old holder sees the takeover");
        drop(handle);
        assert!(lock_path(&project).exists(), "the old holder does not remove the new lock");
        drop(h2);
        assert!(!lock_path(&project).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
