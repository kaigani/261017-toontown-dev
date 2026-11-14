//! Independent round-two kernel probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tt_executor::{Invocation, Origin};
fn ready() -> tt_executor::ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    h
}
fn apply(m: &Value, x: f64, y: f64) -> (f64, f64) {
    let f = |i: usize| m[i].as_f64().unwrap();
    (f(0) * x + f(2) * y + f(4), f(1) * x + f(3) * y + f(5))
}
#[test]
fn mapping_extreme_grid() {
    let h = ready();
    for z in [0.0625, 64.0] {
        for degrees in [0.0, 90.0, 180.0, 270.0, 37.5, -123.45] {
            ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z}));
            ok(&h, "view.rotation.set", json!({"viewId":"drawingView","degrees":degrees}));
            ok(&h, "view.pan.set", json!({"viewId":"drawingView","centre":{"x":1e7,"y":-1e7}}));
            let m = ok(&h, "draw.viewMapping.get", json!({"view":"drawingView"}));
            for (x, y) in [(0.0, 0.0), (1e7, -1e7), (1e7 - 17.25, -1e7 + 23.5)] {
                let a = apply(&m["drawingToView"], x, y);
                let b = apply(&m["viewToDrawing"], a.0, a.1);
                // 64*1e7 terms have ulps ~1.2e-7; 2e-5 admits >100 ulps.
                assert!((b.0 - x).abs() < 2e-5 && (b.1 - y).abs() < 2e-5, "{b:?}");
                let c = ok(
                    &h,
                    "view.coordinates.convert",
                    json!({"viewId":"drawingView","from":"drawing","to":"view","point":{"x":x,"y":y}}),
                );
                assert!((c["point"]["x"].as_f64().unwrap() - a.0).abs() < 2e-5);
                assert!((c["point"]["y"].as_f64().unwrap() - a.1).abs() < 2e-5);
            }
        }
    }
    for z in [0.0, -1.0, 0.06249, 64.001] {
        assert_eq!(err(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z})).code, "arg.invalid");
    }
}
#[test]
fn capture_snapshot_before_following_edit() {
    let h = ready();
    let old = ok(&h, "inspect.screenshot", json!({}));
    // Batch ordering makes the capture snapshot precede the edit without scheduler assumptions.
    let r = batch(
        &h,
        json!({"label":"snapshot review","steps":[{"id":"inspect.screenshot","params":{}},{"id":"draw.rectangle.add","params":{"from":{"x":20,"y":20},"to":{"x":180,"y":180}}}]}),
    );
    assert!(r.is_ok(), "{r:?}");
    let jobs = h.jobs().list(None, None);
    let j = jobs.last().unwrap();
    let (done, timed) = h.jobs().wait(&j.job_id, Duration::from_secs(30)).unwrap();
    assert!(!timed);
    assert_eq!(done.result.unwrap()["png"], old["png"]);
    assert_ne!(ok(&h, "inspect.screenshot", json!({}))["png"], old["png"]);
}
#[test]
fn cancelled_capture_jobs_finish_after_request_returns() {
    let h = ready();
    let before = thread_count();
    let mut states = Vec::new();
    for _ in 0..10 {
        let count = h.jobs().list(None, None).len();
        let (e, _) = h.invoke_served_until(Invocation::user("inspect.screenshot", json!({})), &|| {
            h.jobs().list(None, None).len() > count
        });
        let error = e.error.unwrap();
        assert_eq!(error.code, "command.timeout");
        assert_eq!(error.details["reason"], "cancelled");
        let id = error.details["jobId"].as_str().unwrap();
        let (done, timed) = h.jobs().wait(id, Duration::from_secs(30)).unwrap();
        assert!(!timed);
        states.push(serde_json::to_value(done.state).unwrap());
    }
    println!("ten cancelled captures: threads before={before:?}, after={:?}", thread_count());
    assert!(states.iter().all(|s| s == "succeeded"), "ADR-0020 decision 8 job continuation changed: {states:?}");
}
#[test]
fn gesture_blocks_session_writers_but_not_users_or_queries() {
    use tt_registry::Scope;
    let setup = ready();
    let pen = ok(&setup, "tool.pen.list", json!({}))["pens"][0]["penId"].clone();
    let colour = json!(
        setup
            .snapshot()
            .unwrap()
            .unwrap()
            .palettes()
            .find_map(|p| p.swatches().first().map(|s| s.colour_id().as_str().to_owned()))
            .unwrap()
    );
    for (cmd, p, get, gp) in [
        ("tool.pen.select", json!({"penId":pen}), "tool.pen.list", json!({})),
        ("colour.current.set", json!({"colourId":colour}), "tool.settings.get", json!({})),
        ("tool.active.set", json!({"toolId":"brush"}), "tool.settings.get", json!({})),
        (
            "view.zoom.set",
            json!({"viewId":"drawingView","zoom":2}),
            "view.transform.get",
            json!({"viewId":"drawingView"}),
        ),
        ("drawAid.grid.set", json!({"visible":true}), "drawAid.state.get", json!({})),
    ] {
        let h = ready();
        h.set_wait_for_user(Duration::from_millis(40));
        h.report_shell(serde_json::from_value(json!({"modal":{"kind":"gesture","title":"drawing"}})).unwrap());
        let before = ok(&h, get, gp.clone());
        let start = Instant::now();
        let e = h.invoke(Invocation {
            command: cmd.into(),
            params: p.clone(),
            options: Default::default(),
            origin: agent(&[Scope::Read, Scope::Ui, Scope::Edit]),
        });
        assert_eq!(e.error.unwrap().code, "ui.busy");
        assert!(start.elapsed() >= Duration::from_millis(30));
        h.set_wait_for_user(Duration::from_secs(2));
        let query_start = Instant::now();
        let query = h.invoke(Invocation {
            command: get.into(),
            params: gp,
            options: Default::default(),
            origin: agent(&[Scope::Read, Scope::Ui]),
        });
        assert!(query.is_ok(), "{query:?}");
        assert_eq!(query.result.unwrap(), before);
        let tables = h.invoke(Invocation {
            command: "drawAid.staticLightTable.list".into(),
            params: json!({}),
            options: Default::default(),
            origin: agent(&[Scope::Read]),
        });
        assert!(tables.is_ok(), "{tables:?}");
        assert!(query_start.elapsed() < Duration::from_secs(1));
        let user_start = Instant::now();
        assert!(
            h.invoke(Invocation { command: cmd.into(), params: p, options: Default::default(), origin: Origin::User })
                .is_ok()
        );
        assert!(user_start.elapsed() < Duration::from_secs(1));
    }
}

