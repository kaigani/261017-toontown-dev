//! Ruling E-29 (`reviews/M5-design-handler-panics.md`): a handler that panics is contained at the invocation boundary;
//! a panic outside an invocation degrades the engine; a stopped engine's answer names the way out.
//!
//! A dev-only registry replaces one not-implemented spec A action (`colour.palette.create`, owned by a later milestone)
//! with a handler that panics at a point the call names, beside every real command (the device of `c12_never_block`).

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, Registry};

use crate::cx::Cx;
use crate::run::{Core, HANDLER_PANICKED};
use crate::session::{Checkpoint, OpenProject, Session};
use crate::types::Invocation;

type Outcome = Result<(), String>;

fn why<E: std::fmt::Debug>(e: E) -> String {
    format!("{e:?}")
}

/// The test handler's parameters: where it panics.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PanicParams {
    /// `before`, `staged`, `job`, or the session state it changes first.
    at: String,
}

/// A defective handler: it changes what `at` names, then panics (in production nothing would). The attribute is written
/// with its path so that source scans that count the product's command attributes do not count this test-only one.
#[tt_registry::command(
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
        "jobDone" => {
            // A job that runs to its end and publishes its result before the handler panics (ruling E-33).
            let jobs = cx.session().jobs.clone();
            let id = jobs.submit("colour.palette.create", "", |_| Ok(json!({ "outputs": ["out.png"] })))?;
            let _ = jobs.wait(&id, Duration::from_secs(30));
        }
        other => std::panic::panic_any(format!("test: unknown point {other}")),
    }
    std::panic::panic_any(format!("test: the handler panics after `{}`", p.at));
}

/// The real registry with the test handler in place of `colour.palette.create`'s stub.
fn registry() -> Result<&'static Registry, String> {
    let mut all: Vec<&'static tt_registry::Command> = crate::commands::ALL.to_vec();
    all.extend(tt_registry::declared::DECLARED.iter().filter(|c| c.decl.id != "colour.palette.create"));
    all.push(&PANICS);
    Ok(Box::leak(Box::new(Registry::assemble(&all).map_err(why)?)))
}

fn core() -> Result<Core, String> {
    let doc = tt_model::fixture::sample_document(1).map_err(why)?;
    let session = Session { project: Some(OpenProject::new(doc, None, false)), ..Session::default() };
    let mut core = Core::new(registry()?, session, Duration::from_secs(30));
    // A first call repairs the application context (current scene and element), so the states compared below are
    // those of a session in use.
    assert!(core.invoke(&Invocation::user("doc.state.get", json!({}))).is_ok());
    core.take_events();
    Ok(core)
}

/// Everything a failed call must leave as it was, as text: the document, the history, the checkpoints, and every
/// session field the `Rollback` restores.
fn state(core: &Core) -> Result<Vec<(&'static str, String)>, String> {
    let s = core.session().ok_or("no session")?;
    let p = s.project.as_ref().ok_or("no project")?;
    Ok(vec![
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
    ])
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
        // The thumbnails' cache, keyed by drawing, revision and size: a failed call's thumbnail is of a revision that
        // never committed, so no committed revision reads it.
        thumbs: _,
        selection_checked: _,
    } = Session::default();
}

/// Ruling E-33: the same for the two structures inside the session whose fields a handler can reach, the open project
/// and the agent hub, so a field added to either stops the build until it is classified. (The session's other fields are
/// restored whole, as values, by the `Rollback`, so their inner fields are covered by construction: the application
/// context, the UI state, the Timeline state, the clipboard and the edit options are cloned and put back entire.)
#[test]
fn every_project_and_agent_hub_field_is_classified_against_the_rollback() -> Outcome {
    let doc = tt_model::fixture::sample_document(1).map_err(why)?;
    let OpenProject {
        // Never changed by a failed call: the staged document lives in the call's `Cx` until `accept`.
        doc: _,
        // Changed only by `accept` (commit, undo step, revision log, the saved revision and dirty state).
        history: _,
        log: _,
        saved_rev: _,
        permanent_dirty: _,
        // Restored by the `Rollback`.
        checkpoints: _,
        // Set by opening and saving (their own requests, not a handler's staging).
        path: _,
        form: _,
        read_only: _,
        store: _,
    } = OpenProject::new(doc, None, false);
    let crate::agents::AgentHub {
        // Restored by the `Rollback`.
        sessions: _,
        limits: _,
        next_session: _,
        persist: _,
        typed_tools: _,
        typed_tools_version: _,
        ttl: _,
        server_enabled: _,
        serving: _,
        // Kept on purpose: a refused call's confirmation requests and activity entries are records (11 §6.3, §11).
        pending: _,
        activity: _,
        next_seq: _,
    } = crate::agents::AgentHub::default();
    Ok(())
}

