//! The folder-save journal (10 SAVE-02; C13-R1-03/04).
//!
//! A folder save never leaves a mixed version behind. Before the journal is written:
//! - every new entry is complete and flushed in a temporary sibling (`.<name>.<token>.tmp`);
//! - every entry that will be replaced has an **old copy** (`.<name>.<token>.old`, a hard link or a copy).
//!
//! The journal records each change with the size and SHA-256 of both versions. The save then renames the temporaries
//! over their targets, renames removed entries to their old copies, writes `manifest.json` last, deletes the journal,
//! and finally deletes the old copies.
//!
//! On open, a leftover journal is **validated completely before anything on disk changes** (format, version, safe and
//! distinct names, siblings in the target's folder, hashes). A malformed journal changes nothing and the open fails
//! (`project.invalidFormat`, `reason: "journal"`). A valid journal is then:
//! - **completed** when every new version is present (pending temporary or already-renamed target);
//! - otherwise **rolled back** when every old version is present (untouched target or old copy): new entries are
//!   removed, replaced and removed entries are restored;
//! - otherwise left alone, and the open fails (`reason: "unrecoverableSave"`): neither version is complete, and the
//!   files are kept for the user.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_model::ModelError;

use crate::fsx;
use crate::layout;
use crate::zip::invalid;

/// The journal's format name.
pub const FORMAT: &str = "animpkg-save-journal";
/// The journal version that records old copies.
pub const VERSION: &str = "2.0";

/// A file with known content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// Entry-style path relative to the project folder.
    pub file: String,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256 (lower-case hex).
    pub sha256: String,
}

/// An entry written by the save: `new` replaces `target`; `old` holds the previous content (`None`: a new entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replace {
    /// The entry.
    pub target: String,
    /// The temporary holding the new content.
    pub new: Version,
    /// The old copy of the previous content.
    pub old: Option<Version>,
}

/// An entry removed by the save: `target` is renamed to `old`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remove {
    /// The entry.
    pub target: String,
    /// Where its content goes (and comes back from on rollback).
    pub old: Version,
}

/// A journal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Journal {
    /// Entries written, in order (`manifest.json` last).
    pub replace: Vec<Replace>,
    /// Entries removed.
    pub remove: Vec<Remove>,
}

fn version_json(v: &Version) -> Value {
    json!({ "file": v.file, "size": v.size, "sha256": v.sha256 })
}

impl Journal {
    /// The journal's JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "format": FORMAT,
            "formatVersion": VERSION,
            "replace": self.replace.iter().map(|r| json!({
                "target": r.target, "new": version_json(&r.new), "old": r.old.as_ref().map(version_json),
            })).collect::<Vec<_>>(),
            "remove": self.remove.iter().map(|r| json!({ "target": r.target, "old": version_json(&r.old) })).collect::<Vec<_>>(),
        })
    }

    /// Parses and validates a journal. Nothing on disk is read or changed.
    ///
    /// # Errors
    /// `project.invalidFormat` (`reason: "journal"`) naming the first problem.
    pub fn parse(v: &Value) -> Result<Self, ModelError> {
        let bad = |m: &str| {
            invalid(
                "journal",
                format!("the save journal is malformed: {m}; nothing was changed"),
                Some(layout::JOURNAL),
            )
        };
        let o = v.as_object().ok_or_else(|| bad("not an object"))?;
        if o.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err(bad("unknown format"));
        }
        if o.get("formatVersion").and_then(Value::as_str) != Some(VERSION) {
            return Err(bad("unsupported version (only journals that record old copies can be recovered)"));
        }
        let replace = o.get("replace").and_then(Value::as_array).ok_or_else(|| bad("`replace` is not a list"))?;
        let remove = o.get("remove").and_then(Value::as_array).ok_or_else(|| bad("`remove` is not a list"))?;
        let version = |x: Option<&Value>, suffix: &str, target: &str| -> Result<Version, ModelError> {
            let x = x.and_then(Value::as_object).ok_or_else(|| bad("a version is not an object"))?;
            let file = x.get("file").and_then(Value::as_str).ok_or_else(|| bad("a version has no file"))?;
            let size = x.get("size").and_then(Value::as_u64).ok_or_else(|| bad("a version has no size"))?;
            let sha = x.get("sha256").and_then(Value::as_str).ok_or_else(|| bad("a version has no sha256"))?;
            if sha.len() != 64 || !sha.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
                return Err(bad("a sha256 is not 64 lower-case hex digits"));
            }
            let (dir, last) = split(file);
            if !crate::zip::safe_name(file)
                || !last.starts_with('.')
                || !last.ends_with(suffix)
                || dir != split(target).0
            {
                return Err(bad("a temporary or old copy is not a hidden sibling of its entry"));
            }
            Ok(Version { file: file.to_owned(), size, sha256: sha.to_owned() })
        };
        let target = |x: Option<&Value>| -> Result<String, ModelError> {
            let t = x.and_then(Value::as_str).ok_or_else(|| bad("a change has no target"))?;
            if !crate::zip::safe_name(t) || layout::is_folder_control(t) || split(t).1.starts_with('.') {
                return Err(bad("a target is not an entry name"));
            }
            Ok(t.to_owned())
        };
        let mut j = Journal::default();
        for r in replace {
            let t = target(r.get("target"))?;
            let new = version(r.get("new"), ".tmp", &t)?;
            let old = match r.get("old") {
                None | Some(Value::Null) => None,
                x => Some(version(x, ".old", &t)?),
            };
            j.replace.push(Replace { target: t, new, old });
        }
        for r in remove {
            let t = target(r.get("target"))?;
            let old = version(r.get("old"), ".old", &t)?;
            j.remove.push(Remove { target: t, old });
        }
        // Every name appears once.
        let mut seen = BTreeSet::new();
        let names = j
            .replace
            .iter()
            .flat_map(|r| [Some(&r.target), Some(&r.new.file), r.old.as_ref().map(|o| &o.file)])
            .chain(j.remove.iter().flat_map(|r| [Some(&r.target), Some(&r.old.file)]))
            .flatten();
        for n in names {
            if !seen.insert(n.to_ascii_lowercase()) {
                return Err(bad("a name appears twice"));
            }
        }
        Ok(j)
    }
}

