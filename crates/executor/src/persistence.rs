//! Persistence inside the executor (C1.3; ADR-0014): path parameters and file-system roots, save snapshots, writing a
//! snapshot (on any thread), applying a finished save to the open project, the `modifiedAt` stamp, the unsaved-changes
//! policy, and closing a project.
//!
//! **Saves serialise a snapshot** taken between transactions (SAVE-04): the document is an immutable copy-on-write
//! value, so the snapshot is a clone; it is written on a job thread, and the result is applied on the document thread,
//! which sets the clean marker to the snapshot's undo position. A save of a modified project first stamps
//! `project.modifiedAt` (10 §4.1 "set by each save") as a *system* transaction: it has its own revision and
//! `doc.changed` event (origin `system`), and no undo step.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use tt_model::document::Document;
use tt_persist::backup::Backups;
use tt_persist::container::{BlobData, Blobs, Container, Form};
use tt_persist::load::Preserved;
use tt_persist::lock::LockHandle;
use tt_persist::save::{SaveInput, Saved};
use tt_persist::{fsx, layout, recovery};
use tt_registry::{CommandError, Scope, codes};

use crate::app::AppContext;
use crate::cx::Cx;
use crate::session::{OpenProject, Session};
use crate::store::{self, Env};
use crate::types::Origin;

/// A path parameter: absolute (01 §12.1 rule 2), and for an agent inside its file-system roots or the open
/// project's folder (11 §6.2; symbolic links resolved before the check).
///
/// # Errors
/// `fs.invalidPath`, `fs.outsideRoots`.
pub(crate) fn path_param(cx: &Cx, path: &str, pointer: &str) -> Result<PathBuf, CommandError> {
    let p = fsx::absolute(path, pointer)?;
    check_roots(cx.session(), cx.origin(), &p, pointer)?;
    Ok(p)
}

/// The file-system roots of 11 §6.2 for an origin.
///
/// # Errors
/// `fs.outsideRoots`.
pub(crate) fn check_roots(s: &Session, origin: &Origin, p: &Path, pointer: &str) -> Result<(), CommandError> {
    let Origin::Agent { session, .. } = origin else { return Ok(()) };
    let mut roots: Vec<PathBuf> = s.env.roots.get(session).cloned().unwrap_or_default();
    if let Some(folder) = s
        .project
        .as_ref()
        .and_then(|p| p.path.as_ref())
        .map(PathBuf::from)
        .and_then(|p| if p.is_dir() { Some(p) } else { p.parent().map(Path::to_path_buf) })
    {
        roots.push(folder);
    }
    if roots.iter().any(|r| fsx::is_within(p, r)) {
        return Ok(());
    }
    Err(CommandError::with_details(
        "fs.outsideRoots",
        format!("`{}` is outside the folders this session may access", p.display()),
        json!({
            "path": p.to_string_lossy(),
            "pointer": pointer,
            "roots": roots.iter().map(|r| r.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        }),
    )
    .hint("ask the user to grant the folder (11 §6.2)"))
}

/// `session.json` (10 §6.12): the last-session UI state of the project.
pub(crate) fn session_json(doc: &Document, app: &AppContext) -> Value {
    json!({
        "format": "toontown-session",
        "currentSceneId": app.scene.as_ref().map(|s| s.as_str()),
        "frame": app.frame,
        "currentElementId": app.element.as_ref().map(|e| e.as_str()),
        "openDrawingId": app.current_drawing(doc).map(|d| d.as_str().to_owned()),
    })
}

/// Restores the UI state of `session.json` when it is valid for the document (ignored otherwise).
pub(crate) fn restore_session(app: &mut AppContext, doc: &Document, v: &Value) {
    let id = |k: &str| v.get(k).and_then(Value::as_str).and_then(|s| tt_model::Id::parse(s).ok());
    if let Some(scene) = id("currentSceneId").filter(|s| doc.scene(s).is_some()) {
        app.scene = Some(scene.clone());
        app.element = id("currentElementId").filter(|e| doc.scene_of(e) == Some(&scene)).or(app.element.clone());
        if let Some(f) = v.get("frame").and_then(Value::as_u64).and_then(|f| u32::try_from(f).ok()).filter(|f| *f >= 1)
        {
            app.frame = f;
        }
        app.repair(doc);
    }
}

/// A save snapshot (SAVE-04).
#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    pub doc: Document,
    pub preserved: Preserved,
    pub blobs: Blobs,
    pub session: Value,
    pub position: usize,
    pub top: Option<String>,
    pub saved_at: String,
}