/// Ruling E-33: what "nothing was changed" does not cover. A job the call started that ran to its end and published its
/// result before the handler panicked is not reversed (cancellation cannot undo a finished job, and loses to a job past
/// its commit point): the caller sees the call answered `internal`, and the job listed as finished with its result.
#[test]
fn a_job_that_finished_before_the_panic_is_not_reversed() -> Outcome {
    let mut core = core()?;
    let env = core.invoke(&Invocation::user("colour.palette.create", json!({ "at": "jobDone" })));
    let e = env.error.ok_or("the call succeeded")?;
    assert_eq!((e.code.as_str(), e.details["reason"].clone()), ("internal", json!(HANDLER_PANICKED)));
    let jobs = core.session().ok_or("no session")?.jobs.list(None, None);
    let job = jobs.last().ok_or("no job")?;
    assert!(job.state.is_final(), "{job:?}");
    assert_eq!(job.result.as_ref().map(|r| r["outputs"].clone()), Some(json!(["out.png"])), "{job:?}");
    // The notice names no cancelled job: there was nothing left to cancel.
    let events = core.take_events();
    let notice = events.iter().find(|e| e.name == "shell.handlerPanicked").ok_or("no notice")?;
    assert_eq!(notice.payload["cancelledJobIds"], json!([]), "the job was already final: nothing to cancel");
    Ok(())
}

/// Ruling E-29 item 1, condition 2: the handler panics before staging, after staging, after starting a job and after
/// changing each kind of session state the `Rollback` covers; after each, the document, the session, the history, the
/// clipboard and the event list equal their state before the call, the call answers `internal` naming the command,
/// and the next call is served.
#[test]
fn a_panicking_handler_fails_its_call_and_changes_nothing() -> Outcome {
    let mut core = core()?;
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
        let jobs_before = core.session().ok_or("no session")?.jobs.list(None, None).len();
        let before = state(&core)?;
        let env = core.invoke(&Invocation::user("colour.palette.create", json!({ "at": at })));
        let e = env.error.clone().ok_or(format!("{at}: the call succeeded"))?;
        assert_eq!(e.code, "internal", "{at}");
        assert_eq!(e.details["reason"], HANDLER_PANICKED, "{at}");
        assert_eq!(e.details["commandId"], "colour.palette.create", "{at}");
        assert!(e.message.contains("failed unexpectedly; nothing was changed"), "{at}: {}", e.message);
        assert!(!env.committed, "{at}");
        // The notice is the only event the failed call leaves; it is announced after the rollback.
        let events = core.take_events();
        let notices: Vec<_> = events.iter().filter(|e| e.name == "shell.handlerPanicked").collect();
        assert_eq!(notices.len(), 1, "{at}: {events:?}");
        assert_eq!(notices.first().map(|e| e.payload["count"].clone()), Some(json!(n + 1)));
        assert!(events.iter().all(|e| e.name != "test.half"), "{at}");
        let after = state(&core)?;
        for ((name, b), (_, a)) in before.iter().zip(&after) {
            if *name == "events" {
                continue; // emptied by `take_events` above; the half-emitted event is checked absent instead
            }
            assert_eq!(a, b, "{at}: {name} changed");
        }
        assert_eq!(core.caught_panics(), u64::try_from(n + 1).map_err(why)?);
        // A job the call started is cancelled.
        let jobs = core.session().ok_or("no session")?.jobs.list(None, None);
        if *at == "job" {
            assert_eq!(jobs.len(), jobs_before + 1);
            let job = jobs.last().ok_or("no job")?;
            assert!(!job.cancellable, "the started job's cancellation was requested: {job:?}");
            let table = core.session().ok_or("no session")?.jobs.clone();
            let (record, _) = table.wait(&job.job_id, Duration::from_secs(30)).map_err(why)?;
            assert!(record.state.is_final(), "{record:?}");
        } else {
            assert_eq!(jobs.len(), jobs_before, "{at}");
        }
        // The next call is served.
        let next = core.invoke(&Invocation::user("doc.state.get", json!({})));
        assert!(next.is_ok(), "{at}: {next:?}");
        core.take_events();
    }
    Ok(())
}

