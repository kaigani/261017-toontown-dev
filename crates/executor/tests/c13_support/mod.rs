//! Shared helpers of the C1.3 executor tests: temporary folders, executors with known user folders, jobs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "test helpers")]

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Maintenance, PersistOptions, UserDirs};

use crate::common::ok;

pub fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-c13-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn spawn(dirs: &Path) -> ExecutorHandle {
    spawn_with(dirs, tt_persist::migrate::CHAIN)
}

pub fn spawn_with(dirs: &Path, migrations: &'static [tt_persist::migrate::Migration]) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(60) },
        persist: PersistOptions {
            user_dirs: Some(UserDirs::under(dirs)),
            maintenance: Maintenance { tick: Duration::ZERO, ..Maintenance::default() },
            migrations,
        },
    })
    .unwrap()
}

/// Waits for a job and returns its final record.
pub fn wait(h: &ExecutorHandle, started: &Value) -> Value {
    let id = started["jobId"].as_str().unwrap().to_owned();
    let r = ok(h, "job.wait", json!({ "jobId": id, "timeoutMs": 60_000 }));
    let rec = if r.get("record").is_some() { r["record"].clone() } else { r };
    assert!(rec["state"] == "succeeded" || rec["state"] == "failed" || rec["state"] == "cancelled", "{rec}");
    rec
}

pub fn job_ok(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    let started = ok(h, id, params.clone());
    let rec = wait(h, &started);
    assert_eq!(rec["state"], "succeeded", "{id} {params}: {rec}");
    rec["result"].clone()
}

pub fn job_err(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    let started = ok(h, id, params.clone());
    let rec = wait(h, &started);
    assert_eq!(rec["state"], "failed", "{id} {params}: {rec}");
    rec["error"].clone()
}
