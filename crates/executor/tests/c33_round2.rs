//! M3 C3.3 round 2 (builder): staging recovery acts only on files a valid sidecar proves are a range job's own
//! (C33-R2-01), recovery receipts stay in cancelled and failed job records (C33-R2-02), and a rollback the file system
//! refuses is reported and retried, never dropped (round-2 item 1). ADR-0020 decision 11.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, JobState};
use tt_persist::lock::{Boundary, hooks};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Clears the spin and boundary hooks however the test ends.
struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
        hooks::set(None);
    }
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn put(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::write(dir.join(name), bytes).unwrap();
}

fn read(dir: &Path, name: &str) -> Vec<u8> {
    std::fs::read(dir.join(name)).unwrap()
}

/// A sidecar exactly as a range job writes it.
fn sidecar(token: &str, pid: u32, host: &str, published: bool, targets: &[&str]) -> Value {
    json!({
        "kind": "toontown.render.range.staging",
        "version": 1,
        "token": token,
        "jobId": "job-999",
        "head": "unknown",
        "productVersion": "0.0.1",
        "host": host,
        "pid": pid,
        "published": published,
        "targets": targets,
        "staged": targets.iter().map(|t| format!(".{t}.tt-partial-{token}")).collect::<Vec<_>>(),
        "backups": targets.iter().map(|t| format!(".{t}.tt-backup-{token}")).collect::<Vec<_>>(),
    })
}

fn put_sidecar(dir: &Path, token: &str, v: &Value) -> String {
    let name = format!(".tt-partial-{token}.job");
    put(dir, &name, serde_json::to_string_pretty(v).unwrap().as_bytes());
    name
}

fn me() -> (u32, String) {
    (std::process::id(), tt_persist::host::host())
}

fn range(h: &ExecutorHandle, dir: &Path, extra: Value) -> Value {
    let mut p = json!({ "from": 1, "to": 1, "width": 32, "directory": dir.to_string_lossy() });
    if let (Some(m), Some(e)) = (p.as_object_mut(), extra.as_object()) {
        m.extend(e.clone());
    }
    c13_support::job_ok(h, "render.range.run", p)
}

fn warning<'a>(m: &'a Value, code: &str) -> Option<&'a Value> {
    m["warnings"].as_array().and_then(|w| w.iter().find(|w| w["code"] == code))
}

fn strings(v: &Value) -> Vec<String> {
    let mut s: Vec<String> = v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_owned()).collect();
    s.sort();
    s
}

#[test]
fn unowned_marker_like_files_are_never_touched_or_reported() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let (pid, host) = me();
    let plain = [
        ".artist-notes.tt-partial-draft",
        ".x.png.tt-partial-ABCDEF123456",
        ".x.png.tt-partial-0a1b2c",
        ".x.png.tt-partial-0123456789abc",
        ".x.png.tt-partial-0123456789ag",
        "x.png.tt-partial-0123456789ab",
        ".y.png.tt-backup-0123456789ab",
        // A well-formed name for a target this very job writes, but no sidecar lists it.
        ".Intro_0001.png.tt-partial-0123456789ab",
        ".tt-partial-0123456789ab.job.bak",
        ".tt-partial-.job",
    ];
    for f in plain {
        put(&dir, f, b"user");
    }
    // Sidecar-like files that are not exactly what a job writes: each lists a real file, which must survive.
    put(&dir, ".k.png.tt-partial-aaaaaaaaaaaa", b"user");
    put(&dir, ".k.png.tt-partial-bbbbbbbbbbbb", b"user");
    put(&dir, ".k.png.tt-partial-cccccccccccc", b"user");
    put(&dir, ".k.png.tt-partial-dddddddddddd", b"user");
    put(
        &dir,
        ".tt-partial-zzzzzzzzzzzz.job",
        sidecar("zzzzzzzzzzzz", pid, &host, false, &["k.png"]).to_string().as_bytes(),
    );
    // Token inside differs from the name.
    put_sidecar(&dir, "aaaaaaaaaaaa", &sidecar("bbbbbbbbbbbb", pid, &host, false, &["k.png"]));
    // A target that is a path.
    put_sidecar(&dir, "cccccccccccc", &sidecar("cccccccccccc", pid, &host, false, &["../k.png"]));
    // Staged names not derived from the targets.
    let mut odd = sidecar("dddddddddddd", pid, &host, false, &["k.png"]);
    odd["staged"] = json!([".other.png.tt-partial-dddddddddddd"]);
    put_sidecar(&dir, "dddddddddddd", &odd);
    // Malformed JSON and an unknown field.
    put(&dir, ".tt-partial-eeeeeeeeeeee.job", b"{ not json");
    let mut extra = sidecar("ffffffffffff", pid, &host, false, &["k.png"]);
    extra["also"] = json!(1);
    put_sidecar(&dir, "ffffffffffff", &extra);
    let before: Vec<(String, Vec<u8>)> = files(&dir).into_iter().map(|f| (f.clone(), read(&dir, &f))).collect();
    let m = range(&h, &dir, json!({}));
    assert!(warning(&m, "render.staleStagingRemoved").is_none(), "nothing unowned is reported: {m}");
    for (f, bytes) in &before {
        assert_eq!(&read(&dir, f), bytes, "{f} untouched");
    }
    let mut want: Vec<String> = before.into_iter().map(|(f, _)| f).collect();
    want.extend(["Intro_0001-0001.manifest.json".to_owned(), "Intro_0001.png".to_owned()]);
    want.sort();
    assert_eq!(files(&dir), want, "only the job's own outputs were added; its own staging and sidecar are gone");
}

