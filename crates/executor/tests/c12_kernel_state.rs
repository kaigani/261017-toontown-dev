//! C1.2 kernel state and liveness: queryable window/panel/workspace state with explicit full-screen `enabled`
//! (readiness bullet 4), the application context, jobs off the document thread, the never-block watchdog, and every
//! declared example run on the fixture project under the watchdog (11 §14.2, §14.6).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, Status};

#[test]
fn window_and_panel_state_is_queryable_and_settable() {
    let h = fixture();
    let panels = ok(&h, "window.panel.list", json!({}));
    let panels = panels.as_array().unwrap();
    assert_eq!(panels.len(), 21, "01 §5.2 panel catalogue");
    let find = |list: &Value, id: &str| list.as_array().unwrap().iter().find(|p| p["panelId"] == id).cloned().unwrap();
    let list = json!(panels);
    assert_eq!(find(&list, "cameraView")["area"], "central");
    assert_eq!(find(&list, "history")["visible"], false);
    let p = ok(&h, "window.panel.set", json!({ "panelId": "history", "visible": true }));
    assert_eq!((p["visible"].clone(), p["focused"].clone()), (json!(true), json!(true)));
    let p = ok(&h, "window.panel.set", json!({ "panelId": "colour", "floating": true }));
    assert_eq!((p["floating"].clone(), p["visible"].clone()), (json!(true), json!(true)));
    let p = ok(&h, "window.panel.set", json!({ "panelId": "colour", "floating": false }));
    assert_eq!(p["area"], "right", "docking again restores the remembered area");
    ok(&h, "window.panel.set", json!({ "panelId": "history", "visible": false }));
    let p = ok(&h, "window.panel.set", json!({ "panelId": "history", "visible": true }));
    assert_eq!(p["area"], "right");
    let e = err(&h, "window.panel.set", json!({ "panelId": "cameraView", "visible": false }));
    assert_eq!(e.code, "arg.invalid");
    assert_eq!(err(&h, "window.panel.set", json!({ "panelId": "nope" })).code, "window.panelNotFound");
    let l = ok(&h, "window.panel.move", json!({ "panelId": "drawingView", "area": "central" }));
    assert_eq!(l["centralPanel"], "drawingView");
    assert_eq!(ok(&h, "app.state.get", json!({}))["activeView"]["panelId"], "drawingView");
    // Full screen is explicit state with its own arrangement (01 §5.6).
    for enabled in [true, true, false] {
        assert_eq!(ok(&h, "window.fullScreen.set", json!({ "enabled": enabled }))["enabled"], enabled);
        assert_eq!(ok(&h, "app.state.get", json!({}))["fullScreen"], enabled);
    }
    let e = err(&h, "window.fullScreen.set", json!({}));
    assert_eq!(e.code, "arg.missing", "no toggle without a value");
    assert_eq!(ok(&h, "window.floatingPanels.set", json!({ "visible": false }))["visible"], false);
    assert_eq!(ok(&h, "window.sidePanelsExtended.set", json!({ "enabled": false }))["enabled"], false);
    // Toolbars.
    let bars = ok(&h, "window.toolbar.list", json!({}));
    assert_eq!(bars.as_array().unwrap().len(), 6);
    assert!(bars[0]["allowedItems"].as_array().unwrap().contains(&json!("edit.undo")));
    let e = err(&h, "window.toolbar.items.set", json!({ "items": ["not.a.command"] }));
    assert_eq!(e.details["pointer"], "/items/0");
    ok(&h, "window.toolbar.items.set", json!({ "items": ["edit.undo", "separator", "playStop"] }));
    let reset = ok(&h, "window.toolbar.items.reset", json!({}));
    assert_eq!(reset["items"][0], "drawAid.grid.set");
    let bar = ok(&h, "window.toolbar.set", json!({ "toolbarId": "boneTools", "visible": false }));
    assert_eq!(bar["visible"], false);
}

