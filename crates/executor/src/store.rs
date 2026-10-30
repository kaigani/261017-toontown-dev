//! Persistence state of the executor (C1.3; ADR-0014): the per-executor environment (user folders, settings, the
//! maintenance schedule, file-system roots) and the per-project store (where the project lives, its binary entries,
//! preserved data, lock, load report, autosave and watch state).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use tt_persist::container::{Blobs, Form};
use tt_persist::load::Preserved;
use tt_persist::lock::LockHandle;
use tt_persist::migrate::Migration;
use tt_persist::settings::{self, UserDirs};

/// When background maintenance runs (autosave, heartbeats, external-change detection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Maintenance {
    /// How often the document thread checks what is due (default 1 s; `Duration::ZERO` disables maintenance).
    pub tick: Duration,
    /// Lock and recovery heartbeat (LOCK-01: 60 s).
    pub heartbeat: Duration,
    /// External-modification and linked-media checks (RO-02; default 2 s).
    pub watch: Duration,
    /// Overrides the `files.autosave.intervalMinutes` preference (tests use seconds).
    pub autosave: Option<Duration>,
}

impl Default for Maintenance {
    fn default() -> Self {
        Self {
            tick: Duration::from_secs(1),
            heartbeat: Duration::from_secs(tt_persist::lock::HEARTBEAT_SECS),
            watch: Duration::from_secs(2),
            autosave: None,
        }
    }
}

/// Persistence options of an executor.
#[derive(Debug, Clone)]
pub struct PersistOptions {
    /// Per-user folders (settings, recovery, backups). `None`: an isolated folder under the system's temporary
    /// directory, created for this executor and removed when it stops (library embedding and tests). Hosts pass
    /// [`UserDirs::platform`].
    pub user_dirs: Option<UserDirs>,
    /// Maintenance schedule.
    pub maintenance: Maintenance,
    /// Migration chain (VER-03); the product chain by default.
    pub migrations: &'static [Migration],
}

impl Default for PersistOptions {
    fn default() -> Self {
        Self { user_dirs: None, maintenance: Maintenance::default(), migrations: tt_persist::migrate::CHAIN }
    }
}

/// A recent-projects entry (01 §3.3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Recent {
    pub path: String,
    pub name: String,
    pub last_opened: String,
}

/// Settings held in memory and written to the settings files (01 §11).
#[derive(Debug, Clone, Default)]
pub(crate) struct Settings {
    /// Preference values that differ from the defaults, by key (01 §7.2).
    pub prefs: Map<String, Value>,
    /// Recent projects, most recent first.
    pub recent: Vec<Recent>,
    /// Last-used values (most recent new-project format, folders).
    pub last_used: Map<String, Value>,
    /// Settings files found corrupt at load (reported once, `prefs.corrupt`).
    pub corrupt: Vec<String>,
}

/// The per-executor persistence environment.
#[derive(Debug, Clone)]
pub(crate) struct Env {
    pub dirs: UserDirs,
    /// The temporary root this executor created (removed when it stops).
    pub owned_root: Option<PathBuf>,
    /// Session cache for binary entries carried across saves (MED-04).
    pub cache: PathBuf,
    pub maintenance: Maintenance,
    pub migrations: &'static [Migration],
    pub settings: Settings,
    /// Posts work to the document thread (jobs installing projects, finishing saves).
    pub poster: Option<crate::Poster>,
    /// File-system roots granted to agent sessions (11 §6.2), by session ID.
    pub roots: BTreeMap<String, Vec<PathBuf>>,
    pub last_heartbeat: Option<Instant>,
    pub last_watch: Option<Instant>,
}

impl Default for Env {
    fn default() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tt-executor-{}-{}",
            std::process::id(),
            tt_persist::fsx::random_token(4)
        ));
        Self {
            dirs: UserDirs::under(&root),
            cache: root.join("Cache"),
            owned_root: Some(root),
            maintenance: Maintenance { tick: Duration::ZERO, ..Maintenance::default() },
            migrations: tt_persist::migrate::CHAIN,
            settings: Settings::default(),
            poster: None,
            roots: BTreeMap::new(),
            last_heartbeat: None,
            last_watch: None,
        }
    }
}

/// Preference defaults this kernel reads (01 §7.3).
pub(crate) fn pref_default(key: &str) -> Option<Value> {
    Some(match key {
        "files.autosave.enabled" => json!(true),
        "files.autosave.intervalMinutes" => json!(5),
        "files.backups.count" => json!(3),
        "general.recentProjects.maxCount" => json!(10),
        "general.workspace.autoSave" => json!(true),
        "project.defaults.frameRate" => json!(24),
        "project.defaults.cameraWidth" => json!(1920),
        "project.defaults.cameraHeight" => json!(1080),
        _ => return None,
    })
}

