//! Ruling E-29 (`reviews/M5-design-handler-panics.md`): a handler that panics is contained at the invocation boundary;
//! a panic outside an invocation degrades the engine; a stopped engine's answer names the way out.
//!
//! A dev-only registry replaces one not-implemented spec A action (`colour.palette.create`, owned by a later milestone)
//! with a handler that panics at a point the call names, beside every real command (the device of `c12_never_block`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "test")]

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, Registry, command};

use crate::cx::Cx;
use crate::run::{Core, HANDLER_PANICKED};
use crate::session::{Checkpoint, OpenProject, Session};
use crate::types::Invocation;

/// The test handler's parameters: where it panics.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PanicParams {
    /// `before`, `staged`, `job`, or the session state it changes first.
    at: String,
}

/// A defective handler: it changes what `at` names, then panics (in production nothing would).
#[command(
    id = "colour.palette.create",
    kind = action,
    label = "Panics",
    scopes = [edit],
    undo = "Panics",
    needs_project,
    example(title = "Panics", params = r#"{"at": "before"}"#)
)]
fn panics(cx: &mut Cx, p: PanicParams) -> Result<Value, CommandError> {
    match p.at.as_str() {
        "before" => {}
        "staged" => {
            let id = cx.resolve_as("scene:#1/element:\"Bob\"", tt_model::ObjectType::Element)?;
            cx.patch_objects(vec![(
                id,
                vec![tt_model::change::PatchOp {
                    op: "replace",
                    from: None,
                    path: "/name".into(),
                    range: None,
                    value: Some(json!("Staged")),
                    old_value: None,
                    elided: false,
                }],
            )])?;
        }
        "job" => {
            let jobs = cx.session().jobs.clone();
            jobs.submit("colour.palette.create", "", |progress| {
                while !progress.cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(Value::Null)
            })?;
        }
        "app" => cx.app_mut().frame = 9,
        "ui" => cx.ui_mut().set_full_screen(true),
        "checkpoints" => {
            let p = cx.project_mut()?;
            let doc = p.doc.clone();
            p.checkpoints.push(Checkpoint {
                name: "half".into(),
                created_at: String::new(),
                document_rev: 0,
                origin: "user".into(),
                doc,
            });
        }
        "clipboard" => cx.session_mut().clipboard = Some(Arc::new(json!({ "kind": "half" }))),
        "editOptions" => {
            cx.session_mut().edit_options.insert("half".into(), Map::new());
        }
        "agents" => {
            let a = &mut cx.session_mut().agents;
            a.server_enabled = !a.server_enabled;
        }
        "timeline" => cx.session_mut().timeline.reveal_count += 1,
        "events" => cx.emit("test.half", json!({})),
        other => return Err(CommandError::new("arg.invalid", format!("unknown point {other}"))),
    }
    panic!("test: the handler panics after `{}`", p.at);
}

/// The real registry with the test handler in place of `colour.palette.create`'s stub.
fn registry() -> &'static Registry {
    let mut all: Vec<&'static tt_registry::Command> = crate::commands::ALL.to_vec();
    all.extend(tt_registry::declared::DECLARED.iter().filter(|c| c.decl.id != "colour.palette.create"));
    all.push(&PANICS);
    Box::leak(Box::new(Registry::assemble(&all).unwrap()))
}

fn core() -> Core {
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let session = Session { project: Some(OpenProject::new(doc, None, false)), ..Session::default() };
    let mut core = Core::new(registry(), session, Duration::from_secs(30));
    // A first call repairs the application context (current scene and element), so the states compared below are
    // those of a session in use.
    assert!(core.invoke(&Invocation::user("doc.state.get", json!({}))).is_ok());
    core.take_events();
    core
}

/// Everything a failed call must leave as it was, as text: the document, the history, the checkpoints, and every
/// session field the `Rollback` restores.
fn state(core: &Core) -> Vec<(&'static str, String)> {
    let s = core.session().unwrap();
    let p = s.project.as_ref().unwrap();
    vec![
        ("document", format!("{:?}", p.doc)),
        ("history", format!("{:?}", p.history)),
        ("checkpoints", format!("{:?}", p.checkpoints)),
        ("app", format!("{:?}", s.app)),
        ("ui", format!("{:?}", s.ui)),
        ("clipboard", format!("{:?}", s.clipboard)),
        ("editOptions", format!("{:?}", s.edit_options)),
        ("agents", format!("{:?}", (&s.agents.sessions, &s.agents.limits, s.agents.server_enabled, s.agents.ttl))),
        ("timeline", format!("{:?}", s.timeline)),
        ("events", format!("{:?}", s.events)),
    ]
}

