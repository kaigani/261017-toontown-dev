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

mod part_b_equivalence {
    use super::*;
    use tt_executor::ExecutorHandle;
    fn events(ps: &[(f64, f64)], mods: &[&str]) -> Value {
        json!(ps.iter().enumerate().map(|(i,(x,y))|json!({"type":if i==0 {"down"} else if i+1==ps.len(){"up"}else{"move"},"x":x,"y":y,"modifiers":mods})).collect::<Vec<_>>())
    }
    fn canonical(h: &ExecutorHandle) -> Vec<String> {
        let d = h.snapshot().unwrap().unwrap();
        d.entry_keys()
            .iter()
            .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
            .collect()
    }
    fn hist(h: &ExecutorHandle) -> Vec<Value> {
        ok(h, "edit.history.get", json!({}))["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| json!({"label":e["label"],"rev":e["rev"]}))
            .collect()
    }
    fn colour(h: &ExecutorHandle) -> Value {
        json!(
            h.snapshot()
                .unwrap()
                .unwrap()
                .palettes()
                .find_map(|p| p.swatches().first().map(|s| s.colour_id().as_str().to_owned()))
                .unwrap()
        )
    }
    fn style(h: &ExecutorHandle, mut p: Value) -> Value {
        let pens = ok(h, "tool.pen.list", json!({}));
        let pen = &pens["pens"].as_array().unwrap().iter().find(|p| p["type"] == "brush").unwrap();
        p["penId"] = pen["penId"].clone();
        p["thickness"] = pen["maxSize"].clone();
        p["colourId"] = colour(h);
        p["layer"] = json!("auto");
        p["symmetry"] = json!(true);
        p
    }
    fn check(a: &ExecutorHandle, b: &ExecutorHandle, tool: &str, ev: Value, id: &str, p: Value) {
        let ha = hist(a);
        let hb = hist(b);
        assert_eq!(ha, hb);
        let e = a.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":tool,"events":ev}));
        let direct = run(b, id, p.clone());
        assert_eq!(e.status, direct.status, "{tool}: pointer {e:?}, direct {direct:?}");
        if e.is_ok() {
            let c = e.result.as_ref().unwrap()["commands"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["commandId"] == id)
                .unwrap();
            assert_eq!(c["commandId"], id);
            assert_eq!(c["params"], p, "{tool}");
        } else {
            assert_eq!(e.error.as_ref().unwrap().code, direct.error.as_ref().unwrap().code);
        }
        assert_eq!(canonical(a), canonical(b), "bytes and ids {tool}");
        assert_eq!(hist(a), hist(b), "history {tool}");
        assert_eq!(ok(a, "draw.selection.get", json!({})), ok(b, "draw.selection.get", json!({})), "selection {tool}");
    }

    fn view(h: &ExecutorHandle, z: f64) {
        ok(h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z}));
        ok(h, "view.rotation.set", json!({"viewId":"drawingView","degrees":37.5}));
        ok(h, "view.pan.set", json!({"viewId":"drawingView","centre":{"x":10000,"y":-10000}}));
    }
    fn mapped(h: &ExecutorHandle, x: f64, y: f64) -> Value {
        ok(
            h,
            "view.coordinates.convert",
            json!({"viewId":"drawingView","from":"view","to":"drawing","point":{"x":x,"y":y}}),
        )["point"]
            .clone()
    }
    // Derivation: 02 section 13.3 rows, explicitly resolved project pen and swatch;
    // point mapping is obtained on a third executor before either tested path runs.
    #[test]
    fn b1_transformed_creation_and_navigation() {
        for z in [4.0, 0.25] {
            for tool in ["brush", "pencil", "rectangle", "ellipse", "line", "grabber", "zoom"] {
                let (a, b, m) = (ready(), ready(), ready());
                for h in [&a, &b, &m] {
                    view(h, z);
                }
                let from = mapped(&m, 1000.0, 500.0);
                let to = mapped(&m, 1080.0, 460.0);
                let (id, p, ev) = match tool {
                    "brush" | "pencil" => {
                        let mut points = vec![from.clone(), to.clone()];
                        points[0]["pressure"] = json!(0.25);
                        points[1]["pressure"] = json!(0.85);
                        let mut p = style(&b, json!({"kind":tool,"points":points,"fit":true}));
                        if tool == "brush" {
                            p.as_object_mut().unwrap().remove("thickness");
                        }
                        let mut ev = events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]);
                        ev[0]["pressure"] = json!(0.25);
                        ev[1]["pressure"] = json!(0.85);
                        ("draw.stroke.add".to_string(), p, ev)
                    }
                    "grabber" => (
                        "view.pan.set".into(),
                        json!({"viewId":"drawingView","by":{"x":80.0,"y":-40.0}}),
                        events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]),
                    ),
                    "zoom" => (
                        "view.zoom.set".into(),
                        json!({"viewId":"drawingView","steps":1,"anchor":{"x":1000.0,"y":500.0}}),
                        events(&[(1000.0, 500.0), (1000.0, 500.0)], &[]),
                    ),
                    _ => {
                        let mut p = json!({"from":from,"to":to});
                        if tool == "line" {
                            p["constrainAngle"] = json!(false);
                            p["continuePrevious"] = json!(false);
                        } else {
                            p[if tool == "rectangle" { "square" } else { "circle" }] = json!(false);
                            p["fromCentre"] = json!(false);
                        }
                        (format!("draw.{tool}.add"), style(&b, p), events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]))
                    }
                };
                check(&a, &b, tool, ev, &id, p);
                assert_eq!(
                    ok(&a, "view.transform.get", json!({"viewId":"drawingView"})),
                    ok(&b, "view.transform.get", json!({"viewId":"drawingView"}))
                );
            }
        }
    }
    #[test]
    fn b1_transformed_lasso_selection_and_eraser() {
        for z in [4.0, 0.25] {
            for tool in ["eraser", "cutter", "select", "polyline"] {
                let (a, b, m) = (ready(), ready(), ready());
                for h in [&a, &b, &m] {
                    view(h, z);
                }
                let ps = [(1000.0, 500.0), (1080.0, 500.0), (1080.0, 460.0), (1000.0, 500.0)];
                let points: Vec<Value> = ps.iter().map(|(x, y)| mapped(&m, *x, *y)).collect();
                let (id, p, ev) = match tool {
                    "eraser" => {
                        let mut path = points.clone();
                        for p in &mut path {
                            p["pressure"] = json!(1.0);
                        }
                        let pens = ok(&b, "tool.pen.list", json!({"type":"eraser"}));
                        ("draw.erase.apply", json!({"path":path,"penId":pens["pens"][0]["penId"]}), events(&ps, &[]))
                    }
                    "cutter" => ("draw.cut.apply", json!({"polygon":points}), events(&ps, &[])),
                    "select" => {
                        let f = &points[0];
                        let t = &points[2];
                        let x = f["x"].as_f64().unwrap();
                        let y = f["y"].as_f64().unwrap();
                        let tx = t["x"].as_f64().unwrap();
                        let ty = t["y"].as_f64().unwrap();
                        (
                            "draw.selection.set",
                            json!({"by":"rect","rect":{"x":x.min(tx),"y":y.min(ty),"width":(tx-x).abs(),"height":(ty-y).abs()},"mode":"replace","rectRule":"inside"}),
                            events(&[ps[0], ps[2]], &[]),
                        )
                    }
                    _ => {
                        let mut ev = Vec::new();
                        for (x, y) in ps {
                            ev.push(json!({"type":"down","x":x,"y":y}));
                            ev.push(json!({"type":"up","x":x,"y":y}));
                        }
                        (
                            "draw.polyline.add",
                            style(
                                &b,
                                json!({"points":&points[..3],"closed":true,"snap":true,"snapRadius":8.0,"view":"drawingView"}),
                            ),
                            json!(ev),
                        )
                    }
                };
                check(&a, &b, tool, ev, id, p);
            }
        }
    }
    #[test]
    fn b2_individual_drawing_settings() {
        // Derived before gesture execution: section 6.1 pen carries size/tip;
        // section 6.2 copies share a single command; SI-131 layer precedence.
        for tool in ["brush", "pencil", "rectangle"] {
            for variant in [
                "minimum",
                "maximum",
                "tip",
                "angle",
                "smoothness",
                "colour",
                "behind",
                "top",
                "both",
                "mirror",
                "kaleidoscope",
            ] {
                let (a, b) = (ready(), ready());
                let pens = ok(&b, "tool.pen.list", json!({}));
                let pen =
                    pens["pens"].as_array().unwrap().iter().find(|p| p["type"] == "brush").unwrap()["penId"].clone();
                for h in [&a, &b] {
                    match variant {
                        "minimum" => {
                            ok(h, "tool.pen.set", json!({"penId":pen,"minSize":1.5}));
                        }
                        "maximum" => {
                            ok(h, "tool.pen.set", json!({"penId":pen,"maxSize":9}));
                        }
                        "tip" => {
                            ok(h, "tool.pen.set", json!({"penId":pen,"tipShape":"square"}));
                        }
                        "angle" => {
                            ok(h, "tool.pen.set", json!({"penId":pen,"tipAngle":45}));
                        }
                        "smoothness" => {
                            ok(h, "tool.pen.set", json!({"penId":pen,"smoothness":7}));
                        }
                        "colour" => {
                            ok(h, "colour.current.set", json!({"colourId":colour(h)}));
                        }
                        "behind" => {
                            ok(h, "tool.placeBehind.set", json!({"enabled":true}));
                        }
                        "top" => {
                            ok(h, "tool.drawTopLayer.set", json!({"enabled":true}));
                        }
                        "both" => {
                            ok(h, "tool.drawTopLayer.set", json!({"enabled":true}));
                            ok(h, "tool.placeBehind.set", json!({"enabled":true}));
                        }
                        "mirror" => {
                            ok(h, "tool.symmetry.set", json!({"mode":"mirror","angle":25}));
                        }
                        _ => {
                            ok(h, "tool.symmetry.set", json!({"mode":"kaleidoscope","mirrors":3,"angle":15}));
                        }
                    }
                }
                let mut p = if tool == "rectangle" {
                    style(
                        &b,
                        json!({"from":{"x":40.0,"y":40.0},"to":{"x":120.0,"y":80.0},"square":false,"fromCentre":false}),
                    )
                } else {
                    style(
                        &b,
                        json!({"kind":tool,"points":[{"x":40.0,"y":40.0,"pressure":1.0},{"x":120.0,"y":80.0,"pressure":1.0}],"fit":true}),
                    )
                };
                if tool == "brush" {
                    p.as_object_mut().unwrap().remove("thickness");
                }
                p["layer"] = json!(match variant {
                    "behind" => "behind",
                    "top" | "both" => "top",
                    _ => "auto",
                });
                check(
                    &a,
                    &b,
                    tool,
                    events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]),
                    if tool == "rectangle" { "draw.rectangle.add" } else { "draw.stroke.add" },
                    p,
                );
            }
        }
    }
    #[test]
    fn b3_zero_size_and_outside_view() {
        for tool in ["rectangle", "ellipse", "line", "brush"] {
            let (a, b) = (ready(), ready());
            let ps = if tool == "brush" {
                vec![(-100.0, -100.0), (-80.0, -60.0)]
            } else {
                vec![(1000.0, 500.0), (1000.0, 500.0)]
            };
            let m = ready();
            let from = mapped(&m, ps[0].0, ps[0].1);
            let to = mapped(&m, ps[1].0, ps[1].1);
            let mut p = if tool == "brush" {
                let mut f = from;
                let mut t = to;
                f["pressure"] = json!(1.0);
                t["pressure"] = json!(1.0);
                let mut p = style(&b, json!({"kind":"brush","points":[f,t],"fit":true}));
                p.as_object_mut().unwrap().remove("thickness");
                p
            } else {
                style(&b, json!({"from":from,"to":to}))
            };
            if tool == "line" {
                p["constrainAngle"] = json!(false);
                p["continuePrevious"] = json!(false);
            } else if tool != "brush" {
                p[if tool == "rectangle" { "square" } else { "circle" }] = json!(false);
                p["fromCentre"] = json!(false);
            }
            check(
                &a,
                &b,
                tool,
                events(&ps, &[]),
                if tool == "brush" {
                    "draw.stroke.add"
                } else if tool == "line" {
                    "draw.line.add"
                } else if tool == "rectangle" {
                    "draw.rectangle.add"
                } else {
                    "draw.ellipse.add"
                },
                p,
            );
        }
    }
    #[test]
    fn b3_successful_paint_and_unpaint() {
        for remove in [false, true] {
            let (a, b, m) = (ready(), ready(), ready());
            for h in [&a, &b] {
                ok(h, "draw.rectangle.add", json!({"from":{"x":300,"y":300},"to":{"x":400,"y":360}}));
                if remove {
                    ok(h, "colour.paint.apply", json!({"points":[{"x":350,"y":330}],"colourId":colour(h)}));
                }
            }
            let mut p =
                json!({"points":[mapped(&m,1310.0,210.0)],"space":"drawing","view":"drawingView","gapClosing":"off"});
            if !remove {
                p["colourId"] = colour(&b);
            }
            check(
                &a,
                &b,
                if remove { "unpaint" } else { "paint" },
                events(&[(1310.0, 210.0), (1310.0, 210.0)], &[]),
                if remove { "colour.paint.remove" } else { "colour.paint.apply" },
                p,
            );
        }
    }
    #[test]
    fn b3_zoom_marquee_independent_fit() {
        for (z, end) in [(4.0, (1200.0, 380.0)), (0.25, (1200.0, 380.0)), (4.0, (1002.0, 500.0))] {
            let (a, b, m) = (ready(), ready(), ready());
            for h in [&a, &b, &m] {
                view(h, z);
            }
            let click = end.0 - 1000.0 <= 3.0;
            if click {
                check(
                    &a,
                    &b,
                    "zoom",
                    events(&[(1000.0, 500.0), end], &[]),
                    "view.zoom.set",
                    json!({"viewId":"drawingView","steps":1,"anchor":{"x":1000.0,"y":500.0}}),
                );
            } else {
                let f = mapped(&m, 1000.0, 500.0);
                let t = mapped(&m, end.0, end.1);
                let centre = json!({"x":(f["x"].as_f64().unwrap()+t["x"].as_f64().unwrap())/2.0,"y":(f["y"].as_f64().unwrap()+t["y"].as_f64().unwrap())/2.0});
                let zoom = (z * (1920.0 / (end.0 - 1000.0)).min(1080.0 / (500.0 - end.1))).clamp(0.0625, 64.0);
                let params =
                    [json!({"viewId":"drawingView","zoom":zoom}), json!({"viewId":"drawingView","centre":centre})];
                let e = a.pointer(
                    tt_executor::Origin::User,
                    json!({"view":"drawing","tool":"zoom","events":events(&[(1000.0,500.0),end],&[])}),
                );
                assert!(e.is_ok(), "{e:?}");
                let cs = e.result.unwrap()["commands"].as_array().unwrap().clone();
                for (i, id) in ["view.zoom.set", "view.pan.set"].iter().enumerate() {
                    assert_eq!(cs[i + 1]["commandId"], *id);
                    assert_eq!(cs[i + 1]["params"], params[i]);
                    ok(&b, id, params[i].clone());
                }
                assert_eq!(canonical(&a), canonical(&b));
                assert_eq!(hist(&a), hist(&b));
            }
            assert_eq!(
                ok(&a, "view.transform.get", json!({"viewId":"drawingView"})),
                ok(&b, "view.transform.get", json!({"viewId":"drawingView"}))
            );
        }
    }
    #[test]
    fn b3_polyline_handles_derived_trigonometry() {
        for constrained in [false, true] {
            let (a, b, m) = (ready(), ready(), ready());
            let mut mid = mapped(&m, 1000.0, 500.0);
            let (x, y) = if constrained {
                let r = 40f64.hypot(13.0);
                (r * 15f64.to_radians().cos(), r * 15f64.to_radians().sin())
            } else {
                (40.0, 13.0)
            };
            mid["outHandle"] = json!({"x":x,"y":y});
            mid["inHandle"] = json!({"x":-x,"y":-y});
            let p = style(
                &b,
                json!({"points":[mapped(&m,900.0,500.0),mid,mapped(&m,1100.0,600.0)],"closed":false,"snap":true,"snapRadius":8.0}),
            );
            let mut ev = events(&[(900.0, 500.0), (900.0, 500.0)], &[]).as_array().unwrap().clone();
            ev.extend(
                events(&[(1000.0, 500.0), (1040.0, 487.0)], if constrained { &["shift"] } else { &[] })
                    .as_array()
                    .unwrap()
                    .clone(),
            );
            ev.extend(events(&[(1100.0, 600.0), (1100.0, 600.0)], &[]).as_array().unwrap().clone());
            ev.push(json!({"type":"doubleClick","x":1100,"y":600}));
            check(&a, &b, "polyline", json!(ev), "draw.polyline.add", p);
        }
    }
    #[test]
    fn b3_gradient_mapping() {
        let (a, b, m) = (ready(), ready(), ready());
        let d = b.snapshot().unwrap().unwrap();
        // Fixture Sky is used because a swatch creation command is not required here.
        let sky = d
            .palettes()
            .flat_map(|p| p.swatches())
            .find(|s| s.name() == "Sky")
            .unwrap()
            .colour_id()
            .as_str()
            .to_owned();
        for h in [&a, &b] {
            ok(h, "draw.rectangle.add", json!({"from":{"x":300,"y":300},"to":{"x":400,"y":360}}));
            ok(h, "colour.paint.apply", json!({"points":[{"x":350,"y":330}],"colourId":colour(h)}));
            ok(h, "colour.current.set", json!({"colourId":sky}));
        }
        let from = mapped(&m, 1310.0, 210.0);
        let to = mapped(&m, 1330.0, 190.0);
        check(
            &a,
            &b,
            "paint",
            events(&[(1310.0, 210.0), (1330.0, 190.0)], &[]),
            "colour.paint.apply",
            json!({"points":[from.clone()],"mapping":{"from":from,"to":to},"space":"drawing","view":"drawingView","colourId":sky,"gapClosing":"off"}),
        );
    }
    #[test]
    fn b3_dropper_then_brush_and_empty_dropper() {
        for painted in [true, false] {
            let (a, b) = (ready(), ready());
            let d = b.snapshot().unwrap().unwrap();
            let cs: Vec<_> =
                d.palettes().flat_map(|p| p.swatches()).map(|s| s.colour_id().as_str().to_owned()).collect();
            for h in [&a, &b] {
                ok(h, "draw.rectangle.add", json!({"from":{"x":300,"y":300},"to":{"x":400,"y":360}}));
                if painted {
                    ok(h, "colour.paint.apply", json!({"points":[{"x":350,"y":330}],"colourId":cs[0]}));
                }
                ok(h, "colour.current.set", json!({"colourId":cs[1]}));
            }
            let point = if painted { json!({"x":350.0,"y":330.0}) } else { json!({"x":550.0,"y":330.0}) };
            let x = if painted { 1310.0 } else { 1510.0 };
            let p = json!({"point":point,"space":"drawing","view":"drawingView"});
            let e = a.pointer(
                tt_executor::Origin::User,
                json!({"view":"drawing","tool":"dropper","events":events(&[(x,210.0),(x,210.0)],&[])}),
            );
            let direct = run(&b, "colour.pick", p.clone());
            assert_eq!(e.status, direct.status);
            if e.is_ok() {
                let cs = e.result.unwrap()["commands"].as_array().unwrap().clone();
                assert_eq!(cs[1]["commandId"], "colour.pick");
                assert_eq!(cs[1]["params"], p);
                if painted {
                    assert_eq!(cs[2]["commandId"], "colour.current.set");
                    assert_eq!(cs[2]["params"], json!({"colourId":colour(&b)}));
                    ok(&b, "colour.current.set", json!({"colourId":colour(&b)}));
                }
            }
            assert_eq!(canonical(&a), canonical(&b));
            assert_eq!(hist(&a), hist(&b));
            let expected = if painted { colour(&b) } else { json!(cs[1]) };
            let mut p = style(
                &b,
                json!({"kind":"brush","points":[{"x":40.0,"y":40.0,"pressure":1.0},{"x":120.0,"y":80.0,"pressure":1.0}],"fit":true}),
            );
            p["colourId"] = expected;
            p.as_object_mut().unwrap().remove("thickness");
            check(&a, &b, "brush", events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]), "draw.stroke.add", p);
        }
    }
}

