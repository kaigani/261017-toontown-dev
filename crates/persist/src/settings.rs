//! Persisted user settings (01 §11) and the per-user folders (01 §3.7).
//!
//! Settings are human-readable, versioned JSON files, one per kind, each `{format: "toontown-settings", kind,
//! formatVersion, data}`, written atomically. A missing file means defaults; a file that cannot be used is kept aside
//! as a copy (`<name>.corrupt-<time>.json`, never replacing an earlier copy), the defaults are used for that file only,
//! and the caller tells the user once (`prefs.corrupt`). Versions follow the documents' policy ([`read_checked`]);
//! older versions would be migrated on load (none exist: 1.0 is the first).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_model::ModelError;

use crate::{fsx, time};

/// Settings format version.
pub const VERSION: &str = "1.0";

/// The settings kinds (one file each).
pub const KINDS: [&str; 6] = ["preferences", "recent", "state", "workspaces", "keymap", "drawing"];

/// The per-user folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDirs {
    /// Settings files.
    pub settings: PathBuf,
    /// Autosave recovery copies.
    pub recovery: PathBuf,
    /// Backups.
    pub backups: PathBuf,
}

impl UserDirs {
    /// All folders under one root (`<root>/Settings`, `<root>/Recovery`, `<root>/Backups`).
    #[must_use]
    pub fn under(root: &Path) -> Self {
        Self { settings: root.join("Settings"), recovery: root.join("Recovery"), backups: root.join("Backups") }
    }

    /// The platform's conventional per-user location (01 §11): `%APPDATA%\Toontown` on Windows,
    /// `~/Library/Application Support/Toontown` on macOS, `$XDG_CONFIG_HOME/toontown` (or `~/.config/toontown`)
    /// elsewhere. `None` when no home folder is known.
    #[must_use]
    pub fn platform() -> Option<Self> {
        let root = if cfg!(windows) {
            std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Toontown"))
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Toontown"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(|x| PathBuf::from(x).join("toontown"))
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/toontown")))
        }?;
        Some(Self::under(&root))
    }
}

/// The outcome of reading a settings file.
#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    /// No file: defaults.
    Missing,
    /// The file's data.
    Data(Value),
    /// The file could not be used; defaults apply. The path is the kept-aside copy, or the file itself when it could
    /// not be moved aside ([`SetAside::kept`] says which).
    Corrupt(PathBuf),
}

/// A settings file that could not be used (01 §11): why, and what happened to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetAside {
    /// The settings kind (`preferences`, `keymap`, ...).
    pub kind: String,
    /// Why: `unreadable`, `notSettings` (not JSON, or not a settings envelope of this kind), `badVersion` (missing or
    /// not `MAJOR.MINOR`), `unsupportedVersion` (another major version), `wrongShape` (the data is not this kind's).
    pub reason: &'static str,
    /// A human-readable detail.
    pub detail: String,
    /// The kept-aside copy, or why the file could not be kept aside (it then stays where it was).
    pub kept: Result<PathBuf, String>,
}

/// The version policy of settings files (the documents' rule; C41-R1-04): the same major version is read (each kind
/// tolerates a newer minor version's unknown members); a missing or malformed version makes the file unusable
/// (`badVersion`); another major version is not read (`unsupportedVersion`). Both are kept aside and their defaults
/// used; the other settings files are unaffected.
fn version_problem(v: &Value) -> Option<(&'static str, String)> {
    let Some(text) = v.get("formatVersion").and_then(Value::as_str) else {
        return Some(("badVersion", "formatVersion is missing or not a string".into()));
    };
    let Some(version) = crate::manifest::Version::parse(text) else {
        return Some(("badVersion", format!("formatVersion `{text}` is not MAJOR.MINOR")));
    };
    let current = crate::manifest::Version::parse(VERSION).unwrap_or(version);
    if version.major != current.major {
        return Some((
            "unsupportedVersion",
            format!("formatVersion {text} is not supported (this build reads {VERSION})"),
        ));
    }
    None
}

/// Reads settings file `<dir>/<kind>.json` with the envelope and version checks only.
#[must_use]
pub fn read(dir: &Path, kind: &str) -> Read {
    read_checked(dir, kind, |_| Ok(())).0
}