const KINDS: [&str; 4] = ["preferences", "recent", "state", "workspaces"];

impl Env {
    /// The environment for options; reads the settings files.
    pub(crate) fn new(opts: &PersistOptions) -> Self {
        let mut env = match &opts.user_dirs {
            Some(d) => Self {
                cache: std::env::temp_dir().join(format!(
                    "tt-cache-{}-{}",
                    std::process::id(),
                    tt_persist::fsx::random_token(4)
                )),
                dirs: d.clone(),
                owned_root: None,
                ..Self::default()
            },
            None => Self::default(),
        };
        env.maintenance = opts.maintenance;
        env.migrations = opts.migrations;
        env.load_settings();
        env
    }

    fn load_settings(&mut self) {
        for kind in KINDS {
            match settings::read(&self.dirs.settings, kind) {
                settings::Read::Missing => {}
                settings::Read::Corrupt(p) => self.settings.corrupt.push(p.to_string_lossy().into_owned()),
                settings::Read::Data(d) => match kind {
                    "preferences" => {
                        if let Value::Object(m) = d {
                            self.settings.prefs = m;
                        }
                    }
                    "recent" => self.settings.recent = serde_json::from_value(d).unwrap_or_default(),
                    "state" => {
                        if let Some(m) = d.get("lastUsed").and_then(Value::as_object) {
                            self.settings.last_used = m.clone();
                        }
                    }
                    _ => {}
                },
            }
        }
    }

    /// The stored workspaces (read when the executor starts).
    pub(crate) fn stored_workspaces(&self) -> Option<Value> {
        match settings::read(&self.dirs.settings, "workspaces") {
            settings::Read::Data(d) => Some(d),
            _ => None,
        }
    }

    /// A preference value (stored or default).
    pub(crate) fn pref(&self, key: &str) -> Option<Value> {
        self.settings.prefs.get(key).cloned().or_else(|| pref_default(key))
    }

    pub(crate) fn pref_u64(&self, key: &str) -> u64 {
        self.pref(key).and_then(|v| v.as_u64()).unwrap_or(0)
    }

    pub(crate) fn pref_f64(&self, key: &str) -> f64 {
        self.pref(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
    }

    pub(crate) fn pref_bool(&self, key: &str) -> bool {
        self.pref(key).and_then(|v| v.as_bool()).unwrap_or(false)
    }

    /// Writes one settings file; failures are reported to the caller (settings are best effort for the kernel).
    pub(crate) fn write(&self, kind: &str) -> Result<(), tt_model::ModelError> {
        let data = match kind {
            "preferences" => Value::Object(self.settings.prefs.clone()),
            "recent" => serde_json::to_value(&self.settings.recent).unwrap_or(Value::Null),
            "state" => json!({ "lastUsed": self.settings.last_used }),
            _ => return Ok(()),
        };
        settings::write(&self.dirs.settings, kind, &data)
    }

    /// Puts a project at the top of the recent list (01 §3.3: no duplicates, paths compared after normalisation).
    pub(crate) fn add_recent(&mut self, path: &Path, name: &str) {
        let p = path.to_string_lossy().into_owned();
        let norm = normalize(&p);
        self.settings.recent.retain(|r| normalize(&r.path) != norm);
        self.settings
            .recent
            .insert(0, Recent { path: p, name: name.to_owned(), last_opened: tt_persist::time::now_iso() });
        let max = usize::try_from(self.pref_u64("general.recentProjects.maxCount")).unwrap_or(10).max(1);
        self.settings.recent.truncate(max);
        let _ = self.write("recent");
    }

    /// The temporary root, created on first use.
    pub(crate) fn ensure_dirs(&self) {
        for d in [&self.dirs.settings, &self.dirs.recovery, &self.dirs.backups] {
            let _ = std::fs::create_dir_all(d);
        }
    }
}

/// Path normalisation for comparisons: resolved as far as it exists, case-folded on Windows.
pub(crate) fn normalize(p: &str) -> String {
    let r = tt_persist::fsx::resolve(Path::new(p)).to_string_lossy().into_owned();
    if cfg!(windows) { r.to_lowercase() } else { r }
}

/// A project's identity for change detection (RO-02): the package file's size and modification time, or for the folder
/// form those of `manifest.json` plus a fingerprint of every entry file's name, size and modification time
/// (C13-R1-06: folder entries are edited directly by text tools and version control, and the folder manifest does not
/// list them). Control files (first path segment starting with `.`) and `session.json` are not project content; hidden
/// names deeper down are preserved entries and count (C13-R2-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub len: u64,
    pub modified: Option<std::time::SystemTime>,
    /// Folder form: the entry-tree fingerprint (SHA-256 hex; empty for a package).
    pub tree: String,
}