#[test]
fn workspaces_are_kernel_state() {
    let h = fixture();
    let list = ok(&h, "workspace.list", json!({}));
    let names: Vec<&str> = list.as_array().unwrap().iter().map(|w| w["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Default", "Staging", "Drawing", "Focused drawing"]);
    ok(&h, "workspace.select", json!({ "name": "Drawing" }));
    let st = ok(&h, "app.state.get", json!({}));
    assert_eq!(st["workspace"], json!({ "name": "Drawing", "modified": false }));
    assert_eq!(st["activeView"]["panelId"], "drawingView");
    ok(&h, "window.panel.set", json!({ "panelId": "history", "visible": true }));
    assert_eq!(ok(&h, "app.state.get", json!({}))["workspace"]["modified"], true);
    let created = ok(&h, "workspace.create", json!({ "name": "Inking", "position": 1 }));
    assert_eq!(created, json!({ "name": "Inking", "position": 1 }));
    assert_eq!(err(&h, "workspace.create", json!({ "name": "Inking" })).code, "workspace.exists");
    assert_eq!(err(&h, "workspace.create", json!({ "name": "  " })).code, "workspace.nameInvalid");
    assert_eq!(err(&h, "workspace.delete", json!({ "name": "Default" })).code, "workspace.builtIn");
    ok(&h, "workspace.rename", json!({ "name": "Inking", "newName": "Ink" }));
    let stored = ok(&h, "workspace.get", json!({ "name": "Ink" }));
    assert!(
        stored["docks"]["right"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["tabs"].as_array().unwrap().contains(&json!("history")))
    );
    ok(&h, "workspace.delete", json!({ "name": "Ink" }));
    assert_eq!(err(&h, "workspace.get", json!({ "name": "Ink" })).code, "workspace.notFound");
    ok(&h, "workspace.reset", json!({ "name": "Drawing", "select": true }));
    assert_eq!(ok(&h, "app.state.get", json!({}))["workspace"]["modified"], false);
    let layout = ok(&h, "workspace.get", json!({}));
    let applied = ok(&h, "workspace.layout.set", json!({ "layout": layout }));
    assert_eq!(applied, layout);
    let mut bad = layout.clone();
    bad["centralPanel"] = json!("timeline");
    assert_eq!(err(&h, "workspace.layout.set", json!({ "layout": bad })).details["pointer"], "/layout/centralPanel");
    let mut extra = layout;
    extra["surprise"] = json!(1);
    assert_eq!(err(&h, "workspace.layout.set", json!({ "layout": extra })).code, "arg.unknown");
}

#[test]
fn the_application_context_follows_commands() {
    let h = fixture();
    let st = ok(&h, "app.state.get", json!({}));
    assert_eq!(st["frame"], 1);
    let len = st["scene"]["frameCount"].as_u64().unwrap();
    assert_eq!(ok(&h, "app.frame.move", json!({ "direction": "next", "count": 3 }))["frame"], 4.min(len));
    assert_eq!(ok(&h, "app.frame.move", json!({ "direction": "previous", "count": 99 }))["frame"], 1);
    let e = err(&h, "app.frame.set", json!({ "frame": len + 1 }));
    assert_eq!(e.details["allowed"]["maximum"], len);
    let next = ok(&h, "app.currentElement.move", json!({ "direction": "next", "order": "timeline" }));
    assert_ne!(next["elementId"], st["element"]["id"]);
    ok(&h, "app.currentElement.set", json!({ "elementId": st["element"]["id"] }));
    assert_eq!(ok(&h, "app.state.get", json!({}))["element"]["id"], st["element"]["id"]);
    // Focus-aware editing: `auto` follows the focused panel.
    ok(&h, "window.panel.set", json!({ "panelId": "timeline", "visible": true, "focus": true }));
    assert_eq!(ok(&h, "edit.selectAll", json!({}))["context"], "timeline");
    ok(&h, "window.panel.set", json!({ "panelId": "library", "visible": true, "focus": true }));
    assert_eq!(ok(&h, "edit.selection.get", json!({}))["context"], "library");
    let e = err(&h, "edit.selection.set", json!({ "context": "drawing", "ids": [st["element"]["id"]] }));
    assert_eq!(e.code, "ref.wrongType");
    // Dirty-project quit (01 §11.2 rule 3): an unsaved project refuses to quit unless told.
    let e = err(&h, "app.quit", json!({}));
    assert_eq!(e.code, "project.unsavedChanges");
    assert_eq!(e.details["projectDirty"], true);
    assert_eq!(ok(&h, "app.quit", json!({ "onUnsaved": "discard" }))["quitting"], true);
    assert_eq!(err(&h, "app.quit", json!({ "onUnsaved": "save" })).code, "command.notImplemented");
}

