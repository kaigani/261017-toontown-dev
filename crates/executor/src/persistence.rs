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
use crate::cx::{Cx, Effect};
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

/// Writes a snapshot at `target` in `form` (any thread). The binary entries of the project's current source that the
/// new container will not hold are first copied to the session cache, so undo can still reach them (C13-R2-02).
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
    // The project's binary entries are about to come from the written container. Entries the old source holds and the
    // new one will not (MED-04: media nothing uses now, still needed by undo) are copied to the session cache first,
    // whatever the target: a same-path save replaces their source, and a Save As elsewhere or a recovered project's
    // save switches away from it (C13-R2-02).
    let mut carried = Vec::new();
    if let Some(base) = snap.blobs.base() {
        let keep: std::collections::BTreeSet<String> =
            tt_persist::save::plan(&snap.input())?.into_iter().map(|p| p.name).collect();
        for name in base.names() {
            let k = layout::classify(&name);
            let binary = matches!(
                k,
                layout::Kind::Media | layout::Kind::Pixels | layout::Kind::Texture | layout::Kind::LibraryFile
            );
            if binary && !keep.contains(&name) {
                carried.push((name.clone(), carry(base, &name, cache)?));
            }
        }
    }
    let sink: Option<&dyn tt_persist::save::BackupSink> = backups.map(|b| b as &dyn tt_persist::save::BackupSink);
    let saved = tt_persist::save::save(target, form, &snap.input(), sink, git_files)?;
    let container =
        Container::open(target, tt_persist::manifest::PACKAGE_MIMETYPE, tt_persist::zip::Limits::default()).ok();
    Ok(Written { path: target.to_path_buf(), form, saved, container, carried })
}

/// Copies one binary entry of `base` into the session cache, content-addressed (C13-R3-03): the bytes are streamed
/// into a private temporary and hashed, then kept as `<cache>/carried/<sha256>/<entry name>`. When that copy exists
/// already (an earlier export or save carried the same bytes), the temporary is dropped and the existing copy reused,
/// so repeated exports and saves never multiply cached bytes.
fn carry(base: &Container, name: &str, cache: &Path) -> Result<PathBuf, CommandError> {
    let dir = cache.join("carried");
    std::fs::create_dir_all(&dir).map_err(|e| CommandError::from(fsx::save_error(&e, &dir, "cannot create")))?;
    let tmp = dir.join(format!(".{}.part", fsx::random_token(8)));
    let copied = (|| -> Result<String, CommandError> {
        let mut r = base.reader(name, base.size(name).unwrap_or(0))?;
        let mut f =
            std::fs::File::create(&tmp).map_err(|e| CommandError::from(fsx::save_error(&e, &tmp, "cannot create")))?;
        std::io::copy(&mut r, &mut f).map_err(|e| CommandError::from(fsx::save_error(&e, &tmp, "cannot write")))?;
        drop(f);
        let (sha, _) = std::fs::File::open(&tmp)
            .and_then(fsx::sha256_reader)
            .map_err(|e| CommandError::from(fsx::save_error(&e, &tmp, "cannot read")))?;
        Ok(sha)
    })();
    let sha = match copied {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    };
    let to = dir.join(&sha).join(name.replace('/', "_"));
    if to.is_file() {
        let _ = std::fs::remove_file(&tmp);
        return Ok(to);
    }
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d).map_err(|e| CommandError::from(fsx::save_error(&e, d, "cannot create")))?;
    }
    if let Err(e) = std::fs::rename(&tmp, &to) {
        let _ = std::fs::remove_file(&tmp);
        if !to.is_file() {
            return Err(CommandError::from(fsx::save_error(&e, &to, "cannot keep")));
        }
    }
    Ok(to)
}