impl Snapshot {
    pub(crate) fn input(&self) -> SaveInput<'_> {
        SaveInput {
            doc: &self.doc,
            preserved: &self.preserved,
            blobs: &self.blobs,
            session: Some(&self.session),
            saved_at: self.saved_at.clone(),
        }
    }
}

/// Stamps `project.modifiedAt` (10 §4.1) as a system transaction of this invocation; returns the stamped document.
///
/// # Errors
/// Staging errors (revision exhaustion).
pub(crate) fn stamp_modified(cx: &mut Cx, now: &str) -> Result<(), CommandError> {
    let project = cx.document()?.project().id().as_str().to_owned();
    let op = tt_model::change::PatchOp {
        op: "replace",
        from: None,
        path: "/modifiedAt".into(),
        range: None,
        value: Some(json!(now)),
        old_value: None,
        elided: false,
    };
    cx.patch_entries(vec![(tt_model::document::EntryKey::Project, vec![op])])?;
    let _ = project;
    cx.seal_system("Save")
}

/// The snapshot of the open project as this invocation sees it (after a `modifiedAt` stamp, when there is one).
///
/// # Errors
/// `project.none`.
pub(crate) fn snapshot(cx: &Cx, saved_at: String) -> Result<Snapshot, CommandError> {
    let p = cx.project()?;
    let doc = cx.document()?.clone();
    Ok(Snapshot {
        session: session_json(&doc, cx.app()),
        doc,
        preserved: p.store.preserved.clone(),
        blobs: p.store.blobs.clone(),
        position: p.history.position(),
        top: p.history.step_id_before(p.history.position()).map(str::to_owned),
        saved_at,
    })
}

/// What a finished write produced.
#[derive(Debug, Clone)]
pub(crate) struct Written {
    pub path: PathBuf,
    pub form: Form,
    pub saved: Saved,
    /// The written container (the project's new blob source).
    pub container: Option<Container>,
    /// Binary entries of the replaced container that the new one does not hold, kept for undo (MED-04).
    pub carried: Vec<(String, PathBuf)>,
}

/// The backup sink of a project (BAK-01…BAK-03).
pub(crate) fn backups(env: &Env, doc: &Document, original: &Path) -> Backups {
    Backups {
        folder: env.dirs.backups.join(doc.project().id().as_str()),
        count: u32::try_from(env.pref_u64("files.backups.count")).unwrap_or(3),
        original_path: original.to_string_lossy().into_owned(),
        project_name: doc.project().name().clone(),
    }
}

/// Writes a snapshot at `target` in `form` (any thread). When the write replaces the container the project's binary
/// entries come from, the entries the new container will not hold are first copied to the session cache.
///
/// # Errors
/// The save errors of 10 SAVE-05.
pub(crate) fn write(
    snap: &Snapshot,
    target: &Path,
    form: Form,
    backups: Option<&Backups>,
    cache: &Path,
    git_files: bool,
) -> Result<Written, CommandError> {
    let mut carried = Vec::new();
    if let Some(base) = snap.blobs.base()
        && store::normalize(&base.path().to_string_lossy()) == store::normalize(&target.to_string_lossy())
    {
        let keep: std::collections::BTreeSet<String> =
            tt_persist::save::plan(&snap.input())?.into_iter().map(|p| p.name).collect();
        for name in base.names() {
            let k = layout::classify(&name);
            let binary = matches!(
                k,
                layout::Kind::Media | layout::Kind::Pixels | layout::Kind::Texture | layout::Kind::LibraryFile
            );
            if binary && !keep.contains(&name) {
                let to = cache.join(fsx::random_token(8)).join(name.replace('/', "_"));
                if let Some(dir) = to.parent() {
                    std::fs::create_dir_all(dir)
                        .map_err(|e| CommandError::from(fsx::save_error(&e, dir, "cannot create")))?;
                }
                let mut r = base.reader(&name, base.size(&name).unwrap_or(0))?;
                let mut f = std::fs::File::create(&to)
                    .map_err(|e| CommandError::from(fsx::save_error(&e, &to, "cannot create")))?;
                std::io::copy(&mut r, &mut f)
                    .map_err(|e| CommandError::from(fsx::save_error(&e, &to, "cannot write")))?;
                carried.push((name, to));
            }
        }
    }
    let sink: Option<&dyn tt_persist::save::BackupSink> = backups.map(|b| b as &dyn tt_persist::save::BackupSink);
    let saved = tt_persist::save::save(target, form, &snap.input(), sink, git_files)?;
    let container =
        Container::open(target, tt_persist::manifest::PACKAGE_MIMETYPE, tt_persist::zip::Limits::default()).ok();
    Ok(Written { path: target.to_path_buf(), form, saved, container, carried })
}

