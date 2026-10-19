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

/// Tries to lock a project path (LOCK-01, LOCK-02).
#[must_use]
pub fn acquire(project: &Path) -> Locked {
    let path = lock_path(project);
    for _ in 0..3 {
        let info = LockInfo::new();
        let text = match serde_json::to_vec_pretty(&info) {
            Ok(t) => t,
            Err(e) => return Locked::NotWritable(e.to_string()),
        };
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                if f.write_all(&text).and_then(|()| f.sync_all()).is_err() {
                    let _ = fs::remove_file(&path);
                    return Locked::NotWritable("cannot write the lock file".to_owned());
                }
                return Locked::Acquired { handle: LockHandle { path, info: Mutex::new(info) }, took_over: None };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let Some(current) = read(&path) else {
                    // An unreadable lock file: treat it as stale.
                    let _ = fs::remove_file(&path);
                    continue;
                };
                if !current.stale() {
                    return Locked::HeldByOther(current);
                }
                // Take over: replace atomically, then confirm we won.
                if fsx::write_atomic(&path, &text, fsx::fs_error).is_err() {
                    return Locked::NotWritable("cannot replace the stale lock".to_owned());
                }
                if read(&path).is_some_and(|i| i.lock_id == info.lock_id) {
                    return Locked::Acquired {
                        handle: LockHandle { path, info: Mutex::new(info) },
                        took_over: Some(current),
                    };
                }
            }
            Err(e) => return Locked::NotWritable(e.to_string()),
        }
    }
    Locked::NotWritable("the lock could not be taken".to_owned())
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
