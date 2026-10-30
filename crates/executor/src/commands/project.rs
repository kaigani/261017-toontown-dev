//! `project.*` (01 §3, §12.1–§12.4; 10 §6–§8): create, open, close, save, save as, project information and
//! properties, presets, recent projects, recovery and backups (C1.3; ADR-0014).
//!
//! Open, save, save as and recovery are jobs (01 §12.1 rule 11): the handler checks the call and the unsaved-changes
//! policy on the document thread, then reading or writing happens on the job's thread, and the result is installed
//! (open) or applied (save) back on the document thread through the guarded paths of the executor. A cancelled open
//! leaves no project open (LOAD-02); a save cannot be cancelled once writing starts (SAVE-03).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_persist::container::Form;
use tt_persist::lock::{self, Locked};
use tt_persist::{backup, recovery};
use tt_registry::{CommandError, codes, command};

use super::common::{Job, NoParams, OnExists, OnUnsaved, ResultWarning, event_not_invocable, invalid};
use crate::cx::{Cx, Effect};
use crate::persistence::{self, Snapshot, Written};
use crate::run::Install;
use crate::session::OpenProject;
use crate::types::{Origin, UnsavedPolicy};

/// How long a job waits for the document thread to install or apply its result.
const POST_TIMEOUT: Duration = Duration::from_secs(600);

fn yes() -> bool {
    true
}

fn poster(cx: &Cx) -> Result<crate::Poster, CommandError> {
    cx.session().env.poster.clone().ok_or_else(|| CommandError::internal("the document thread cannot be reached"))
}

fn exists_project(path: &Path) -> bool {
    path.is_file() || path.join(tt_persist::layout::PROJECT).is_file()
}

// ----- create ----------------------------------------------------------------------------------------------------

/// `project.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CreateParams {
    /// Project name (01 §9.6 shared name rule).
    #[schemars(length(min = 1, max = 100))]
    name: String,
    /// Save immediately at this absolute path (package; a path ending with a separator is the folder form).
    #[serde(default)]
    path: Option<String>,
    /// Format preset id (`project.presets.list`).
    #[serde(default)]
    preset: Option<String>,
    /// Frame rate.
    #[serde(default)]
    #[schemars(range(min = 1.0, max = 120.0))]
    frame_rate: Option<f64>,
    /// Camera width.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_width: Option<u32>,
    /// Camera height.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_height: Option<u32>,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
}

/// `project.create` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateResult {
    /// Project id.
    project_id: String,
    /// Saved path.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// First scene.
    scene_id: String,
    /// Its camera.
    camera_id: String,
    /// Its drawing element.
    element_id: String,
}

/// A preset (01 §3.1).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Preset {
    /// Id: `mostRecent`, `default`, or a bundled preset id.
    id: String,
    /// Display name.
    name: String,
    /// `mostRecent`, `default`, `bundled`.
    kind: String,
    /// Frame rate.
    frame_rate: f64,
    /// Camera width.
    width: u32,
    /// Camera height.
    height: u32,
}

fn presets(cx: &Cx) -> Vec<Preset> {
    let mut out = Vec::new();
    let env = &cx.session().env;
    if let Some(m) = env.settings.last_used.get("newProject")
        && let (Some(f), Some(w), Some(h)) = (
            m.get("frameRate").and_then(Value::as_f64),
            m.get("cameraWidth").and_then(Value::as_u64).and_then(|x| u32::try_from(x).ok()),
            m.get("cameraHeight").and_then(Value::as_u64).and_then(|x| u32::try_from(x).ok()),
        )
    {
        out.push(Preset {
            id: "mostRecent".into(),
            name: "Most recent".into(),
            kind: "mostRecent".into(),
            frame_rate: f,
            width: w,
            height: h,
        });
    }
    out.push(Preset {
        id: "default".into(),
        name: "Default".into(),
        kind: "default".into(),
        frame_rate: env.pref_f64("project.defaults.frameRate"),
        width: u32::try_from(env.pref_u64("project.defaults.cameraWidth")).unwrap_or(1920),
        height: u32::try_from(env.pref_u64("project.defaults.cameraHeight")).unwrap_or(1080),
    });
    // Bundled presets are data (01 §3.1); an unreadable preset file leaves only most recent and default.
    let bundled: Value = serde_json::from_str(include_str!("../presets.json")).unwrap_or(Value::Null);
    for p in bundled.get("presets").and_then(Value::as_array).into_iter().flatten() {
        let (Some(id), Some(name), Some(f), Some(w), Some(h)) = (
            p.get("id").and_then(Value::as_str),
            p.get("name").and_then(Value::as_str),
            p.get("frameRate").and_then(Value::as_f64),
            p.get("width").and_then(Value::as_u64).and_then(|x| u32::try_from(x).ok()),
            p.get("height").and_then(Value::as_u64).and_then(|x| u32::try_from(x).ok()),
        ) else {
            continue;
        };
        out.push(Preset {
            id: id.into(),
            name: name.into(),
            kind: "bundled".into(),
            frame_rate: f,
            width: w,
            height: h,
        });
    }
    out
}

fn name_error(e: &tt_model::ModelError) -> CommandError {
    let rule = e.details().get("rule").and_then(Value::as_str).unwrap_or_default();
    let code = match rule {
        "empty" => "name.empty",
        "tooLong" => "name.tooLong",
        _ => "name.invalidCharacter",
    };
    CommandError::with_details(code, e.message(), json!({ "pointer": "/name", "rule": rule }))
}

