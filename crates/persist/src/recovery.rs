//! Autosave recovery copies (10 §8.3 AUTO-01…AUTO-04, §8.4 REC-01/REC-02, 01 §3.7).
//!
//! A recovery copy is a folder-form copy of the project in `<recovery>/<recoveryId>/` (only changed entries are
//! rewritten), with `recovery.json` `{recoveryId, projectId, projectName, originalPath, savedAt, documentRev,
//! appVersion, processId, host, heartbeatAt}`. Its owner refreshes `heartbeatAt` while it runs. At launch, recovery
//! folders whose owner is not running — heartbeat older than 5 minutes, or the process known dead on this host — are
//! offered after being validated like a project (§7); unusable data is listed with the reason.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tt_model::ModelError;

use crate::container::Form;
use crate::save::{FolderOptions, SaveInput, Saved, save_folder};
use crate::{codes, fsx, host, lock, time};

/// `recovery.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryInfo {
    /// Recovery ID (folder name).
    pub recovery_id: String,
    /// Project ID.
    pub project_id: String,
    /// Project name.
    pub project_name: String,
    /// The project's location (`null` for an untitled project, AUTO-03).
    pub original_path: Option<String>,
    /// When the copy was written.
    pub saved_at: String,
    /// The copy's `documentRev`.
    pub document_rev: u64,
    /// Application version.
    pub app_version: String,
    /// Owner process.
    pub process_id: u32,
    /// Owner host.
    #[serde(default)]
    pub host: String,
    /// Owner's last sign of life.
    #[serde(default)]
    pub heartbeat_at: String,
}

impl RecoveryInfo {
    /// Whether the owner is gone (heartbeat stale, or its process known dead on this host).
    #[must_use]
    pub fn orphaned(&self) -> bool {
        let beat = if self.heartbeat_at.is_empty() { &self.saved_at } else { &self.heartbeat_at };
        let age = time::ms_from_iso(beat).map_or(i64::MAX, |t| time::now_ms().saturating_sub(t));
        #[allow(clippy::cast_possible_wrap, reason = "constant")]
        if age > (lock::STALE_SECS as i64) * 1000 {
            return true;
        }
        let same_host = self.host.is_empty() || self.host == host::host();
        same_host && host::process_alive(self.process_id) == Some(false)
    }
}

/// A valid recovery ID: `rc_` and 4–64 lower-case hex digits (folder names under our control).
#[must_use]
pub fn valid_id(id: &str) -> bool {
    id.strip_prefix("rc_").is_some_and(|h| (4..=64).contains(&h.len()) && h.bytes().all(|c| c.is_ascii_hexdigit()))
}

/// A new recovery ID.
#[must_use]
pub fn new_id() -> String {
    format!("rc_{}", fsx::random_token(8))
}

/// Writes (or updates) a recovery copy (AUTO-01). Returns what was written.
///
/// # Errors
/// `project.saveFailed` and other I/O errors.
pub fn write(root: &Path, info: &RecoveryInfo, input: &SaveInput<'_>) -> Result<Saved, ModelError> {
    if !valid_id(&info.recovery_id) {
        return Err(ModelError::new(codes::INTERNAL, "invalid recovery id"));
    }
    let folder = root.join(&info.recovery_id);
    let saved = save_folder(&folder.join("project"), input, None, FolderOptions::default())?;
    let text = serde_json::to_vec_pretty(info).map_err(|e| ModelError::new(codes::INTERNAL, e.to_string()))?;
    fsx::write_atomic(&folder.join("recovery.json"), &text, fsx::save_error)?;
    Ok(saved)
}

/// Recovery ownership in this process (ruling O-18): an engine's fenced write or heartbeat and the release of an
/// abandoned engine's entries each hold it from their check to their last write, so a release never interleaves with a
/// write: a write that passed its fence check finishes before the release, and a write after the release sees the
/// fence set (the fence is set before the release is asked). It is held only by these file writes, never across a
/// call into another engine.
static OWNERSHIP: Mutex<()> = Mutex::new(());

fn ownership() -> std::sync::MutexGuard<'static, ()> {
    OWNERSHIP.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Writes a recovery copy as engine `engine` (ruling O-18): under the ownership lock, nothing is written once `fence`
