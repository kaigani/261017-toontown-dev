//! Lock files (10 §8.6, LOCK-01…LOCK-03; D13): opening a project for writing creates `<package>.lock` next to a package
//! or `.lock` inside a folder-form project, holding `{user, host, processId, appVersion, openedAt, heartbeatAt,
//! lockId}`. The holder refreshes `heartbeatAt` (every 60 s) and removes the file on close. A live lock held by someone
//! else makes the open read-only (`project.lockedByOther`); a lock whose heartbeat is older than 5 minutes, or whose
//! process is known dead on the same host, is stale and is taken over with a warning.
//!
//! `lockId` is a random token: two executors in one process (tests, embedding hosts) are still "someone else" to each
//! other. A fresh lock is created exclusively (`create_new`), so two openers cannot both acquire it.

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

    /// Refreshes the heartbeat (LOCK-01). A lock taken over by someone else is not overwritten.
    ///
    /// # Errors
    /// `fs.ioError` when the file cannot be written.
    pub fn refresh(&self) -> Result<(), ModelError> {
        let mut info = self.info.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(current) = read(&self.path)
            && current.lock_id != info.lock_id
        {
            return Err(ModelError::with_details(
                codes::LOCKED_BY_OTHER,
                "the lock was taken over by someone else",
                json!({ "lock": current }),
            ));
        }
        info.heartbeat_at = time::now_iso();
        let text = serde_json::to_vec_pretty(&*info).map_err(|e| ModelError::new(codes::INTERNAL, e.to_string()))?;
        fsx::write_atomic(&self.path, &text, fsx::fs_error)
    }
}

impl Drop for LockHandle {
    fn drop(&mut self) {
        let ours = self.info.lock().map(|i| i.lock_id.clone()).unwrap_or_default();
        if read(&self.path).is_some_and(|i| i.lock_id == ours) {
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

/// How long a takeover arbitration file may live before it counts as left behind by a crashed process.
const TAKEOVER_STALE: std::time::Duration = std::time::Duration::from_secs(10);

fn takeover_path(lock: &Path) -> PathBuf {
    let mut s = lock.as_os_str().to_owned();
    s.push(".takeover");
    PathBuf::from(s)
}

/// Tries to lock a project path (LOCK-01, LOCK-02).
///
/// A fresh lock is created exclusively (`create_new`). A stale lock is taken over under an arbitration file
/// (`<lock>.takeover`, also created exclusively; C13-R1-02): only its creator replaces the lock, after checking that the
/// lock is still the same stale one, by removing it and creating its own lock exclusively. Everyone else waits for the
/// takeover to finish and then sees the new live lock, so concurrent takeovers yield one owner.
#[must_use]
pub fn acquire(project: &Path) -> Locked {
    let path = lock_path(project);
    let arbiter = takeover_path(&path);
    let info = LockInfo::new();
    let text = match serde_json::to_vec_pretty(&info) {
        Ok(t) => t,
        Err(e) => return Locked::NotWritable(e.to_string()),
    };
    let create = |path: &Path| -> std::io::Result<()> {
        let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
        if let Err(e) = f.write_all(&text).and_then(|()| f.sync_all()) {
            drop(f);
            let _ = fs::remove_file(path);
            return Err(e);
        }
        Ok(())
    };
    let acquired = |took_over| Locked::Acquired {
        handle: LockHandle { path: path.clone(), info: Mutex::new(info.clone()) },
        took_over,
    };
    for _ in 0..400 {
        match create(&path) {
            Ok(()) => return acquired(None),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Locked::NotWritable(e.to_string()),
        }
        let current = read(&path);
        if let Some(c) = &current
            && !c.stale()
        {
            return Locked::HeldByOther(c.clone());
        }
        // Stale or unreadable: take it over, one taker at a time.
        match OpenOptions::new().write(true).create_new(true).open(&arbiter) {
            Ok(guard) => {
                drop(guard);
                let outcome = (|| {
                    let again = read(&path);
                    let same = match (&current, &again) {
                        (Some(a), Some(b)) => a.lock_id == b.lock_id && b.stale(),
                        (None, None) => path.exists(),
                        _ => false,
                    };
                    if !same {
                        return None; // Someone else changed the lock meanwhile: look again.
                    }
                    if let Err(e) = fs::remove_file(&path)
                        && e.kind() != std::io::ErrorKind::NotFound
                    {
                        return Some(Locked::NotWritable(format!("cannot replace the stale lock: {e}")));
                    }
                    match create(&path) {
                        Ok(()) => Some(acquired(current.clone())),
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                        Err(e) => Some(Locked::NotWritable(e.to_string())),
                    }
                })();
                let _ = fs::remove_file(&arbiter);
                if let Some(o) = outcome {
                    return o;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Another taker is at work; an arbitration file left by a crashed process is cleared after a while.
                let old = fs::metadata(&arbiter)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > TAKEOVER_STALE);
                if old {
                    let _ = fs::remove_file(&arbiter);
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            Err(e) => return Locked::NotWritable(e.to_string()),
        }
    }
    match read(&path) {
        Some(c) => Locked::HeldByOther(c),
        None => Locked::NotWritable("the lock could not be taken".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test")]
    use super::*;

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