/// Applies a finished save to the open project (document thread): clean marker at the snapshot's position, saved
/// revision, location, binary-entry source, recovery data deleted (AUTO-04), `project.saved`, the recent list.
pub(crate) fn finish_save(
    s: &mut Session,
    snap: &Snapshot,
    w: &Written,
    becomes_current: bool,
    lock: Option<Arc<LockHandle>>,
) -> Option<String> {
    let recovery_root = s.env.dirs.recovery.clone();
    let p = s.project.as_mut()?;
    if p.doc.project().id() != snap.doc.project().id() {
        return None;
    }
    if becomes_current {
        p.history.mark_clean_at(snap.position, snap.top.as_deref());
        p.permanent_dirty = false;
        p.saved_rev = Some(snap.doc.document_rev());
        p.path = Some(w.path.to_string_lossy().into_owned());
        p.form = Some(w.form.as_str().to_owned());
        p.read_only = false;
        let mut blobs = match &w.container {
            Some(c) => Blobs::from_container(c.clone()),
            None => p.store.blobs.clone(),
        };
        // Entries added in the session and not yet in the new container stay available (undo, MED-04).
        for (name, data) in p.store.blobs.extra() {
            if !blobs.contains(name) {
                blobs.insert(name.clone(), data.clone());
            }
        }
        for (name, path) in &w.carried {
            blobs.insert(name.clone(), BlobData::File(path.clone()));
        }
        p.store.blobs = blobs;
        p.store.migrated_from = None;
        p.store.saved_at = Some(snap.saved_at.clone());
        p.store.stamp = store::stamp_of(&w.path, w.form);
        p.store.changed_on_disk = None;
        if let Some(l) = lock {
            p.store.lock = Some(l);
        }
        let _ = recovery::delete(&recovery_root, &p.store.recovery_id);
        p.store.autosaved_rev = None;
    }
    p.store.saving = false;
    let project_id = p.doc.project().id().as_str().to_owned();
    let name = p.doc.project().name().clone();
    s.emit(
        "project.saved",
        json!({ "projectId": project_id, "path": w.path.to_string_lossy(), "projectSaved": true, "globalLibrarySaved": false }),
    );
    if becomes_current {
        s.env.add_recent(&w.path, &name);
    }
    w.saved.backup_id.clone()
}

/// The form a Save As target gets: an existing folder or a path ending with a separator is the folder form (10
/// FMT-02, D7), anything else the package form.
pub(crate) fn form_for(path: &str) -> Form {
    if path.ends_with('/') || path.ends_with('\\') || Path::new(path).is_dir() { Form::Folder } else { Form::Package }
}