/// is set (the engine was abandoned); otherwise the owner is named first ([`set_engine`], so whoever sees the entry
/// also sees its owner) and the copy written, as one step with respect to [`release_engine`]. `Ok(None)`: fenced,
/// nothing written.
///
/// # Errors
/// As [`write`] and [`set_engine`].
pub fn write_owned(
    root: &Path,
    info: &RecoveryInfo,
    input: &SaveInput<'_>,
    engine: &str,
    fence: &AtomicBool,
) -> Result<Option<Saved>, ModelError> {
    let _owner = ownership();
    if fence.load(Ordering::Acquire) {
        return Ok(None);
    }
    lock::boundary(lock::Boundary::RecoveryWriteChecked, &root.join(&info.recovery_id));
    set_engine(root, &info.recovery_id, engine)?;
    write(root, info, input).map(Some)
}

/// [`heartbeat`] of an engine, under the ownership lock and its `fence` (ruling O-18). It runs on the document
/// thread, so it never waits for the lock: while any engine of the process is writing a recovery copy (which writes a
/// fresh `heartbeatAt` itself) or a release runs, this heartbeat is skipped; the next one comes a minute later.
pub fn heartbeat_owned(root: &Path, recovery_id: &str, fence: &AtomicBool) {
    let _owner = match OWNERSHIP.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return,
    };
    if !fence.load(Ordering::Acquire) {
        heartbeat(root, recovery_id);
    }
}

/// Refreshes a recovery copy's heartbeat.
pub fn heartbeat(root: &Path, recovery_id: &str) {
    if !valid_id(recovery_id) {
        return;
    }
    let p = root.join(recovery_id).join("recovery.json");
    let Some(mut info) = read_info(&p) else { return };
    info.heartbeat_at = time::now_iso();
    if let Ok(text) = serde_json::to_vec_pretty(&info) {
        let _ = fsx::write_atomic(&p, &text, fsx::fs_error);
    }
}

/// Releases the recovery data of an executor of this process that stopped answering (M4 C4.1,
/// reviews/M4-design-engine-restart.md): every entry of `process_id` on this host, except `keep`, gets a heartbeat
/// time so old that the entry counts as orphaned, so the restarted executor offers it as recoverable work. Returns the
/// number of entries released.
pub fn release_process(root: &Path, process_id: u32, keep: Option<&str>) -> usize {
    let Ok(rd) = fs::read_dir(root) else { return 0 };
    let mut released = 0;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !valid_id(&name) || keep == Some(name.as_str()) {
            continue;
        }
        let p = e.path().join("recovery.json");
        let Some(mut info) = read_info(&p) else { continue };
        if info.process_id != process_id || (!info.host.is_empty() && info.host != host::host()) {
            continue;
        }
        "1970-01-01T00:00:00Z".clone_into(&mut info.heartbeat_at);
        if let Ok(text) = serde_json::to_vec_pretty(&info)
            && fsx::write_atomic(&p, &text, fsx::fs_error).is_ok()
        {
            released += 1;
        }
    }
    released
}

/// Records which engine owns a recovery entry: the sidecar `<root>/<id>/engine` holds the engine's ownership token,
/// then its process id and host, one per line (M4 C4.1 round 1, C41-R1-15; the process and host since round 2, for
/// [`scavenge`]). `recovery.json` keeps its format.
///
/// # Errors
/// An invalid id, or the write error.
pub fn set_engine(root: &Path, recovery_id: &str, engine: &str) -> Result<(), ModelError> {
    if !valid_id(recovery_id) {
        return Err(ModelError::new(codes::INTERNAL, "invalid recovery id"));
    }
    let text = format!(
        "{engine}
{}
{}",
        host::pid(),
        host::host()
    );
    fsx::write_atomic(&root.join(recovery_id).join("engine"), text.as_bytes(), fsx::fs_error)
}

/// The owner sidecar of an entry: engine token, process id, host (`None` when absent or unreadable).
fn owner_of(folder: &Path) -> Option<(String, Option<u32>, String)> {
    let b = fsx::read_limited(&folder.join("engine"), 512).ok()?;
    let text = String::from_utf8(b).ok()?;
    let mut lines = text.lines();
    let engine = lines.next()?.to_owned();
    let pid = lines.next().and_then(|l| l.parse().ok());
    Some((engine, pid, lines.next().unwrap_or_default().to_owned()))
}

