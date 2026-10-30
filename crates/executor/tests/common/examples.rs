//! The fixture state every declared example expects (copied from `c12_kernel_state.rs`, whose runner stays
//! unchanged): one checkpoint, a user workspace, a job, undo and redo steps; file-based examples run on the fixture
//! saved as a package in a temporary folder that stands for the examples' `/tmp`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "test helpers")]

use std::time::Duration;

use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Status};

use super::common::*;

pub fn prepared() -> ExecutorHandle {
    let h = fixture();
    edits(&h);
    h
}

pub fn edits(h: &ExecutorHandle) {
    let scene = first(h, "scene");
    ok(h, "checkpoint.create", json!({ "name": "before-walk" }));
    ok(h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "p": 1 } } }));
    ok(h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "p": 2 } } }));
    ok(h, "edit.undo", json!({}));
    ok(h, "workspace.create", json!({ "name": "Inking" }));
    h.jobs().submit("test.job", "user", |_| Ok(json!({}))).unwrap();
}

/// Commands whose examples work with files (C1.3): they run on the fixture saved as a package with its embedded sound,
/// with a backup `bk_1a2b3c4d`, recovery data `rc_1a2b3c4d`, `Short.animpkg` and a `media` folder in a temporary folder
/// that stands for the examples' `/tmp`.
pub fn with_files(id: &str) -> bool {
    ["project.", "doc.asset.", "doc.blob.", "doc.package.", "agent."].iter().any(|p| id.starts_with(p))
}

pub fn wait_job(h: &ExecutorHandle, started: &Value) {
    let r = ok(h, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60_000 }));
    let rec = if r.get("record").is_some() { r["record"].clone() } else { r };
    assert_eq!(rec["state"], "succeeded", "{rec}");
}

pub fn prepared_with_files(id: &str, dir: &std::path::Path) -> ExecutorHandle {
    use tt_persist::container::{BlobData, Blobs};
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let mut blobs = Blobs::default();
    blobs.insert(
        "media/as_01K742SG0G5HPQ0RBF5X4DSR0W.wav".into(),
        BlobData::Bytes(std::sync::Arc::new(vec![0; 352_844])),
    );
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-10-01T09:00:00Z".into(),
    };
    let sample = dir.join("Sample.animpkg");
    tt_persist::save::save_package(&sample, &input, None).unwrap();
    std::fs::copy(&sample, dir.join("Short.animpkg")).unwrap();
    std::fs::create_dir_all(dir.join("media")).unwrap();
    let user = tt_persist::settings::UserDirs::under(&dir.join("user"));
    let project_id = doc.project().id().to_string();
    // Only the files an example uses are made (file creation is slow on CI runners).
    if id.starts_with("project.backup.") {
        let backups = user.backups.join(&project_id);
        std::fs::create_dir_all(&backups).unwrap();
        std::fs::copy(&sample, backups.join("20261001T090000Z.animpkg")).unwrap();
        let index = json!({ "format": "toontown-backups", "formatVersion": "1.0", "backups": [{
            "backupId": "bk_1a2b3c4d", "originalPath": sample.to_string_lossy(), "projectName": "Sample",
            "savedAt": "2026-10-01T09:00:00Z", "documentRev": 1, "sizeBytes": 1, "file": "20261001T090000Z.animpkg" }] });
        std::fs::write(backups.join("backup.json"), serde_json::to_vec(&index).unwrap()).unwrap();
    }
    if id.starts_with("project.recovery.") {
        let info = tt_persist::recovery::RecoveryInfo {
            recovery_id: "rc_1a2b3c4d".into(),
            project_id,
            project_name: "Sample".into(),
            original_path: Some(sample.to_string_lossy().into_owned()),
            saved_at: "2026-10-01T09:00:00Z".into(),
            document_rev: 1,
            app_version: "0.1.0".into(),
            process_id: 0,
            host: String::new(),
            heartbeat_at: "2026-10-01T09:00:00Z".into(),
        };
        tt_persist::recovery::write(&user.recovery, &info, &input).unwrap();
    }
    let h = tt_executor::ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(60) },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(user),
            maintenance: tt_executor::Maintenance { tick: Duration::ZERO, ..Default::default() },
            ..Default::default()
        },
    })
    .unwrap();
    wait_job(&h, &ok(&h, "project.open", json!({ "path": sample.to_string_lossy() })));
    edits(&h);
    // Examples that replace the open project run on a saved one.
    if matches!(
        id,
        "project.close" | "project.create" | "project.open" | "project.backup.open" | "project.recovery.open"
    ) {
        wait_job(&h, &ok(&h, "project.save", json!({})));
    }
    h
}

/// Maps the examples' `/tmp/…` paths into `dir`.
pub fn map_tmp(v: &mut Value, dir: &std::path::Path) {
    match v {
        Value::String(s) if s.starts_with("/tmp/") => *s = dir.join(&s["/tmp/".len()..]).to_string_lossy().into_owned(),
        Value::Array(a) => a.iter_mut().for_each(|x| map_tmp(x, dir)),
        Value::Object(m) => m.values_mut().for_each(|x| map_tmp(x, dir)),
        _ => {}
    }
}

/// `agent.*` examples (C1.4) name a session `a1` and a confirmation `cf_1`: an agent session is connected and a
/// confirmable call of it (an overwriting Save As) leaves a pending confirmation; the placeholders become their ids.
pub fn agent_placeholders(h: &ExecutorHandle, dir: &std::path::Path, params: &mut Value) {
    let a = h
        .connect_agent(tt_executor::agents::AgentConnect { client: "Example agent".into(), ..Default::default() })
        .unwrap();
    let origin =
        tt_executor::Origin::Agent { session: a.session_id.clone(), name: a.client.clone(), scopes: a.scopes.clone() };
    let env = h.invoke(tt_executor::Invocation {
        command: "project.saveAs".into(),
        params: json!({ "path": dir.join("Short.animpkg").to_string_lossy(), "onExists": "overwrite" }),
        options: Default::default(),
        origin,
    });
    assert_eq!(env.status, Status::NeedsConfirmation, "{env:?}");
    let token = env.result.unwrap()["confirmToken"].as_str().unwrap().to_owned();
    fn walk(v: &mut Value, map: &[(&str, &str)]) {
        match v {
            Value::String(s) => {
                if let Some((_, to)) = map.iter().find(|(from, _)| s == from) {
                    *s = (*to).to_owned();
                }
            }
            Value::Array(x) => x.iter_mut().for_each(|y| walk(y, map)),
            Value::Object(m) => m.values_mut().for_each(|y| walk(y, map)),
            _ => {}
        }
    }
    walk(params, &[("a1", &a.session_id), ("cf_1", &token)]);
}

/// The clipboard a paste example expects (paste examples paste cells of `Bob`).
pub fn clipboard_for(h: &ExecutorHandle, id: &str) {
    if matches!(id, "edit.paste" | "edit.pasteSpecial" | "edit.pasteAsNewElement") {
        ok(h, "edit.copy", json!({ "context": "xsheet", "objectIds": ["scene:#1/element:\"Bob\"/cell:#1"] }));
    }
}