/// `project.unsavedChanges` / the save of `onUnsaved: "save"` / the scope check of `"discard"` (01 §12.1 rule 3).
///
/// # Errors
/// `project.unsavedChanges`, `project.pathRequired`, `project.readOnly`, the save's errors, `permission.denied`.
pub(crate) fn resolve_unsaved(cx: &mut Cx, policy: crate::commands::common::OnUnsaved) -> Result<(), CommandError> {
    use crate::commands::common::OnUnsaved;
    let Some(p) = cx.session().project.as_ref() else { return Ok(()) };
    if !p.dirty() {
        return Ok(());
    }
    match policy {
        OnUnsaved::Fail => Err(crate::run::unsaved_changes(p)),
        OnUnsaved::Discard => {
            if let Origin::Agent { scopes, .. } = cx.origin()
                && !scopes.contains(&Scope::App)
            {
                return Err(CommandError::with_details(
                    codes::PERMISSION_DENIED,
                    "discarding unsaved changes needs the scope app",
                    json!({ "commandId": cx.command(), "missingScopes": ["app"] }),
                ));
            }
            Ok(())
        }
        OnUnsaved::Save => save_now(cx, false).map(|_| ()),
    }
}

/// Checks that the open project may be saved to its location.
///
/// # Errors
/// `project.pathRequired`, `project.readOnly`, `project.formatUpgrade`, `project.changedOnDisk`.
pub(crate) fn check_saveable(p: &OpenProject, allow_upgrade: bool) -> Result<(PathBuf, Form), CommandError> {
    let Some(path) = p.path.clone() else {
        return Err(CommandError::with_details(
            "project.pathRequired",
            "the project has no location yet",
            json!({ "projectId": p.doc.project().id().as_str() }),
        )
        .hint("save with project.saveAs {path}"));
    };
    if p.read_only {
        return Err(CommandError::with_details(
            tt_persist::codes::READ_ONLY,
            "the project was opened read-only",
            json!({ "path": path, "lock": p.store.locked_by }),
        )
        .hint("save a copy with project.saveAs"));
    }
    check_upgrade(p, allow_upgrade)?;
    if let Some(at) = &p.store.changed_on_disk {
        return Err(CommandError::with_details(
            tt_persist::codes::CHANGED_ON_DISK,
            "the project file was changed by another program since it was opened",
            json!({ "path": path, "detectedAt": at }),
        )
        .hint("reopen it (project.open with onUnsaved) to reload, or keep yours with project.saveAs {path, onExists: \"overwrite\"}"));
    }
    Ok((PathBuf::from(path), p.form_kind()))
}

/// VER-03: the first save of a migrated project needs `allowFormatUpgrade`.
///
/// # Errors
/// `project.formatUpgrade`.
pub(crate) fn check_upgrade(p: &OpenProject, allow_upgrade: bool) -> Result<(), CommandError> {
    if let Some(from) = &p.store.migrated_from
        && !allow_upgrade
    {
        return Err(CommandError::with_details(
            tt_persist::codes::FORMAT_UPGRADE,
            format!(
                "saving upgrades the project from format {from} to {}; older versions will no longer open it",
                tt_model::FORMAT_VERSION
            ),
            json!({ "from": from, "to": tt_model::FORMAT_VERSION }),
        )
        .hint("pass allowFormatUpgrade: true"));
    }
    Ok(())
}

/// Saves the open project to its location now, on the document thread (the `onUnsaved: "save"` paths). Returns the
/// backup ID.
///
/// # Errors
/// As [`check_saveable`] and [`write`].
pub(crate) fn save_now(cx: &mut Cx, allow_upgrade: bool) -> Result<Option<String>, CommandError> {
    let (path, form) = check_saveable(cx.project()?, allow_upgrade)?;
    let now = crate::time::now_iso();
    if cx.project()?.dirty() {
        stamp_modified(cx, &now)?;
    }
    let snap = snapshot(cx, now)?;
    let env = cx.session().env.clone();
    let b = backups(&env, &snap.doc, &path);
    let w = write(&snap, &path, form, Some(&b), &env.cache, form == Form::Folder)?;
    Ok(finish_save(cx.session_mut(), &snap, &w, true, None))
}

/// Closes the open project: `project.closed`, the lock released, the recovery data deleted when the close discards
/// or the project is clean (01 §3.4, AUTO-04).
pub(crate) fn close(s: &mut Session) {
    let Some(p) = s.project.take() else { return };
    let _ = recovery::delete(&s.env.dirs.recovery, &p.store.recovery_id);
    s.emit("project.closed", json!({ "projectId": p.doc.project().id().as_str() }));
    drop(p);
    s.app = AppContext::default();
}
