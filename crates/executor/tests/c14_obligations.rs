//! C1.4 obligations from the C1.3 review (ADR-0014 §11, §17): macOS normalisation and case collisions in folder
//! conversion (3), the external-change confirmation for agents (6, SI-045), and session cache bounds (8).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use c13_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::agents::{AgentConnect, Policy};
use tt_executor::{Envelope, ExecutorHandle, Invocation, InvokeOptions, Origin, Status};

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Rewrites a package with extra entries outside the reserved names (ZIP-04 keeps them).
fn add_entries(pkg: &Path, extras: &[(&str, &[u8])]) {
    let c = tt_persist::Container::open(pkg, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
    let mut names = c.names();
    names.sort_by_key(|n| n != "mimetype");
    let mut w = tt_persist::zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for n in &names {
        w.add_bytes(n, &c.read(n, 1 << 30).unwrap(), tt_persist::zip::Method::for_name(n)).unwrap();
    }
    for (n, b) in extras {
        w.add_bytes(n, b, tt_persist::zip::Method::Stored).unwrap();
    }
    let bytes = w.finish().unwrap().into_inner();
    drop(c);
    std::fs::write(pkg, bytes).unwrap();
}

#[test]
fn folder_conversion_refuses_entry_names_a_file_system_would_merge() {
    let nfc = "extras/\u{c4}rger.txt";
    let nfd = "extras/A\u{308}rger.txt";
    for (label, a, b) in [("normalisation", nfc, nfd), ("case", "extras/Notes.txt", "extras/notes.txt")] {
        let dir = tmp();
        let pkg = dir.join("Names.animpkg");
        let h = spawn(&dir.join("user"));
        ok(&h, "project.create", json!({ "name": "Names", "path": path(&pkg) }));
        ok(&h, "project.close", json!({}));
        add_entries(&pkg, &[(a, b"first"), (b, b"second")]);
        job_ok(&h, "project.open", json!({ "path": path(&pkg) }));
        let folder = dir.join("Names folder");
        let target = format!("{}{}", path(&folder), std::path::MAIN_SEPARATOR);
        let env = run(&h, "project.saveAs", json!({ "path": target }));
        let error = match (&env.error, &env.result) {
            (Some(e), _) => serde_json::to_value(e).unwrap(),
            (None, Some(started)) => wait(&h, started)["error"].clone(),
            _ => panic!("{label}: {env:?}"),
        };
        assert_eq!(error["code"], "project.saveFailed", "{label}: {error}");
        assert_eq!(error["details"]["reason"], "nameCollision", "{label}: {error}");
        assert!(!folder.exists(), "{label}: preflight refuses before anything is written");
        // The package keeps both entries, byte for byte.
        ok(&h, "doc.object.patch", json!({ "targets": [first(&h, "scene")], "props": { "meta": { "n": 1 } } }));
        job_ok(&h, "project.save", json!({}));
        let c = tt_persist::Container::open(&pkg, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()).unwrap();
        assert_eq!(c.read(a, 64).unwrap(), b"first", "{label}");
        assert_eq!(c.read(b, 64).unwrap(), b"second", "{label}");
        drop(c);
        drop(h);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn agent_session(h: &ExecutorHandle, roots: Vec<PathBuf>, policy: Policy) -> Origin {
    let s = h
        .connect_agent(AgentConnect {
            client: "c14".into(),
            policy: Some(policy),
            scopes: None,
            roots,
            preauthorized: BTreeSet::new(),
            transport: "test".into(),
        })
        .unwrap();
    Origin::Agent { session: s.session_id, name: s.client, scopes: s.scopes }
}

fn call(h: &ExecutorHandle, origin: &Origin, id: &str, params: Value, token: Option<&str>) -> Envelope {
    let options = InvokeOptions { confirm_token: token.map(str::to_owned), ..InvokeOptions::default() };
    h.invoke(Invocation { command: id.into(), params, options, origin: origin.clone() })
}

fn token(env: &Envelope) -> String {
    assert_eq!(env.status, Status::NeedsConfirmation, "{env:?}");
    env.result.as_ref().unwrap()["confirmToken"].as_str().unwrap().to_owned()
}

fn finish(h: &ExecutorHandle, env: &Envelope) -> Value {
    assert_eq!(env.status, Status::Ok, "{env:?}");
    wait(h, env.result.as_ref().unwrap())
}

/// Changes the package's modification time as another program would (contents stay loadable).
fn touch(p: &Path, secs: u64) {
    let t = SystemTime::now() + Duration::from_secs(secs);
    std::fs::File::options().write(true).open(p).unwrap().set_modified(t).unwrap();
}

#[test]
fn an_external_change_is_a_bound_confirmation_for_agents() {
    let dir = tmp();
    let pkg = dir.join("External.animpkg");
    let h = spawn(&dir.join("user"));
    let a = agent_session(&h, vec![dir.clone()], Policy::Assist);
    assert!(call(&h, &a, "project.create", json!({ "name": "External", "path": path(&pkg) }), None).is_ok());
    let scene = first(&h, "scene");
    let edit = |n: u64| {
        let e = call(&h, &a, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": n } } }), None);
        assert!(e.is_ok(), "{e:?}");
    };
    edit(1);
    touch(&pkg, 10);
    let before = std::fs::read(&pkg).unwrap();
    // The user gets the C1.3 error; the agent a confirmation that writes nothing.
    assert_eq!(err(&h, "project.save", json!({})).code, "project.changedOnDisk");
    let asked = call(&h, &a, "project.save", json!({}), None);
    let t1 = token(&asked);
    let r = asked.result.as_ref().unwrap();
    assert_eq!(r["category"], "externalChange");
    assert!(r["consequences"].as_array().unwrap().iter().any(|c| c["choice"] == "reload"), "{r}");
    assert_eq!(std::fs::read(&pkg).unwrap(), before, "nothing written while asking");
    // Identity rechecked after the confirmation: the file changed again, the token is refused.
    touch(&pkg, 20);
    let e = call(&h, &a, "project.save", json!({}), Some(&t1));
    assert_eq!(e.error.as_ref().unwrap().code, "confirmation.expired", "{e:?}");
    // Expiry.
    h.set_confirmation_ttl(Duration::from_millis(50)).unwrap();
    let t2 = token(&call(&h, &a, "project.save", json!({}), None));
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(call(&h, &a, "project.save", json!({}), Some(&t2)).error.unwrap().code, "confirmation.expired");
    h.set_confirmation_ttl(Duration::from_secs(300)).unwrap();
    // Keep mine: the token overwrites, once.
    let t3 = token(&call(&h, &a, "project.save", json!({}), None));
    let rec = finish(&h, &call(&h, &a, "project.save", json!({}), Some(&t3)));
    assert_eq!(rec["state"], "succeeded", "{rec}");
    assert_ne!(std::fs::read(&pkg).unwrap(), before);
    assert_eq!(ok(&h, "project.info.get", json!({}))["dirty"], false);
    assert_eq!(
        call(&h, &a, "project.save", json!({}), Some(&t3)).status,
        Status::Ok,
        "no external change: no confirmation needed"
    );
    edit(2);
    touch(&pkg, 30);
    let replay = call(&h, &a, "project.save", json!({}), Some(&t3));
    assert_eq!(replay.error.unwrap().code, "confirmation.expired", "a used token never replays");
    // Reload discards unsaved work: the user's choice (11 §6.1 `app` scope), never the agent's.
    let denied = call(&h, &a, "project.open", json!({ "path": path(&pkg), "onUnsaved": "discard" }), None);
    assert_eq!(denied.error.unwrap().code, "permission.denied");
    job_ok(&h, "project.open", json!({ "path": path(&pkg), "onUnsaved": "discard" }));
    assert_eq!(get(&h, &scene)["meta"]["n"], 1, "the file's content (n = 1) is back");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lock_ownership_is_rechecked_after_the_confirmation() {
    let dir = tmp();
    let pkg = dir.join("Owner.animpkg");
    let h = spawn(&dir.join("user"));
    let a = agent_session(&h, vec![dir.clone()], Policy::Assist);
    assert!(call(&h, &a, "project.create", json!({ "name": "Owner", "path": path(&pkg) }), None).is_ok());
    let scene = first(&h, "scene");
    assert!(
        call(&h, &a, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 1 } } }), None).is_ok()
    );
    touch(&pkg, 10);
    let t = token(&call(&h, &a, "project.save", json!({}), None));
    let lock = tt_persist::lock::lock_path(&pkg);
    let mut info: Value = serde_json::from_slice(&std::fs::read(&lock).unwrap()).unwrap();
    info["lockId"] = json!("someone-else");
    std::fs::write(&lock, serde_json::to_vec(&info).unwrap()).unwrap();
    let env = call(&h, &a, "project.save", json!({}), Some(&t));
    let code = match (&env.error, &env.result) {
        (Some(e), _) => e.code.clone(),
        (None, Some(s)) => wait(&h, s)["error"]["code"].as_str().unwrap().to_owned(),
        _ => panic!("{env:?}"),
    };
    assert_eq!(code, "project.saveFailed", "{env:?}");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The executor caches of this process (`tt-cache-<pid>-*`) and their sizes.
fn caches() -> Vec<(PathBuf, u64)> {
    fn size(p: &Path) -> u64 {
        std::fs::read_dir(p).map_or(0, |d| {
            d.flatten()
                .map(|e| if e.path().is_dir() { size(&e.path()) } else { e.metadata().map_or(0, |m| m.len()) })
                .sum()
        })
    }
    let prefix = format!("tt-cache-{}-", std::process::id());
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| (e.path(), size(&e.path())))
        .collect()
}

static SPAWN: Mutex<()> = Mutex::new(());

#[test]
fn session_cache_is_bounded_kept_while_needed_and_released_on_close_and_stop() {
    use std::sync::Arc;
    use tt_persist::container::{BlobData, Blobs, Form};
    let dir = tmp();
    let doc = tt_model::fixture::sample_document(901).unwrap();
    let mut blobs = Blobs::default();
    for (n, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(n, BlobData::Bytes(Arc::new(vec![7; 60 * 1024])));
    }
    let source = dir.join("Media.animpkg");
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &Default::default(),
        blobs: &blobs,
        session: None,
        saved_at: "2026-10-29T12:00:00.000Z".into(),
    };
    tt_persist::save::save(&source, Form::Package, &input, None, false).unwrap();
    let other = dir.join("Other.animpkg");
    tt_persist::save::save(&other, Form::Package, &input, None, false).unwrap();
    let (h, cache) = {
        let _g = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
        let before: BTreeSet<PathBuf> = caches().into_iter().map(|c| c.0).collect();
        let h = spawn(&dir.join("user"));
        job_ok(&h, "project.open", json!({ "path": path(&source) }));
        let id = ok(&h, "doc.asset.list", json!({}))["assets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["storage"] == "embedded")
            .unwrap()["id"]
            .clone();
        ok(&h, "checkpoint.create", json!({ "name": "before removal" }));
        ok(&h, "doc.asset.remove", json!({ "assetIds": [id.clone()], "force": true }));
        job_ok(&h, "doc.package.export", json!({ "path": path(&dir.join("e.animpkg")) }));
        let cache = caches().into_iter().map(|c| c.0).find(|p| !before.contains(p)).expect("this executor's cache");
        (h, (cache, id))
    };
    let (cache, asset) = cache;
    let size = |p: &Path| caches().into_iter().find(|c| c.0 == p).map_or(0, |c| c.1);
    let once = size(&cache);
    assert!(once >= 60 * 1024, "the omitted media was carried: {once}");
    // Repeated exports, failed exports and saves do not multiply cached bytes.
    for i in 0..5 {
        job_ok(&h, "doc.package.export", json!({ "path": path(&dir.join("e.animpkg")), "onExists": "overwrite" }));
        let bad = dir.join("missing-folder").join(format!("{i}")).join("x.animpkg");
        let _ = run(&h, "doc.package.export", json!({ "path": path(&bad) }));
    }
    let after_exports = size(&cache);
    assert_eq!(after_exports, once, "exports and failures reuse the content-addressed copy");
    // Undo and the checkpoint still find the bytes the cache holds.
    ok(&h, "edit.undo", json!({}));
    assert!(ok(&h, "doc.asset.list", json!({}))["assets"].as_array().unwrap().iter().any(|a| a["id"] == asset));
    job_ok(&h, "doc.package.export", json!({ "path": path(&dir.join("undone.animpkg")) }));
    ok(&h, "doc.asset.remove", json!({ "assetIds": [asset.clone()], "force": true }));
    ok(&h, "checkpoint.restore", json!({ "name": "before removal" }));
    job_ok(&h, "doc.package.export", json!({ "path": path(&dir.join("restored.animpkg")) }));
    let restored = tt_persist::Container::open(
        &dir.join("restored.animpkg"),
        tt_persist::manifest::PACKAGE_MIMETYPE,
        Default::default(),
    )
    .unwrap();
    assert!(
        restored.names().iter().any(|n| n.starts_with("media/") || n.starts_with("assets/")),
        "{:?}",
        restored.names()
    );
    drop(restored);
    // Project switches: closing releases the cache; the next project starts from nothing.
    let before_close = size(&cache);
    job_ok(&h, "project.open", json!({ "path": path(&other), "onUnsaved": "discard" }));
    let after_switch = size(&cache);
    assert_eq!(after_switch, 0, "a project switch empties the closed project's cache");
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    let after_close = size(&cache);
    drop(h);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while cache.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let report = json!({
        "obligation": "C1.4 obligation 8",
        "platform": std::env::consts::OS,
        "fixture": "sample_document(901), every embedded medium 60 KiB",
        "cacheBytes": { "afterFirstExport": once, "afterFiveExportsFiveFailures": after_exports,
                        "beforeClose": before_close, "afterProjectSwitch": after_switch, "afterClose": after_close,
                        "afterExecutorStopExists": cache.exists() },
    });
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c14");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join(format!("cache-bounds-{}.json", std::env::consts::OS)),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert!(!cache.exists(), "the executor stop removes its cache: {report}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Rewrites one byte of a file in place, keeping its size and modification time (an edit RO-02's watch cannot see).
fn flip_keeping_size_and_mtime(file: &Path) {
    let mtime = std::fs::metadata(file).unwrap().modified().unwrap();
    let mut bytes = std::fs::read(file).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(file, &bytes).unwrap();
    std::fs::File::options().write(true).open(file).unwrap().set_modified(mtime).unwrap();
}

#[test]
fn every_save_path_verifies_the_content_recorded_at_open_or_last_save() {
    // C14-R1-09 (SI-047 interim ruling): size + mtime for the watch tick, content hashes before a save replaces.
    for folder in [false, true] {
        let dir = tmp();
        let target = if folder { dir.join("Same") } else { dir.join("Same.animpkg") };
        let h = spawn(&dir.join("user"));
        let create_path =
            if folder { format!("{}{}", path(&target), std::path::MAIN_SEPARATOR) } else { path(&target) };
        ok(&h, "project.create", json!({ "name": "Same", "path": create_path }));
        let scene = first(&h, "scene");
        ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 1 } } }));
        job_ok(&h, "project.save", json!({}));
        let victim = if folder { target.join("project.json") } else { target.clone() };
        flip_keeping_size_and_mtime(&victim);
        h.run_maintenance(false).unwrap();
        let edited = std::fs::read(&victim).unwrap();
        // The user: a clean save, a dirty save, and close-with-save are refused.
        assert_eq!(err(&h, "project.save", json!({})).code, "project.changedOnDisk", "folder {folder}: clean save");
        ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 2 } } }));
        assert_eq!(err(&h, "project.save", json!({})).code, "project.changedOnDisk", "folder {folder}: dirty save");
        let closing = run(&h, "project.close", json!({ "onUnsaved": "save" }));
        assert_eq!(closing.error.unwrap().code, "project.changedOnDisk", "folder {folder}: close with save");
        assert_eq!(std::fs::read(&victim).unwrap(), edited, "folder {folder}: nothing replaced the external edit");
        // An agent: a bound confirmation (SI-045), and the token keeps mine.
        let a = agent_session(&h, vec![dir.clone()], Policy::Assist);
        let t = token(&call(&h, &a, "project.save", json!({}), None));
        assert_eq!(finish(&h, &call(&h, &a, "project.save", json!({}), Some(&t)))["state"], "succeeded");
        assert_ne!(std::fs::read(&victim).unwrap(), edited, "folder {folder}: keep mine overwrote");
        // After the save its content is the recorded one: the next save needs nothing.
        assert!(call(&h, &a, "project.save", json!({}), None).is_ok());
        drop(h);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn an_activity_export_is_written_only_when_its_call_commits() {
    // C14-R1-03: creation and replacement are staged effects; a later failing step keeps the old file / no file.
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Staged", "path": path(&dir.join("Staged.animpkg")) }));
    let existing = dir.join("existing.jsonl");
    std::fs::write(&existing, b"old").unwrap();
    let fresh = dir.join("fresh.jsonl");
    for (target, extra) in [(&existing, json!({ "onExists": "overwrite" })), (&fresh, json!({}))] {
        let mut params = json!({ "path": path(target) });
        params.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let failed = batch(
            &h,
            json!({ "label": "Export then fail", "atomic": true, "steps": [
                { "id": "agent.activity.export", "params": params },
                { "id": "doc.object.patch", "params": { "targets": ["el_missing"], "props": {} } },
            ] }),
        );
        assert!(failed.error.is_some(), "{failed:?}");
        if target == &existing {
            assert_eq!(std::fs::read(&existing).unwrap(), b"old", "a failed batch kept the old file");
        } else {
            assert!(!fresh.exists(), "a failed batch created no file");
        }
        // The same call alone commits and writes; its result matches the declared result schema (both formats).
        let done = run(&h, "agent.activity.export", params);
        assert!(done.is_ok(), "{done:?}");
        let schema = tt_executor::registry().unwrap().get("agent.activity.export").unwrap().result_schema();
        let validator = jsonschema::options().build(&schema).unwrap();
        let result = done.result.clone().unwrap();
        let errors: Vec<String> = validator.iter_errors(&result).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?} {result}");
        assert!(target.exists() && std::fs::read(target).unwrap() != b"old");
    }
    // No staged temporaries are left beside the targets.
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}