/// Locks a project path for writing (LOCK-01/02, RO-01): `(lock, readOnly, warnings, lock holder)`.
fn lock_for(path: &Path, read_only: bool) -> (Option<Arc<lock::LockHandle>>, bool, Vec<ResultWarning>, Option<Value>) {
    if read_only {
        return (None, true, Vec::new(), None);
    }
    let attr_ro = std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly());
    if attr_ro {
        return (
            None,
            true,
            vec![ResultWarning {
                id: tt_persist::codes::READ_ONLY.into(),
                message: "the project is on a read-only location; it opens read-only".into(),
                details: json!({ "path": path.to_string_lossy() }),
            }],
            None,
        );
    }
    match lock::acquire(path) {
        Locked::Acquired { handle, took_over } => {
            let w = took_over
                .map(|old| ResultWarning {
                    id: tt_persist::codes::LOCKED_BY_OTHER.into(),
                    message: "a stale lock was taken over".into(),
                    details: json!({ "stale": true, "takenOver": true, "lock": old }),
                })
                .into_iter()
                .collect();
            (Some(Arc::new(handle)), false, w, None)
        }
        Locked::HeldByOther(info) => {
            let holder = serde_json::to_value(&info).unwrap_or(Value::Null);
            (
                None,
                true,
                vec![ResultWarning {
                    id: tt_persist::codes::LOCKED_BY_OTHER.into(),
                    message: format!("the project is open on {} by {}; it opens read-only", info.host, info.user),
                    details: json!({ "lock": holder }),
                }],
                Some(holder),
            )
        }
        Locked::NotWritable(why) => (
            None,
            true,
            vec![ResultWarning {
                id: tt_persist::codes::READ_ONLY.into(),
                message: "the project's location is not writable; it opens read-only".into(),
                details: json!({ "path": path.to_string_lossy(), "reason": why }),
            }],
            None,
        ),
    }
}

/// Writes a snapshot for `project.create {path}` and returns the lock and the written result.
fn write_new(
    cx: &Cx,
    snap: &Snapshot,
    path: &Path,
    on_exists: OnExists,
) -> Result<(Written, Option<Arc<lock::LockHandle>>), CommandError> {
    if path.exists() && on_exists == OnExists::Fail {
        return Err(CommandError::with_details(
            tt_persist::codes::EXISTS,
            format!("`{}` exists", path.display()),
            json!({ "path": path.to_string_lossy() }),
        )
        .hint("pass onExists: \"overwrite\""));
    }
    let form = persistence::form_for(&path.to_string_lossy());
    let env = &cx.session().env;
    let w = persistence::write(snap, path, form, None, &env.cache, form == Form::Folder)?;
    let (lock, _, _, _) = lock_for(path, false);
    Ok((w, lock))
}

#[command(
    id = "project.create",
    kind = action,
    label = "New Project",
    scopes = [edit],
    confirm = ["overwrite", "discardUnsaved"],
    example(title = "A 1920×1080, 24 fps project", params = r#"{"name": "Walk cycle", "frameRate": 24, "cameraWidth": 1920, "cameraHeight": 1080}"#),
)]
pub(crate) fn project_create(cx: &mut Cx, p: CreateParams) -> Result<CreateResult, CommandError> {
    let name =
        tt_model::name::normalize_and_check(tt_model::ObjectType::Project, &p.name).map_err(|e| name_error(&e))?;
    let path = p.path.as_deref().map(|x| persistence::path_param(cx, x, "/path")).transpose()?;
    if let Some(path) = &path
        && cx.origin() != &Origin::User
        && !cx.origin().has_scope(tt_registry::Scope::FilesWrite)
    {
        return Err(CommandError::with_details(
            codes::PERMISSION_DENIED,
            "saving a new project needs the scope files.write",
            json!({ "commandId": "project.create", "missingScopes": ["files.write"], "path": path.to_string_lossy() }),
        ));
    }
    let preset = match &p.preset {
        Some(id) => Some(presets(cx).into_iter().find(|x| &x.id == id).ok_or_else(|| {
            CommandError::with_details(
                codes::ARG_INVALID,
                format!("no preset `{id}`"),
                json!({ "pointer": "/preset", "preset": id }),
            )
            .hint("project.presets.list names the presets")
        })?),
        None => presets(cx)
            .into_iter()
            .find(|x| x.kind == "mostRecent")
            .or_else(|| presets(cx).into_iter().find(|x| x.kind == "default")),
    };
    let frame_rate = p.frame_rate.or(preset.as_ref().map(|x| x.frame_rate)).unwrap_or(24.0);
    let camera_width = p.camera_width.or(preset.as_ref().map(|x| x.width)).unwrap_or(1920);
    let camera_height = p.camera_height.or(preset.as_ref().map(|x| x.height)).unwrap_or(1080);
    persistence::resolve_unsaved(cx, p.on_unsaved)?;
    let spec = tt_model::defaults::NewProject { name, frame_rate, camera_width, camera_height };
    let mut ids = tt_model::IdGenerator::system();
    let now = crate::time::now_iso();
    let (doc, created) = tt_model::defaults::new_project(&spec, &mut ids, &now).map_err(crate::cx::model_error)?;
    let mut project = OpenProject::new(doc.clone(), None, false);
    project.permanent_dirty = false;
    if let Some(path) = &path
        && p.on_exists == OnExists::Overwrite
    {
        persistence::confirm_overwrite(cx, path)?;
    }
    if let Some(path) = &path
        && !cx.dry_run()
    {
        let snap = Snapshot {
            session: json!({}),
            doc: doc.clone(),
            preserved: tt_persist::Preserved::default(),
            blobs: tt_persist::Blobs::default(),
            position: 0,
            top: None,
            saved_at: now.clone(),
        };
        let (w, lock) = write_new(cx, &snap, path, p.on_exists)?;
        project.path = Some(path.to_string_lossy().into_owned());
        project.form = Some(w.form.as_str().to_owned());
        project.saved_rev = Some(doc.document_rev());
        if let Some(c) = &w.container {
            project.store.blobs = tt_persist::Blobs::from_container(c.clone());
        }
        project.store.lock = lock;
        project.store.saved_at = Some(now.clone());
        project.store.stamp = crate::store::stamp_of(path, w.form);
    }
    if !cx.dry_run() {
        let env = &mut cx.session_mut().env;
        env.settings.last_used.insert(
            "newProject".into(),
            json!({ "frameRate": frame_rate, "cameraWidth": camera_width, "cameraHeight": camera_height }),
        );
        let _ = env.write("state");
        if let Some(path) = &path {
            env.add_recent(path, doc.project().name());
        }
    }
    if cx.has_project() {
        cx.defer(Effect::Close);
    }
    cx.defer(Effect::Install(Box::new(Install { project, repairs: None, restore: None })));
    Ok(CreateResult {
        project_id: created.project.as_str().to_owned(),
        path: path.map(|x| x.to_string_lossy().into_owned()),
        scene_id: created.scene.as_str().to_owned(),
        camera_id: created.camera.as_str().to_owned(),
        element_id: created.element.as_str().to_owned(),
    })
}

// ----- open ------------------------------------------------------------------------------------------------------

