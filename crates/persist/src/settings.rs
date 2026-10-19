//! Persisted user settings (01 §11) and the per-user folders (01 §3.7).
//!
//! Settings are human-readable, versioned JSON files, one per kind, each `{format: "toontown-settings", kind,
//! formatVersion, data}`, written atomically. A missing file means defaults; a corrupt file is kept aside as a copy
//! (`<name>.corrupt-<time>.json`), the defaults are used for that file only, and the caller tells the user once
//! (`prefs.corrupt`). Older versions are migrated on load (none exist: 1.0 is the first).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_model::ModelError;

use crate::{fsx, time};

/// Settings format version.
pub const VERSION: &str = "1.0";

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
    /// The file was corrupt; it was kept aside at this path and defaults apply.
    Corrupt(PathBuf),
}

/// Reads settings file `<dir>/<kind>.json`.
#[must_use]
pub fn read(dir: &Path, kind: &str) -> Read {
    let p = dir.join(format!("{kind}.json"));
    let Ok(bytes) = fsx::read_limited(&p, 64 << 20) else {
        return if p.exists() { set_aside(&p) } else { Read::Missing };
    };
    let parsed = String::from_utf8(bytes)
        .ok()
        .and_then(|t| tt_model::json_text::parse::<Value>(&t).ok())
        .filter(|v| {
            v.get("format").and_then(Value::as_str) == Some("toontown-settings")
                && v.get("kind").and_then(Value::as_str) == Some(kind)
        })
        .and_then(|v| v.get("data").cloned());
    match parsed {
        Some(d) => Read::Data(d),
        None => set_aside(&p),
    }
}

fn set_aside(p: &Path) -> Read {
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let aside = p.with_file_name(format!("{stem}.corrupt-{}.json", time::file_stamp(&time::now_iso())));
    let _ = fs::rename(p, &aside);
    Read::Corrupt(aside)
}

/// Writes settings file `<dir>/<kind>.json` atomically.
///
/// # Errors
/// `fs.ioError`.
pub fn write(dir: &Path, kind: &str, data: &Value) -> Result<(), ModelError> {
    let v = json!({ "format": "toontown-settings", "kind": kind, "formatVersion": VERSION, "data": data });
    let text = tt_model::canonical::to_canonical_string(&v)?;
    fsx::write_atomic(&dir.join(format!("{kind}.json")), text.as_bytes(), fsx::fs_error)
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
}
