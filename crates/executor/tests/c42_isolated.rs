//! M4 C4.2 builder tests of a process-wide quantity, in their own binary (orchestrator ruling D-2): the OS threads of
//! headless captures stay within their stated bound (design note 5 §1c: at most two captures at once, one job thread
//! each; at most eight callers wait for a slot on their own threads; one more answers `limit.exceeded`). Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use serde_json::json;
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, Status};

/// The OS threads of this process.
fn threads() -> usize {
    #[cfg(windows)]
    let text = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).Threads.Count", std::process::id())])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap();
    #[cfg(target_os = "macos")]
    let text = std::process::Command::new("ps")
        .args(["-M", "-p", &std::process::id().to_string()])
        .output()
        .map(|o| (String::from_utf8_lossy(&o.stdout).lines().count().saturating_sub(1)).to_string())
        .unwrap();
    #[cfg(target_os = "linux")]
    let text = std::fs::read_dir("/proc/self/task").unwrap().count().to_string();
    text.parse().unwrap()
}

fn ok(h: &ExecutorHandle, id: &str, params: serde_json::Value) -> serde_json::Value {
    let e = h.invoke(Invocation::user(id, params));
    assert_eq!(e.status, Status::Ok, "{id}: {:?}", e.error);
    e.result.unwrap_or_default()
}

#[test]
fn concurrent_captures_are_capped_and_their_threads_counted() {
    let h = Arc::new(ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(120) }).unwrap());
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(&h, "app.context.set", json!({ "elementId": "scene:#1/element:\"Bob\"", "frame": 1 }));
    // A heavy drawing, so a capture lasts long enough to be observed: wide brush strokes across the view.
    for k in 0..120 {
        let points: Vec<_> = (0..400)
            .map(|i| json!({ "x": -900.0 + f64::from(i) * 4.5, "y": -500.0 + f64::from(k % 30) * 33.0 + f64::from(k / 30) + f64::from(i % 9) * 3.0 }))
            .collect();
        ok(
            &h,
            "draw.stroke.add",
            json!({ "kind": "brush", "points": points, "pen": { "tipShape": "round", "minSize": 20, "maxSize": 40 } }),
        );
    }
    let t = Instant::now();
    ok(&h, "inspect.screenshot", json!({ "region": { "x": 0, "y": 0, "w": 2048, "h": 1080 } }));
    let one = t.elapsed();
    let t = Instant::now();
    let base = threads();
    let sample = t.elapsed();
    eprintln!("one capture {one:?}; one thread sample {sample:?}");
    let callers = 14;
    let barrier = Arc::new(Barrier::new(callers + 1));
    let workers: Vec<_> = (0..callers)
        .map(|_| {
            let (h, b) = (h.clone(), barrier.clone());
            std::thread::spawn(move || {
                b.wait();
                h.invoke(Invocation::user(
                    "inspect.screenshot",
                    json!({ "region": { "x": 0, "y": 0, "w": 2048, "h": 1080 } }),
                ))
            })
        })
        .collect();
    barrier.wait();
    // Sample the process's threads while the captures run.
    let mut peak = 0;
    let start = Instant::now();
    while workers.iter().any(|w| !w.is_finished()) && start.elapsed() < Duration::from_secs(60) {
        peak = peak.max(threads());
    }
    let answers: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    let refused = answers.iter().filter(|e| e.error.as_ref().is_some_and(|x| x.code == "limit.exceeded")).count();
    let done = answers.iter().filter(|e| e.status == Status::Ok).count();
    eprintln!(
        "captures: {done} answered, {refused} refused at the cap; threads: base {base}, peak {peak} with {callers} callers"
    );
    assert_eq!(done + refused, callers, "every caller is answered: a capture or the typed refusal");
    assert!(refused >= 1, "fourteen simultaneous callers exceed two running and eight waiting");
    assert!(done >= 10, "two run and eight wait for a slot");
    let e = answers.iter().find_map(|e| e.error.clone().filter(|x| x.code == "limit.exceeded")).unwrap();
    assert_eq!((e.details["limit"].as_str(), e.details["max"].as_u64()), (Some("concurrentCaptures"), Some(2)));
    // At the cap: the callers' own threads, at most two capture job threads, and a small allowance for the runtime's
    // own (the PowerShell sampling child is another process and is not counted).
    assert!(peak > base, "the sampler saw the captures running (a capture outlasts a sample: {one:?} vs {sample:?})");
    assert!(peak <= base + callers + 2 + 2, "peak {peak} threads over base {base}");
}

/// A heavy drawing on an executor with the given budget, so a capture lasts long enough to hold its slot. The drawing
/// is built on an executor with a long budget (a short budget would cut the building commands on a slow runner) and
/// its document opened in the one returned.
fn heavy(budget: Duration, strokes: u32) -> Arc<ExecutorHandle> {
    let builder = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(120) }).unwrap();
    assert!(builder.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(&builder, "app.context.set", json!({ "elementId": "scene:#1/element:\"Bob\"", "frame": 1 }));
    for k in 0..strokes {
        let points: Vec<_> = (0..400)
            .map(|i| json!({ "x": -900.0 + f64::from(i) * 4.5, "y": -500.0 + f64::from(k % 30) * 33.0 + f64::from(k / 30) + f64::from(i % 9) * 3.0 }))
            .collect();
        ok(
            &builder,
            "draw.stroke.add",
            json!({ "kind": "brush", "points": points, "pen": { "tipShape": "round", "minSize": 20, "maxSize": 40 } }),
        );
    }
    let doc = builder.snapshot().unwrap().unwrap();
    drop(builder);
    let h = Arc::new(ExecutorHandle::spawn(ExecutorConfig { budget }).unwrap());
    assert!(h.open_document(doc, None, false).is_ok());
    ok(&h, "app.context.set", json!({ "elementId": "scene:#1/element:\"Bob\"", "frame": 1 }));
    h
}