/// `project.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OpenParams {
    /// Absolute path of a package, or of a folder-form project.
    path: String,
    /// Open read-only.
    #[serde(default)]
    read_only: bool,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// The final result of `project.open`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenResult {
    /// Project id.
    project_id: String,
    /// Opened read-only.
    read_only: bool,
    /// The file's format version.
    format_version: String,
    /// Warnings of the open (load report issues, locks; 01 §12.1 rule 6).
    warnings: Vec<ResultWarning>,
}

/// What the load gives the executor to install.
pub(crate) fn project_from_loaded(
    loaded: tt_persist::Loaded,
    path: Option<String>,
    read_only: bool,
    lock: Option<Arc<lock::LockHandle>>,
    locked_by: Option<Value>,
) -> (Install, Vec<ResultWarning>) {
    let mut warnings: Vec<ResultWarning> = loaded
        .report
        .issues
        .iter()
        .filter(|i| i.severity != tt_model::document::Severity::Info)
        .map(|i| ResultWarning { id: i.code.clone(), message: i.message.clone(), details: i.details.clone() })
        .collect();
    let mut report = serde_json::to_value(&loaded.report).unwrap_or(Value::Null);
    if let Some(m) = report.as_object_mut() {
        m.insert("readOnly".into(), json!(read_only));
    }
    let form = loaded.form;
    let doc = loaded.doc;
    let mut project = OpenProject::new(doc.clone(), path.clone(), read_only);
    project.form = Some(form.as_str().to_owned());
    project.saved_rev = Some(loaded.saved_rev);
    project.permanent_dirty = path.is_none();
    let repairs = if loaded.repaired {
        // REP-01: the repairs are one transaction (origin `load`, "Repair on Open"); the project is then modified.
        let mut history = crate::history::History::default();
        let id = history.next_step_id();
        project.history = crate::history::History::with_base_step(crate::history::Step {
            id,
            label: "Repair on Open".into(),
            origin: Origin::Load,
            time: crate::time::now_iso(),
            edit: crate::history::StepEdit::Snapshot { before: Box::new(doc.clone()), after: Box::new(doc.clone()) },
            touched: Vec::new(),
            structural: true,
        });
        project.permanent_dirty = true;
        let mut changes: Vec<Value> = Vec::new();
        for i in loaded.report.issues.iter().filter(|i| i.repaired) {
            let Some(id) = &i.object_id else { continue };
            if changes.iter().any(|c| c["id"] == json!(id)) {
                continue;
            }
            match doc.locate(id) {
                Some(l) => changes.push(json!({ "id": l.key, "type": l.ty.as_str(), "op": "modified", "rev": l.rev })),
                None => {
                    changes.push(json!({ "id": id, "type": "unknown", "op": "deleted", "rev": doc.document_rev() }))
                }
            }
        }
        Some(Value::Array(changes))
    } else {
        None
    };
    project.store.blobs = loaded.blobs;
    project.store.preserved = loaded.preserved;
    project.store.lock = lock;
    project.store.locked_by = locked_by;
    project.store.load_report = Some(report);
    project.store.migrated_from = loaded.migrated_from.map(|v| v.to_string());
    project.store.format_version = loaded.report.format_version.clone();
    project.store.saved_at = loaded.saved_at;
    project.store.linked = loaded.linked;
    project.store.stamp = path.as_ref().and_then(|p| crate::store::stamp_of(Path::new(p), form));
    for m in &loaded.missing {
        if m.kind == "linked" {
            project.store.asset_status.insert(m.object_id.clone(), "missing".into());
        }
    }
    for a in doc.project().assets() {
        project.store.asset_status.entry(a.id().as_str().to_owned()).or_insert_with(|| "ok".into());
    }
    if let Some(from) = &project.store.migrated_from {
        warnings.push(ResultWarning {
            id: "project.migrated".into(),
            message: format!("the project was migrated from format {from}; saving it upgrades the file"),
            details: json!({ "migratedFrom": from }),
        });
    }
    (Install { project, repairs, restore: loaded.session }, warnings)
}

fn job_failed(e: &CommandError) -> CommandError {
    e.clone()
}

#[command(
    id = "project.open",
    kind = job,
    label = "Open…",
    scopes = [edit, files_read],
    confirm = ["discardUnsaved"],
    example(title = "Open a package", params = r#"{"path": "/tmp/Short.animpkg"}"#),
)]
pub(crate) fn project_open(cx: &mut Cx, p: OpenParams) -> Result<Job<OpenResult>, CommandError> {
    let path = persistence::path_param(cx, &p.path, "/path")?;
    if !path.exists() {
        return Err(tt_persist::fsx::open_error(&std::io::Error::from(std::io::ErrorKind::NotFound), &path).into());
    }
    if cx.dry_run() {
        return Ok(Job::dry());
    }
    persistence::resolve_unsaved(cx, p.on_unsaved)?;
    // 01 §3.2: the current project is closed before the new one is loaded; a failed or cancelled open leaves none.
    if cx.has_project() {
        cx.defer(Effect::Close);
    }
    let poster = poster(cx)?;
    let origin = cx.origin().clone();
    let migrations = cx.session().env.migrations;
    let read_only = p.read_only;
    let job = cx.session().jobs.submit("project.open", &origin.session_key(), move |progress| {
        let (lock, read_only, lock_warnings, locked_by) = lock_for(&path, read_only);
        let report = |f: f64, text: &str| {
            progress.report(f * 0.95, text);
            !progress.cancelled()
        };
        let opts = tt_persist::LoadOptions { read_only, migrations, progress: &report, ..Default::default() };
        let loaded = tt_persist::load(&path, &opts)?;
        let format_version = loaded.report.format_version.clone();
        let project_id = loaded.doc.project().id().as_str().to_owned();
        let (inst, mut warnings) =
            project_from_loaded(loaded, Some(path.to_string_lossy().into_owned()), read_only, lock, locked_by);
        warnings.extend(lock_warnings);
        let name = inst.project.doc.project().name().clone();
        let progress2 = progress.clone();
        let recent = path.clone();
        let env = poster.run(POST_TIMEOUT, move |core| {
            let env = core.install_guarded(inst, UnsavedPolicy::Fail, &origin, Some(&progress2));
            if env.is_ok()
                && let Some(s) = core.session_mut()
            {
                s.env.add_recent(&recent, &name);
            }
            env
        })?;
        if let Some(e) = &env.error {
            return Err(job_failed(e));
        }
        progress.report(1.0, "Finalisation");
        Ok(serde_json::to_value(OpenResult { project_id, read_only, format_version, warnings }).unwrap_or(Value::Null))
    })?;
    Ok(Job::new(job))
}

