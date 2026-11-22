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
    /// Messages about the settings files waiting to be shown (`prefs.corrupt` at load, `prefs.saveFailed` when a
    /// write fails; ruling O-12): `ui.message.shown` payloads, emitted by the session.
    pub notices: Vec<Value>,
    /// Settings files whose failed write was reported this session (once per file per session; ruling O-12).
    pub save_failed: std::collections::BTreeSet<String>,
    /// The stored workspaces read at load (`None`: missing or set aside).
    pub workspaces: Option<Value>,
    /// The stored drawing settings (M4 C4.2), read at start.
    pub drawing: Option<Value>,
    /// Keymap overrides relative to the bundled defaults (01 §8.1; `keymap.json`, M4 C4.1).
    pub keymap: crate::keymap::Overrides,
    /// Suppressed message ids (01 §9.4) and first-run flags (01 §11), kept in `state.json`.
    pub suppressed: std::collections::BTreeSet<String>,
    /// First-run flags (01 §1.1, §11).
    pub first_run: Map<String, Value>,
    /// The main window geometry restored at launch (01 §11).
    pub window: Option<Value>,
    /// Installed user themes (01 §5.8; `<settings>/Themes/*.json`), read at start and by `ui.theme.list`.
    pub themes: Vec<crate::themes::ThemeInfo>,
}