/// Ruling E-29 item 1, condition 1: every field of the session is classified against the `Rollback`. The pattern names
/// every field: a field added to `Session` stops this test from compiling, with the field's name in the error, until it
/// is classified here (and restored by the `Rollback`, or stated why not).
#[test]
fn every_session_field_is_classified_against_the_rollback() {
    let Session {
        // Restored by the `Rollback` (`run.rs`).
        app: _,
        ui: _,
        clipboard: _,
        edit_options: _,
        agents: _, // sessions, limits, policy restored; confirmation requests and the activity log kept on purpose
        timeline: _,
        events: _, // truncated to its length before the call
        // The open project: the staged document is never committed by a failed call (it lives in the call's `Cx`
        // until `accept`); its checkpoints are restored; history, log and store change only in `accept`.
        project: _,
        // Shared with job threads: jobs the failed call started are cancelled (`JobTable::cancel_since`).
        jobs: _,
        // Settings and the persistence environment: a call changes them only through deferred effects, which a failed
        // call never applies.
        env: _,
        // Derived caches keyed by content and revision: a stale entry cannot be read for other content.
        planar: _,
        selection_checked: _,
    } = Session::default();
}

/// Ruling E-29 item 1, condition 2: the handler panics before staging, after staging, after starting a job and after
/// changing each kind of session state the `Rollback` covers; after each, the document, the session, the history, the
/// clipboard and the event list equal their state before the call, the call answers `internal` naming the command,
/// and the next call is served.
#[test]
fn a_panicking_handler_fails_its_call_and_changes_nothing() {
    let mut core = core();
    let points = [
        "before",
        "staged",
        "job",
        "app",
        "ui",
        "checkpoints",
        "clipboard",
        "editOptions",
        "agents",
        "timeline",
        "events",
    ];
    for (n, at) in points.iter().enumerate() {
        let jobs_before = core.session().unwrap().jobs.list(None, None).len();
        let before = state(&core);
        let env = core.invoke(&Invocation::user("colour.palette.create", json!({ "at": at })));
        let e = env.error.clone().unwrap_or_else(|| panic!("{at}: the call succeeded"));
        assert_eq!(e.code, "internal", "{at}");
        assert_eq!(e.details["reason"], HANDLER_PANICKED, "{at}");
        assert_eq!(e.details["commandId"], "colour.palette.create", "{at}");
        assert!(e.message.contains("failed unexpectedly; nothing was changed"), "{at}: {}", e.message);
        assert!(!env.committed, "{at}");
        // The notice is the only event the failed call leaves; it is announced after the rollback.
        let events = core.take_events();
        let notices: Vec<_> = events.iter().filter(|e| e.name == "shell.handlerPanicked").collect();
        assert_eq!(notices.len(), 1, "{at}: {events:?}");
        assert_eq!(notices[0].payload["count"], n + 1);
        assert!(events.iter().all(|e| e.name != "test.half"), "{at}");
        let after = state(&core);
        for ((name, b), (_, a)) in before.iter().zip(&after) {
            if *name == "events" {
                continue; // emptied by `take_events` above; the half-emitted event is checked absent instead
            }
            assert_eq!(a, b, "{at}: {name} changed");
        }
        assert_eq!(core.caught_panics(), u64::try_from(n + 1).unwrap());
        // A job the call started is cancelled.
        let jobs = core.session().unwrap().jobs.list(None, None);
        if *at == "job" {
            assert_eq!(jobs.len(), jobs_before + 1);
            let job = jobs.last().unwrap();
            assert!(!job.cancellable, "the started job's cancellation was requested: {job:?}");
            let (record, _) = core.session().unwrap().jobs.wait(&job.job_id, Duration::from_secs(30)).unwrap();
            assert!(record.state.is_final(), "{record:?}");
        } else {
            assert_eq!(jobs.len(), jobs_before, "{at}");
        }
        // The next call is served.
        let next = core.invoke(&Invocation::user("doc.state.get", json!({})));
        assert!(next.is_ok(), "{at}: {next:?}");
        core.take_events();
    }
}