#[test]
fn a_dead_jobs_listed_staging_is_recovered_and_reported() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let (pid, host) = me();
    // A job of this process whose token is not live: what a job that died mid-publication leaves.
    let t = "0a1b2c3d4e5f";
    let side = put_sidecar(&dir, t, &sidecar(t, pid, &host, false, &["dead.png", "orig.png", "over.png", "gone.png"]));
    put(&dir, &format!(".dead.png.tt-partial-{t}"), b"never published");
    put(&dir, &format!(".orig.png.tt-backup-{t}"), b"original orig");
    put(&dir, &format!(".over.png.tt-backup-{t}"), b"original over");
    put(&dir, "over.png", b"interrupted publication");
    // Same token, not listed: not the job's.
    put(&dir, &format!(".unlisted.png.tt-partial-{t}"), b"user");
    let m = range(&h, &dir, json!({}));
    let w = warning(&m, "render.staleStagingRemoved").expect("reported");
    assert_eq!(
        strings(&w["files"]),
        strings(&json!([
            format!(".dead.png.tt-partial-{t}"),
            format!(".orig.png.tt-backup-{t}"),
            format!(".over.png.tt-backup-{t}"),
            side
        ]))
    );
    assert_eq!(strings(&w["restored"]), vec!["orig.png", "over.png"]);
    assert!(w.get("unrestored").is_none());
    assert_eq!(read(&dir, "orig.png"), b"original orig", "a missing target gets its original back");
    assert_eq!(read(&dir, "over.png"), b"original over", "the interrupted publication is undone");
    assert_eq!(read(&dir, &format!(".unlisted.png.tt-partial-{t}")), b"user");
    assert!(!dir.join("dead.png").exists());
}

#[test]
fn a_completed_publications_sidecar_keeps_its_targets() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let (pid, host) = me();
    let t = "1234567890ab";
    let side = put_sidecar(&dir, t, &sidecar(t, pid, &host, true, &["done.png"]));
    put(&dir, &format!(".done.png.tt-backup-{t}"), b"original");
    put(&dir, "done.png", b"published");
    let m = range(&h, &dir, json!({}));
    let w = warning(&m, "render.staleStagingRemoved").expect("reported");
    assert_eq!(strings(&w["files"]), strings(&json!([format!(".done.png.tt-backup-{t}"), side])));
    assert!(w.get("restored").is_none());
    assert_eq!(read(&dir, "done.png"), b"published");
}

#[test]
fn sidecars_of_live_processes_and_other_machines_are_left_alone() {
    if std::env::var_os("TT_C33_R2_LIVE_CHILD").is_some() {
        return;
    }
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let (_, host) = me();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "live_child", "--nocapture"])
        .env("TT_C33_R2_LIVE_CHILD", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let (a, b) = ("aaaa00000000", "bbbb00000000");
    put_sidecar(&dir, a, &sidecar(a, child.id(), &host, false, &["a.png"]));
    put(&dir, &format!(".a.png.tt-partial-{a}"), b"live job");
    put_sidecar(&dir, b, &sidecar(b, std::process::id(), "another-machine", false, &["b.png"]));
    put(&dir, &format!(".b.png.tt-partial-{b}"), b"remote job");
    let m = range(&h, &dir, json!({}));
    assert!(warning(&m, "render.staleStagingRemoved").is_none(), "{m}");
    assert!(dir.join(format!(".a.png.tt-partial-{a}")).exists() && dir.join(format!(".b.png.tt-partial-{b}")).exists());
    // Once the process has exited, its job is known to be gone: the next job recovers its staging.
    child.kill().unwrap();
    child.wait().unwrap();
    let m = range(&h, &dir, json!({ "onConflict": "overwrite" }));
    let w = warning(&m, "render.staleStagingRemoved").expect("reported");
    assert_eq!(
        strings(&w["files"]),
        strings(&json!([format!(".a.png.tt-partial-{a}"), format!(".tt-partial-{a}.job")]))
    );
    assert!(dir.join(format!(".b.png.tt-partial-{b}")).exists(), "another machine's job is never assumed dead");
}