fn running_captures(h: &ExecutorHandle) -> usize {
    h.jobs().list(None, None).iter().filter(|j| j.command_id == "inspect.screenshot" && !j.state.is_final()).count()
}

#[test]
fn a_panicking_capture_job_releases_its_slot() {
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(60) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    // Two capture jobs that panic: each ends failed (typed), so neither holds a slot.
    for _ in 0..2 {
        let id = h.jobs().submit("inspect.screenshot", "user", |_| panic!("capture failure for the test")).unwrap();
        let (record, timed_out) = h.jobs().wait(&id, Duration::from_secs(10)).unwrap();
        assert!(!timed_out);
        assert_eq!(record.state, tt_executor::JobState::Failed, "{record:?}");
        assert_eq!(record.error.unwrap().code, "internal");
    }
    assert_eq!(running_captures(&h), 0);
    let e = h.invoke(Invocation::user("inspect.screenshot", json!({ "region": { "x": 0, "y": 0, "w": 64, "h": 64 } })));
    assert_eq!(e.status, Status::Ok, "the slots are free: {:?}", e.error);
}

#[test]
fn a_waiting_capture_honours_its_cancellation_and_budget_and_starts_nothing_afterwards() {
    // No wall-clock figure close to a budget is asserted: the order of events, the refusal's code, what was started,
    // and generous upper bounds (ten times the budget or more), on a monotonic clock.
    let region = json!({ "region": { "x": 0, "y": 0, "w": 2048, "h": 1080 } });
    let holders_of = |h: &Arc<ExecutorHandle>| -> Vec<std::thread::JoinHandle<tt_executor::Envelope>> {
        (0..2)
            .map(|_| {
                let (h, r) = (h.clone(), region.clone());
                std::thread::spawn(move || h.invoke(Invocation::user("inspect.screenshot", r)))
            })
            .collect()
    };
    let wait_both_running = |h: &ExecutorHandle| {
        let start = Instant::now();
        while running_captures(h) < 2 && start.elapsed() < Duration::from_secs(60) {
            std::thread::yield_now();
        }
        assert_eq!(running_captures(h), 2, "both slots held");
    };

    // 1. Cancellation. The holders have a long budget, so both slots stay held while the third caller waits; it
    //    cancels soon after it starts waiting and must answer the cancellation, and no capture of its own is started.
    let h = heavy(Duration::from_secs(120), 120);
    let holders = holders_of(&h);
    wait_both_running(&h);
    let asked = Instant::now();
    let (env, _) = h.invoke_served_until(Invocation::user("inspect.screenshot", region.clone()), &|| {
        asked.elapsed() >= Duration::from_millis(20)
    });
    let took = asked.elapsed();
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{env:?}");
    assert!(took < Duration::from_secs(10), "answered after its cancellation, well within bounds ({took:?})");
    for t in holders {
        assert_eq!(t.join().unwrap().status, Status::Ok);
    }
    let captures = h.jobs().list(None, None).iter().filter(|j| j.command_id == "inspect.screenshot").count();
    assert_eq!(captures, 2, "the holders' two capture jobs, none for the cancelled waiter");
    drop(h);

    // 2. Budget. Every call of this executor has a 250 ms budget, so the holders' calls end at their budget too and
    //    free their slots while their capture jobs run on. The waiter therefore meets one of: its own budget, or a
    //    freed slot with two capture jobs still running (both `limit.exceeded`, nothing started); or a freed slot after
    //    a holder's job ended (it captures: an answer or `job.waitTimeout`). In every case it answers within ten times
    //    its budget and the capture jobs running at once never exceed two.
    let budget = Duration::from_millis(250);
    let h = heavy(budget, 120);
    let peak = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sampler = {
        let (h, peak, stop) = (h.clone(), peak.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                peak.fetch_max(running_captures(&h), std::sync::atomic::Ordering::SeqCst);
                std::thread::yield_now();
            }
        })
    };
    let holders = holders_of(&h);
    wait_both_running(&h);
    let before = h.jobs().list(None, None).len();
    let asked = Instant::now();
    let env = h.invoke(Invocation::user("inspect.screenshot", region.clone()));
    let took = asked.elapsed();
    let after = h.jobs().list(None, None).len();
    for t in holders {
        let _ = t.join().unwrap();
    }
    let start = Instant::now();
    while running_captures(&h) > 0 && start.elapsed() < Duration::from_secs(60) {
        std::thread::yield_now();
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    sampler.join().unwrap();
    let code = env.error.as_ref().map(|e| e.code.clone());
    eprintln!(
        "budget waiter: {code:?} after {took:?}; capture jobs {before} -> {after}; peak running {}",
        peak.load(std::sync::atomic::Ordering::SeqCst)
    );
    assert!(took < budget * 10, "answered within ten times its budget ({took:?})");
    match code.as_deref() {
        Some("limit.exceeded") => assert_eq!(after, before, "a refused waiter starts nothing"),
        None | Some("job.waitTimeout") => {
            assert_eq!(after, before + 1, "a waiter that took a freed slot started one capture")
        }
        other => panic!("unexpected answer {other:?}: {env:?}"),
    }
    assert!(peak.load(std::sync::atomic::Ordering::SeqCst) <= 2, "never more than two capture jobs at once");
}