/// Empties the session cache when its project goes (C13-R3-03): nothing in it serves another project, and the
/// closed project's history is gone. Not while that project is being saved (the write may read from it).
pub(crate) fn clear_cache(env: &Env, closing: &OpenProject) {
    if !closing.store.saving {
        let _ = std::fs::remove_dir_all(&env.cache);
    }
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
    // Ownership lost while the save ran (C1.4 obligation 1: possible only where the file system does not carry the
    // holder's protection): the clean marker, the saved revision and the stamp do not advance, and the session
    // continues read-only (LOCK-02).
    if becomes_current
        && lock.is_none()
        && let Some(l) = p.store.lock.as_ref().filter(|l| !l.held())
    {
        p.store.locked_by = tt_persist::lock::read(l.path()).and_then(|i| serde_json::to_value(i).ok());
        p.store.lock = None;
        p.read_only = true;
        p.store.saving = false;
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
        p.store.content = store::content_of(&w.path, w.form);
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
    if !unsaved(cx)? {
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
            let name = cx.project()?.doc.project().name().clone();
            cx.confirm(
                crate::agents::Category::Discard,
                format!("Discard the unsaved changes of `{name}`"),
                vec![json!({ "kind": "discardUnsaved", "project": name })],
                None,
            )
        }
        // The same RO-02 boundary as project.save (SI-047 ruling, C14-R2-07): an agent gets the bound
        // `externalChange` confirmation of this call (close, quit, open, …), the user `project.changedOnDisk`.
        OnUnsaved::Save => {
            let external_ok = guard_external_change(cx)?;
            save_now(cx, false, external_ok)
        }
    }
}

/// The project file changed on disk since it was opened or saved (10 RO-02): its path, the detection time and the
/// file's current identity (stamp and lock), which a confirmation to overwrite it is bound to (SI-045).
pub(crate) fn external_change(p: &OpenProject) -> Option<(String, String, Value)> {
    let path = p.path.clone()?;
    let now = store::stamp_of(Path::new(&path), p.form_kind());
    // Size and modification time first (cheap); then the content recorded at open or the last save, so an edit that
    // keeps both is still seen before a save replaces the file (C14-R1-09).
    let content = p.store.content.as_ref().and_then(|_| store::content_of(Path::new(&path), p.form_kind()));
    let at = match &p.store.changed_on_disk {
        Some(at) => at.clone(),
        None if p.store.stamp.is_some() && now != p.store.stamp => crate::time::now_iso(),
        None if p.store.content.is_some() && content != p.store.content => crate::time::now_iso(),
        None => return None,
    };
    let binding = json!({
        "path": path, "file": format!("{now:?}"), "content": content,
        "lock": p.store.lock.as_ref().map(|l| l.info().lock_id),
    });
    Some((path, at, binding))
}

/// RO-02 for an agent's save over the project's location (SI-045, C14-R1-09): after an external change the call
/// needs confirmation (category `externalChange`), bound to the file's identity, its content and the session's lock.
/// `true`: confirmed (the save may replace the changed file). For the user it does nothing ([`check_external`]
/// answers `project.changedOnDisk`).
///
/// # Errors
/// `confirmation.required` and the other confirmation errors.
pub(crate) fn guard_external_change(cx: &mut Cx) -> Result<bool, CommandError> {
    if !matches!(cx.origin(), crate::types::Origin::Agent { .. }) {
        return Ok(false);
    }
    let Some((path, at, binding)) = external_change(cx.project()?) else { return Ok(false) };
    cx.confirm(
        crate::agents::Category::ExternalChange,
        format!("`{path}` was changed by another program since it was opened; saving overwrites those changes"),
        vec![
            json!({ "kind": "overwriteExternalChange", "path": path, "detectedAt": at }),
            // Reloading discards unsaved work: the `app` scope's, so the user's choice (11 §6.1).
            json!({ "kind": "alternative", "choice": "reload", "by": "user", "commandId": "project.open",
                    "params": { "path": path, "onUnsaved": "discard" } }),
            json!({ "kind": "alternative", "choice": "keepBoth", "commandId": "project.saveAs",
                    "params": { "path": "<another path>" } }),
        ],
        Some(binding),
    )?;
    Ok(true)
}

/// `project.changedOnDisk` when the project on disk changed since it was opened or saved (RO-02): size, modification
/// time or content.
///
/// # Errors
/// `project.changedOnDisk`.
pub(crate) fn check_external(p: &OpenProject) -> Result<(), CommandError> {
    match external_change(p) {
        Some((path, at, _)) => Err(changed_on_disk(&path, &at)),
        None => Ok(()),
    }
}