/// Removes owner records left without a completed recovery (ruling O-18): an entry whose owner sidecar exists, whose
/// `recovery.json` does not (the process died between naming itself the owner and completing its first copy), and
/// whose owner is gone by the lock liveness rule (its process known dead on this host, or the record older than the
/// stale time). A completed recovery (`recovery.json` present) is never removed here: it is offered. Returns the number
/// of entries removed.
pub fn scavenge(root: &Path) -> usize {
    let Ok(rd) = fs::read_dir(root) else { return 0 };
    let mut removed = 0;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let folder = e.path();
        if !valid_id(&name) || folder.join("recovery.json").exists() {
            continue;
        }
        let Some((_, pid, owner_host)) = owner_of(&folder) else { continue };
        let same_host = owner_host.is_empty() || owner_host == host::host();
        let dead = same_host && pid.is_some_and(|p| p != host::pid() && host::process_alive(p) == Some(false));
        let age =
            fs::metadata(folder.join("engine")).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok());
        let alive = same_host && pid.is_some_and(|p| p == host::pid() || host::process_alive(p) == Some(true));
        let stale = !alive && age.is_some_and(|a| a.as_secs() > lock::STALE_SECS);
        // Checked again right before removing: a completed copy that appeared meanwhile is kept.
        if (dead || stale) && !folder.join("recovery.json").exists() && fs::remove_dir_all(&folder).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Releases the recovery data of one abandoned engine, named by its ownership token: only entries of this process on
/// this host whose sidecar names `engine` (except `keep`) get a heartbeat time so old that they count as orphaned. The
/// entries of any other engine, live or not, are left alone. Returns the number of entries released.
pub fn release_engine(root: &Path, engine: &str, keep: Option<&str>) -> usize {
    if engine.is_empty() {
        return 0;
    }
    // One step with respect to the engine's own writes (ruling O-18).
    let _owner = ownership();
    let Ok(rd) = fs::read_dir(root) else { return 0 };
    let mut released = 0;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !valid_id(&name) || keep == Some(name.as_str()) {
            continue;
        }
        if owner_of(&e.path()).is_none_or(|(owner, _, _)| owner != engine) {
            continue;
        }
        let p = e.path().join("recovery.json");
        let Some(mut info) = read_info(&p) else { continue };
        if info.process_id != host::pid() || (!info.host.is_empty() && info.host != host::host()) {
            continue;
        }
        "1970-01-01T00:00:00Z".clone_into(&mut info.heartbeat_at);
        if let Ok(text) = serde_json::to_vec_pretty(&info)
            && fsx::write_atomic(&p, &text, fsx::fs_error).is_ok()
        {
            released += 1;
        }
    }
    released
}

fn read_info(p: &Path) -> Option<RecoveryInfo> {
    let b = fsx::read_limited(p, 1 << 20).ok()?;
    serde_json::from_slice(&b).ok()
}

/// One recovery folder found at launch.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEntry {
    /// `recovery.json`.
    pub info: RecoveryInfo,
    /// Whether the data loads (§7).
    pub usable: bool,
    /// Why not.
    pub reason: Option<String>,
}

/// The recovery copies of a project folder.
#[must_use]
pub fn project_folder(root: &Path, recovery_id: &str) -> PathBuf {
    root.join(recovery_id).join("project")
}

/// Lists recovery folders (REC-01). `include_live`: also folders whose owner still runs (the owner's own listing).
/// `validate`: load each copy to check it is usable.
#[must_use]
pub fn list(root: &Path, include_live: bool, validate: bool) -> Vec<RecoveryEntry> {
    let Ok(rd) = fs::read_dir(root) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !valid_id(&name) {
            continue;
        }
        let Some(info) = read_info(&e.path().join("recovery.json")) else { continue };
        if info.recovery_id != name || (!include_live && !info.orphaned()) {
            continue;
        }
        let (usable, reason) = if validate {
            match crate::load::load(
                &project_folder(root, &name),
                &crate::load::LoadOptions { read_only: true, ..Default::default() },
            ) {
                Ok(_) => (true, None),
                Err(err) => (false, Some(format!("{}: {}", err.code(), err.message()))),
            }
        } else {
            (true, None)
        };
        out.push(RecoveryEntry { info, usable, reason });
    }
    out.sort_by(|a, b| b.info.saved_at.cmp(&a.info.saved_at));
    out
}