impl Settings {
    /// Ids of the valid installed user themes.
    pub(crate) fn theme_ids(&self) -> Vec<String> {
        self.themes.iter().filter(|t| t.valid).map(|t| t.theme_id.clone()).collect()
    }
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
    /// The last recovery heartbeat was skipped (another engine held the ownership lock): retried at the next
    /// maintenance pass (closure round 5).
    pub recovery_beat_pending: bool,
    pub last_watch: Option<Instant>,
    /// This engine's recovery ownership token (written beside each recovery entry; C41-R1-15).
    pub engine: String,
    /// Set when the host abandons this engine (it stopped answering): from then on it writes no autosave, recovery
    /// data or heartbeat, so the restarted engine's release of its data stays true if it wakes up.
    pub fence: std::sync::Arc<std::sync::atomic::AtomicBool>,
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
            recovery_beat_pending: false,
            last_watch: None,
            engine: format!("en_{}", tt_persist::fsx::random_token(12)),
            fence: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

/// The default of a preference (01 §7.3): the catalogue's ([`crate::prefs`]).
pub(crate) fn pref_default(key: &str) -> Option<Value> {
    crate::prefs::default_of(key)
}

use tt_persist::settings::KINDS;

/// The whole-file shape of each settings kind (C41-R1-08; ADR-0021): a file whose data is not this shape is corrupt
/// (kept aside, defaults for that file). Inside a well-shaped file, a value that fails its own validation is stale, not
/// corrupt: a preference falls back to its default, a binding or workspace entry is skipped.
fn shape(kind: &str, d: &Value) -> Result<(), String> {
    let member = |name: &str, ok: fn(&Value) -> bool, what: &str| -> Result<(), String> {
        match d.get(name) {
            None => Ok(()),
            Some(v) if ok(v) => Ok(()),
            Some(_) => Err(format!("`{name}` is not {what}")),
        }
    };
    let strings = |v: &Value| v.as_array().is_some_and(|a| a.iter().all(Value::is_string));
    match kind {
        "preferences" => d.is_object().then_some(()).ok_or_else(|| "not an object of key to value".to_owned()),
        "recent" => serde_json::from_value::<Vec<Recent>>(d.clone())
            .map(|_| ())
            .map_err(|e| format!("not a list of {{path, name, lastOpened}}: {e}")),
        "state" => {
            if !d.is_object() {
                return Err("not an object".into());
            }
            member("lastUsed", Value::is_object, "an object")?;
            member("suppressedMessages", strings, "a list of message ids")?;
            member("firstRun", Value::is_object, "an object")?;
            member("window", Value::is_object, "an object")
        }
        "keymap" => {
            if !d.is_object() {
                return Err("not an object".into());
            }
            member("baseSet", Value::is_string, "a string")?;
            member("added", Value::is_array, "a list")?;
            member("removed", Value::is_array, "a list")
        }
        "workspaces" => {
            let list = d.get("workspaces").and_then(Value::as_array).ok_or("no `workspaces` list")?;
            if list.iter().all(|w| w.get("name").is_some_and(Value::is_string)) {
                Ok(())
            } else {
                Err("a workspace without a name".into())
            }
        }
        "drawing" => crate::drawing::shape(d),
        _ => Ok(()),
    }
}

/// The `prefs.corrupt` message of a file that could not be used (01 §11), saying what actually happened to it.
fn corrupt_notice(a: &settings::SetAside) -> Value {
    let (text, kept) = match &a.kept {
        Ok(path) => (
            format!(
                "The {} settings file could not be used ({}); it was kept aside and its defaults are used.",
                a.kind, a.detail
            ),
            json!({ "keptAt": path.to_string_lossy() }),
        ),
        Err(why) => (
            format!(
                "The {} settings file could not be used ({}); its defaults are used, but it could not be kept aside: {why}",
                a.kind, a.detail
            ),
            json!({ "notKept": why }),
        ),
    };
    let mut details = json!({ "kind": a.kind, "reason": a.reason, "detail": a.detail });
    if let (Some(d), Some(k)) = (details.as_object_mut(), kept.as_object()) {
        d.extend(k.clone());
    }
    json!({ "messageId": "prefs.corrupt", "severity": "warning", "text": text, "details": details,
            "source": "user", "suppressed": false })
}

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
        // Owner records a dead process left without a completed recovery (ruling O-18).
        let _ = tt_persist::recovery::scavenge(&env.dirs.recovery);
        env
    }

    fn load_settings(&mut self) {
        // Temporary files of interrupted writes (C41-R1-09); only when no process is writing here.
        let _ = settings::clean_temporaries(&self.dirs.settings);
        for kind in KINDS {
            let (read, problem) = settings::read_checked(&self.dirs.settings, kind, |d| shape(kind, d));
            if let Some(a) = &problem {
                self.settings.notices.push(corrupt_notice(a));
            }
            match read {
                settings::Read::Missing | settings::Read::Corrupt(_) => {}
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
                        if let Some(a) = d.get("suppressedMessages").and_then(Value::as_array) {
                            self.settings.suppressed = a.iter().filter_map(Value::as_str).map(str::to_owned).collect();
                        }
                        if let Some(m) = d.get("firstRun").and_then(Value::as_object) {
                            self.settings.first_run = m.clone();
                        }
                        self.settings.window = d.get("window").filter(|w| w.is_object()).cloned();
                    }
                    "keymap" => self.settings.keymap = crate::keymap::Overrides::from_stored(&d),
                    "workspaces" => self.settings.workspaces = Some(d),
                    "drawing" => self.settings.drawing = Some(d),
                    _ => {}
                },
            }
        }
        self.settings.themes = crate::themes::scan(&self.dirs.settings.join("Themes"));
    }

    /// The stored workspaces (read when the executor starts).
    pub(crate) fn stored_workspaces(&self) -> Option<Value> {
        self.settings.workspaces.clone()
    }

    /// A preference value: the stored one while it still validates, else the default (M4 C4.1: a hand-edited or
    /// stale entry never breaks a read).
    pub(crate) fn pref(&self, key: &str) -> Option<Value> {
        crate::prefs::effective(key, self.settings.prefs.get(key), &self.settings.theme_ids())
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

    /// Writes one settings file and reports a failure to the user once per file per session (ruling O-12): the change
    /// stays in memory, the message names the file, the path and the reason.
    pub(crate) fn save(&mut self, kind: &str) {
        if let Err(e) = self.write(kind) {
            self.save_failed(kind, &e);
        }
    }

    /// Writes the stored workspaces (`data`), reporting a failure as [`Env::save`] does.
    /// The stored drawing settings (read when the executor starts).
    pub(crate) fn stored_drawing(&self) -> Option<Value> {
        self.settings.drawing.clone()
    }

    /// Writes the drawing settings file (a failure is told once, ruling O-12).
    pub(crate) fn save_drawing(&mut self, data: &Value) {
        if let Err(e) = settings::write(&self.dirs.settings, "drawing", data) {
            self.save_failed("drawing", &e);
        }
    }

    pub(crate) fn save_workspaces(&mut self, data: &Value) {
        if let Err(e) = settings::write(&self.dirs.settings, "workspaces", data) {
            self.save_failed("workspaces", &e);
        }
    }

    fn save_failed(&mut self, kind: &str, e: &tt_model::ModelError) {
        if !self.settings.save_failed.insert(kind.to_owned()) {
            return;
        }
        let path = self.dirs.settings.join(format!("{kind}.json"));
        self.settings.notices.push(json!({
            "messageId": "prefs.saveFailed",
            "severity": "warning",
            "text": format!("The {kind} settings could not be saved to {}: {}. The change applies until the application quits.", path.display(), e.message()),
            "details": { "kind": kind, "path": path.to_string_lossy(), "reason": e.details().get("reason").cloned().unwrap_or(Value::Null), "message": e.message() },
            "source": "user",
            "suppressed": false,
        }));
    }

    /// Messages waiting to be shown (the session emits them as `ui.message.shown`).
    pub(crate) fn take_notices(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.settings.notices)
    }

    /// Writes one settings file; the caller decides how to report a failure ([`Env::save`] reports it).
    pub(crate) fn write(&self, kind: &str) -> Result<(), tt_model::ModelError> {
        let data = match kind {
            "preferences" => Value::Object(self.settings.prefs.clone()),
            "recent" => serde_json::to_value(&self.settings.recent).unwrap_or(Value::Null),
            "state" => {
                let mut m = Map::new();
                m.insert("lastUsed".into(), Value::Object(self.settings.last_used.clone()));
                m.insert("suppressedMessages".into(), json!(self.settings.suppressed));
                m.insert("firstRun".into(), Value::Object(self.settings.first_run.clone()));
                if let Some(w) = &self.settings.window {
                    m.insert("window".into(), w.clone());
                }
                Value::Object(m)
            }
            "keymap" => self.settings.keymap.to_stored(),
            _ => return Ok(()),
        };
        settings::write(&self.dirs.settings, kind, &data)
    }

    /// Applies an accepted settings change and writes what it changed (C23-R3-01).
    pub(crate) fn apply(&mut self, change: crate::cx::SettingsChange) {
        match change {
            crate::cx::SettingsChange::NewProject { last_used, recent } => {
                self.settings.last_used.insert("newProject".into(), last_used);
                self.save("state");
                if let Some((path, name)) = recent {
                    self.add_recent(&path, &name);
                }
            }
            crate::cx::SettingsChange::RemoveRecent(path) => {
                self.settings.recent = recent_after_removal(&self.settings.recent, path.as_deref());
                self.save("recent");
            }
            crate::cx::SettingsChange::Keymap(o) => {
                self.settings.keymap = o;
                self.save("keymap");
            }
            crate::cx::SettingsChange::Suppressed(s) => {
                self.settings.suppressed = s;
                self.save("state");
            }
            crate::cx::SettingsChange::Window(w) => {
                self.settings.window = Some(w);
                self.save("state");
            }
        }
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
        self.save("recent");
    }

    /// Creates the per-user folders; a folder that cannot be created is reported once (ruling O-12).
    pub(crate) fn ensure_dirs(&mut self) {
        for d in [self.dirs.settings.clone(), self.dirs.recovery.clone(), self.dirs.backups.clone()] {
            if let Err(e) = std::fs::create_dir_all(&d) {
                let key = format!("folder:{}", d.display());
                if self.settings.save_failed.insert(key) {
                    self.settings.notices.push(json!({
                        "messageId": "prefs.saveFailed",
                        "severity": "warning",
                        "text": format!("The folder {} could not be created: {e}. Settings, recovery copies or backups there cannot be saved.", d.display()),
                        "details": { "path": d.to_string_lossy(), "reason": tt_persist::fsx::reason(&e), "message": e.to_string() },
                        "source": "user",
                        "suppressed": false,
                    }));
                }
            }
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

/// The recent list without `path` (compared after normalisation), or empty when no path is given (remove all).
pub(crate) fn recent_after_removal(recent: &[Recent], path: Option<&str>) -> Vec<Recent> {
    match path {
        None => Vec::new(),
        Some(path) => {
            let norm = normalize(path);
            recent.iter().filter(|r| normalize(&r.path) != norm).cloned().collect()
        }
    }
}