fn changed_on_disk(path: &str, at: &str) -> CommandError {
    CommandError::with_details(
        tt_persist::codes::CHANGED_ON_DISK,
        "the project file was changed by another program since it was opened",
        json!({ "path": path, "detectedAt": at }),
    )
    .hint("reopen it (project.open with onUnsaved) to reload, or keep yours with project.saveAs {path, onExists: \"overwrite\"}")
}

/// Asks before overwriting an existing file (11 §6.2: "overwrite" is a confirmable action under Assist). The
/// confirmation is bound to the file's identity (size and modification time).
///
/// # Errors
/// `confirmation.required` and the other confirmation errors of `Cx::confirm`.
pub(crate) fn confirm_overwrite(cx: &mut Cx, path: &Path) -> Result<(), CommandError> {
    if !path.exists() {
        return Ok(());
    }
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(std::fs::Metadata::len);
    let modified = meta.and_then(|m| m.modified().ok()).map(|t| format!("{t:?}"));
    cx.confirm(
        crate::agents::Category::Overwrite,
        format!("Overwrite `{}`", path.display()),
        vec![json!({ "kind": "overwrite", "path": path.to_string_lossy(), "bytes": size })],
        Some(json!({ "path": path.to_string_lossy(), "bytes": size, "modified": modified })),
    )
}

