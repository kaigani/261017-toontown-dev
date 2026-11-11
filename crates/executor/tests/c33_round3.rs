//! M3 C3.3 round 3 (builder): a cancelled job record keeps its typed error with its state and warnings, through
//! `job.list` and `job.wait` (C33-R3-01), and a cancellation that meets a refused rollback keeps
//! `export.rollbackIncomplete`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, JobState};
use tt_persist::lock::{Boundary, hooks};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
        hooks::set(None);
    }
}

fn listed(h: &ExecutorHandle, job: &str) -> Value {
    let list = ok(h, "job.list", json!({ "jobId": job }));
    let jobs = if list.is_array() { list } else { list["jobs"].clone() };
    jobs.as_array().and_then(|j| j.first()).cloned().expect("the job's record")
}

fn waited(h: &ExecutorHandle, job: &str) -> Value {
    let w = ok(h, "job.wait", json!({ "jobId": job, "timeoutMs": 60_000 }));
    if w.get("record").is_some() { w["record"].clone() } else { w }
}

#[test]
fn a_cancelled_record_keeps_its_typed_error_and_warnings_through_list_and_wait() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let job = ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 48, "width": 32, "directory": dir.to_string_lossy() }),
    )["jobId"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "job.cancel", json!({ "jobId": job }));
    tt_vector::work::hooks::set_spin_micros(0);
    let rec = h.jobs().wait(&job, Duration::from_secs(60)).unwrap().0;
    assert_eq!(rec.state, JobState::Cancelled, "the state is not changed");
    assert_eq!(rec.error.as_ref().map(|e| e.code.as_str()), Some("job.cancelled"), "{rec:?}");
    assert!(rec.result.is_none());
    for r in [waited(&h, &job), listed(&h, &job)] {
        assert_eq!(r["state"], "cancelled", "{r}");
        assert_eq!(r["error"]["code"], "job.cancelled", "{r}");
        assert!(
            r["warnings"].as_array().is_some_and(|w| w.iter().any(|w| w["code"] == "render.stagingDiscarded")),
            "{r}"
        );
    }
}

#[test]
fn a_cancellation_meeting_a_refused_rollback_keeps_rollback_incomplete() {
    let _t = turn();
    let h = Arc::new(fixture());
    let dir = c13_support::tmp();
    let _reset = Reset;
    std::fs::write(dir.join("shot-1.png"), b"original 1").unwrap();
    let job: Arc<Mutex<Option<String>>> = Arc::default();
    let cancel_answers: Arc<Mutex<Vec<String>>> = Arc::default();
    let (hj, jj, answers) = (h.clone(), job.clone(), cancel_answers.clone());
    hooks::set(Some(Arc::new(move |at: Boundary, path: &Path| {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if at == Boundary::RangePublish && name == "shot-2.png" {
            // The caller cancels during publication, which then fails.
            if let Some(id) = jj.lock().unwrap().clone() {
                let state = hj.jobs().cancel(&id).map(|r| format!("{:?}", r.state)).unwrap_or_default();
                answers.lock().unwrap().push(state);
            }
            let _ = std::fs::create_dir(path);
        }
        if at == Boundary::RangeRestore && name == "shot-1.png" {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::create_dir(path);
        }
    })));
    tt_vector::work::hooks::set_spin_micros(20_000);
    let started = ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 2, "width": 32, "directory": dir.to_string_lossy(), "namePattern": "shot-{frame}", "onConflict": "overwrite" }),
    );
    let id = started["jobId"].as_str().unwrap().to_owned();
    *job.lock().unwrap() = Some(id.clone());
    let rec = h.jobs().wait(&id, Duration::from_secs(60)).unwrap().0;
    hooks::set(None);
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(cancel_answers.lock().unwrap().len(), 1, "the cancellation reached the job during publication");
    // Past the commit point the cancellation cannot take effect; the rollback's error is what the record keeps.
    assert_eq!(rec.state, JobState::Failed, "{rec:?}");
    assert_eq!(rec.error.as_ref().map(|e| e.code.as_str()), Some("export.rollbackIncomplete"), "{rec:?}");
    let r = waited(&h, &id);
    assert_eq!(r["error"]["code"], "export.rollbackIncomplete");
    assert!(r["error"]["details"]["unrestored"].as_array().is_some_and(|u| u.len() == 1), "{r}");
    assert!(r["warnings"].as_array().is_some_and(|w| w.iter().any(|w| w["code"] == "render.stagingDiscarded")), "{r}");
    std::fs::remove_dir(dir.join("shot-1.png")).unwrap();
    std::fs::remove_dir(dir.join("shot-2.png")).unwrap();
}
