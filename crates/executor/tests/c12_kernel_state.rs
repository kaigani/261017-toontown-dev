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
    // `save` needs a location (01 §12.1 rule 3: `project.pathRequired`); a discarding quit closes the project.
    assert_eq!(err(&h, "app.quit", json!({ "onUnsaved": "save" })).code, "project.pathRequired");
    assert_eq!(ok(&h, "app.quit", json!({ "onUnsaved": "discard" }))["quitting"], true);
    assert_eq!(err(&h, "doc.state.get", json!({})).code, "project.none");
}

#[test]
fn jobs_run_off_the_document_thread() {
    let h = fixture();
    let events = h.subscribe();
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
    let names: Vec<String> = events.try_iter().map(|e| e.name).collect();
    assert!(names.iter().any(|n| n == "job.progress"), "{names:?}");
    assert_eq!(names.iter().filter(|n| *n == "job.finished").count(), 2, "{names:?}");
}

#[test]
fn the_watchdog_answers_within_its_budget() {
    // A tiny budget and a slow query: the caller gets `command.timeout` in time; the document thread finishes the
    // query and answers the next call.
    // Opening the fixture is itself bound by the 1 ms budget; on a loaded machine it may time out (the document
    // thread still completes it), so it is retried until it answers in time or the earlier open is seen to be done.
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_millis(1) }).unwrap();
    let mut opened = false;
    for _ in 0..200 {
        let env = h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false);
        match env.error {
            None => {
                opened = true;
                break;
            }
            // A previous attempt that timed out has completed meanwhile: the untitled fixture is open (and modified).
            Some(e) if e.code == "project.unsavedChanges" => {
                opened = true;
                break;
            }
            Some(e) => assert_eq!(e.code, "command.timeout", "{e:?}"),
        }
    }
    // Setup only: the fixture is open (possibly by an attempt that finished after its caller timed out). What the test
    // proves is below: every caller invocation returns within 500 ms, and a 1 ms budget yields `command.timeout`.
    assert!(opened, "setup: the fixture project is installed (an open may complete after its caller timed out)");
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
    edits(&h);
    h
}

fn edits(h: &ExecutorHandle) {
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
fn with_files(id: &str) -> bool {
    ["project.", "doc.asset.", "doc.blob.", "doc.package.", "agent."].iter().any(|p| id.starts_with(p))
}

fn wait_job(h: &ExecutorHandle, started: &Value) {
    let r = ok(h, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60_000 }));
    let rec = if r.get("record").is_some() { r["record"].clone() } else { r };
    assert_eq!(rec["state"], "succeeded", "{rec}");
}

