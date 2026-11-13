//! M3 C3.4 round 1 (builder, orchestrator item O-1): a request cancellation during a waited job step. The contract
//! (ADR-0020 decision 8): the waited step answers at once (`command.timeout`, `reason: "cancelled"`, `jobId`), every
//! later step is refused, the document is unchanged; the job is not the request's: it stays in `job.list` and reaches
//! a terminal state on its own or after `job.cancel`, leaving no staging; a direct `wait: true` call behaves the same.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{BatchRequest, Envelope, ExecutorHandle, Invocation, JobState, Origin};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
    }
}

fn document(h: &ExecutorHandle) -> Value {
    let mut v = ok(h, "doc.export.json", json!({ "scope": "project", "binaries": "omit" }));
    v["bundle"].as_object_mut().unwrap().remove("exportedAt");
    v
}

fn staging(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.contains(".tt-partial-") || n.contains(".tt-backup-"))
                .collect()
        })
        .unwrap_or_default()
}

fn wait_for_staging(dir: &Path) {
    let start = Instant::now();
    while staging(dir).is_empty() {
        assert!(start.elapsed() < Duration::from_secs(30), "the job never staged");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Runs `call` on a thread, cancels its request once the job stages, and returns the answer and how long the call took
/// after the cancellation.
fn cancelled_after_staging(dir: &Path, call: impl FnOnce(&AtomicBool) -> Envelope + Send) -> (Envelope, Duration) {
    let flag = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let f = &flag;
        let t = scope.spawn(move || call(f));
        wait_for_staging(dir);
        let at = Instant::now();
        flag.store(true, Ordering::SeqCst);
        let env = t.join().unwrap();
        (env, at.elapsed())
    })
}

fn waited_error(env: &Envelope) -> (String, String) {
    let e = env.error.as_ref().unwrap_or_else(|| panic!("a cancelled wait answers an error: {env:?}"));
    assert_eq!(e.code, "command.timeout", "{e:?}");
    assert_eq!(e.details["reason"], "cancelled", "{e:?}");
    (e.code.clone(), e.details["jobId"].as_str().unwrap().to_owned())
}

fn job_listed(h: &ExecutorHandle, job: &str) -> Value {
    let list = ok(h, "job.list", json!({ "jobId": job }));
    let jobs = if list.is_array() { list } else { list["jobs"].clone() };
    jobs.as_array().and_then(|j| j.first()).cloned().expect("the job stays in job.list")
}

fn range(dir: &Path) -> Value {
    json!({ "from": 1, "to": 48, "width": 32, "directory": dir.to_string_lossy(), "createDirs": true, "wait": true })
}

#[test]
fn a_cancelled_batch_stops_at_its_waited_step_and_its_job_runs_on() {
    let _t = turn();
    for stop_on_error in [true, false] {
        let h = fixture();
        let dir = c13_support::tmp().join("out");
        let before = document(&h);
        let _reset = Reset;
        tt_vector::work::hooks::set_spin_micros(20_000);
        let req: BatchRequest = serde_json::from_value(json!({
            "label": "export then edit",
            "atomic": false,
            "stopOnError": stop_on_error,
            "steps": [
                { "id": "render.range.run", "params": range(&dir) },
                { "id": "element.create", "params": { "type": "peg", "name": "Late", "sceneId": "scene:#1" } }
            ]
        }))
        .unwrap();
        let (env, took) =
            cancelled_after_staging(&dir, |f| h.batch_until(req, Origin::User, &|| f.load(Ordering::SeqCst)));
        assert!(took < Duration::from_secs(5), "the waited step answers at its next poll, not at the budget: {took:?}");
        let steps = env.steps.clone().unwrap();
        let (_, job) = waited_error(&steps[0]);
        // (a) The later step does not change the document: not run (stopOnError) or refused at its acceptance boundary.
        if let Some(later) = steps.get(1) {
            assert_eq!(later.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{later:?}");
        }
        tt_vector::work::hooks::set_spin_micros(0);
        assert_eq!(
            document(&h),
            before,
            "stopOnError {stop_on_error}: nothing after the cancellation changed the document"
        );
        // (b) The job is the job system's: listed, and terminal on its own (here: it completes).
        let listed = job_listed(&h, &job);
        assert!(listed["state"] == "running" || listed["state"] == "succeeded", "{listed}");
        let rec = h.jobs().wait(&job, Duration::from_secs(120)).unwrap().0;
        assert_eq!(rec.state, JobState::Succeeded, "{rec:?}");
        // (c) Terminal: no staging, no sidecar; the delivery is complete.
        assert!(staging(&dir).is_empty(), "{:?}", staging(&dir));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 49, "48 frames and the manifest");
    }
}

#[test]
fn a_cancelled_batch_job_can_be_cancelled_and_leaves_no_staging() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp().join("out");
    let before = document(&h);
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let req: BatchRequest = serde_json::from_value(json!({
        "label": "export then edit",
        "atomic": false,
        "steps": [
            { "id": "render.range.run", "params": range(&dir) },
            { "id": "element.create", "params": { "type": "peg", "name": "Late", "sceneId": "scene:#1" } }
        ]
    }))
    .unwrap();
    let (env, _) = cancelled_after_staging(&dir, |f| h.batch_until(req, Origin::User, &|| f.load(Ordering::SeqCst)));
    let (_, job) = waited_error(&env.steps.unwrap()[0]);
    assert_eq!(job_listed(&h, &job)["state"], "running");
    ok(&h, "job.cancel", json!({ "jobId": job }));
    tt_vector::work::hooks::set_spin_micros(0);
    let rec = h.jobs().wait(&job, Duration::from_secs(60)).unwrap().0;
    assert_eq!(rec.state, JobState::Cancelled, "{rec:?}");
    assert_eq!(rec.error.as_ref().map(|e| e.code.as_str()), Some("job.cancelled"));
    assert!(std::fs::read_dir(&dir).map(|rd| rd.count()).unwrap_or(0) == 0, "nothing published, no staging left");
    assert_eq!(document(&h), before);
}

#[test]
fn a_cancelled_direct_waited_call_behaves_like_the_batch_step() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp().join("out");
    let before = document(&h);
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let inv = Invocation::user("render.range.run", range(&dir));
    let (env, took) = cancelled_after_staging(&dir, |f| h.invoke_served_until(inv, &|| f.load(Ordering::SeqCst)).0);
    assert!(took < Duration::from_secs(5), "{took:?}");
    let (_, job) = waited_error(&env);
    assert_eq!(job_listed(&h, &job)["state"], "running");
    tt_vector::work::hooks::set_spin_micros(0);
    let rec = h.jobs().wait(&job, Duration::from_secs(120)).unwrap().0;
    assert_eq!(rec.state, JobState::Succeeded, "the job completes on its own");
    assert!(staging(&dir).is_empty());
    assert_eq!(document(&h), before);
}

#[test]
fn an_uncancelled_waited_call_still_answers_the_manifest() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp().join("out");
    let never = AtomicBool::new(false);
    let params = json!({ "from": 1, "to": 2, "width": 32, "directory": dir.to_string_lossy(), "createDirs": true, "wait": true });
    let (env, _) =
        h.invoke_served_until(Invocation::user("render.range.run", params), &|| never.load(Ordering::SeqCst));
    assert_eq!(env.result.as_ref().unwrap()["frameCount"], 2, "{env:?}");
}
