//! Background maintenance on the document thread (C1.3): lock and recovery heartbeats (LOCK-01), detection of
//! external changes to the project (RO-02) and to linked media (`doc.asset.status.changed`), and autosave (AUTO-01…
//! AUTO-04): a copy-on-write snapshot taken between transactions, written to the recovery folder on a background
//! thread, never touching the project, its clean marker, undo history or location.
//!
//! Hosts tick it every second (`Maintenance::tick`); `ExecutorHandle::run_maintenance` runs it at once.

use std::time::Instant;

use serde_json::json;
use tt_persist::recovery::{self, RecoveryInfo};
use tt_persist::{host, lock, time};

use crate::run::Core;
use crate::session::Session;

/// Restores the persisted user settings into a new session (workspaces) and reports corrupt settings files once
/// (01 §11 `prefs.corrupt`).
pub(crate) fn start(s: &mut Session) {
    s.env.ensure_dirs();
    if let Some(w) = s.env.stored_workspaces() {
        s.ui.restore(&w);
    }
    for f in s.env.settings.corrupt.clone() {
        s.emit(
            "ui.message.shown",
            json!({
                "messageId": "prefs.corrupt",
                "severity": "warning",
                "text": "A settings file was damaged; it was kept aside and its defaults are used.",
                "details": { "path": f },
                "source": "user",
                "suppressed": false,
            }),
        );
    }
}

/// Runs what is due; `force_autosave` writes a recovery copy now if the project is modified.
/// How long each part of a maintenance pass held the document thread (`None`: not due).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timings {
    /// Lock and recovery heartbeat.
    pub heartbeat: Option<std::time::Duration>,
    /// External-change check (the folder fingerprint, RO-02).
    pub watch: Option<std::time::Duration>,
    /// The autosave snapshot taken on the document thread (AUTO-02); the copy is written on its own thread.
    pub autosave_snapshot: Option<std::time::Duration>,
}

pub(crate) fn tick(core: &mut Core, force_autosave: bool) -> Timings {
    let mut t = Timings::default();
    let Some(s) = core.session_mut() else { return t };
    let now = Instant::now();
    let m = s.env.maintenance;
    if force_autosave || s.env.last_heartbeat.is_none_or(|t| now.duration_since(t) >= m.heartbeat) {
        let start = Instant::now();
        heartbeat(s);
        t.heartbeat = Some(start.elapsed());
        s.env.last_heartbeat = Some(now);
    }
    if force_autosave || s.env.last_watch.is_none_or(|t| now.duration_since(t) >= m.watch) {
        let start = Instant::now();
        watch(s);
        t.watch = Some(start.elapsed());
        s.env.last_watch = Some(now);
    }
    let start = Instant::now();
    if autosave(s, force_autosave) {
        t.autosave_snapshot = Some(start.elapsed());
    }
    t
}

fn heartbeat(s: &mut Session) {
    let root = s.env.dirs.recovery.clone();
    let Some(p) = s.project.as_mut() else { return };
    if let Some(l) = &p.store.lock
        && let Err(e) = l.refresh()
    {
        // Someone took the lock over (it was stale to them): continue read-only (LOCK-02).
        p.read_only = true;
        p.store.locked_by = e.details().get("lock").cloned();
        p.store.lock = None;
    }
    if p.store.autosaved_rev.is_some() {
        recovery::heartbeat(&root, &p.store.recovery_id);
    }
}

fn watch(s: &mut Session) {
    let mut events = Vec::new();
    if let Some(p) = s.project.as_mut() {
        if let (Some(path), false) = (p.path.clone(), p.store.saving) {
            let now = crate::store::stamp_of(std::path::Path::new(&path), p.form_kind());
            if p.store.stamp.is_some() && now != p.store.stamp && p.store.changed_on_disk.is_none() {
                let at = time::now_iso();
                p.store.changed_on_disk = Some(at.clone());
                events.push(("doc.package.changedOnDisk", json!({ "path": path, "detectedAt": at })));
            }
        }
        // Linked media (10 §4.16 transient `status`; links are relative to the folder containing the project).
        let project_path = p.path.clone().map(std::path::PathBuf::from);
        let mut changed = Vec::new();
        for a in p.doc.project().assets() {
            let Some(link) = a.link() else { continue };
            if link.kind != tt_model::entities::asset::LinkKind::File {
                continue;
            }
            let path = link.path.as_deref().unwrap_or_default();
            let found =
                tt_persist::load::resolve_link(project_path.as_deref(), path, link.absolute_path.as_deref(), *a.size());
            let status = match &found {
                None => "missing",
                Some(f) if std::fs::metadata(f).is_ok_and(|m| m.len() != *a.size()) => "changed",
                Some(_) => "ok",
            };
            let id = a.id().as_str().to_owned();
            match &found {
                Some(f) => {
                    p.store.linked.insert(id.clone(), f.clone());
                }
                None => {
                    p.store.linked.remove(&id);
                }
            }
            if p.store.asset_status.get(&id).map(String::as_str) != Some(status) {
                let had = p.store.asset_status.insert(id.clone(), status.to_owned());
                if had.is_some() {
                    changed.push(json!({ "assetId": id, "status": status }));
                }
            }
        }
        if !changed.is_empty() {
            events.push(("doc.asset.status.changed", json!({ "assets": changed })));
        }
    }
    for (name, payload) in events {
        s.emit(name, payload);
    }
}