// ----- close ------------------------------------------------------------------------------------------------------

/// `{onUnsaved}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct UnsavedParams {
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// `{closed}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ClosedResult {
    /// The project was closed.
    closed: bool,
}

#[command(
    id = "project.close",
    kind = action,
    label = "Close Project",
    scopes = [edit],
    confirm = ["discardUnsaved"],
    needs_project,
    example(title = "Close, failing if unsaved", params = "{}"),
)]
pub(crate) fn project_close(cx: &mut Cx, p: UnsavedParams) -> Result<ClosedResult, CommandError> {
    persistence::resolve_unsaved(cx, p.on_unsaved)?;
    cx.defer(Effect::Close);
    Ok(ClosedResult { closed: true })
}

// ----- save, save as ------------------------------------------------------------------------------------------------

/// `project.save` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SaveParams {
    /// Also save the global library (08; libraries arrive with M9: never modified before).
    #[serde(default = "yes")]
    #[allow(dead_code, reason = "no global library is open before M9")]
    include_global_library: bool,
    /// Allow writing a newer format version (VER-03).
    #[serde(default)]
    allow_format_upgrade: bool,
}

/// The final result of `project.save`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveResult {
    /// Where.
    path: String,
    /// The project was saved.
    project_saved: bool,
    /// The global library was saved.
    global_library_saved: bool,
    /// The backup of the previous version (BAK-01).
    #[serde(skip_serializing_if = "Option::is_none")]
    backup_id: Option<String>,
}

/// Starts the save job of a snapshot.
#[allow(clippy::too_many_arguments)]
fn submit_save(
    cx: &mut Cx,
    command: &'static str,
    snap: Snapshot,
    target: PathBuf,
    form: Form,
    backups: Option<backup::Backups>,
    lock: bool,
) -> Result<String, CommandError> {
    let poster = poster(cx)?;
    // The lock this session holds on its current location: a write there must still own it (C13-R1-02).
    let held = if lock { None } else { cx.project()?.store.lock.clone() };
    let origin = cx.origin().clone();
    let cache = cx.session().env.cache.clone();
    if let Ok(p) = cx.project_mut() {
        p.store.saving = true;
    }
    let job = cx.session().jobs.submit(command, &origin.session_key(), move |progress| {
        if !progress.commit() {
            return Err(CommandError::with_details(
                "job.cancelled",
                "the save was cancelled before writing started",
                json!({}),
            ));
        }
        progress.report(0.05, "Serialise");
        let new_lock = if lock {
            match lock::acquire(&target) {
                Locked::Acquired { handle, .. } => Some(Arc::new(handle)),
                Locked::HeldByOther(info) => {
                    let _ = poster.run(POST_TIMEOUT, |core| {
                        if let Some(p) = core.session_mut().and_then(|s| s.project.as_mut()) {
                            p.store.saving = false;
                        }
                    });
                    return Err(CommandError::with_details(
                        tt_persist::codes::SAVE_FAILED,
                        "the destination is open in another session",
                        json!({ "reason": "fileInUse", "lock": info }),
                    ));
                }
                Locked::NotWritable(_) => None,
            }
        } else {
            if let Some(h) = &held
                && !h.held()
            {
                let _ = poster.run(POST_TIMEOUT, |core| {
                    if let Some(p) = core.session_mut().and_then(|s| s.project.as_mut()) {
                        p.store.saving = false;
                    }
                });
                return Err(CommandError::with_details(
                    tt_persist::codes::SAVE_FAILED,
                    "this session no longer holds the project's lock; another session took it over",
                    json!({ "reason": "fileInUse", "lock": lock::read(h.path()) }),
                ));
            }
            None
        };
        progress.report(0.1, "Write");
        let written = persistence::write(&snap, &target, form, backups.as_ref(), &cache, form == Form::Folder);
        progress.report(0.95, "Replace");
        let applied = written.clone();
        let snap2 = snap.clone();
        let backup_id = poster.run(POST_TIMEOUT, move |core| {
            let s = core.session_mut()?;
            match &applied {
                Ok(w) => persistence::finish_save(s, &snap2, w, true, new_lock),
                Err(_) => {
                    if let Some(p) = s.project.as_mut() {
                        p.store.saving = false;
                    }
                    None
                }
            }
        })?;
        let w = written?;
        Ok(serde_json::to_value(SaveResult {
            path: w.path.to_string_lossy().into_owned(),
            project_saved: true,
            global_library_saved: false,
            backup_id,
        })
        .unwrap_or(Value::Null))
    })?;
    Ok(job)
}

#[command(
    id = "project.save",
    kind = job,
    label = "Save",
    scopes = [edit, files_write],
    confirm = ["externalChange"],
    needs_project,
    example(title = "Save", params = "{}"),
)]
pub(crate) fn project_save(cx: &mut Cx, p: SaveParams) -> Result<Job<SaveResult>, CommandError> {
    // RO-02 / SI-045: an agent gets the choice as `needs_confirmation` (accept = keep mine: overwrite; the other choice
    // is reloading with project.open). The confirmation is bound to the file's identity and this session's lock, so
    // a file changed again, or a lock lost, after the confirmation was requested refuses the token.
    let mut external_ok = false;
    if matches!(cx.origin(), crate::types::Origin::Agent { .. })
        && let Some((path, at, binding)) = persistence::external_change(cx.project()?)
    {
        cx.confirm(
            crate::agents::Category::ExternalChange,
            format!("`{path}` was changed by another program since it was opened; saving overwrites those changes"),
            vec![
                json!({ "kind": "overwriteExternalChange", "path": path, "detectedAt": at }),
                json!({ "kind": "alternative", "choice": "reload", "commandId": "project.open",
                        "params": { "path": path, "onUnsaved": "discard" } }),
            ],
            Some(binding),
        )?;
        external_ok = true;
    }
    let (path, form) = persistence::check_saveable(cx.project()?, p.allow_format_upgrade, external_ok)?;
    if cx.dry_run() {
        return Ok(Job::dry());
    }
    let dirty = cx.project()?.dirty();
    if !dirty && exists_project(&path) {
        // LAY-04: a clean project saves without rewriting any entry.
        let result = SaveResult {
            path: path.to_string_lossy().into_owned(),
            project_saved: true,
            global_library_saved: false,
            backup_id: None,
        };
        let value = serde_json::to_value(result).unwrap_or(Value::Null);
        let origin = cx.origin().clone();
        let job = cx.session().jobs.submit("project.save", &origin.session_key(), move |_| Ok(value))?;
        return Ok(Job::new(job));
    }
    let now = crate::time::now_iso();
    persistence::stamp_modified(cx, &now)?;
    let snap = persistence::snapshot(cx, now)?;
    let backups = persistence::backups(&cx.session().env, &snap.doc, &path);
    let job = submit_save(cx, "project.save", snap, path, form, Some(backups), false)?;
    Ok(Job::new(job))
}