fn prepared_with_files(id: &str, dir: &std::path::Path) -> ExecutorHandle {
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
fn map_tmp(v: &mut Value, dir: &std::path::Path) {
    match v {
        Value::String(s) if s.starts_with("/tmp/") => *s = dir.join(&s["/tmp/".len()..]).to_string_lossy().into_owned(),
        Value::Array(a) => a.iter_mut().for_each(|x| map_tmp(x, dir)),
        Value::Object(m) => m.values_mut().for_each(|x| map_tmp(x, dir)),
        _ => {}
    }
}

/// `agent.*` examples (C1.4) name a session `a1` and a confirmation `cf_1`: an agent session is connected and a
/// confirmable call of it (an overwriting Save As) leaves a pending confirmation; the placeholders become their ids.
fn agent_placeholders(h: &ExecutorHandle, dir: &std::path::Path, params: &mut Value) {
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
fn clipboard_for(h: &ExecutorHandle, id: &str) {
    if matches!(id, "edit.paste" | "edit.pasteSpecial" | "edit.pasteAsNewElement") {
        ok(h, "edit.copy", json!({ "context": "xsheet", "objectIds": ["scene:#1/element:\"Bob\"/cell:#1"] }));
    }
}

#[test]
fn every_example_runs_on_the_fixture_under_the_watchdog() {
    let registry = tt_executor::registry().unwrap();
    let envelope_schema = tt_registry::schema_of::<tt_executor::Envelope>();
    let mut schema_checked = 0;
    let mut ran = 0;
    let mut failures = Vec::new();
    for e in registry.entries() {
        let Some(c) = e.command else { continue };
        for ex in c.decl.examples {
            let dir = std::env::temp_dir().join(format!("tt-examples-{}", tt_persist::fsx::random_token(6)));
            let files = with_files(e.id()) && !matches!(e.status(), tt_registry::Status::NotImplemented { .. });
            let h = if files {
                std::fs::create_dir_all(&dir).unwrap();
                prepared_with_files(e.id(), &dir)
            } else {
                prepared()
            };
            clipboard_for(&h, e.id());
            let mut params: Value = serde_json::from_str(ex.params).unwrap();
            if files {
                map_tmp(&mut params, &dir);
            }
            if e.id().starts_with("agent.") {
                agent_placeholders(&h, &dir, &mut params);
            }
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
                _ => {
                    if env.status != Status::Ok {
                        failures.push(format!("{} example `{}` failed: {:?}", e.id(), ex.title, env.error));
                    } else {
                        // C1.4 r2 (CI schema-failure adjudication): every successful example's result matches its
                        // declared result schema and the envelope its schema, on every OS (portable paths above);
                        // a job example must finish `succeeded` (the job table checks its final result against
                        // `x-jobResult`, C12-R1-13).
                        let v = env.result.clone().unwrap_or(Value::Null);
                        for (what, schema, value) in [
                            ("result", e.result_schema(), v.clone()),
                            ("envelope", envelope_schema.clone(), serde_json::to_value(&env).unwrap()),
                        ] {
                            let validator = jsonschema::options()
                                .with_draft(jsonschema::Draft::Draft202012)
                                .build(&schema)
                                .unwrap();
                            let errors: Vec<String> = validator.iter_errors(&value).map(|x| x.to_string()).collect();
                            if !errors.is_empty() {
                                failures.push(format!("{} example `{}`: {what} schema: {errors:?}", e.id(), ex.title));
                            }
                        }
                        if e.spec.kind == tt_registry::Kind::Job
                            && let Some(job) = v.get("jobId").and_then(Value::as_str).filter(|j| *j != "dryRun")
                        {
                            let rec = h.jobs().wait(job, Duration::from_secs(30)).map(|(r, _)| r);
                            match rec {
                                Ok(r) if r.state == tt_executor::JobState::Succeeded => schema_checked += 1,
                                other => failures.push(format!("{} example `{}`: job {other:?}", e.id(), ex.title)),
                            }
                        } else {
                            schema_checked += 1;
                        }
                    }
                }
            }
            ran += 1;
            if files {
                drop(h);
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}",
        failures.join(
            "
"
        )
    );
    println!("examples run: {ran}; successful results schema-checked: {schema_checked}");
    assert!(ran >= 140, "examples: {ran}");
}

#[test]
fn every_generated_declaration_answers_with_its_owner() {
    let registry = tt_executor::registry().unwrap();
    let h = fixture();
    let mut n = 0;
    for c in tt_registry::declared::DECLARED {
        let e = registry.get(c.decl.id).unwrap();
        assert!(std::ptr::eq(e.command.unwrap(), c), "{}: the generated declaration is attached", e.id());
        let params: Value = serde_json::from_str(c.decl.examples[0].params).unwrap();
        let env = h.invoke(Invocation::user(e.id(), params));
        let err = env.error.unwrap();
        if e.spec.kind == tt_registry::Kind::Event {
            assert_eq!(err.code, "command.disabled", "{}", e.id());
        } else {
            assert_eq!(err.code, "command.notImplemented", "{}: {err:?}", e.id());
            assert_eq!(err.details["milestone"], json!(tt_registry::owner_of(e.id())));
        }
        n += 1;
    }
    println!("generated declarations: {n}");
    assert!(n > 600);
}