/// Starts an autosave when one is due; `true` when it took a snapshot.
fn autosave(s: &mut Session, force: bool) -> bool {
    if !s.env.pref_bool("files.autosave.enabled") && !force {
        return false;
    }
    let interval = s.env.maintenance.autosave.unwrap_or_else(|| {
        std::time::Duration::from_secs(s.env.pref_u64("files.autosave.intervalMinutes").max(1).saturating_mul(60))
    });
    let Some(poster) = s.env.poster.clone() else { return false };
    let root = s.env.dirs.recovery.clone();
    let Some(p) = s.project.as_mut() else { return false };
    let rev = p.doc.document_rev();
    let due = force || p.store.last_autosave.elapsed() >= interval;
    if !due || !p.dirty() || p.store.autosaving || p.store.saving || p.store.autosaved_rev == Some(rev) {
        return false;
    }
    p.store.autosaving = true;
    p.store.last_autosave = Instant::now();
    let saved_at = time::now_iso();
    let info = RecoveryInfo {
        recovery_id: p.store.recovery_id.clone(),
        project_id: p.doc.project().id().as_str().to_owned(),
        project_name: p.doc.project().name().clone(),
        original_path: p.path.clone(),
        saved_at: saved_at.clone(),
        document_rev: rev,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        process_id: host::pid(),
        host: host::host(),
        heartbeat_at: saved_at.clone(),
    };
    // The snapshot: immutable copies (copy-on-write); serialisation and writing happen on the writer thread.
    let doc = p.doc.clone();
    let preserved = p.store.preserved.clone();
    let blobs = p.store.blobs.clone();
    let session = crate::persistence::session_json(&doc, &s.app);
    let spawned = std::thread::Builder::new().name("tt-autosave".to_owned()).spawn(move || {
        let input = tt_persist::SaveInput {
            doc: &doc,
            preserved: &preserved,
            blobs: &blobs,
            session: Some(&session),
            saved_at: saved_at.clone(),
        };
        let outcome = recovery::write(&root, &info, &input);
        let id = info.recovery_id.clone();
        let _ = poster.run(std::time::Duration::from_secs(600), move |core| {
            let Some(s) = core.session_mut() else { return };
            let root = s.env.dirs.recovery.clone();
            let mine = s.project.as_ref().is_some_and(|p| p.store.recovery_id == id);
            match (mine, outcome) {
                (true, Ok(_)) => {
                    let still_dirty = s.project.as_ref().is_some_and(crate::session::OpenProject::dirty);
                    if let Some(p) = s.project.as_mut() {
                        p.store.autosaving = false;
                        p.store.autosaved_rev = Some(rev);
                    }
                    if still_dirty {
                        let project_id = s.project.as_ref().map(|p| p.doc.project().id().as_str().to_owned());
                        s.emit(
                            "project.autosaved",
                            json!({ "projectId": project_id, "recoveryId": id, "savedAt": saved_at }),
                        );
                    } else {
                        // Saved or discarded meanwhile: the copy is not needed (AUTO-04).
                        let _ = recovery::delete(&root, &id);
                    }
                }
                (true, Err(_)) => {
                    if let Some(p) = s.project.as_mut() {
                        p.store.autosaving = false;
                    }
                }
                (false, _) => {
                    // The project was closed or replaced meanwhile (AUTO-04).
                    let _ = recovery::delete(&root, &id);
                }
            }
        });
    });
    if spawned.is_err()
        && let Some(p) = s.project.as_mut()
    {
        p.store.autosaving = false;
    }
    true
}

/// The executor stops: the project is closed without deleting its recovery data (a stop is not a clean quit), its
/// lock released; a temporary user folder this executor created is removed.
pub(crate) fn stop(core: &mut Core) {
    let Some(s) = core.session_mut() else { return };
    s.project = None;
    let _ = std::fs::remove_dir_all(&s.env.cache);
    if let Some(root) = s.env.owned_root.take() {
        let _ = std::fs::remove_dir_all(root);
    }
    let _ = lock::STALE_SECS;
}
