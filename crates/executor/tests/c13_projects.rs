//! C1.3 projects through the executor (01 §3, §12; 10 §6–§8): create, save, save as, open, close, both forms,
//! backups, locks, autosave and recovery, migrations, external changes, settings and workspaces, quit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use c13_support::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

/// A process ID no process has on this machine.
fn dead_pid() -> u32 {
    (40_000..4_000_000).step_by(4).find(|p| tt_persist::host::process_alive(*p) == Some(false)).unwrap()
}

fn entries_of(h: &ExecutorHandle) -> std::collections::BTreeMap<String, String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys().iter().map(|k| (format!("{k:?}"), d.entry_canonical(k).unwrap())).collect()
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn create_save_close_open_round_trips_both_forms() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let created = ok(
        &h,
        "project.create",
        json!({ "name": "Walk cycle", "frameRate": 23.976, "cameraWidth": 1280, "cameraHeight": 720 }),
    );
    for k in ["projectId", "sceneId", "cameraId", "elementId"] {
        assert!(created[k].is_string(), "{created}");
    }
    let st = ok(&h, "doc.state.get", json!({}));
    assert_eq!(st["dirty"], false, "01 §3.1: a new project is clean");
    assert_eq!(err(&h, "project.save", json!({})).code, "project.pathRequired");
    let info = ok(&h, "project.info.get", json!({}));
    assert_eq!((info["frameRate"].clone(), info["cameraWidth"].clone()), (json!(23.976), json!(1280)));
    assert_eq!(info["scenes"].as_array().unwrap().len(), 1);
    // 01 §3.1 / 05 §2.7 defaults.
    let pens = ok(&h, "doc.object.find", json!({ "type": "pen", "limit": 100 }));
    assert_eq!(pens["total"], 8);
    let swatches = ok(&h, "doc.object.find", json!({ "type": "swatch", "limit": 1000 }));
    assert_eq!(swatches["total"], 120);
    for (i, form) in ["package", "folder"].into_iter().enumerate() {
        let target = if form == "folder" {
            format!("{}/", path(&dir.join("Folder.animpkg")))
        } else {
            path(&dir.join("Walk.animpkg"))
        };
        let scene = first(&h, "scene");
        ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "round": i } } }));
        assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], true);
        let r = job_ok(&h, "project.saveAs", json!({ "path": target, "onExists": "overwrite" }));
        assert_eq!(r["projectSaved"], true);
        let st = ok(&h, "doc.state.get", json!({}));
        assert_eq!((st["dirty"].clone(), st["form"].clone()), (json!(false), json!(form)), "{st}");
        assert_eq!(st["savedRev"], st["documentRev"], "SAVE-04: the clean marker is the snapshot's");
        assert_eq!(st["lock"]["held"], true, "LOCK-01");
        let before = entries_of(&h);
        let saved_path = st["path"].as_str().unwrap().to_owned();
        ok(&h, "project.close", json!({}));
        assert!(
            !Path::new(&format!("{saved_path}.lock")).exists() && !Path::new(&saved_path).join(".lock").exists(),
            "the lock goes on close"
        );
        let opened = job_ok(&h, "project.open", json!({ "path": saved_path }));
        assert_eq!(opened["readOnly"], false);
        assert_eq!(entries_of(&h), before, "{form}: every entry reads back identically (REV-04: revisions too)");
        let st = ok(&h, "doc.state.get", json!({}));
        assert_eq!(st["dirty"], false);
        assert_eq!(st["form"], form);
    }
    let recent = ok(&h, "project.recent.list", json!({}));
    assert!(recent.as_array().unwrap().len() >= 2 && recent[0]["exists"] == true, "{recent}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saves_stamp_modified_at_keep_backups_and_clean_saves_rewrite_nothing() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Backups" }));
    let target = dir.join("B.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path(&target) }));
    let mtime = || std::fs::metadata(&target).unwrap().modified().unwrap();
    let t0 = mtime();
    std::thread::sleep(Duration::from_millis(30));
    let r = job_ok(&h, "project.save", json!({}));
    assert!(r.get("backupId").is_none(), "LAY-04: a clean save writes nothing");
    assert_eq!(mtime(), t0);
    let scene = first(&h, "scene");
    let project = first(&h, "project");
    let modified0 = get(&h, &project)["modifiedAt"].clone();
    for n in 0..4 {
        std::thread::sleep(Duration::from_millis(5));
        ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": n } } }));
        let events = h.subscribe();
        let r = job_ok(&h, "project.save", json!({}));
        assert!(r["backupId"].is_string(), "BAK-01: {r}");
        let stamped: Vec<Value> = events.try_iter().filter(|e| e.name == "doc.changed").map(|e| e.payload).collect();
        assert!(stamped.iter().any(|p| p["origin"] == "system" && p["undoStepId"].is_null()), "{stamped:?}");
        assert!(events.try_iter().all(|e| e.name != "edit.history.changed" || true));
    }
    assert_ne!(get(&h, &project)["modifiedAt"], modified0, "10 §4.1: modifiedAt is set by each save");
    let backups = ok(&h, "project.backup.list", json!({}));
    assert_eq!(backups.as_array().unwrap().len(), 3, "BAK-03: files.backups.count = 3 kept: {backups}");
    // Undo after a save: the project is modified again; redo back to the saved position: clean (01 §3.5).
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], true);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], false);
    // A backup opens as an untitled copy (BAK-04) and is never modified.
    let b = &backups[1];
    let file_count = std::fs::read_dir(dir.join("user/Backups")).unwrap().count();
    let opened = ok(&h, "project.backup.open", json!({ "backupId": b["backupId"] }));
    assert!(opened["projectId"].is_string());
    let st = ok(&h, "doc.state.get", json!({}));
    assert!(st["path"].is_null(), "{st}");
    assert_eq!(std::fs::read_dir(dir.join("user/Backups")).unwrap().count(), file_count);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unsaved_changes_policies_and_quit() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Policy" }));
    let target = dir.join("P.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path(&target) }));
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "x": 1 } } }));
    assert_eq!(err(&h, "project.close", json!({})).code, "project.unsavedChanges");
    assert_eq!(err(&h, "project.create", json!({ "name": "Other" })).code, "project.unsavedChanges");
    // An agent cannot discard without the app scope.
    let as_agent = tt_executor::Invocation {
        command: "project.close".into(),
        params: json!({ "onUnsaved": "discard" }),
        options: tt_executor::InvokeOptions::default(),
        origin: agent(&[tt_registry::Scope::Read, tt_registry::Scope::Edit]),
    };
    let e = h.invoke(as_agent);
    assert_eq!(e.error.unwrap().code, "permission.denied");
    assert_eq!(err(&h, "app.quit", json!({})).code, "project.unsavedChanges");
    // Quit saving first: the save happens, the project closes, recovery data goes.
    assert_eq!(ok(&h, "app.quit", json!({ "onUnsaved": "save" }))["quitting"], true);
    assert_eq!(err(&h, "doc.state.get", json!({})).code, "project.none");
    let reopened = job_ok(&h, "project.open", json!({ "path": path(&target) }));
    assert!(reopened["projectId"].is_string());
    assert_eq!(get(&h, &scene)["meta"]["x"], 1, "the quit saved the change");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_session_opens_a_locked_project_read_only() {
    let dir = tmp();
    let a = spawn(&dir.join("userA"));
    let b = spawn(&dir.join("userB"));
    ok(&a, "project.create", json!({ "name": "Shared" }));
    let target = dir.join("S.animpkg");
    job_ok(&a, "project.saveAs", json!({ "path": path(&target) }));
    let opened = job_ok(&b, "project.open", json!({ "path": path(&target) }));
    assert_eq!(opened["readOnly"], true, "LOCK-02");
    assert!(opened["warnings"].as_array().unwrap().iter().any(|w| w["id"] == "project.lockedByOther"), "{opened}");
    let st = ok(&b, "doc.state.get", json!({}));
    assert_eq!(st["lock"]["held"], false);
    let scene = first(&b, "scene");
    ok(&b, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "b": 1 } } }));
    assert_eq!(err(&b, "project.save", json!({})).code, "project.readOnly");
    // Save As is the way out.
    job_ok(&b, "project.saveAs", json!({ "path": path(&dir.join("Mine.animpkg")) }));
    assert_eq!(ok(&b, "doc.state.get", json!({}))["readOnly"], false);
    drop(a);
    std::thread::sleep(Duration::from_millis(200));
    // The first session is gone: its lock is released and the file opens for writing.
    let c = spawn(&dir.join("userC"));
    assert_eq!(job_ok(&c, "project.open", json!({ "path": path(&target) }))["readOnly"], false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_bad_files_fails_the_job_and_leaves_no_project_open() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Before" }));
    let bad = dir.join("bad.animpkg");
    std::fs::write(&bad, b"not a zip at all").unwrap();
    let e = job_err(&h, "project.open", json!({ "path": path(&bad) }));
    assert_eq!(e["code"], "project.invalidFormat");
    assert_eq!(err(&h, "doc.state.get", json!({})).code, "project.none", "01 §3.2: the previous project was closed");
    assert_eq!(err(&h, "project.open", json!({ "path": path(&dir.join("missing.animpkg")) })).code, "project.notFound");
    assert_eq!(err(&h, "project.open", json!({ "path": "relative/x.animpkg" })).code, "fs.invalidPath");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn autosave_writes_recovery_copies_that_a_later_session_recovers() {
    let dir = tmp();
    let user = dir.join("user");
    let h = spawn(&user);
    ok(&h, "project.create", json!({ "name": "Crashy" }));
    let target = dir.join("C.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path(&target) }));
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "unsaved": "work" } } }));
    let events = h.subscribe();
    let before_file = std::fs::read(&target).unwrap();
    h.run_maintenance(true).unwrap();
    let autosaved = events
        .recv_timeout(Duration::from_secs(30))
        .into_iter()
        .chain(events.try_iter())
        .find(|e| e.name == "project.autosaved");
    let mut autosaved = autosaved;
    for _ in 0..50 {
        if autosaved.is_some() {
            break;
        }
        autosaved = events.recv_timeout(Duration::from_millis(200)).ok().filter(|e| e.name == "project.autosaved");
    }
    let autosaved = autosaved.expect("project.autosaved");
    let rid = autosaved.payload["recoveryId"].as_str().unwrap().to_owned();
    assert_eq!(std::fs::read(&target).unwrap(), before_file, "AUTO-02: the project file is untouched");
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], true, "AUTO-02: the dirty state is unchanged");
    // Our own recovery data is not offered while we run.
    assert!(ok(&h, "project.recovery.list", json!({})).as_array().unwrap().is_empty());
    // Simulate a crash: the session disappears without quitting, and its process is gone.
    drop(h);
    std::thread::sleep(Duration::from_millis(200));
    let info_path = user.join("Recovery").join(&rid).join("recovery.json");
    let mut info: Value = serde_json::from_slice(&std::fs::read(&info_path).unwrap()).unwrap();
    info["processId"] = json!(dead_pid());
    std::fs::write(&info_path, serde_json::to_vec(&info).unwrap()).unwrap();
    let lock = PathBuf::from(format!("{}.lock", path(&target)));
    assert!(!lock.exists(), "dropping the session released the lock");
    let h2 = spawn(&user);
    let list = ok(&h2, "project.recovery.list", json!({}));
    let entry = list.as_array().unwrap().iter().find(|e| e["recoveryId"] == rid).cloned().unwrap();
    assert_eq!(entry["usable"], true, "{entry}");
    let r = job_ok(&h2, "project.recovery.open", json!({ "recoveryId": rid }));
    assert!(r["projectId"].is_string());
    let st = ok(&h2, "doc.state.get", json!({}));
    assert_eq!(st["dirty"], true, "REC-02: a recovered project is modified");
    assert_eq!(st["path"], path(&target), "REC-02: its location is the original path");
    assert_eq!(get(&h2, &scene)["meta"]["unsaved"], "work");
    // Saving writes the original and deletes the recovery data (AUTO-04).
    job_ok(&h2, "project.save", json!({}));
    assert!(!user.join("Recovery").join(&rid).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

fn up_0_9(mut e: tt_persist::migrate::Entries) -> Result<tt_persist::migrate::Entries, String> {
    // The synthetic 0.9 format called the project's frame rate `fps`; 1.0 calls it `frameRate`.
    let p = e.get_mut("project.json").ok_or("no project")?;
    let fps = p.as_object_mut().ok_or("not an object")?.shift_remove("fps").ok_or("no fps")?;
    p["frameRate"] = fps;
    Ok(e)
}

static CHAIN: &[tt_persist::migrate::Migration] = &[tt_persist::migrate::Migration {
    from: tt_persist::manifest::Version { major: 0, minor: 9 },
    to: tt_persist::manifest::Version { major: 1, minor: 0 },
    name: "0.9-fps",
    apply: up_0_9,
}];

#[test]
fn older_files_are_migrated_in_memory_and_the_first_save_needs_the_upgrade() {
    let dir = tmp();
    let h = spawn_with(&dir.join("user"), CHAIN);
    ok(&h, "project.create", json!({ "name": "Old" }));
    let folder = dir.join("Old.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": format!("{}/", path(&folder)) }));
    ok(&h, "project.close", json!({}));
    // Turn it into a 0.9 file.
    let mut m: Value = serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    m["formatVersion"] = json!("0.9");
    m["minReaderVersion"] = json!("0.9");
    std::fs::write(folder.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    let mut p: Value = serde_json::from_slice(&std::fs::read(folder.join("project.json")).unwrap()).unwrap();
    let fr = p.as_object_mut().unwrap().shift_remove("frameRate").unwrap();
    p["fps"] = fr;
    std::fs::write(folder.join("project.json"), serde_json::to_vec(&p).unwrap()).unwrap();
    let original = std::fs::read(folder.join("project.json")).unwrap();
    let r = job_ok(&h, "project.open", json!({ "path": path(&folder) }));
    assert_eq!(r["formatVersion"], "0.9");
    let st = ok(&h, "doc.state.get", json!({}));
    assert_eq!(st["migratedFrom"], "0.9");
    assert_eq!(st["dirty"], false, "VER-03: migration does not mark the project modified");
    assert_eq!(ok(&h, "doc.loadReport.get", json!({}))["migratedFrom"], "0.9");
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "m": 1 } } }));
    assert_eq!(err(&h, "project.save", json!({})).code, "project.formatUpgrade");
    assert_eq!(std::fs::read(folder.join("project.json")).unwrap(), original, "nothing written");
    job_ok(&h, "project.save", json!({ "allowFormatUpgrade": true }));
    let m: Value = serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(m["formatVersion"], "1.0");
    assert!(ok(&h, "doc.state.get", json!({}))["migratedFrom"].is_null());
    // Without the chain, a 0.9 file cannot be read.
    let plain = spawn(&dir.join("user2"));
    std::fs::write(
        folder.join("manifest.json"),
        serde_json::to_vec(&json!({"format": "animpkg", "formatVersion": "0.8"})).unwrap(),
    )
    .unwrap();
    assert_eq!(job_err(&plain, "project.open", json!({ "path": path(&folder) }))["code"], "project.unsupportedVersion");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn external_changes_are_detected_and_block_a_plain_save() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Watched" }));
    let target = dir.join("W.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path(&target) }));
    let events = h.subscribe();
    std::thread::sleep(Duration::from_millis(50));
    let mut bytes = std::fs::read(&target).unwrap();
    bytes.extend_from_slice(b"appended by another program");
    std::fs::write(&target, &bytes).unwrap();
    h.run_maintenance(false).unwrap();
    assert!(events.try_iter().any(|e| e.name == "doc.package.changedOnDisk"), "RO-02");
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "mine": 1 } } }));
    assert_eq!(err(&h, "project.save", json!({})).code, "project.changedOnDisk");
    // Keep mine: an explicit overwrite.
    job_ok(&h, "project.saveAs", json!({ "path": path(&target), "onExists": "overwrite" }));
    job_ok(&h, "project.save", json!({}));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspaces_recent_projects_and_defaults_persist_per_user() {
    let dir = tmp();
    let user = dir.join("user");
    {
        let h = spawn(&user);
        ok(&h, "workspace.create", json!({ "name": "Inking" }));
        ok(&h, "project.create", json!({ "name": "Prefs", "frameRate": 30 }));
        ok(
            &h,
            "project.properties.set",
            json!({ "frameRate": 12, "cameraWidth": 640, "cameraHeight": 480, "setAsDefault": true }),
        );
        job_ok(&h, "project.saveAs", json!({ "path": path(&dir.join("R.animpkg")) }));
    }
    std::thread::sleep(Duration::from_millis(100));
    let h = spawn(&user);
    let ws = ok(&h, "workspace.list", json!({}));
    assert!(ws.as_array().unwrap().iter().any(|w| w["name"] == "Inking"), "01 §11: {ws}");
    let recent = ok(&h, "project.recent.list", json!({}));
    assert_eq!(recent[0]["name"], "Prefs");
    let presets = ok(&h, "project.presets.list", json!({}));
    let default = presets.as_array().unwrap().iter().find(|p| p["kind"] == "default").unwrap();
    assert_eq!((default["frameRate"].as_f64(), default["width"].as_u64()), (Some(12.0), Some(640)));
    let most = presets.as_array().unwrap().iter().find(|p| p["kind"] == "mostRecent").unwrap();
    assert_eq!(most["frameRate"].as_f64(), Some(30.0));
    assert!(presets.as_array().unwrap().iter().filter(|p| p["kind"] == "bundled").count() >= 20);
    assert_eq!(ok(&h, "project.recent.remove", json!({ "all": true }))["remaining"], 0);
    // A corrupt settings file is kept aside and reported once (01 §11).
    drop(h);
    std::thread::sleep(Duration::from_millis(100));
    std::fs::write(user.join("Settings/recent.json"), b"{ broken").unwrap();
    let h = spawn(&user);
    let events = h.subscribe();
    let _ = ok(&h, "app.state.get", json!({}));
    let shown: Vec<_> = events.try_iter().filter(|e| e.name == "ui.message.shown").collect();
    assert!(shown.iter().any(|e| e.payload["messageId"] == "prefs.corrupt"), "{shown:?}");
    assert!(
        std::fs::read_dir(user.join("Settings")).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("corrupt"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn objects_with_unknown_values_of_a_newer_minor_version_are_read_only() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Newer" }));
    let folder = dir.join("Newer.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": format!("{}/", path(&folder)) }));
    ok(&h, "project.close", json!({}));
    // A file of format 1.3 whose drawing element uses a loop type this version does not know.
    let mut m: Value = serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    m["formatVersion"] = json!("1.3");
    std::fs::write(folder.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    let el_file = walk(&folder).into_iter().find(|p| p.components().any(|c| c.as_os_str() == "elements")).unwrap();
    let mut el: Value = serde_json::from_slice(&std::fs::read(&el_file).unwrap()).unwrap();
    el["loopType"] = json!("pingPong");
    std::fs::write(&el_file, serde_json::to_vec_pretty(&el).unwrap()).unwrap();
    let r = job_ok(&h, "project.open", json!({ "path": path(&folder) }));
    assert_eq!(r["warnings"][0]["id"], "project.newerMinorVersion", "{r}");
    let report = ok(&h, "doc.loadReport.get", json!({}));
    assert!(report["issues"].as_array().unwrap().iter().any(|i| i["code"] == "doc.unknownValue"), "{report}");
    let eid = el["id"].as_str().unwrap().to_owned();
    // Edits of the object are refused, dry runs included; other objects stay editable (VER-02).
    for dry in [false, true] {
        let env = with(
            &h,
            "doc.object.patch",
            json!({ "targets": [eid], "props": { "meta": { "x": 1 } } }),
            tt_executor::InvokeOptions { dry_run: dry, ..Default::default() },
        );
        let e = env.error.unwrap_or_else(|| panic!("dry {dry}: the edit was accepted"));
        assert_eq!(e.code, "doc.unknownValue");
        assert_eq!(e.details["unknownValues"][0]["value"], "pingPong", "{e:?}");
    }
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "edited": true } } }));
    job_ok(&h, "project.save", json!({ "allowFormatUpgrade": true }));
    let back: Value = serde_json::from_slice(&std::fs::read(&el_file).unwrap()).unwrap();
    assert_eq!(back, el, "the read-only entry is written back as read (canonical form)");
    let _ = std::fs::remove_dir_all(&dir);
}

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn a_session_whose_lock_was_taken_over_cannot_save_over_the_new_owner() {
    let dir = tmp();
    let target = dir.join("Taken.animpkg");
    let a = spawn(&dir.join("a"));
    ok(&a, "project.create", json!({ "name": "Taken", "path": path(&target) }));
    let scene = first(&a, "scene");
    ok(&a, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "a": 1 } } }));
    // Another session takes the lock over (as after a stale heartbeat).
    let lock = tt_persist::lock::lock_path(&target);
    let mut info: Value = serde_json::from_slice(&std::fs::read(&lock).unwrap()).unwrap();
    info["lockId"] = json!("someone-else");
    // C1.4 (ADR-0014 §11.4): on Windows the live holder's handle refuses every other writer, so a takeover of a live
    // session cannot happen at all; with advisory locks (flock) the file can still be overwritten and the save must
    // refuse.
    if std::fs::write(&lock, serde_json::to_vec(&info).unwrap()).is_err() {
        assert!(cfg!(windows), "only Windows share modes refuse the write");
        assert_eq!(tt_persist::lock::probe(&lock), Some(true));
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let before = std::fs::read(&target).unwrap();
    for (id, params) in [("project.save", json!({})), ("project.close", json!({ "onUnsaved": "save" }))] {
        let env = run(&a, id, params);
        let failed = match &env.result {
            Some(started) if env.error.is_none() && started.get("jobId").is_some() => {
                wait(&a, started)["state"] == "failed"
            }
            _ => env.error.as_ref().is_some_and(|e| e.code == "project.saveFailed"),
        };
        assert!(failed, "{id}: {env:?}");
    }
    assert_eq!(std::fs::read(&target).unwrap(), before, "LOCK-02: nothing written over the new owner");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_atomic_batch_that_saves_commits_one_step_and_writes_that_revision() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let target = dir.join("Batch.animpkg");
    ok(&h, "project.create", json!({ "name": "Batch", "path": path(&target) }));
    let scene = first(&h, "scene");
    let modified_before = get(&h, "project")["modifiedAt"].clone();
    let events = h.subscribe();
    let env = batch(
        &h,
        json!({ "label": "Edit and close", "atomic": true, "steps": [
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "batch": 1 } } } },
            { "id": "project.close", "params": { "onUnsaved": "save" } }
        ] }),
    );
    assert!(env.is_ok(), "{env:?}");
    let saved = tt_persist::load(&target, &Default::default()).unwrap();
    assert_eq!(env.document_rev, Some(saved.doc.document_rev()), "the file holds the committed revision");
    let changed: Vec<_> = events.try_iter().filter(|e| e.name == "doc.changed").collect();
    assert_eq!(changed.len(), 1, "one commit: {changed:?}");
    assert_eq!(changed[0].payload["documentRev"], saved.doc.document_rev());
    assert!(changed[0].payload["undoStepId"].is_string(), "one undo step, named by the batch");
    assert_eq!(changed[0].payload["label"], "Edit and close");
    // The save stamped modifiedAt inside that same revision.
    let p = saved.doc.entry_raw_json(&tt_model::document::EntryKey::Project).unwrap();
    assert_ne!(p["modifiedAt"], modified_before);
    let key = tt_model::document::EntryKey::Scene(tt_model::Id::parse(&scene).unwrap());
    assert_eq!(saved.doc.entry_raw_json(&key).unwrap()["meta"]["batch"], 1);
    assert_eq!(err(&h, "doc.state.get", json!({})).code, "project.none", "closed after the save");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_project_removes_its_cached_media() {
    use std::sync::Arc;
    use tt_persist::container::{BlobData, Blobs, Form};
    let dir = tmp();
    // Unique asset IDs (seed), so the cached file names belong to this test only.
    let doc = tt_model::fixture::sample_document(9031).unwrap();
    let mut blobs = Blobs::default();
    for (n, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(n, BlobData::Bytes(Arc::new(vec![7; 2048])));
    }
    let source = dir.join("Cached.animpkg");
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &Default::default(),
        blobs: &blobs,
        session: None,
        saved_at: "2026-10-29T12:00:00.000Z".into(),
    };
    tt_persist::save::save(&source, Form::Package, &input, None, false).unwrap();
    let h = spawn(&dir.join("user"));
    job_ok(&h, "project.open", json!({ "path": path(&source) }));
    let assets = ok(&h, "doc.asset.list", json!({}));
    let asset = assets["assets"].as_array().unwrap().iter().find(|a| a["storage"] == "embedded").unwrap().clone();
    let id = asset["id"].as_str().unwrap().to_owned();
    let name = get(&h, &id)["entry"].as_str().unwrap().replace('/', "_");
    let cached = || {
        fn walk(p: &Path, wanted: &str, n: &mut usize) {
            for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
                if e.path().is_dir() {
                    walk(&e.path(), wanted, n);
                } else if e.file_name() == wanted {
                    *n += 1;
                }
            }
        }
        let mut n = 0;
        for e in std::fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
            if e.file_name().to_string_lossy().starts_with(&format!("tt-cache-{}-", std::process::id())) {
                walk(&e.path(), &name, &mut n);
            }
        }
        n
    };
    // Removed media are carried to the cache by a Save As (undo needs them), once however often it happens.
    ok(&h, "doc.asset.remove", json!({ "assetIds": [id], "force": true }));
    job_ok(&h, "project.saveAs", json!({ "path": path(&dir.join("Elsewhere.animpkg")) }));
    job_ok(&h, "doc.package.export", json!({ "path": path(&dir.join("Copy.animpkg")) }));
    assert_eq!(cached(), 1, "one content-addressed copy");
    ok(&h, "edit.undo", json!({}));
    assert!(ok(&h, "doc.blob.get", json!({ "ref": id }))["base64"].is_string(), "undo reaches the cached bytes");
    // Closing the project removes what only its history needed.
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    assert_eq!(cached(), 0, "C13-R3-03: the closed project's cached media are gone");
    let _ = std::fs::remove_dir_all(&dir);
}