fn split(name: &str) -> (&str, &str) {
    name.rsplit_once('/').unwrap_or(("", name))
}

/// The path of an entry-style name under `root`.
#[must_use]
pub fn path(root: &Path, name: &str) -> PathBuf {
    name.split('/').fold(root.to_path_buf(), |p, s| p.join(s))
}

/// Whether the file holds exactly this version.
fn holds(root: &Path, file: &str, v: &Version) -> bool {
    File::open(path(root, file))
        .ok()
        .and_then(|f| fsx::sha256_reader(f).ok())
        .is_some_and(|(h, n)| h == v.sha256 && n == v.size)
}

/// An old-copy sibling name for `target` (entry-style).
#[must_use]
pub fn old_name(target: &str) -> String {
    let (dir, last) = split(target);
    let name = format!(".{last}.{}.old", fsx::random_token(6));
    if dir.is_empty() { name } else { format!("{dir}/{name}") }
}

fn rename(root: &Path, from: &str, to: &str) -> Result<(), ModelError> {
    let (a, b) = (path(root, from), path(root, to));
    if let Some(dir) = b.parent() {
        fs::create_dir_all(dir).map_err(|e| fsx::save_error(&e, dir, "cannot create the folder"))?;
    }
    fs::rename(&a, &b).map_err(|e| fsx::save_error(&e, &b, "cannot replace"))?;
    if let Some(dir) = b.parent() {
        fsx::sync_dir(dir);
    }
    Ok(())
}

fn remove_if_present(root: &Path, file: &str) -> Result<(), ModelError> {
    let p = path(root, file);
    match fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(fsx::save_error(&e, &p, "cannot remove")),
    }
}

/// Removes the empty folders left above a removed entry.
fn prune(root: &Path, name: &str) {
    let mut dir = path(root, name).parent().map(Path::to_path_buf);
    while let Some(d) = dir {
        if d == root || fs::remove_dir(&d).is_err() {
            break;
        }
        dir = d.parent().map(Path::to_path_buf);
    }
}

/// Applies a journal forward from any point (`steps`: stop after this many renames; tests of interrupted saves).
///
/// # Errors
/// I/O errors.
pub fn apply(root: &Path, j: &Journal, steps: Option<usize>) -> Result<bool, ModelError> {
    let mut done = 0usize;
    let mut step = || {
        done += 1;
        steps.is_some_and(|s| done > s)
    };
    // Ordinary replacements, then removals, then `manifest.json` last (SAVE-02; C13-R2-06).
    for r in j.replace.iter().filter(|r| r.target != layout::MANIFEST) {
        if path(root, &r.new.file).is_file() {
            if step() {
                return Ok(false);
            }
            rename(root, &r.new.file, &r.target)?;
        }
    }
    for r in &j.remove {
        if path(root, &r.target).is_file() {
            if step() {
                return Ok(false);
            }
            rename(root, &r.target, &r.old.file)?;
            prune(root, &r.target);
        }
    }
    for r in j.replace.iter().filter(|r| r.target == layout::MANIFEST) {
        if path(root, &r.new.file).is_file() {
            if step() {
                return Ok(false);
            }
            rename(root, &r.new.file, &r.target)?;
        }
    }
    Ok(true)
}