#[test]
fn clearing_gesture_releases_wait_and_pointer_is_busy() {
    use tt_registry::Scope;
    let h = ready();
    h.set_wait_for_user(Duration::from_millis(80));
    h.report_shell(serde_json::from_value(json!({"modal":{"kind":"gesture","title":"drawing"}})).unwrap());
    let e = h.pointer(
        agent(&[Scope::Read, Scope::Ui, Scope::Edit, Scope::Input]),
        json!({"view":"drawing","events":[{"type":"hover","x":0,"y":0}]}),
    );
    assert_eq!(e.error.unwrap().code, "ui.busy");
    h.set_wait_for_user(Duration::from_secs(2));
    let start = Instant::now();
    let (e, _) = h.invoke_served_until(
        Invocation {
            command: "tool.active.set".into(),
            params: json!({"toolId":"brush"}),
            options: Default::default(),
            origin: agent(&[Scope::Read, Scope::Ui]),
        },
        &|| {
            if start.elapsed() >= Duration::from_millis(25) {
                h.report_shell(serde_json::from_value(json!({"modal":null})).unwrap());
            }
            false
        },
    );
    assert!(e.is_ok(), "{e:?}");
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn concurrent_captures_finish_and_budget_is_bounded() {
    let h = ready();
    let mut workers = Vec::new();
    for _ in 0..6 {
        let h = h.clone();
        workers.push(std::thread::spawn(move || ok(&h, "inspect.screenshot", json!({}))));
    }
    let mut peak = 0;
    while workers.iter().any(|w| !w.is_finished()) {
        peak = peak.max(h.jobs().list(None, None).iter().filter(|j| !j.state.is_final()).count());
        std::thread::yield_now();
    }
    let captures: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    for c in &captures {
        assert_eq!(c["png"], captures[0]["png"]);
    }
    println!("six concurrent captures: peak unfinished jobs={peak}; all completed with identical PNG");
    let h = fixture_with(tt_executor::ExecutorConfig { budget: Duration::from_millis(40) });
    let start = Instant::now();
    let e = h.invoke(Invocation::user("inspect.screenshot", json!({"region":{"x":0,"y":0,"w":2048,"h":2048}})));
    assert!(start.elapsed() < Duration::from_secs(1));
    println!("40ms capture budget: {:?}, elapsed {:?}", e.error, start.elapsed());
    for j in h.jobs().list(None, None) {
        let (_, timed) = h.jobs().wait(&j.job_id, Duration::from_secs(30)).unwrap();
        assert!(!timed);
    }
}

fn thread_count() -> Option<usize> {
    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).Threads.Count", std::process::id())])
        .output()
        .ok()?;
    #[cfg(target_os = "windows")]
    return String::from_utf8(output.stdout).ok()?.trim().parse().ok();
    #[cfg(target_os = "macos")]
    {
        let output =
            std::process::Command::new("ps").args(["-M", "-p", &std::process::id().to_string()]).output().ok()?;
        Some(String::from_utf8(output.stdout).ok()?.lines().count().saturating_sub(1))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    None
}
