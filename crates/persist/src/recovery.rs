//! Autosave recovery copies (10 §8.3 AUTO-01…AUTO-04, §8.4 REC-01/REC-02, 01 §3.7).
//!
//! A recovery copy is a folder-form copy of the project in `<recovery>/<recoveryId>/` (only changed entries are
//! rewritten), with `recovery.json` `{recoveryId, projectId, projectName, originalPath, savedAt, documentRev,
//! appVersion, processId, host, heartbeatAt}`. Its owner refreshes `heartbeatAt` while it runs. At launch, recovery
//! folders whose owner is not running — heartbeat older than 5 minutes, or the process known dead on this host — are
//! offered after being validated like a project (§7); unusable data is listed with the reason.

use std::fs;
use std::path::{Path, PathBuf};

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