/// The same containment inside an atomic batch: the whole batch fails and changes nothing.
#[test]
fn a_panicking_step_fails_its_atomic_batch_and_changes_nothing() {
    let mut core = core();
    let before = state(&core);
    let req: crate::types::BatchRequest = serde_json::from_value(json!({
        "label": "two", "atomic": true,
        "steps": [
            { "id": "element.rename", "params": { "elementId": "scene:#1/element:\"Bob-P\"", "name": "Renamed" } },
            { "id": "colour.palette.create", "params": { "at": "staged" } }
        ]
    }))
    .unwrap();
    let env = core.batch(&req, &crate::types::Origin::User);
    assert!(!env.is_ok());
    assert_eq!(env.steps.as_ref().unwrap()[1].error.as_ref().unwrap().details["reason"], HANDLER_PANICKED);
    let events = core.take_events();
    assert_eq!(events.iter().filter(|e| e.name == "shell.handlerPanicked").count(), 1, "{events:?}");
    for ((name, b), (_, a)) in before.iter().zip(&state(&core)) {
        if *name != "events" {
            assert_eq!(a, b, "{name} changed");
        }
    }
    assert_eq!(core.caught_panics(), 1);
}

/// Ruling E-29 item 2: a panic outside an invocation degrades the engine: the host-only notice once, then every call
/// answers `internal` with reason `engineStopped` and the hint that names the way out.
#[test]
fn a_panic_outside_an_invocation_degrades_the_engine() {
    let h = crate::ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let events = h.subscribe();
    assert!(!h.degraded());
    let _ = h.tx.send(crate::Request::Run(Box::new(|_| panic!("test: a panic outside an invocation"))));
    for command in ["doc.state.get", "timeline.rows.list", "element.rename"] {
        let env = h.invoke(Invocation::user(command, json!({})));
        let e = env.error.unwrap();
        assert_eq!(e.code, "internal", "{command}");
        assert_eq!(e.details["reason"], crate::ENGINE_STOPPED, "{command}");
        let hint = e.hint.unwrap_or_default();
        assert!(hint.contains("Restart the engine") && hint.contains("project.recovery.list"), "{hint}");
    }
    assert!(h.degraded());
    let degraded: Vec<_> = events.try_iter().filter(|e| e.name == "shell.engineDegraded").collect();
    assert_eq!(degraded.len(), 1);
    assert_eq!(h.caught_panics().unwrap_err().details["reason"], crate::ENGINE_STOPPED);
}

/// Ruling E-29 item 4: after the document thread has died, a new executor over the same user folders lists the
/// recovery data of the last autosave (in one process the dead engine is named first, as the desktop's restart does;
/// a new process finds the old one gone).
#[test]
fn after_the_document_thread_died_a_new_executor_lists_the_last_autosave() {
    use crate::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Maintenance, PersistOptions, UserDirs};
    let dir = std::env::temp_dir().join(format!("tt-e29-recovery-{}", crate::random_token(8)));
    let over = || {
        ExecutorHandle::spawn_with_options(ExecutorOptions {
            config: ExecutorConfig { budget: Duration::from_secs(60) },
            persist: PersistOptions {
                user_dirs: Some(UserDirs::under(&dir)),
                maintenance: Maintenance { tick: Duration::ZERO, autosave: Some(Duration::ZERO), ..Default::default() },
                ..Default::default()
            },
        })
        .unwrap()
    };
    let first = over();
    let created = first.invoke(Invocation::user("project.create", json!({ "name": "Died with its thread" })));
    let scene = created.result.unwrap()["sceneId"].as_str().unwrap().to_owned();
    let patched = first.invoke(Invocation::user(
        "doc.object.patch",
        json!({ "targets": [scene], "props": { "meta": { "unsaved": 1 } } }),
    ));
    assert!(patched.is_ok(), "{patched:?}");
    first.run_maintenance(true).unwrap();
    let started = std::time::Instant::now();
    let written = || {
        std::fs::read_dir(dir.join("Recovery"))
            .map(|rd| rd.filter_map(Result::ok).any(|e| e.path().join("recovery.json").exists()))
            .unwrap_or(false)
    };
    while !written() {
        assert!(started.elapsed() < Duration::from_secs(30), "no autosave was written");
        std::thread::yield_now();
    }
    // The document thread dies (a panic outside an invocation).
    let _ = first.tx.send(crate::Request::Run(Box::new(|_| panic!("test: the document thread dies"))));
    let dead = first.invoke(Invocation::user("doc.state.get", json!({})));
    assert_eq!(dead.error.unwrap().details["reason"], crate::ENGINE_STOPPED);
    // The restart: a new executor, the dead engine named, its recovery data released and offered.
    let second = over();
    let owner = first.abandon();
    assert_eq!(second.release_recovery_of(&owner).unwrap(), 1);
    let list = second.invoke(Invocation::user("project.recovery.list", json!({})));
    let list = list.result.unwrap();
    assert_eq!(list.as_array().unwrap()[0]["projectName"], "Died with its thread", "{list}");
    drop(first);
    drop(second);
    let _ = std::fs::remove_dir_all(&dir);
}