/// `project.saveAs` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SaveAsParams {
    /// Absolute target path (a path ending with a separator, or an existing folder, is the folder form).
    path: String,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
    /// Also save the global library (08; M9).
    #[serde(default = "yes")]
    #[allow(dead_code, reason = "no global library is open before M9")]
    include_global_library: bool,
    /// Allow writing a newer format version (VER-03).
    #[serde(default)]
    allow_format_upgrade: bool,
}

/// The final result of `project.saveAs`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveAsResult {
    /// Where.
    path: String,
    /// The project was saved.
    project_saved: bool,
    /// The global library was saved.
    global_library_saved: bool,
}

#[command(
    id = "project.saveAs",
    kind = job,
    label = "Save As…",
    scopes = [edit, files_write],
    confirm = ["overwrite"],
    needs_project,
    example(title = "Save a copy", params = r#"{"path": "/tmp/Copy.animpkg"}"#),
)]
pub(crate) fn project_save_as(cx: &mut Cx, p: SaveAsParams) -> Result<Job<SaveAsResult>, CommandError> {
    let target = persistence::path_param(cx, &p.path, "/path")?;
    let form = persistence::form_for(&p.path);
    let project = cx.project()?;
    persistence::check_upgrade(project, p.allow_format_upgrade)?;
    let current = project.path.as_ref().map(PathBuf::from);
    let same = current.as_ref().is_some_and(|c| {
        crate::store::normalize(&c.to_string_lossy()) == crate::store::normalize(&target.to_string_lossy())
    });
    if let Some(c) = &current
        && project.form_kind() == Form::Folder
        && !same
        && tt_persist::fsx::is_within(&target, c)
    {
        return Err(CommandError::with_details(
            "project.pathInsideProject",
            "a project cannot be saved inside itself",
            json!({ "path": target.to_string_lossy(), "project": c.to_string_lossy() }),
        ));
    }
    if target.exists() && !same && p.on_exists == OnExists::Fail {
        return Err(CommandError::with_details(
            tt_persist::codes::EXISTS,
            format!("`{}` exists", target.display()),
            json!({ "path": target.to_string_lossy() }),
        )
        .hint("pass onExists: \"overwrite\""));
    }
    if !same {
        persistence::confirm_overwrite(cx, &target)?;
    }
    if cx.dry_run() {
        return Ok(Job::dry());
    }
    let now = crate::time::now_iso();
    if cx.project()?.dirty() {
        persistence::stamp_modified(cx, &now)?;
    }
    let snap = persistence::snapshot(cx, now)?;
    let backups = same.then(|| persistence::backups(&cx.session().env, &snap.doc, &target));
    // Saving over the current location needs this session's lock: a session that opened the project read-only (a live
    // lock held by someone else, LOCK-02) must take the lock first, and fails while the owner holds it (C13-R1-02).
    let owns_lock = cx.project()?.store.lock.is_some();
    let job = submit_save(cx, "project.saveAs", snap, target, form, backups, !same || !owns_lock)?;
    Ok(Job::new(job))
}

// ----- information and properties ---------------------------------------------------------------------------------

/// A scene in `ProjectInfo`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SceneInfo {
    /// Scene id.
    id: String,
    /// Name.
    name: String,
    /// Scene length.
    frame_count: u64,
}

/// `ProjectInfo` (01 §12.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectInfo {
    /// Project id.
    id: String,
    /// Name.
    name: String,
    /// Location (`null` when never saved).
    path: Option<String>,
    /// Opened read-only.
    read_only: bool,
    /// Format version of the project (`MAJOR.MINOR`).
    format_version: String,
    /// Modified.
    dirty: bool,
    /// The global library is modified (08; M9).
    global_library_dirty: bool,
    /// Last save (`null` when never saved).
    last_saved: Option<String>,
    /// Frame rate.
    frame_rate: f64,
    /// Camera width.
    camera_width: u32,
    /// Camera height.
    camera_height: u32,
    /// Scenes in storyline order.
    scenes: Vec<SceneInfo>,
    /// The current scene.
    current_scene_id: Option<String>,
}

#[command(
    id = "project.info.get",
    kind = query,
    label = "Project information",
    scopes = [read],
    needs_project,
    example(title = "ProjectInfo (01 §12.4)", params = "{}"),
    gui = "Project › Properties (M4)",
)]
pub(crate) fn project_info_get(cx: &mut Cx, _p: NoParams) -> Result<ProjectInfo, CommandError> {
    let p = cx.project()?;
    let doc = cx.document()?;
    let project = doc.project();
    Ok(ProjectInfo {
        id: project.id().as_str().to_owned(),
        name: project.name().clone(),
        path: p.path.clone(),
        read_only: p.read_only,
        format_version: p.store.format_version.clone(),
        dirty: p.dirty(),
        global_library_dirty: false,
        last_saved: p.store.saved_at.clone(),
        frame_rate: *project.frame_rate(),
        camera_width: *project.camera_width(),
        camera_height: *project.camera_height(),
        scenes: doc
            .scenes()
            .map(|s| SceneInfo {
                id: s.id().as_str().to_owned(),
                name: s.name().clone(),
                frame_count: doc.scene_length(s),
            })
            .collect(),
        current_scene_id: cx.app().scene.as_ref().map(|s| s.as_str().to_owned()),
    })
}

/// Project properties.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Properties {
    /// Frame rate.
    frame_rate: f64,
    /// Camera width.
    camera_width: u32,
    /// Camera height.
    camera_height: u32,
}

fn properties(cx: &Cx) -> Result<Properties, CommandError> {
    let project = cx.document()?.project();
    Ok(Properties {
        frame_rate: *project.frame_rate(),
        camera_width: *project.camera_width(),
        camera_height: *project.camera_height(),
    })
}