/// The live process of `sidecars_of_live_processes_and_other_machines_are_left_alone`.
#[test]
fn live_child() {
    if std::env::var_os("TT_C33_R2_LIVE_CHILD").is_some() {
        std::thread::sleep(Duration::from_secs(120));
    }
}

fn staged_of(dir: &Path, prefix: &str) -> Vec<String> {
    files(dir).into_iter().filter(|f| f.starts_with(prefix) && f.contains(".tt-partial-")).collect()
}

fn wait_until(what: &str, f: impl Fn() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < Duration::from_secs(60), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn record(h: &ExecutorHandle, job: &str) -> Value {
    ok(h, "job.wait", json!({ "jobId": job, "timeoutMs": 60_000 }));
    let list = ok(h, "job.list", json!({ "jobId": job }));
    let jobs = if list.is_array() { list } else { list["jobs"].clone() };
    jobs.as_array().and_then(|j| j.first()).cloned().expect("the job's record")
}

#[test]
fn two_jobs_in_one_directory_recover_only_their_own_staging() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let start = |pattern: &str| {
        ok(
            &h,
            "render.range.run",
            json!({ "from": 1, "to": 48, "width": 32, "directory": dir.to_string_lossy(), "namePattern": pattern }),
        )["jobId"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let a = start("a-{frame}");
    let b = start("b-{frame}");
    wait_until("both jobs staging", || !staged_of(&dir, ".a-").is_empty() && !staged_of(&dir, ".b-").is_empty());
    let sidecars = || files(&dir).into_iter().filter(|f| f.ends_with(".job")).count();
    assert_eq!(sidecars(), 2);
    // A third job into the directory while both run: their sidecars are live, so it recovers nothing.
    tt_vector::work::hooks::set_spin_micros(0);
    let m = range(&h, &dir, json!({ "namePattern": "c-{frame}" }));
    assert!(warning(&m, "render.staleStagingRemoved").is_none(), "{m}");
    tt_vector::work::hooks::set_spin_micros(20_000);
    assert_eq!(sidecars(), 2);
    ok(&h, "job.cancel", json!({ "jobId": a }));
    let rec = record(&h, &a);
    assert_eq!(rec["state"], "cancelled", "{rec}");
    let w =
        rec["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "render.stagingDiscarded").expect("receipt");
    let listed = strings(&w["files"]);
    assert!(listed.iter().all(|f| f.starts_with(".a-") || f.starts_with(".tt-partial-")), "{listed:?}");
    assert!(listed.iter().any(|f| f.ends_with(".job")));
    assert!(staged_of(&dir, ".a-").is_empty(), "A's staging is gone");
    assert!(!staged_of(&dir, ".b-").is_empty(), "B's staging is untouched by A's recovery");
    assert_eq!(sidecars(), 1);
    ok(&h, "job.cancel", json!({ "jobId": b }));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(record(&h, &b)["state"], "cancelled");
    assert_eq!(files(&dir), vec!["Intro_0001-0001.manifest.json", "c-1.png"]);
}

#[test]
fn a_failed_jobs_record_carries_its_error_and_recovery_receipt() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    std::fs::create_dir(dir.join("shot-2.png")).unwrap();
    let started = ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 2, "width": 32, "directory": dir.to_string_lossy(), "namePattern": "shot-{frame}", "onConflict": "overwrite" }),
    );
    let job = started["jobId"].as_str().unwrap().to_owned();
    let rec = record(&h, &job);
    assert_eq!(rec["state"], "failed", "{rec}");
    assert_eq!(rec["error"]["code"], "export.targetInvalid");
    let w =
        rec["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "render.stagingDiscarded").expect("receipt");
    let listed = strings(&w["files"]);
    assert_eq!(listed.len(), 4, "two frames, the manifest and the sidecar: {listed:?}");
    assert_eq!(files(&dir), vec!["shot-2.png"]);
    // The same record through `job.wait` after completion.
    let waited = ok(&h, "job.wait", json!({ "jobId": job, "timeoutMs": 1000 }));
    let waited = if waited.get("record").is_some() { waited["record"].clone() } else { waited };
    assert_eq!(waited["warnings"], rec["warnings"]);
    assert_eq!(h.jobs().wait(&job, Duration::from_secs(1)).unwrap().0.state, JobState::Failed);
}