#[test]
fn jobs_run_off_the_document_thread() {
    let h = fixture();
    let started = h
        .jobs()
        .submit("test.job", "user", |p| {
            p.report(0.5, "half");
            std::thread::sleep(Duration::from_millis(300));
            Ok(json!({ "done": true }))
        })
        .unwrap();
    let waiter = {
        let h = h.clone();
        let id = started.clone();
        std::thread::spawn(move || run(&h, "job.wait", json!({ "jobId": id, "timeoutMs": 10_000 })))
    };
    // While a caller waits, the document thread answers others at once.
    std::thread::sleep(Duration::from_millis(30));
    let t = Instant::now();
    assert!(run(&h, "app.state.get", json!({})).is_ok());
    assert!(t.elapsed() < Duration::from_millis(250), "the document thread is not blocked by job.wait");
    let env = waiter.join().unwrap();
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.result.as_ref().unwrap()["state"], "succeeded");
    assert_eq!(env.result.as_ref().unwrap()["result"]["done"], true);
    let slow = h.jobs().submit("test.slow", "user", |p| {
        while !p.cancelled() {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(json!({}))
    });
    let slow = slow.unwrap();
    let env = run(&h, "job.wait", json!({ "jobId": slow, "timeoutMs": 20 }));
    assert_eq!(env.result.as_ref().unwrap()["timedOut"], true);
    ok(&h, "job.cancel", json!({ "jobId": slow }));
    let env = run(&h, "job.wait", json!({ "jobId": slow, "timeoutMs": 5000 }));
    assert_eq!(env.result.as_ref().unwrap()["state"], "cancelled");
    let all = ok(&h, "job.list", json!({}));
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert_eq!(err(&h, "job.wait", json!({ "jobId": "job-99" })).code, "job.notFound");
}

#[test]
fn the_watchdog_answers_within_its_budget() {
    // A tiny budget and a slow query: the caller gets `command.timeout` in time; the document thread finishes the
    // query and answers the next call.
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(1) });
    let mut timed_out = false;
    for _ in 0..50 {
        let t = Instant::now();
        let env = h.invoke(Invocation::user(
            "doc.object.find",
            json!({ "where": { "meta": { "op": "exists", "value": false } }, "limit": 1000 }),
        ));
        assert!(t.elapsed() < Duration::from_millis(500));
        if env.error.as_ref().is_some_and(|e| e.code == "command.timeout") {
            timed_out = true;
            assert_eq!(env.error.unwrap().details["budgetMs"], 1);
            break;
        }
    }
    assert!(timed_out, "a 1 ms budget times out a full-document find");
}

/// The state every example expects: one checkpoint, a user workspace, a job, undo and redo steps.
fn prepared() -> ExecutorHandle {
    let h = fixture();
    let scene = first(&h, "scene");
    ok(&h, "checkpoint.create", json!({ "name": "before-walk" }));
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "p": 1 } } }));
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "p": 2 } } }));
    ok(&h, "edit.undo", json!({}));
    ok(&h, "workspace.create", json!({ "name": "Inking" }));
    h.jobs().submit("test.job", "user", |_| Ok(json!({}))).unwrap();
    h
}

#[test]
fn every_example_runs_on_the_fixture_under_the_watchdog() {
    let registry = tt_executor::registry().unwrap();
    let mut ran = 0;
    for e in registry.entries() {
        let Some(c) = e.command else { continue };
        for ex in c.decl.examples {
            let h = prepared();
            let params: Value = serde_json::from_str(ex.params).unwrap();
            let t = Instant::now();
            let env = h.invoke(Invocation::user(e.id(), params));
            assert!(t.elapsed() < Duration::from_secs(30), "{}: `{}` took {:?}", e.id(), ex.title, t.elapsed());
            let code = env.error.as_ref().map(|x| x.code.clone());
            match (e.spec.kind, e.status()) {
                (tt_registry::Kind::Event, _) => assert_eq!(code.as_deref(), Some("command.disabled"), "{}", e.id()),
                (_, tt_registry::Status::NotImplemented { owner }) => {
                    assert_eq!(code.as_deref(), Some("command.notImplemented"), "{}: {:?}", e.id(), env.error);
                    assert_eq!(env.error.unwrap().details["milestone"], owner);
                }
                _ => assert_eq!(env.status, Status::Ok, "{} example `{}` failed: {:?}", e.id(), ex.title, env.error),
            }
            ran += 1;
        }
    }
    println!("examples run: {ran}");
    assert!(ran >= 140, "examples: {ran}");
}

#[test]
fn every_inventory_stub_answers_with_its_owner() {
    let registry = tt_executor::registry().unwrap();
    let h = fixture();
    let mut n = 0;
    for e in registry.entries().filter(|e| e.command.is_none()) {
        let env = h.invoke(Invocation::user(e.id(), json!({})));
        let err = env.error.unwrap();
        if e.spec.kind == tt_registry::Kind::Event {
            assert_eq!(err.code, "command.disabled", "{}", e.id());
        } else {
            assert_eq!(err.code, "command.notImplemented", "{}", e.id());
            assert_eq!(err.details["milestone"], json!(tt_registry::owner_of(e.id())));
        }
        n += 1;
    }
    println!("stubs: {n}");
    assert!(n > 600);
}