#[command(
    id = "project.properties.get",
    kind = query,
    label = "Project properties",
    scopes = [read],
    needs_project,
    example(title = "Frame rate and camera size", params = "{}"),
    gui = "Project › Properties (M4)",
)]
pub(crate) fn project_properties_get(cx: &mut Cx, _p: NoParams) -> Result<Properties, CommandError> {
    properties(cx)
}

/// `project.properties.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PropertiesSetParams {
    /// Frame rate.
    #[serde(default)]
    #[schemars(range(min = 1.0, max = 120.0))]
    frame_rate: Option<f64>,
    /// Camera width.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_width: Option<u32>,
    /// Camera height.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_height: Option<u32>,
    /// Make these the defaults for new projects (`project.defaults.*`).
    #[serde(default)]
    set_as_default: bool,
}

#[command(
    id = "project.properties.set",
    kind = action,
    label = "Set project properties",
    scopes = [edit],
    undo = "Change Project Properties",
    needs_project,
    example(title = "25 fps", params = r#"{"frameRate": 25}"#),
    gui = "Project › Properties (M4)",
)]
pub(crate) fn project_properties_set(cx: &mut Cx, p: PropertiesSetParams) -> Result<Properties, CommandError> {
    let id = cx.document()?.project().id().as_str().to_owned();
    let mut ops = Vec::new();
    let op = |path: &str, v: Value| tt_model::change::PatchOp {
        op: "replace",
        from: None,
        path: path.into(),
        range: None,
        value: Some(v),
        old_value: None,
        elided: false,
    };
    if let Some(f) = p.frame_rate {
        ops.push(op("/frameRate", json!(f)));
    }
    if let Some(w) = p.camera_width {
        ops.push(op("/cameraWidth", json!(w)));
    }
    if let Some(h) = p.camera_height {
        ops.push(op("/cameraHeight", json!(h)));
    }
    if !ops.is_empty() {
        cx.patch_objects(vec![(id, ops)])?;
    }
    let now = properties(cx)?;
    if p.set_as_default {
        let mut m = Map::new();
        m.insert("project.defaults.frameRate".into(), json!(now.frame_rate));
        m.insert("project.defaults.cameraWidth".into(), json!(now.camera_width));
        m.insert("project.defaults.cameraHeight".into(), json!(now.camera_height));
        cx.defer(Effect::Prefs(m));
        cx.warn(
            "project.defaultsChanged",
            "the values are stored as defaults; they affect new projects only",
            Vec::new(),
        );
    }
    Ok(now)
}

#[command(
    id = "project.presets.list",
    kind = query,
    label = "Format presets",
    scopes = [read],
    example(title = "Presets for new projects", params = "{}"),
    gui = "Start screen new-project form (M4)",
)]
pub(crate) fn project_presets_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<Preset>, CommandError> {
    Ok(presets(cx))
}

// ----- recent ------------------------------------------------------------------------------------------------------

/// A recent project (01 §3.3).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecentEntry {
    /// Path.
    path: String,
    /// Project name.
    name: String,
    /// When it was last opened or saved.
    last_opened: String,
    /// Whether the project still exists.
    exists: bool,
}

#[command(
    id = "project.recent.list",
    kind = query,
    label = "Recent projects",
    scopes = [read],
    example(title = "Recent projects", params = "{}"),
    gui = "File › Open Recent and the start screen (M4)",
)]
pub(crate) fn project_recent_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<RecentEntry>, CommandError> {
    Ok(cx
        .session()
        .env
        .settings
        .recent
        .iter()
        .map(|r| RecentEntry {
            exists: exists_project(Path::new(&r.path)),
            path: r.path.clone(),
            name: r.name.clone(),
            last_opened: r.last_opened.clone(),
        })
        .collect())
}

/// `project.recent.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecentRemoveParams {
    /// One entry.
    #[serde(default)]
    path: Option<String>,
    /// Every entry.
    #[serde(default)]
    all: bool,
}

/// `{remaining}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Remaining {
    /// Entries left.
    remaining: u32,
}

#[command(
    id = "project.recent.remove",
    kind = ui,
    label = "Remove from recent",
    scopes = [ui],
    example(title = "Clear the list", params = r#"{"all": true}"#),
    gui = "File › Open Recent › Clear Menu (M4)",
)]
pub(crate) fn project_recent_remove(cx: &mut Cx, p: RecentRemoveParams) -> Result<Remaining, CommandError> {
    if p.path.is_none() && !p.all {
        return Err(invalid("/path", "give `path` or `all: true`"));
    }
    if !cx.dry_run() {
        let env = &mut cx.session_mut().env;
        if p.all {
            env.settings.recent.clear();
        } else if let Some(path) = &p.path {
            let norm = crate::store::normalize(path);
            env.settings.recent.retain(|r| crate::store::normalize(&r.path) != norm);
        }
        let _ = env.write("recent");
    }
    Ok(Remaining { remaining: u32::try_from(cx.session().env.settings.recent.len()).unwrap_or(u32::MAX) })
}

// ----- recovery ----------------------------------------------------------------------------------------------------

/// Recovery data at launch (REC-01).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryEntry {
    /// Recovery id.
    recovery_id: String,
    /// Project name.
    project_name: String,
    /// The project's location (`null` for an untitled project).
    original_path: Option<String>,
    /// When the copy was written.
    saved_at: String,
    /// Whether the copy loads (validated like a project, 10 §7).
    usable: bool,
    /// Why it is not usable.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[command(
    id = "project.recovery.list",
    kind = query,
    label = "Recovery data",
    scopes = [read],
    example(title = "Recoverable sessions", params = "{}"),
    gui = "Recovery dialog at launch (M4)",
)]
pub(crate) fn project_recovery_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<RecoveryEntry>, CommandError> {
    let root = cx.session().env.dirs.recovery.clone();
    Ok(recovery::list(&root, false, true)
        .into_iter()
        .map(|e| RecoveryEntry {
            recovery_id: e.info.recovery_id,
            project_name: e.info.project_name,
            original_path: e.info.original_path,
            saved_at: e.info.saved_at,
            usable: e.usable,
            reason: e.reason,
        })
        .collect())
}