mod part_c_equivalence {
    use super::*;
    use tt_executor::ExecutorHandle;
    fn events(ps: &[(f64, f64)], mods: &[&str]) -> Value {
        json!(ps.iter().enumerate().map(|(i,(x,y))|json!({"type":if i==0 {"down"} else if i+1==ps.len(){"up"}else{"move"},"x":x,"y":y,"modifiers":mods})).collect::<Vec<_>>())
    }
    fn canonical(h: &ExecutorHandle) -> Vec<String> {
        let d = h.snapshot().unwrap().unwrap();
        d.entry_keys()
            .iter()
            .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
            .collect()
    }
    fn hist(h: &ExecutorHandle) -> Vec<Value> {
        ok(h, "edit.history.get", json!({}))["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| json!({"label":e["label"],"rev":e["rev"]}))
            .collect()
    }
    fn colour(h: &ExecutorHandle) -> Value {
        json!(
            h.snapshot()
                .unwrap()
                .unwrap()
                .palettes()
                .find_map(|p| p.swatches().first().map(|s| s.colour_id().as_str().to_owned()))
                .unwrap()
        )
    }
    fn style(h: &ExecutorHandle, mut p: Value) -> Value {
        let pens = ok(h, "tool.pen.list", json!({}));
        let pen = &pens["pens"].as_array().unwrap().iter().find(|p| p["type"] == "brush").unwrap();
        p["penId"] = pen["penId"].clone();
        p["thickness"] = pen["maxSize"].clone();
        p["colourId"] = colour(h);
        p["layer"] = json!("auto");
        p["symmetry"] = json!(true);
        p
    }
    fn check(a: &ExecutorHandle, b: &ExecutorHandle, tool: &str, ev: Value, id: &str, p: Value) {
        let ha = hist(a);
        let hb = hist(b);
        assert_eq!(ha, hb);
        let e = a.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":tool,"events":ev}));
        let direct = run(b, id, p.clone());
        assert_eq!(e.status, direct.status, "{tool}: pointer {e:?}, direct {direct:?}");
        if e.is_ok() {
            let c = e.result.as_ref().unwrap()["commands"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["commandId"] == id)
                .unwrap();
            assert_eq!(c["commandId"], id);
            assert_eq!(c["params"], p, "{tool}");
        } else {
            assert_eq!(e.error.as_ref().unwrap().code, direct.error.as_ref().unwrap().code);
        }
        assert_eq!(canonical(a), canonical(b), "bytes and ids {tool}");
        assert_eq!(hist(a), hist(b), "history {tool}");
        assert_eq!(ok(a, "draw.selection.get", json!({})), ok(b, "draw.selection.get", json!({})), "selection {tool}");
    }

    fn view(h: &ExecutorHandle, z: f64) {
        ok(h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z}));
        ok(h, "view.rotation.set", json!({"viewId":"drawingView","degrees":37.5}));
        ok(h, "view.pan.set", json!({"viewId":"drawingView","centre":{"x":10000,"y":-10000}}));
    }
    fn mapped(h: &ExecutorHandle, x: f64, y: f64) -> Value {
        ok(
            h,
            "view.coordinates.convert",
            json!({"viewId":"drawingView","from":"view","to":"drawing","point":{"x":x,"y":y}}),
        )["point"]
            .clone()
    }

    fn rect(h: &ExecutorHandle, f: Value, t: Value) {
        ok(h, "draw.rectangle.add", json!({"from":f,"to":t}));
    }
    #[test]
    fn c1_move() {
        let (a, b, m) = (ready(), ready(), ready());
        for h in [&a, &b, &m] {
            view(h, 4.0);
        }
        let p = mapped(&m, 1000.0, 500.0);
        for h in [&a, &b] {
            rect(
                h,
                json!({"x":p["x"].as_f64().unwrap()-10.0,"y":p["y"].as_f64().unwrap()-10.0}),
                json!({"x":p["x"].as_f64().unwrap()+10.0,"y":p["y"].as_f64().unwrap()+10.0}),
            );
        }
        check(
            &a,
            &b,
            "select",
            events(&[(1000.0, 500.0), (1000.0, 500.0)], &[]),
            "draw.selection.set",
            json!({"by":"point","point":p,"mode":"replace"}),
        );
        let q = mapped(&m, 1080.0, 460.0);
        let n = hist(&a).len();
        check(
            &a,
            &b,
            "select",
            events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]),
            "draw.transform.apply",
            json!({"translate":{"x":q["x"].as_f64().unwrap()-p["x"].as_f64().unwrap(),"y":q["y"].as_f64().unwrap()-p["y"].as_f64().unwrap()},"copy":false}),
        );
        assert_eq!(hist(&a).len(), n + 1);
    }
    #[test]
    fn c1_paint() {
        let (a, b, m) = (ready(), ready(), ready());
        for h in [&a, &b, &m] {
            view(h, 4.0);
        }
        let p = mapped(&m, 1000.0, 500.0);
        for h in [&a, &b] {
            rect(
                h,
                json!({"x":p["x"].as_f64().unwrap()-10.0,"y":p["y"].as_f64().unwrap()-10.0}),
                json!({"x":p["x"].as_f64().unwrap()+10.0,"y":p["y"].as_f64().unwrap()+10.0}),
            );
        }
        check(
            &a,
            &b,
            "paint",
            events(&[(1000.0, 500.0), (1000.0, 500.0)], &[]),
            "colour.paint.apply",
            json!({"points":[p],"colourId":colour(&b),"space":"drawing","view":"drawingView","gapClosing":"off"}),
        );
    }
    #[test]
    fn c1_press_mapping() {
        let (a, b, m) = (ready(), ready(), ready());
        for h in [&a, &b, &m] {
            view(h, 4.0);
        }
        for z in [4.0, 0.25] {
            for h in [&a, &b, &m] {
                ok(h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z}));
            }
            let mut ps = vec![mapped(&m, 1000.0, 500.0), mapped(&m, 1080.0, 460.0)];
            for p in &mut ps {
                p["pressure"] = json!(1.0);
            }
            let mut p = style(&b, json!({"kind":"brush","points":ps,"fit":true}));
            p.as_object_mut().unwrap().remove("thickness");
            check(&a, &b, "brush", events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]), "draw.stroke.add", p);
        }
    }
    #[test]
    fn c1_click_view_tolerance() {
        for z in [4.0, 0.25] {
            let (a, b, m) = (ready(), ready(), ready());
            for h in [&a, &b, &m] {
                view(h, z);
            }
            let f = mapped(&m, 1000.0, 500.0);
            let t = mapped(&m, 1100.0, 500.0);
            for h in [&a, &b] {
                ok(h, "draw.line.add", json!({"from":f,"to":t,"thickness":0.1}));
            }
            // Fixed two view pixels outside the line; du distance is 2 / zoom.
            let p = mapped(&m, 1050.0, 502.0);
            check(
                &a,
                &b,
                "select",
                events(&[(1050.0, 502.0), (1050.0, 502.0)], &[]),
                "draw.selection.set",
                json!({"by":"point","point":p,"mode":"replace","view":"drawingView"}),
            );
        }
    }
    fn stroke(h: &ExecutorHandle) -> Value {
        let mut p = style(
            h,
            json!({"kind":"brush","points":[{"x":40.0,"y":40.0,"pressure":1.0},{"x":120.0,"y":80.0,"pressure":1.0}],"fit":true}),
        );
        p.as_object_mut().unwrap().remove("thickness");
        p
    }
    #[test]
    fn c2_current_pen() {
        let (a, b) = (ready(), ready());
        let pens = ok(&b, "tool.pen.list", json!({"type":"brush"}));
        let pen = pens["pens"][0]["penId"].clone();
        for h in [&a, &b] {
            ok(h, "tool.pen.set", json!({"penId":pen,"minSize":1.5,"maxSize":11,"tipShape":"square"}));
            ok(h, "tool.pen.select", json!({"penId":pen}));
        }
        let mut p = stroke(&b);
        p["penId"] = pen;
        check(&a, &b, "brush", events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]), "draw.stroke.add", p);
    }
    #[test]
    fn c2_pencil_shape_source() {
        let (a, b) = (ready(), ready());
        for h in [&a, &b] {
            ok(h, "tool.pencil.options.set", json!({"thicknessSource":"fixed","thickness":2.5}));
        }
        let mut p =
            style(&b, json!({"from":{"x":40.0,"y":40.0},"to":{"x":120.0,"y":80.0},"square":false,"fromCentre":false}));
        p["thickness"] = json!(2.5);
        check(&a, &b, "rectangle", events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]), "draw.rectangle.add", p);
    }
    #[test]
    fn c2_eraser_pen() {
        let (a, b) = (ready(), ready());
        let pens = ok(&b, "tool.pen.list", json!({"type":"eraser"}));
        let pen = pens["pens"][0]["penId"].clone();
        for h in [&a, &b] {
            ok(h, "tool.pen.set", json!({"penId":pen,"maxSize":19,"tipShape":"square"}));
            ok(h, "tool.pen.select", json!({"penId":pen}));
            rect(h, json!({"x":30,"y":30}), json!({"x":130,"y":90}));
        }
        check(
            &a,
            &b,
            "eraser",
            events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]),
            "draw.erase.apply",
            json!({"path":[{"x":40.0,"y":40.0,"pressure":1.0},{"x":120.0,"y":80.0,"pressure":1.0}],"penId":pen}),
        );
    }
    #[test]
    fn c2_gap_modes() {
        for mode in ["off", "small", "medium", "large"] {
            let (a, b) = (ready(), ready());
            for h in [&a, &b] {
                rect(h, json!({"x":300,"y":300}), json!({"x":400,"y":360}));
                ok(h, "colour.gapClosing.set", json!({"mode":mode}));
            }
            check(
                &a,
                &b,
                "paint",
                events(&[(1310.0, 210.0), (1310.0, 210.0)], &[]),
                "colour.paint.apply",
                json!({"points":[{"x":350.0,"y":330.0}],"colourId":colour(&b),"space":"drawing","view":"drawingView","gapClosing":mode}),
            );
        }
    }
    #[test]
    fn c2_selection_options() {
        // Â§7.1.1 supplies alternate-lasso and constrain-add; the option is rectRule.
        for lasso in [false, true] {
            for rule in ["inside", "intersect"] {
                let (a, b) = (ready(), ready());
                for h in [&a, &b] {
                    ok(h, "tool.select.options.set", json!({"rectRule":rule}));
                }
                let p = if lasso {
                    json!({"by":"lasso","polygon":[{"x":40.0,"y":40.0},{"x":120.0,"y":80.0}],"rectRule":rule,"mode":"add"})
                } else {
                    json!({"by":"rect","rect":{"x":40.0,"y":40.0,"width":80.0,"height":40.0},"rectRule":rule,"mode":"add"})
                };
                check(
                    &a,
                    &b,
                    "select",
                    events(&[(1000.0, 500.0), (1080.0, 460.0)], if lasso { &["shift", "alt"] } else { &["shift"] }),
                    "draw.selection.set",
                    p,
                );
            }
        }
    }
    #[test]
    fn c2_temporary_overrides() {
        for kind in ["eraser", "select", "dropper"] {
            let (a, b) = (ready(), ready());
            for h in [&a, &b] {
                rect(h, json!({"x":300,"y":300}), json!({"x":400,"y":360}));
                ok(h, "colour.paint.apply", json!({"points":[{"x":350,"y":330}],"colourId":colour(h)}));
            }
            let tool = if kind == "dropper" { "paint" } else { "brush" };
            let ev = events(
                &[(1310.0, 210.0), (1310.0, 210.0)],
                match kind {
                    "select" => &["ctrl"],
                    "dropper" => &["alt"],
                    _ => &[],
                },
            );
            let mut req = json!({"view":"drawing","tool":tool,"events":ev});
            if kind == "eraser" {
                req["pointerType"] = json!("penEraser");
            }
            let (id, p) = match kind {
                "eraser" => {
                    let pens = ok(&b, "tool.pen.list", json!({"type":"eraser"}));
                    (
                        "draw.erase.apply",
                        json!({"path":[{"x":350.0,"y":330.0,"pressure":1.0},{"x":350.0,"y":330.0,"pressure":1.0}],"penId":pens["pens"][0]["penId"]}),
                    )
                }
                "select" => {
                    ("draw.selection.set", json!({"by":"point","point":{"x":350.0,"y":330.0},"mode":"replace"}))
                }
                _ => ("colour.pick", json!({"point":{"x":350.0,"y":330.0},"space":"drawing","view":"drawingView"})),
            };
            let e = a.pointer(Origin::User, req);
            assert!(e.is_ok(), "{e:?}");
            let cs = e.result.unwrap()["commands"].as_array().unwrap().clone();
            assert_eq!(cs[1]["commandId"], id);
            assert_eq!(cs[1]["params"], p);
            let r = ok(&b, id, p);
            if kind == "dropper" {
                assert_eq!(cs.len(), 3);
                assert_eq!(cs[2]["params"], json!({"colourId":r["colourId"]}));
                ok(&b, "colour.current.set", json!({"colourId":r["colourId"]}));
            }
            assert_eq!(canonical(&a), canonical(&b));
            assert_eq!(hist(&a), hist(&b));
            assert_eq!(ok(&a, "tool.active.get", json!({}))["toolId"], tool);
        }
    }
}