/// Reads settings file `<dir>/<kind>.json`; `shape` validates the kind's whole data value (C41-R1-08: a valid envelope
/// with the wrong data shape is a corrupt file). An unusable file is kept aside and described.
pub fn read_checked(dir: &Path, kind: &str, shape: impl Fn(&Value) -> Result<(), String>) -> (Read, Option<SetAside>) {
    let p = dir.join(format!("{kind}.json"));
    let bytes = match fsx::read_limited(&p, 64 << 20) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Read::Missing, None),
        Err(e) => return aside(&p, kind, "unreadable", e.to_string()),
    };
    let Some(v) = String::from_utf8(bytes).ok().and_then(|t| tt_model::json_text::parse::<Value>(&t).ok()) else {
        return aside(&p, kind, "notSettings", "not UTF-8 JSON".into());
    };
    if v.get("format").and_then(Value::as_str) != Some("toontown-settings")
        || v.get("kind").and_then(Value::as_str) != Some(kind)
    {
        return aside(&p, kind, "notSettings", format!("not a `{kind}` settings file"));
    }
    if let Some((reason, detail)) = version_problem(&v) {
        return aside(&p, kind, reason, detail);
    }
    let Some(data) = v.get("data").cloned() else {
        return aside(&p, kind, "wrongShape", "no `data` member".into());
    };
    if let Err(why) = shape(&data) {
        return aside(&p, kind, "wrongShape", why);
    }
    (Read::Data(data), None)
}

fn aside(p: &Path, kind: &str, reason: &'static str, detail: String) -> (Read, Option<SetAside>) {
    let kept = set_aside(p);
    let path = kept.as_ref().map_or_else(|_| p.to_path_buf(), Clone::clone);
    (Read::Corrupt(path), Some(SetAside { kind: kind.to_owned(), reason, detail, kept }))
}

/// Keeps a file aside as `<stem>.corrupt-<time>.json` (`-1`, `-2`, ... on a collision; ruling O-13). The copy is
/// created with create-new semantics, so an existing copy is never replaced: a hard link where the file system allows
/// it, else a byte copy into a new file; the original is then removed. Copies are the user's data and are never
/// pruned.
///
/// # Errors
/// What went wrong, in words; the file then stays where it was (when only its removal failed, both remain).
pub fn set_aside(p: &Path) -> Result<PathBuf, String> {
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = time::file_stamp(&time::now_iso());
    let mut last = String::new();
    for n in 0..1000u32 {
        let name =
            if n == 0 { format!("{stem}.corrupt-{stamp}.json") } else { format!("{stem}.corrupt-{stamp}-{n}.json") };
        let target = p.with_file_name(name);
        let made = fs::hard_link(p, &target).or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists || e.kind() == std::io::ErrorKind::NotFound {
                return Err(e);
            }
            copy_new(p, &target)
        });
        match made {
            Ok(()) => {
                return match fs::remove_file(p) {
                    Ok(()) => Ok(target),
                    Err(e) => {
                        Err(format!("a copy was kept at {} but the file could not be removed: {e}", target.display()))
                    }
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = e.to_string(),
            Err(e) => return Err(format!("the file could not be kept aside: {e}")),
        }
    }
    Err(format!("no free name to keep the file aside: {last}"))
}

/// Copies `from` into a file created new at `to` (fails with `AlreadyExists` when `to` exists).
fn copy_new(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut src = fs::File::open(from)?;
    let mut dst = fs::OpenOptions::new().write(true).create_new(true).open(to)?;
    std::io::copy(&mut src, &mut dst)?;
    dst.sync_all()
}

/// The folder's write lock (`<dir>/.settings.lock`): writers hold it shared while they write; the start-up clean-up
/// holds it exclusively, so it never runs while any process of this application is writing settings here.
fn lock_file(dir: &Path) -> std::io::Result<fs::File> {
    fs::create_dir_all(dir)?;
    fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join(".settings.lock"))
}

/// Removes temporary files that interrupted settings writes left behind (`.<kind>.json.<hex>.tmp`; C41-R1-09). Safe
/// while other processes of this application use the folder: it runs only when it gets the folder's write lock
/// exclusively, which then no writer holds; while a writer holds it nothing is removed (the next start cleans).
/// Returns the number of files removed.
#[must_use]
pub fn clean_temporaries(dir: &Path) -> usize {
    let Ok(lock) = lock_file(dir) else { return 0 };
    if lock.try_lock().is_err() {
        return 0;
    }
    let Ok(rd) = fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for e in rd.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        let ours = KINDS.iter().any(|k| {
            name.strip_prefix(&format!(".{k}.json."))
                .and_then(|r| r.strip_suffix(".tmp"))
                .is_some_and(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()))
        });
        if ours && fs::remove_file(e.path()).is_ok() {
            removed += 1;
        }
    }
    drop(lock);
    removed
}