/// Deletes a recovery folder (AUTO-04, `project.recovery.delete`).
///
/// # Errors
/// `fs.ioError`.
pub fn delete(root: &Path, recovery_id: &str) -> Result<bool, ModelError> {
    if !valid_id(recovery_id) {
        return Ok(false);
    }
    let p = root.join(recovery_id);
    if !p.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(&p).map_err(|e| fsx::fs_error(&e, &p, "cannot delete"))?;
    Ok(true)
}

/// The form recovery copies use.
pub const FORM: Form = Form::Folder;

/// Reads `recovery.json` of one recovery folder.
#[must_use]
pub fn info(root: &Path, recovery_id: &str) -> Option<RecoveryInfo> {
    if !valid_id(recovery_id) {
        return None;
    }
    read_info(&root.join(recovery_id).join("recovery.json"))
}

/// The outcome of claiming recovery data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    /// The data is now this process's; the previous `recovery.json` is returned (to [`release`] on failure).
    Claimed(RecoveryInfo),
    /// Its owner still runs (or another session has just claimed it).
    InUse(RecoveryInfo),
    /// No such recovery data.
    NotFound,
}

/// Claims orphaned recovery data for this process before it is recovered (REC-01/REC-02; C13-R1-08): under an
/// exclusively created arbitration file (`<recoveryId>/.claim`), the data is re-read and, while still orphaned, its
/// `recovery.json` gets this process as owner with a fresh heartbeat. A second session then sees a live owner and is
/// refused (`project.recoveryInUse`). The new owner keeps the heartbeat fresh while the project is open; saving or
/// closing deletes the data (AUTO-04).
#[must_use]
pub fn claim(root: &Path, recovery_id: &str) -> Claim {
    if !valid_id(recovery_id) {
        return Claim::NotFound;
    }
    let folder = root.join(recovery_id);
    let info_path = folder.join("recovery.json");
    let guard = folder.join(".claim");
    for _ in 0..400 {
        match fs::OpenOptions::new().write(true).create_new(true).open(&guard) {
            Ok(g) => {
                drop(g);
                let outcome = match read_info(&info_path) {
                    None => Claim::NotFound,
                    Some(info) if !info.orphaned() => Claim::InUse(info),
                    Some(info) => {
                        let mut mine = info.clone();
                        mine.process_id = host::pid();
                        mine.host = host::host();
                        mine.heartbeat_at = time::now_iso();
                        match serde_json::to_vec_pretty(&mine) {
                            Ok(text) if fsx::write_atomic(&info_path, &text, fsx::fs_error).is_ok() => {
                                Claim::Claimed(info)
                            }
                            _ => Claim::InUse(info),
                        }
                    }
                };
                let _ = fs::remove_file(&guard);
                return outcome;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let old = fs::metadata(&guard)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > std::time::Duration::from_secs(10));
                if old {
                    let _ = fs::remove_file(&guard);
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            Err(_) => {
                return if folder.is_dir() {
                    read_info(&info_path).map_or(Claim::NotFound, Claim::InUse)
                } else {
                    Claim::NotFound
                };
            }
        }
    }
    read_info(&info_path).map_or(Claim::NotFound, Claim::InUse)
}

/// Gives claimed recovery data back (a recovery that failed): `previous` is written back while the data is still this
/// process's.
pub fn release(root: &Path, previous: &RecoveryInfo) {
    let p = root.join(&previous.recovery_id).join("recovery.json");
    if read_info(&p).is_some_and(|i| i.process_id == host::pid() && i.host == host::host())
        && let Ok(text) = serde_json::to_vec_pretty(previous)
    {
        let _ = fsx::write_atomic(&p, &text, fsx::fs_error);
    }
}

/// The save time of a project file (manifest `savedAt`), to compare with a recovery copy (REC-02).
#[must_use]
pub fn file_saved_at(path: &Path) -> Option<String> {
    let c = crate::container::Container::open(path, crate::manifest::PACKAGE_MIMETYPE, crate::zip::Limits::default())
        .ok()?;
    let b = c.read(crate::layout::MANIFEST, 64 << 20).ok()?;
    crate::manifest::Manifest::parse(&b).ok()?.saved_at
}

/// A JSON summary of an entry for `project.recovery.list`.
#[must_use]
pub fn summary(e: &RecoveryEntry) -> Value {
    serde_json::json!({
        "recoveryId": e.info.recovery_id,
        "projectName": e.info.project_name,
        "originalPath": e.info.original_path,
        "savedAt": e.info.saved_at,
        "usable": e.usable,
        "reason": e.reason,
    })
}
