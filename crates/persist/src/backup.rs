//! Backups (10 §8.2 BAK-01…BAK-04, 01 §3.7): after a successful save that replaces an existing version, the previous
//! version is kept in the per-user backups folder, `<backups>/<projectId>/<savedAt>.animpkg`, listed in
//! `<backups>/<projectId>/backup.json` as `{backupId, originalPath, projectName, savedAt, documentRev, sizeBytes}`.
//! `files.backups.count` versions are kept per project (0 = off), oldest removed first. Backups are never modified.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::ModelError;

use crate::container::Container;
use crate::manifest::Manifest;
use crate::save::{BackupSink, StagedBackup};
use crate::zip::{Limits, ZipArchive};
use crate::{codes, fsx, layout, time};

/// One backup record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    /// Backup ID.
    pub backup_id: String,
    /// The project's path when the backup was made.
    pub original_path: String,
    /// The project's name.
    pub project_name: String,
    /// When the backed-up version was saved.
    pub saved_at: String,
    /// Its `documentRev`.
    pub document_rev: u64,
    /// File size.
    pub size_bytes: u64,
    /// The backup file name in the project's backup folder.
    pub file: String,
}

/// The backups folder of one project.
#[derive(Debug, Clone)]
pub struct Backups {
    /// `<backups>/<projectId>`.
    pub folder: PathBuf,
    /// Versions kept (0 = off).
    pub count: u32,
    /// The project's current path.
    pub original_path: String,
    /// The project's name.
    pub project_name: String,
}

fn index_path(folder: &Path) -> PathBuf {
    folder.join("backup.json")
}

/// Reads a project's backup index (newest first).
#[must_use]
pub fn list(folder: &Path) -> Vec<BackupRecord> {
    let Ok(b) = fsx::read_limited(&index_path(folder), 16 << 20) else { return Vec::new() };
    let mut v: Vec<BackupRecord> = serde_json::from_slice::<Value>(&b)
        .ok()
        .and_then(|v| v.get("backups").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    v.retain(|r| crate::zip::safe_name(&r.file) && !r.file.contains('/') && folder.join(&r.file).is_file());
    v.sort_by(|a, b| b.saved_at.cmp(&a.saved_at).then(b.backup_id.cmp(&a.backup_id)));
    v
}

fn write_index(folder: &Path, records: &[BackupRecord]) -> Result<(), ModelError> {
    let text =
        serde_json::to_vec_pretty(&json!({ "format": "toontown-backups", "formatVersion": "1.0", "backups": records }))
            .map_err(|e| ModelError::new(codes::INTERNAL, e.to_string()))?;
    fsx::write_atomic(&index_path(folder), &text, fsx::fs_error)
}

/// The manifest's save time and revision of a package (the version being backed up), falling back to the file time.
fn version_of(path: &Path) -> (String, u64) {
    let from_manifest = ZipArchive::open(path, None, Limits::default())
        .ok()
        .and_then(|z| z.read(layout::MANIFEST, 64 << 20).ok())
        .and_then(|b| Manifest::parse(&b).ok());
    let mtime = || {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or_else(time::now_iso, |d| time::iso_from_ms(i64::try_from(d.as_millis()).unwrap_or(0)))
    };
    match from_manifest {
        Some(m) => (m.saved_at.unwrap_or_else(mtime), m.document_rev.unwrap_or(0)),
        None => (mtime(), 0),
    }
}

impl Backups {
    fn staged_name(&self) -> PathBuf {
        self.folder.join(format!(".staged-{}.animpkg", fsx::random_token(6)))
    }

    /// The backups, newest first.
    #[must_use]
    pub fn list(&self) -> Vec<BackupRecord> {
        list(&self.folder)
    }
}

impl BackupSink for Backups {
    fn stage_package(&self, previous: &Path) -> Result<Option<StagedBackup>, ModelError> {
        if self.count == 0 {
            return Ok(None);
        }
        fs::create_dir_all(&self.folder)
            .map_err(|e| fsx::save_error(&e, &self.folder, "cannot create the backups folder"))?;
        let to = self.staged_name();
        // A hard link makes the final rename-over a move into the backups folder (BAK-01); across volumes, copy.
        if fs::hard_link(previous, &to).is_err() {
            fsx::copy_file(previous, &to)
                .map_err(|e| fsx::save_error(&e, &to, "cannot back up the previous version"))?;
        }
        Ok(Some(StagedBackup { path: to }))
    }

    fn stage_folder(&self, previous: &Container) -> Result<Option<StagedBackup>, ModelError> {
        if self.count == 0 {
            return Ok(None);
        }
        fs::create_dir_all(&self.folder)
            .map_err(|e| fsx::save_error(&e, &self.folder, "cannot create the backups folder"))?;
        let to = self.staged_name();
        // BAK-02: a package-form snapshot of the previous folder version.
        if let Err(e) = crate::save::container_to_package(previous, &to) {
            let _ = fs::remove_file(&to);
            return Err(e);
        }
        Ok(Some(StagedBackup { path: to }))
    }

    fn commit(&self, staged: StagedBackup) -> Result<String, ModelError> {
        let (saved_at, document_rev) = version_of(&staged.path);
        let mut file = format!("{}.animpkg", time::file_stamp(&saved_at));
        let mut records = list(&self.folder);
        while records.iter().any(|r| r.file == file) || self.folder.join(&file).exists() {
            file = format!("{}-{}.animpkg", time::file_stamp(&saved_at), fsx::random_token(2));
        }
        let to = self.folder.join(&file);
        fs::rename(&staged.path, &to).map_err(|e| fsx::save_error(&e, &to, "cannot record the backup"))?;
        let backup_id = format!("bk_{}", fsx::random_token(8));
        let size_bytes = fs::metadata(&to).map(|m| m.len()).unwrap_or(0);
        records.insert(
            0,
            BackupRecord {
                backup_id: backup_id.clone(),
                original_path: self.original_path.clone(),
                project_name: self.project_name.clone(),
                saved_at,
                document_rev,
                size_bytes,
                file,
            },
        );
        records.sort_by(|a, b| b.saved_at.cmp(&a.saved_at).then(b.backup_id.cmp(&a.backup_id)));
        // BAK-03: oldest removed first.
        while records.len() > self.count as usize {
            if let Some(old) = records.pop() {
                let _ = fs::remove_file(self.folder.join(&old.file));
            }
        }
        write_index(&self.folder, &records)?;
        Ok(backup_id)
    }

    fn discard(&self, staged: StagedBackup) {
        let _ = fs::remove_file(&staged.path);
    }
}

/// Finds a backup by ID under the backups root (any project).
#[must_use]
pub fn find(root: &Path, backup_id: &str) -> Option<(PathBuf, BackupRecord)> {
    let rd = fs::read_dir(root).ok()?;
    for e in rd.flatten() {
        let folder = e.path();
        if !folder.is_dir() {
            continue;
        }
        if let Some(r) = list(&folder).into_iter().find(|r| r.backup_id == backup_id) {
            return Some((folder.join(&r.file), r));
        }
    }
    None
}