/// `project.recovery.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecoveryOpenParams {
    /// Recovery id.
    recovery_id: String,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// The final result of `project.recovery.open`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryOpened {
    /// Project id.
    project_id: String,
    /// Warnings (load issues, `project.recoveryOlderThanFile`).
    warnings: Vec<ResultWarning>,
}

fn recovery_not_found(id: &str) -> CommandError {
    CommandError::with_details(
        "project.recoveryNotFound",
        format!("no recovery data `{id}`"),
        json!({ "recoveryId": id }),
    )
    .hint("project.recovery.list names the recovery data")
}

#[command(
    id = "project.recovery.open",
    kind = job,
    label = "Recover",
    scopes = [edit],
    confirm = ["discardUnsaved"],
    example(title = "Recover a session", params = r#"{"recoveryId": "rc_1a2b3c4d"}"#),
    gui = "Recovery dialog at launch (M4)",
)]
pub(crate) fn project_recovery_open(cx: &mut Cx, p: RecoveryOpenParams) -> Result<Job<RecoveryOpened>, CommandError> {
    let root = cx.session().env.dirs.recovery.clone();
    let info = recovery::info(&root, &p.recovery_id).ok_or_else(|| recovery_not_found(&p.recovery_id))?;
    let own = cx.session().project.as_ref().is_some_and(|x| x.store.recovery_id == p.recovery_id);
    if own || !info.orphaned() {
        return Err(CommandError::with_details(
            "project.recoveryInUse",
            "the recovery data belongs to a running session",
            json!({ "recoveryId": p.recovery_id, "processId": info.process_id }),
        ));
    }
    if cx.dry_run() {
        return Ok(Job::dry());
    }
    persistence::resolve_unsaved(cx, p.on_unsaved)?;
    // Claim the data now, so that a second session is refused at once (C13-R1-08).
    let previous = match recovery::claim(&root, &p.recovery_id) {
        recovery::Claim::Claimed(prev) => prev,
        recovery::Claim::InUse(info) => {
            return Err(CommandError::with_details(
                "project.recoveryInUse",
                "the recovery data belongs to a running session",
                json!({ "recoveryId": p.recovery_id, "processId": info.process_id }),
            ));
        }
        recovery::Claim::NotFound => return Err(recovery_not_found(&p.recovery_id)),
    };
    if cx.has_project() {
        cx.defer(Effect::Close);
    }
    let poster = poster(cx)?;
    let origin = cx.origin().clone();
    let migrations = cx.session().env.migrations;
    let id = p.recovery_id;
    let release_root = root.clone();
    let job = cx.session().jobs.submit("project.recovery.open", &origin.session_key(), move |progress| {
        let outcome = recover_claimed(&root, &id, &info, migrations, &poster, &origin, progress);
        if outcome.is_err() {
            recovery::release(&release_root, &previous);
        }
        outcome
    })?;
    Ok(Job::new(job))
}

/// The job half of `project.recovery.open`, once the data is claimed.
fn recover_claimed(
    root: &Path,
    id: &str,
    info: &recovery::RecoveryInfo,
    migrations: &'static [tt_persist::migrate::Migration],
    poster: &crate::Poster,
    origin: &crate::types::Origin,
    progress: &crate::jobs::JobProgress,
) -> Result<Value, CommandError> {
    {
        let folder = recovery::project_folder(root, id);
        let report = |f: f64, text: &str| {
            progress.report(f * 0.95, text);
            !progress.cancelled()
        };
        let loaded = tt_persist::load(
            &folder,
            &tt_persist::LoadOptions { read_only: true, migrations, progress: &report, ..Default::default() },
        )?;
        let project_id = loaded.doc.project().id().as_str().to_owned();
        // REC-02: the copy opens as a modified project whose location is the original path.
        let original = info.original_path.clone();
        let mut warnings = Vec::new();
        let (lock, read_only, lock_warnings, locked_by) = match &original {
            Some(o) if Path::new(o).exists() => lock_for(Path::new(o), false),
            _ => (None, false, Vec::new(), None),
        };
        if let Some(o) = &original
            && let Some(file_saved) = recovery::file_saved_at(Path::new(o))
            && file_saved > info.saved_at
        {
            warnings.push(ResultWarning {
                id: tt_persist::codes::RECOVERY_OLDER.into(),
                message: "the project file was saved after this recovery copy; saving will replace it".into(),
                details: json!({ "fileSavedAt": file_saved, "recoverySavedAt": info.saved_at }),
            });
        }
        let original_form = original.as_ref().map(|o| if Path::new(o).is_dir() { Form::Folder } else { Form::Package });
        let (mut inst, w) = project_from_loaded(loaded, original.clone(), read_only, lock, locked_by);
        warnings.extend(w);
        warnings.extend(lock_warnings);
        inst.project.form = original_form.map(|f| f.as_str().to_owned());
        inst.project.permanent_dirty = true;
        inst.project.store.recovery_id = id.to_owned();
        inst.project.store.autosaved_rev = Some(info.document_rev);
        inst.project.store.stamp = original
            .as_ref()
            .and_then(|o| crate::store::stamp_of(Path::new(o), original_form.unwrap_or(Form::Package)));
        // The blobs of the copy stay readable from the recovery folder; the original's location is the save target.
        let progress2 = progress.clone();
        let origin = origin.clone();
        let env = poster.run(POST_TIMEOUT, move |core| {
            core.install_guarded(inst, UnsavedPolicy::Fail, &origin, Some(&progress2))
        })?;
        if let Some(e) = &env.error {
            return Err(job_failed(e));
        }
        Ok(serde_json::to_value(RecoveryOpened { project_id, warnings }).unwrap_or(Value::Null))
    }
}

/// `project.recovery.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecoveryDeleteParams {
    /// One entry.
    #[serde(default)]
    recovery_id: Option<String>,
    /// Every entry.
    #[serde(default)]
    all: bool,
}

/// `{deleted}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Deleted {
    /// Entries deleted.
    deleted: u32,
}