/// Checks that the open project may be saved to its location.
///
/// # Errors
/// `project.pathRequired`, `project.readOnly`, `project.formatUpgrade`, `project.changedOnDisk`.
pub(crate) fn check_saveable(
    p: &OpenProject,
    allow_upgrade: bool,
    external_ok: bool,
) -> Result<(PathBuf, Form), CommandError> {
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
    // The lock may have been taken over meanwhile (a stale heartbeat): never write over another owner (C13-R1-02).
    if let Some(l) = &p.store.lock
        && !l.held()
    {
        return Err(CommandError::with_details(
            tt_persist::codes::SAVE_FAILED,
            "this session no longer holds the project's lock; another session took it over",
            json!({ "reason": "fileInUse", "path": path, "lock": tt_persist::lock::read(l.path()) }),
        )
        .hint("save a copy with project.saveAs"));
    }
    check_upgrade(p, allow_upgrade)?;
    // Maintenance may not have looked since the last change: look again before replacing anything (C13-R1-06).
    let now = crate::time::now_iso();
    let changed = p.store.changed_on_disk.clone().filter(|_| !external_ok).or_else(|| {
        if external_ok {
            return None;
        }
        external_change(p).map(|(_, at, _)| at).or_else(|| {
            (p.store.stamp.is_some() && store::stamp_of(Path::new(&path), p.form_kind()) != p.store.stamp)
                .then(|| now.clone())
        })
    });
    if let Some(at) = changed {
        return Err(changed_on_disk(&path, &at));
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

/// A save requested inside an invocation (`onUnsaved: "save"`). Nothing is written while the invocation runs: the
/// executor writes it once every step has succeeded and only for a real (not dry-run) invocation, and a failed write
/// rolls the whole invocation back (11 §7.2–§7.3; C13-R1-01). The snapshot is the document the invocation commits
/// (its last transaction), so the file and the committed revision are the same one, and the clean marker is set at
/// the history position after the commit (C13-R2-01).
#[derive(Debug, Clone)]
pub(crate) struct PendingSave {
    pub path: PathBuf,
    pub form: Form,
    pub backups: Backups,
    pub saved_at: String,
    /// Inside an atomic batch: `modifiedAt` is stamped into the batch's own changes when it ends (one transaction,
    /// one undo step), not sealed separately.
    pub in_batch: bool,
    /// The snapshot written (taken just before the commit).
    pub snap: Option<Snapshot>,
    /// The finished write, once the executor has performed it.
    pub written: Option<Written>,
}

/// Whether the project has work to save as this invocation sees it: the live project is modified, or this invocation
/// (an atomic batch included) has staged or sealed changes (C13-R2-01).
pub(crate) fn unsaved(cx: &Cx) -> Result<bool, CommandError> {
    Ok(cx.project()?.dirty() || cx.has_staged())
}

/// Saves the open project to its location as part of this invocation (the `onUnsaved: "save"` paths): the
/// `modifiedAt` stamp is staged now (at the end of an atomic batch), the write is deferred (see [`PendingSave`]).
///
/// # Errors
/// As [`check_saveable`].
pub(crate) fn save_now(cx: &mut Cx, allow_upgrade: bool, external_ok: bool) -> Result<(), CommandError> {
    let (path, form) = check_saveable(cx.project()?, allow_upgrade, external_ok)?;
    let now = crate::time::now_iso();
    let in_batch = cx.in_atomic_batch();
    if !in_batch && unsaved(cx)? {
        stamp_modified(cx, &now)?;
    }
    let b = backups(&cx.session().env, cx.document()?, &path);
    cx.defer(Effect::Save(Box::new(PendingSave {
        path,
        form,
        backups: b,
        saved_at: now,
        in_batch,
        snap: None,
        written: None,
    })));
    Ok(())
}

/// The save time of a pending save of an atomic batch, if any (the batch stamps `modifiedAt` with it when it ends).
pub(crate) fn batch_save_time(effects: &[Effect]) -> Option<String> {
    effects.iter().find_map(|e| match e {
        Effect::Save(p) if p.in_batch => Some(p.saved_at.clone()),
        _ => None,
    })
}

/// Stamps `project.modifiedAt` into the staged changes (no seal): the stamp of an atomic batch's save.
///
/// # Errors
/// Staging errors.
pub(crate) fn stamp_staged(cx: &mut Cx, now: &str) -> Result<(), CommandError> {
    let op = tt_model::change::PatchOp {
        op: "replace",
        from: None,
        path: "/modifiedAt".into(),
        range: None,
        value: Some(json!(now)),
        old_value: None,
        elided: false,
    };
    cx.patch_entries(vec![(tt_model::document::EntryKey::Project, vec![op])])
}

/// Writes the pending saves of an invocation that is about to commit `final_doc` (the document after its last
/// transaction).
///
/// # Errors
/// The save errors of 10 SAVE-05 (the caller rolls the invocation back).
pub(crate) fn write_pending(s: &Session, final_doc: &Document, effects: &mut [Effect]) -> Result<(), CommandError> {
    for e in effects.iter_mut() {
        if let Effect::Save(p) = e
            && p.written.is_none()
        {
            let project = s.project.as_ref().ok_or_else(crate::cx::no_project)?;
            let snap = Snapshot {
                session: session_json(final_doc, &s.app),
                doc: final_doc.clone(),
                preserved: project.store.preserved.clone(),
                blobs: project.store.blobs.clone(),
                position: 0,
                top: None,
                saved_at: p.saved_at.clone(),
            };
            let git = p.form == Form::Folder;
            p.written = Some(write(&snap, &p.path, p.form, Some(&p.backups), &s.env.cache, git)?);
            p.snap = Some(snap);
        }
    }
    Ok(())
}

/// Applies a written pending save after the commit: the clean marker goes to the history position the commit reached.
pub(crate) fn apply_pending(s: &mut Session, p: &PendingSave) {
    let (Some(w), Some(mut snap)) = (&p.written, p.snap.clone()) else { return };
    if let Some(project) = s.project.as_ref() {
        snap.position = project.history.position();
        snap.top = project.history.step_id_before(snap.position).map(str::to_owned);
    }
    finish_save(s, &snap, w, true, None);
}

/// Closes the open project: `project.closed`, the lock released, the recovery data deleted when the close discards
/// or the project is clean (01 §3.4, AUTO-04).
pub(crate) fn close(s: &mut Session) {
    let Some(p) = s.project.take() else { return };
    let _ = recovery::delete(&s.env.dirs.recovery, &p.store.recovery_id);
    clear_cache(&s.env, &p);
    s.emit("project.closed", json!({ "projectId": p.doc.project().id().as_str() }));
    drop(p);
    s.app = AppContext::default();
}