/// Writes settings file `<dir>/<kind>.json` atomically, holding the folder's write lock shared.
///
/// # Errors
/// `fs.ioError`.
pub fn write(dir: &Path, kind: &str, data: &Value) -> Result<(), ModelError> {
    let v = json!({ "format": "toontown-settings", "kind": kind, "formatVersion": VERSION, "data": data });
    let text = tt_model::canonical::to_canonical_string(&v)?;
    let lock = lock_file(dir).map_err(|e| fsx::fs_error(&e, dir, "cannot open the settings lock"))?;
    lock.lock_shared().map_err(|e| fsx::fs_error(&e, dir, "cannot lock the settings folder"))?;
    let written = fsx::write_atomic(&dir.join(format!("{kind}.json")), text.as_bytes(), fsx::fs_error);
    drop(lock);
    written
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]
    use super::*;

    #[test]
    fn missing_data_and_corrupt() {
        let dir = std::env::temp_dir().join(format!("tt-settings-{}", fsx::random_token(4)));
        assert_eq!(read(&dir, "recent"), Read::Missing);
        write(&dir, "recent", &json!([{"path": "/x"}])).unwrap();
        assert_eq!(read(&dir, "recent"), Read::Data(json!([{"path": "/x"}])));
        fs::write(dir.join("recent.json"), b"{not json").unwrap();
        let Read::Corrupt(aside) = read(&dir, "recent") else { unreachable!("corrupt") };
        assert!(aside.exists() && !dir.join("recent.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    fn file(dir: &Path, kind: &str, version: &Value, data: &Value) {
        fs::create_dir_all(dir).unwrap();
        let v = json!({ "format": "toontown-settings", "kind": kind, "formatVersion": version, "data": data });
        fs::write(dir.join(format!("{kind}.json")), v.to_string()).unwrap();
    }

    #[test]
    fn versions_follow_the_document_policy() {
        let dir = std::env::temp_dir().join(format!("tt-settings-v-{}", fsx::random_token(4)));
        for (version, reason) in [
            (json!("2.0"), Some("unsupportedVersion")),
            (json!("999.0"), Some("unsupportedVersion")),
            (json!(1.0), Some("badVersion")),
            (Value::Null, Some("badVersion")),
            (json!("1"), Some("badVersion")),
            (json!("1.7"), None),
            (json!("1.0"), None),
        ] {
            file(&dir, "state", &version, &json!({}));
            let (r, why) = read_checked(&dir, "state", |_| Ok(()));
            assert_eq!(why.as_ref().map(|w| w.reason), reason, "{version}");
            assert_eq!(matches!(r, Read::Corrupt(_)), reason.is_some(), "{version}");
        }
        // A wrong shape is corrupt as well; the other kinds are unaffected.
        file(&dir, "recent", &json!("1.0"), &json!({"wrong": true}));
        file(&dir, "keymap", &json!("1.0"), &json!({}));
        let (_, why) = read_checked(&dir, "recent", |d| if d.is_array() { Ok(()) } else { Err("not an array".into()) });
        assert_eq!(why.map(|w| w.reason), Some("wrongShape"));
        assert_eq!(read(&dir, "keymap"), Read::Data(json!({})));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_aside_never_replaces_an_earlier_copy() {
        let dir = std::env::temp_dir().join(format!("tt-settings-a-{}", fsx::random_token(4)));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("recent.json");
        let mut kept = Vec::new();
        for i in 0..5 {
            fs::write(&p, format!("copy {i}")).unwrap();
            kept.push(set_aside(&p).unwrap());
        }
        kept.sort();
        kept.dedup();
        assert_eq!(kept.len(), 5, "five distinct copies, whether or not they share a millisecond");
        let mut texts: Vec<String> = kept.iter().map(|k| fs::read_to_string(k).unwrap()).collect();
        texts.sort();
        assert_eq!(texts, (0..5).map(|i| format!("copy {i}")).collect::<Vec<_>>());
        // A file that cannot be kept aside says so.
        assert!(set_aside(&dir.join("absent.json")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn temporaries_are_cleaned_only_when_no_writer_holds_the_folder() {
        let dir = std::env::temp_dir().join(format!("tt-settings-t-{}", fsx::random_token(4)));
        fs::create_dir_all(&dir).unwrap();
        let orphan = dir.join(".preferences.json.0123456789ab.tmp");
        let foreign = dir.join(".agent-discovery.json.0123456789ab.tmp");
        fs::write(&orphan, b"x").unwrap();
        fs::write(&foreign, b"x").unwrap();
        // A writer (another process in real use) holds the lock shared: nothing is removed.
        let writer = lock_file(&dir).unwrap();
        writer.lock_shared().unwrap();
        assert_eq!(clean_temporaries(&dir), 0);
        assert!(orphan.exists());
        drop(writer);
        assert_eq!(clean_temporaries(&dir), 1);
        assert!(!orphan.exists() && foreign.exists(), "only settings temporaries are ours");
        let _ = fs::remove_dir_all(&dir);
    }
}