/// The same containment inside an atomic batch: the whole batch fails and changes nothing.
#[test]
fn a_panicking_step_fails_its_atomic_batch_and_changes_nothing() -> Outcome {
    let mut core = core()?;
    let before = state(&core)?;
    let req: crate::types::BatchRequest = serde_json::from_value(json!({
        "label": "two", "atomic": true,
        "steps": [
            { "id": "element.rename", "params": { "elementId": "scene:#1/element:\"Bob-P\"", "name": "Renamed" } },
            { "id": "colour.palette.create", "params": { "at": "staged" } }
        ]
    }))
    .map_err(why)?;
    let env = core.batch(&req, &crate::types::Origin::User);
    assert!(!env.is_ok());
    let step = env.steps.as_ref().and_then(|s| s.get(1)).and_then(|s| s.error.clone()).ok_or("no second step error")?;
    assert_eq!(step.details["reason"], HANDLER_PANICKED);
    let events = core.take_events();
    assert_eq!(events.iter().filter(|e| e.name == "shell.handlerPanicked").count(), 1, "{events:?}");
    for ((name, b), (_, a)) in before.iter().zip(&state(&core)?) {
        if *name != "events" {
            assert_eq!(a, b, "{name} changed");
        }
    }
    assert_eq!(core.caught_panics(), 1);
    Ok(())
}

/// Ruling E-29 item 2: a panic outside an invocation degrades the engine: the host-only notice once, then every call
/// answers `internal` with reason `engineStopped` and the hint that names the way out.
#[test]
fn a_panic_outside_an_invocation_degrades_the_engine() -> Outcome {
    let h = crate::ExecutorHandle::spawn(Default::default()).map_err(why)?;
    assert!(h.open_document(tt_model::fixture::sample_document(1).map_err(why)?, None, false).is_ok());
    let events = h.subscribe();
    assert!(!h.degraded());
    let _ = h.tx.send(crate::Request::Run(Box::new(|_| {
        std::panic::panic_any("test: a panic outside an invocation".to_owned())
    })));
    for command in ["doc.state.get", "timeline.rows.list", "element.rename"] {
        let env = h.invoke(Invocation::user(command, json!({})));
        let e = env.error.ok_or(format!("{command} succeeded"))?;
        assert_eq!(e.code, "internal", "{command}");
        assert_eq!(e.details["reason"], crate::ENGINE_STOPPED, "{command}");
        let hint = e.hint.unwrap_or_default();
        assert!(hint.contains("Restart the engine") && hint.contains("project.recovery.list"), "{hint}");
    }
    // The notice follows the degraded mark on the document thread; the callers above may have been answered first.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut degraded = 0;
    while degraded == 0 && std::time::Instant::now() < deadline {
        if let Ok(e) = events.recv_timeout(Duration::from_millis(100))
            && e.name == "shell.engineDegraded"
        {
            degraded += 1;
        }
    }
    assert_eq!(degraded, 1);
    assert!(h.degraded());
    assert_eq!(events.try_iter().filter(|e| e.name == "shell.engineDegraded").count(), 0, "raised once");
    let counted = h.caught_panics();
    assert_eq!(counted.err().map(|e| e.details["reason"].clone()), Some(json!(crate::ENGINE_STOPPED)));
    Ok(())
}

/// Ruling E-29 item 4: after the document thread has died, a new executor over the same user folders lists the
/// recovery data of the last autosave (in one process the dead engine is named first, as the desktop's restart does;
/// a new process finds the old one gone).
#[test]
fn after_the_document_thread_died_a_new_executor_lists_the_last_autosave() -> Outcome {
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
        .map_err(why)
    };
    let first = over()?;
    let created = first.invoke(Invocation::user("project.create", json!({ "name": "Died with its thread" })));
    let scene = created.result.ok_or("no project")?["sceneId"].as_str().ok_or("no scene")?.to_owned();
    let patched = first.invoke(Invocation::user(
        "doc.object.patch",
        json!({ "targets": [scene], "props": { "meta": { "unsaved": 1 } } }),
    ));
    assert!(patched.is_ok(), "{patched:?}");
    first.run_maintenance(true).map_err(why)?;
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
    let _ = first
        .tx
        .send(crate::Request::Run(Box::new(|_| std::panic::panic_any("test: the document thread dies".to_owned()))));
    let dead = first.invoke(Invocation::user("doc.state.get", json!({})));
    assert_eq!(dead.error.map(|e| e.details["reason"].clone()), Some(json!(crate::ENGINE_STOPPED)));
    // The restart: a new executor, the dead engine named, its recovery data released and offered.
    let second = over()?;
    let owner = first.abandon();
    assert_eq!(second.release_recovery_of(&owner).map_err(why)?, 1);
    let list = second.invoke(Invocation::user("project.recovery.list", json!({}))).result.ok_or("no list")?;
    assert_eq!(list.get(0).map(|e| e["projectName"].clone()), Some(json!("Died with its thread")), "{list}");
    drop(first);
    drop(second);
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