/// The number of renames [`apply`] performs on a fresh save.
#[must_use]
pub fn steps(j: &Journal) -> usize {
    j.replace.len() + j.remove.len()
}

/// Deletes the old copies of a completed save.
pub fn discard_old(root: &Path, j: &Journal) {
    for f in j.replace.iter().filter_map(|r| r.old.as_ref()).chain(j.remove.iter().map(|r| &r.old)) {
        let _ = remove_if_present(root, &f.file);
        prune(root, &f.file);
    }
}

/// What recovery did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The new version is complete.
    Completed,
    /// The previous version is restored.
    RolledBack,
}

/// Recovers an interrupted save described by a validated journal (see the module docs). The journal file itself is
/// left for the caller to delete.
///
/// # Errors
/// `project.invalidFormat` (`reason: "unrecoverableSave"`) when neither version is complete (nothing is changed);
/// I/O errors.
pub fn recover(root: &Path, j: &Journal) -> Result<Outcome, ModelError> {
    let forward = j.replace.iter().all(|r| holds(root, &r.new.file, &r.new) || holds(root, &r.target, &r.new));
    if forward {
        // Damaged leftovers of steps already done are not needed.
        apply_pending(root, j)?;
        discard_old(root, j);
        return Ok(Outcome::Completed);
    }
    let target_present = |t: &str| path(root, t).is_file();
    let backward = j.replace.iter().all(|r| match &r.old {
        Some(o) => holds(root, &r.target, o) || holds(root, &o.file, o),
        None => !target_present(&r.target) || holds(root, &r.target, &r.new),
    }) && j.remove.iter().all(|r| holds(root, &r.target, &r.old) || holds(root, &r.old.file, &r.old));
    if !backward {
        return Err(invalid(
            "unrecoverableSave",
            "an interrupted save of this folder left neither the new nor the previous version complete; nothing was \
             changed (the temporaries, old copies and journal are kept)",
            Some(layout::JOURNAL),
        ));
    }
    for r in &j.replace {
        remove_if_present(root, &r.new.file)?;
        match &r.old {
            Some(o) if !holds(root, &r.target, o) => rename(root, &o.file, &r.target)?,
            Some(o) => remove_if_present(root, &o.file)?,
            None => {
                remove_if_present(root, &r.target)?;
                prune(root, &r.target);
            }
        }
    }
    for r in &j.remove {
        if holds(root, &r.target, &r.old) {
            remove_if_present(root, &r.old.file)?;
        } else {
            rename(root, &r.old.file, &r.target)?;
        }
    }
    Ok(Outcome::RolledBack)
}

/// Forward completion that tolerates temporaries already renamed.
fn apply_pending(root: &Path, j: &Journal) -> Result<(), ModelError> {
    let replace = |r: &Replace| -> Result<(), ModelError> {
        if holds(root, &r.new.file, &r.new) {
            rename(root, &r.new.file, &r.target)
        } else {
            remove_if_present(root, &r.new.file)
        }
    };
    // The same order as a save: replacements, removals, the manifest last (C13-R2-06).
    for r in j.replace.iter().filter(|r| r.target != layout::MANIFEST) {
        replace(r)?;
    }
    for r in &j.remove {
        if path(root, &r.target).is_file() {
            rename(root, &r.target, &r.old.file)?;
            prune(root, &r.target);
        }
    }
    for r in j.replace.iter().filter(|r| r.target == layout::MANIFEST) {
        replace(r)?;
    }
    Ok(())
}

/// Makes the old copy of an entry about to be replaced (a hard link, or a flushed copy across file systems that do
/// not support links).
///
/// # Errors
/// I/O errors.
pub fn keep_old(root: &Path, target: &str) -> Result<Version, ModelError> {
    let from = path(root, target);
    let name = old_name(target);
    let to = path(root, &name);
    if fs::hard_link(&from, &to).is_err() {
        fsx::copy_file(&from, &to).map_err(|e| fsx::save_error(&e, &to, "cannot keep the previous version"))?;
        if let Ok(f) = File::open(&to) {
            let _ = f.sync_all();
        }
    }
    let (sha256, size) = File::open(&to)
        .and_then(fsx::sha256_reader)
        .map_err(|e| fsx::save_error(&e, &to, "cannot read the previous version"))?;
    Ok(Version { file: name, size, sha256 })
}