#[command(
    id = "project.recovery.delete",
    kind = ui,
    label = "Discard recovery data",
    scopes = [ui],
    example(title = "Discard everything", params = r#"{"all": true}"#),
    gui = "Recovery dialog at launch (M4)",
)]
pub(crate) fn project_recovery_delete(cx: &mut Cx, p: RecoveryDeleteParams) -> Result<Deleted, CommandError> {
    let root = cx.session().env.dirs.recovery.clone();
    let targets: Vec<String> = if p.all {
        recovery::list(&root, false, false).into_iter().map(|e| e.info.recovery_id).collect()
    } else if let Some(id) = &p.recovery_id {
        let info = recovery::info(&root, id).ok_or_else(|| recovery_not_found(id))?;
        if !info.orphaned() {
            return Err(CommandError::with_details(
                "project.recoveryInUse",
                "the recovery data belongs to a running session",
                json!({ "recoveryId": id }),
            ));
        }
        vec![id.clone()]
    } else {
        return Err(invalid("/recoveryId", "give `recoveryId` or `all: true`"));
    };
    let mut deleted = 0u32;
    if !cx.dry_run() {
        for id in targets {
            if recovery::delete(&root, &id)? {
                deleted += 1;
            }
        }
    }
    Ok(Deleted { deleted })
}

// ----- backups -----------------------------------------------------------------------------------------------------

/// `project.backup.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupListParams {
    /// Project path (default: the current project).
    #[serde(default)]
    path: Option<String>,
}

/// A backup (01 §3.7).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupEntry {
    /// Backup id.
    backup_id: String,
    /// When the backed-up version was saved.
    saved_at: String,
    /// Size.
    size_bytes: u64,
    /// Its `documentRev`.
    document_rev: u64,
}

#[command(
    id = "project.backup.list",
    kind = query,
    label = "Backups",
    scopes = [read],
    example(title = "Backups of the current project", params = "{}"),
    gui = "File › Open Backup (M4)",
)]
pub(crate) fn project_backup_list(cx: &mut Cx, p: BackupListParams) -> Result<Vec<BackupEntry>, CommandError> {
    let project_id = match &p.path {
        Some(path) => {
            let path = persistence::path_param(cx, path, "/path")?;
            let c = tt_persist::Container::open(&path, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default())?;
            let m = tt_persist::manifest::Manifest::parse(&c.read(tt_persist::layout::MANIFEST, 64 << 20)?)?;
            m.project_id.ok_or_else(|| invalid("/path", "the project's manifest names no project"))?
        }
        None => cx.document()?.project().id().as_str().to_owned(),
    };
    if !tt_model::id::is_valid_id(&project_id) {
        return Ok(Vec::new());
    }
    let folder = cx.session().env.dirs.backups.join(&project_id);
    Ok(backup::list(&folder)
        .into_iter()
        .map(|r| BackupEntry {
            backup_id: r.backup_id,
            saved_at: r.saved_at,
            size_bytes: r.size_bytes,
            document_rev: r.document_rev,
        })
        .collect())
}

/// `project.backup.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BackupOpenParams {
    /// Backup id.
    backup_id: String,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// `{projectId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectId {
    /// Project id.
    project_id: String,
}

#[command(
    id = "project.backup.open",
    kind = action,
    label = "Open backup",
    scopes = [edit],
    confirm = ["discardUnsaved"],
    example(title = "Open a backup as an untitled copy", params = r#"{"backupId": "bk_1a2b3c4d"}"#),
    gui = "File › Open Backup (M4)",
)]
pub(crate) fn project_backup_open(cx: &mut Cx, p: BackupOpenParams) -> Result<ProjectId, CommandError> {
    let root = cx.session().env.dirs.backups.clone();
    let (file, _) = backup::find(&root, &p.backup_id).ok_or_else(|| {
        CommandError::with_details(
            "project.backupNotFound",
            format!("no backup `{}`", p.backup_id),
            json!({ "backupId": p.backup_id }),
        )
        .hint("project.backup.list names the backups")
    })?;
    // BAK-04: the backup is read, never modified; it opens as an untitled copy.
    let loaded = tt_persist::load(
        &file,
        &tt_persist::LoadOptions { read_only: true, migrations: cx.session().env.migrations, ..Default::default() },
    )?;
    let project_id = loaded.doc.project().id().as_str().to_owned();
    persistence::resolve_unsaved(cx, p.on_unsaved)?;
    let (mut inst, warnings) = project_from_loaded(loaded, None, false, None, None);
    inst.project.form = None;
    inst.project.permanent_dirty = inst.repairs.is_some();
    for w in warnings {
        cx.warn(&w.id, w.message, Vec::new());
    }
    if cx.has_project() {
        cx.defer(Effect::Close);
    }
    cx.defer(Effect::Install(Box::new(inst)));
    Ok(ProjectId { project_id })
}

// ----- events ------------------------------------------------------------------------------------------------------

/// `project.autosaved` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Autosaved {
    /// Project id.
    project_id: String,
    /// Recovery id.
    recovery_id: String,
    /// When.
    saved_at: String,
}

#[command(
    id = "project.autosaved",
    kind = event,
    label = "Autosaved",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_autosaved(_cx: &mut Cx, _p: NoParams) -> Result<Autosaved, CommandError> {
    Err(event_not_invocable("project.autosaved"))
}

#[command(
    id = "project.closed",
    kind = event,
    label = "Project closed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_closed(_cx: &mut Cx, _p: NoParams) -> Result<ProjectId, CommandError> {
    Err(event_not_invocable("project.closed"))
}

/// `project.dirty.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirtyChanged {
    /// Project modified.
    project_dirty: bool,
    /// Global library modified.
    global_library_dirty: bool,
}

#[command(
    id = "project.dirty.changed",
    kind = event,
    label = "Modified state changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_dirty_changed(_cx: &mut Cx, _p: NoParams) -> Result<DirtyChanged, CommandError> {
    Err(event_not_invocable("project.dirty.changed"))
}

/// `project.opened` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Opened {
    /// Project id.
    project_id: String,
    /// Path.
    path: Option<String>,
    /// Read-only.
    read_only: bool,
}

#[command(
    id = "project.opened",
    kind = event,
    label = "Project opened",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_opened(_cx: &mut Cx, _p: NoParams) -> Result<Opened, CommandError> {
    Err(event_not_invocable("project.opened"))
}

/// `project.saved` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedEvent {
    /// Project id.
    project_id: String,
    /// Where.
    path: String,
    /// The project was saved.
    project_saved: bool,
    /// The global library was saved.
    global_library_saved: bool,
}

#[command(
    id = "project.saved",
    kind = event,
    label = "Project saved",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_saved(_cx: &mut Cx, _p: NoParams) -> Result<SavedEvent, CommandError> {
    Err(event_not_invocable("project.saved"))
}