#[test]
fn a_refused_rollback_fails_the_job_with_the_unrestored_targets_and_the_next_job_retries() {
    let _t = turn();
    let h = fixture();
    let dir = c13_support::tmp();
    let _reset = Reset;
    put(&dir, "shot-1.png", b"original 1");
    let blocked: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
    let seen = blocked.clone();
    hooks::set(Some(Arc::new(move |at: Boundary, path: &Path| {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Publication of frame 2 fails: a directory appears at its target.
        if at == Boundary::RangePublish && name == "shot-2.png" {
            let _ = std::fs::create_dir(path);
        }
        // Restoring frame 1 is refused: its target becomes a directory.
        if at == Boundary::RangeRestore && name == "shot-1.png" {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::create_dir(path);
            seen.lock().unwrap().push(path.to_path_buf());
        }
    })));
    let started = ok(
        &h,
        "render.range.run",
        json!({ "from": 1, "to": 2, "width": 32, "directory": dir.to_string_lossy(), "namePattern": "shot-{frame}", "onConflict": "overwrite" }),
    );
    let rec = record(&h, started["jobId"].as_str().unwrap());
    hooks::set(None);
    assert!(!blocked.lock().unwrap().is_empty());
    assert_eq!(rec["state"], "failed", "{rec}");
    assert_eq!(rec["error"]["code"], "export.rollbackIncomplete", "{rec}");
    let unrestored = strings(&rec["error"]["details"]["unrestored"]);
    assert_eq!(unrestored.len(), 1);
    assert!(unrestored[0].ends_with("shot-1.png"), "{unrestored:?}");
    assert_eq!(rec["error"]["details"]["cause"]["code"], "fs.io");
    let w =
        rec["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "render.stagingDiscarded").expect("receipt");
    assert_eq!(w["unrestored"].as_array().unwrap().len(), 1, "{w}");
    // The original and the sidecar are kept for the retry.
    let backup = files(&dir).into_iter().find(|f| f.starts_with(".shot-1.png.tt-backup-")).expect("backup kept");
    assert_eq!(read(&dir, &backup), b"original 1");
    assert_eq!(files(&dir).iter().filter(|f| f.ends_with(".job")).count(), 1);
    // Once the file system allows it, the next job into the directory puts the original back.
    std::fs::remove_dir(dir.join("shot-1.png")).unwrap();
    std::fs::remove_dir(dir.join("shot-2.png")).unwrap();
    let m = range(&h, &dir, json!({ "namePattern": "next-{frame}" }));
    let w = warning(&m, "render.staleStagingRemoved").expect("retried");
    assert_eq!(strings(&w["restored"]), vec!["shot-1.png"]);
    assert_eq!(read(&dir, "shot-1.png"), b"original 1");
    assert_eq!(files(&dir), vec!["Intro_0001-0001.manifest.json", "next-1.png", "shot-1.png"]);
}

#[test]
fn a_waiting_caller_leaves_reads_free_and_the_cancelled_record_keeps_its_receipt() {
    let _t = turn();
    let h = fixture_with(tt_executor::ExecutorConfig { budget: Duration::from_millis(500) });
    let dir = c13_support::tmp();
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let env = std::thread::scope(|scope| {
        let h = &h;
        let d = dir.clone();
        let t = scope.spawn(move || {
            run(
                h,
                "render.range.run",
                json!({ "from": 1, "to": 48, "width": 32, "directory": d.to_string_lossy(), "wait": true }),
            )
        });
        wait_until("staging", || !staged_of(&dir, ".Intro_").is_empty());
        let start = Instant::now();
        ok(h, "app.state.get", json!({}));
        assert!(start.elapsed() < Duration::from_millis(300), "a document read waited {:?}", start.elapsed());
        t.join().unwrap()
    });
    let e = env.error.expect("timeout");
    assert_eq!(e.code, "job.waitTimeout");
    let job = e.details["jobId"].as_str().unwrap().to_owned();
    ok(&h, "job.cancel", json!({ "jobId": job }));
    tt_vector::work::hooks::set_spin_micros(0);
    let rec = record(&h, &job);
    assert_eq!(rec["state"], "cancelled", "{rec}");
    let w =
        rec["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "render.stagingDiscarded").expect("receipt");
    let listed = strings(&w["files"]);
    assert!(listed.iter().any(|f| f.starts_with(".Intro_0001.png.tt-partial-")), "{listed:?}");
    assert!(listed.iter().any(|f| f.ends_with(".job")), "{listed:?}");
    assert!(files(&dir).is_empty(), "{:?}", files(&dir));
}