pub(crate) fn stamp_of(path: &Path, form: Form) -> Option<Stamp> {
    match form {
        Form::Package => {
            let m = std::fs::metadata(path).ok()?;
            Some(Stamp { len: m.len(), modified: m.modified().ok(), tree: String::new() })
        }
        Form::Folder => {
            let m = std::fs::metadata(path.join(tt_persist::layout::MANIFEST)).ok()?;
            let mut lines = Vec::new();
            walk(path, path, 0, &mut lines);
            lines.sort();
            let tree = tt_persist::fsx::sha256(lines.join("\n").as_bytes());
            Some(Stamp { len: m.len(), modified: m.modified().ok(), tree })
        }
    }
}

/// The content of the project on disk (C14-R1-09; SI-047 interim ruling): a package's SHA-256; for a folder, the
/// SHA-256 of the sorted `name sha256` lines of every entry file (the fingerprint's file set; folder manifests carry
/// no per-entry hashes). Reads the whole project: computed at open, after saves and before a save replaces it, never
/// by the watch tick.
pub(crate) fn content_of(path: &Path, form: Form) -> Option<String> {
    match form {
        Form::Package => std::fs::File::open(path).and_then(tt_persist::fsx::sha256_reader).ok().map(|(h, _)| h),
        Form::Folder => {
            let mut files = Vec::new();
            entry_files(path, path, 0, &mut files);
            files.sort();
            let mut lines = Vec::with_capacity(files.len());
            for rel in files {
                let (h, _) = std::fs::File::open(path.join(&rel)).and_then(tt_persist::fsx::sha256_reader).ok()?;
                lines.push(format!("{} {h}", rel.to_string_lossy()));
            }
            Some(tt_persist::fsx::sha256(lines.join("\n").as_bytes()))
        }
    }
}

/// The entry files of a folder-form project, relative to its root (the rule of [`walk`]).
fn entry_files(root: &Path, dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if depth == 0 && (name.starts_with('.') || name == tt_persist::layout::SESSION) {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            if depth < 64 {
                entry_files(root, &p, depth + 1, out);
            }
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.push(rel.to_path_buf());
        }
    }
}

/// Lists `name size mtime` for every entry file under `dir`.
fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // Exactly what the folder container leaves out (ADR-0014 §2; C13-R2-04): control files, whose first path
        // segment starts with `.`, and `session.json`. Hidden names deeper down are preserved content (ZIP-04).
        if depth == 0 && (name.starts_with('.') || name == tt_persist::layout::SESSION) {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            if depth < 64 {
                walk(root, &p, depth + 1, out);
            }
        } else if let Ok(m) = e.metadata() {
            let rel = p.strip_prefix(root).map(|r| r.to_string_lossy().into_owned()).unwrap_or_default();
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            out.push(format!("{rel} {} {t}", m.len()));
        }
    }
}

/// The per-project persistence state.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    pub blobs: Blobs,
    pub preserved: Preserved,
    pub lock: Option<Arc<LockHandle>>,
    pub load_report: Option<Value>,
    pub migrated_from: Option<String>,
    pub format_version: String,
    pub recovery_id: String,
    /// `documentRev` of the last recovery copy.
    pub autosaved_rev: Option<u64>,
    pub last_autosave: Instant,
    pub autosaving: bool,
    pub saving: bool,
    pub stamp: Option<Stamp>,
    /// The content hash of the project on disk at open or the last save ([`content_of`]; C14-R1-09).
    pub content: Option<String>,
    /// Set when the file changed on disk (RO-02), with the detection time.
    pub changed_on_disk: Option<String>,
    pub saved_at: Option<String>,
    /// Resolved linked assets (MED-02).
    pub linked: BTreeMap<String, PathBuf>,
    /// Transient asset status (`ok`, `missing`, `changed`).
    pub asset_status: BTreeMap<String, String>,
    /// Lock holder when the project opened read-only because of it.
    pub locked_by: Option<Value>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            blobs: Blobs::default(),
            preserved: Preserved::default(),
            lock: None,
            load_report: None,
            migrated_from: None,
            format_version: tt_model::FORMAT_VERSION.to_owned(),
            recovery_id: tt_persist::recovery::new_id(),
            autosaved_rev: None,
            last_autosave: Instant::now(),
            autosaving: false,
            saving: false,
            stamp: None,
            content: None,
            changed_on_disk: None,
            saved_at: None,
            linked: BTreeMap::new(),
            asset_status: BTreeMap::new(),
            locked_by: None,
        }
    }
}
