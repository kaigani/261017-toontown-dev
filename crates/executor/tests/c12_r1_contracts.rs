//! Independent C1.2 round-1 regressions. Assertions state the specification, not the observed defect.
//! Reviewer-owned; no pre-existing test or product source changed. See reviews/M1-C1.2-r1.md.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review regression tests")]

mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, InvokeOptions};
use tt_model::change::PatchOp;
use tt_registry::Scope;

fn patch(h: &ExecutorHandle, r: &str, n: u32) -> Value {
    ok(h, "doc.object.patch", json!({"targets":[r],"props":{"meta":{"review":n}}}))
}

fn dry() -> InvokeOptions {
    InvokeOptions { dry_run: true, ..Default::default() }
}

fn clip() -> Value {
    json!({"content":{"kind":"elements","itemCount":1,"items":[{"elementId":"e1"}]}})
}

// The release channel prevents a failed assertion from leaving a live job behind.
fn held_job(h: &ExecutorHandle, owner: &str) -> (String, mpsc::Sender<()>) {
    let (release, wait) = mpsc::channel();
    let (started, ready) = mpsc::channel();
    let id = h
        .jobs()
        .submit("review.held", owner, move |_| {
            started.send(()).unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(10));
            Ok(json!({"privateResult":"owner only"}))
        })
        .unwrap();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    (id, release)
}

#[test]
fn dry_run_cannot_cancel_a_running_job() {
    let h = fixture();
    let (id, release) = held_job(&h, "user");
    let env = with(&h, "job.cancel", json!({"jobId":id}), dry());
    assert!(env.is_ok(), "{env:?}");
    release.send(()).unwrap();
    let (finished, _) = h.jobs().wait(&id, Duration::from_secs(5)).unwrap();
    assert_eq!(finished.state, tt_executor::JobState::Succeeded, "11 §7.2: dryRun changes nothing");
}

#[test]
fn failed_atomic_batch_cannot_cancel_a_running_job() {
    let h = fixture();
    let (id, release) = held_job(&h, "user");
    let env = batch(
        &h,
        json!({"label":"rollback", "steps":[
            {"id":"job.cancel","params":{"jobId":id}},
            {"id":"review.unknown","params":{}}
        ]}),
    );
    assert!(env.is_error());
    release.send(()).unwrap();
    let (finished, _) = h.jobs().wait(&id, Duration::from_secs(5)).unwrap();
    assert_eq!(finished.state, tt_executor::JobState::Succeeded, "11 §7.3: all-or-nothing");
}

#[test]
fn dry_run_clipboard_emits_no_change_event() {
    let h = fixture();
    let rx = h.subscribe();
    assert!(with(&h, "edit.clipboard.set", clip(), dry()).is_ok());
    assert_eq!(ok(&h, "edit.clipboard.get", json!({}))["clipboard"], Value::Null);
    h.snapshot().unwrap(); // queue barrier: event broadcasting precedes this request
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "edit.clipboard.changed").collect();
    assert!(events.is_empty(), "dry run emitted {events:?}");
}

#[test]
fn failed_batch_clipboard_emits_no_change_event() {
    let h = fixture();
    let rx = h.subscribe();
    assert!(
        batch(
            &h,
            json!({"label":"rollback", "steps":[
                {"id":"edit.clipboard.set","params":clip()}, {"id":"review.unknown","params":{}}
            ]})
        )
        .is_error()
    );
    h.snapshot().unwrap();
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "edit.clipboard.changed").collect();
    assert!(events.is_empty(), "rolled-back batch emitted {events:?}");
}

#[test]
fn parameter_dry_run_is_honoured_inside_atomic_batch() {
    let h = fixture();
    let before = rev(&h);
    let env = batch(
        &h,
        json!({"label":"preview only", "steps":[
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"review":1}},"dryRun":true}}
        ]}),
    );
    assert!(env.is_ok(), "{env:?}");
    assert_eq!(rev(&h), before, "doc.object.patch dryRun is also a command parameter (10 AG-02)");
    assert!(get(&h, "scene:#1")["meta"].get("review").is_none());
}

#[test]
fn sequential_batch_dry_run_queries_see_previous_simulated_edits() {
    let h = fixture();
    let before = rev(&h);
    let req = json!({"label":"preview", "atomic":false, "steps":[
        {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"review":73}}}},
        {"id":"doc.object.get","params":{"ref":"scene:#1"}}
    ]});
    let mut preview = req.clone();
    preview["dryRun"] = json!(true);
    let simulated = batch(&h, preview);
    assert!(simulated.is_ok(), "{simulated:?}");
    assert_eq!(rev(&h), before);
    let real = batch(&h, req);
    assert!(real.is_ok(), "{real:?}");
    assert_eq!(
        simulated.steps.unwrap()[1].result.as_ref().unwrap()["object"]["meta"],
        real.steps.unwrap()[1].result.as_ref().unwrap()["object"]["meta"],
        "11 §7.2–7.3: predict the actual sequence"
    );
}

#[test]
fn job_wait_obeys_request_id_and_retains_document_revision() {
    let h = fixture();
    let (id, release) = held_job(&h, "user");
    let options = InvokeOptions { request_id: Some("wait-retry".into()), ..Default::default() };
    let first = with(&h, "job.wait", json!({"jobId":id,"timeoutMs":0}), options.clone());
    release.send(()).unwrap();
    h.jobs().wait(&id, Duration::from_secs(5)).unwrap();
    let second = with(&h, "job.wait", json!({"jobId":id,"timeoutMs":0}), options);
    assert_eq!(first, second, "11 §7.2 idempotency applies to run_command job.wait");
    assert_eq!(first.document_rev, Some(rev(&h)));
}

#[test]
fn job_wait_timeout_policy_is_explicitly_measured() {
    // 11 15.2 explicitly gives job.wait a separate timeout. A watchdog bypass is evidence,
    // not by itself proof of a modal-UI violation under 11 14.6.
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(100) });
    let id = h
        .jobs()
        .submit("review.slow", "user", |_| {
            std::thread::sleep(Duration::from_millis(650));
            Ok(json!({}))
        })
        .unwrap();
    let start = Instant::now();
    let env = run(&h, "job.wait", json!({"jobId":id,"timeoutMs":2000}));
    let elapsed = start.elapsed();
    assert!(env.is_ok(), "{env:?}");
    if let Ok(dir) = std::env::var("TT_C12_REVIEW_ARTIFACT_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join("job-wait-budget.json"),
            json!({"executorBudgetMs":100,"jobWaitTimeoutMs":2000,"elapsedMs":elapsed.as_millis(),"status":"ok"})
                .to_string(),
        )
        .unwrap();
    }
}

#[test]
fn agent_cannot_read_another_sessions_jobs_with_all_true() {
    let h = fixture();
    let (id, release) = held_job(&h, "agent:other");
    let env = h.invoke(Invocation {
        command: "job.list".into(),
        params: json!({"all":true}),
        options: Default::default(),
        origin: agent(&[Scope::Read]),
    });
    release.send(()).unwrap();
    assert!(
        env.is_error() || !env.result.unwrap().as_array().unwrap().iter().any(|r| r["jobId"] == id),
        "11 §15.2: all jobs are for the GUI"
    );
}

#[test]
fn agent_cannot_cancel_another_sessions_job() {
    let h = fixture();
    let (id, release) = held_job(&h, "agent:other");
    let env = h.invoke(Invocation {
        command: "job.cancel".into(),
        params: json!({"jobId":id}),
        options: Default::default(),
        origin: agent(&[Scope::Ui]),
    });
    release.send(()).unwrap();
    let (record, _) = h.jobs().wait(&id, Duration::from_secs(5)).unwrap();
    assert!(env.is_error(), "cross-session cancellation succeeded: {record:?}");
}

#[test]
fn request_id_collision_returns_original_result_per_literal_spec() {
    let h = fixture();
    let options = InvokeOptions { request_id: Some("collision".into()), ..Default::default() };
    let original = with(&h, "app.frame.set", json!({"frame":1}), options.clone());
    assert!(original.is_ok());
    let env = with(&h, "app.frame.set", json!({"frame":2}), options);
    // The charter requests collision rejection, but 11 7.2 says a repeated ID returns
    // the original result without qualifying the parameter values. Report that ambiguity.
    assert_eq!(env, original);
}

#[test]
fn expect_rev_error_reports_current_rev_and_small_current_object() {
    let h = fixture();
    let scene = first(&h, "scene");
    let current = obj_rev(&h, &scene);
    let e = err(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"review":9}},"expectRev":current+1}));
    assert_eq!(e.code, "rev.conflict");
    assert_eq!(e.details["currentRev"], current, "10 REV-05 and 11 §10");
    assert!(
        e.details.as_object().unwrap().values().any(|v| v.get("id") == Some(&json!(scene))),
        "REV-05: current JSON under 64 KiB"
    );
}

#[test]
fn expect_rev_map_checks_every_listed_object_not_only_patch_targets() {
    let h = fixture();
    let scene = first(&h, "scene");
    let palette = first(&h, "palette");
    let before = rev(&h);
    let env = run(
        &h,
        "doc.object.patch",
        json!({"targets":[scene],"props":{"meta":{"review":9}},"expectRev":{palette:999999}}),
    );
    assert!(env.is_error(), "10 REV-05: any listed mismatch must fail: {env:?}");
    assert_eq!(rev(&h), before);
}

#[test]
fn external_safe_code_cannot_install_a_mutated_stale_snapshot() {
    let h = fixture();
    let stale = h.snapshot().unwrap().unwrap();
    let scene = first(&h, "scene");
    patch(&h, &scene, 1);
    let live_rev = rev(&h);
    let (entry, _, at) = stale.object_pointer(&scene).unwrap();
    let ops = vec![PatchOp {
        op: "add",
        path: format!("{at}/meta/review"),
        from: None,
        value: Some(json!(99)),
        old_value: None,
        range: None,
        elided: false,
    }];
    let (outside, _) = stale.with_patched_entries(&[(entry, ops)]).unwrap();
    // This is an external integration crate, safe Rust only. Neither Cx nor registry invocation is used.
    let replaced = h.open_document(outside, Some("review.animpkg".into()), false);
    assert!(
        replaced.is_error(),
        "unprivileged public replace erased history and rolled rev {live_rev} back to {:?}; 11 P2 / plan §3.2",
        replaced.document_rev
    );
}

#[test]
fn checkpoint_in_atomic_batch_has_the_committed_content_revision() {
    let h = fixture();
    let before = rev(&h);
    let env = batch(
        &h,
        json!({"label":"with checkpoint", "steps":[
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"review":1}}}},
            {"id":"checkpoint.create","params":{"name":"inside"}}
        ]}),
    );
    assert!(env.is_ok(), "{env:?}");
    let list = ok(&h, "checkpoint.list", json!({}));
    assert_eq!(list[0]["documentRev"], before + 1, "checkpoint contains changed content with the base revision");
}

#[test]
fn doc_diff_sees_prior_uncommitted_atomic_steps() {
    let h = fixture();
    let before = rev(&h);
    let env = batch(
        &h,
        json!({"label":"read own writes", "steps":[
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"review":1}}}},
            {"id":"doc.diff","params":{"sinceRev":before}}
        ]}),
    );
    assert!(env.is_ok(), "{env:?}");
    assert!(
        !env.steps.unwrap()[1].result.as_ref().unwrap()["changes"].as_array().unwrap().is_empty(),
        "11 §7.3 queries see earlier staged state"
    );
}

#[test]
fn changes_report_drawing_revision_when_a_stroke_changes() {
    let h = fixture();
    let stroke = first(&h, "stroke");
    let drawing = stroke.split('/').next().unwrap();
    let env = run(&h, "doc.object.patch", json!({"targets":[stroke],"props":{"meta":{"review":1}}}));
    assert!(env.is_ok(), "{env:?}");
    assert_eq!(obj_rev(&h, drawing), rev(&h));
    assert!(
        env.changes.iter().any(|c| c.id == drawing),
        "11 P6: every changed object's new revision, including REV-02 drawing; {env:?}"
    );
}

#[test]
fn workspace_switch_defaults_to_autosaving_current_arrangement() {
    let h = fixture();
    ok(&h, "window.panel.set", json!({"panelId":"history","visible":true}));
    let layout = ok(&h, "workspace.get", json!({}));
    ok(&h, "workspace.select", json!({"name":"Drawing"}));
    ok(&h, "workspace.select", json!({"name":"Default"}));
    assert_eq!(ok(&h, "workspace.get", json!({})), layout, "01 §5.5 autoSave is ON by default");
}

#[test]
fn edit_clipboard_is_disabled_without_a_project() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    let env = run(&h, "edit.clipboard.set", clip());
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("project.none"), "01 §1.3 all edit commands disabled");
}

#[test]
fn idle_query_does_not_repair_context_after_atomic_current_element_change() {
    let h = fixture();
    // A state query must read the same current drawing inside and outside a batch.
    let drawing_element =
        ok(&h, "doc.object.find", json!({"type":"element","where":{"elementType":"drawing"}}))["items"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
    let env = batch(
        &h,
        json!({"label":"context", "steps":[
            {"id":"app.currentElement.set","params":{"elementId":drawing_element}},
            {"id":"app.state.get","params":{}}
        ]}),
    );
    assert!(env.is_ok(), "{env:?}");
    assert_eq!(
        env.steps.unwrap()[1].result.as_ref().unwrap()["drawing"],
        ok(&h, "app.state.get", json!({}))["drawing"]
    );
}

#[test]
fn registry_stubs_have_actual_parameter_constraints_and_examples() {
    let r = tt_executor::registry().unwrap();
    let missing: Vec<_> = r.entries().filter(|e| incomplete_declaration(e)).map(|e| e.id()).collect();
    assert!(
        missing.is_empty(),
        "plan §3.2 / 11 §14.2: {} entries have incomplete declarations; first: {:?}",
        missing.len(),
        &missing[..missing.len().min(8)]
    );
}

#[test]
fn registry_file_and_network_commands_declare_required_scopes() {
    let r = tt_executor::registry().unwrap();
    for (id, required) in
        [("export.movie.run", Scope::FilesWrite), ("import.file.run", Scope::FilesRead), ("prefs.set", Scope::Prefs)]
    {
        let e = r.get(id).unwrap_or_else(|| panic!("spec id missing: {id}"));
        assert!(e.scopes().contains(&required), "11 §6.1: {id} scopes {:?} omit {required:?}", e.scopes());
    }
}

#[test]
fn fullscreen_state_and_active_view_agree_with_visible_central_panel() {
    let h = fixture();
    let mut layout = ok(&h, "workspace.get", json!({}));
    layout["fullScreen"]["centralPanel"] = json!("drawingView");
    ok(&h, "workspace.layout.set", json!({"layout":layout}));
    ok(&h, "window.fullScreen.set", json!({"enabled":true}));
    let panels = ok(&h, "window.panel.list", json!({}));
    let central = panels.as_array().unwrap().iter().find(|p| p["area"] == "central").unwrap();
    assert_eq!(central["panelId"], "drawingView");
    let app = ok(&h, "app.state.get", json!({}));
    assert_eq!(app["activeView"]["panelId"], central["panelId"], "01 §5.6 / 11 §13: no hidden state traps");
}

#[test]
fn renamed_builtin_workspace_can_still_be_restored() {
    let h = fixture();
    ok(&h, "workspace.rename", json!({"name":"Default","newName":"Review default"}));
    let env = run(&h, "workspace.reset", json!({"name":"Review default","select":true}));
    assert!(env.is_ok(), "01 §5.5 built-in arrangements are always restorable: {env:?}");
}

#[test]
fn revision_exhaustion_is_an_error_and_does_not_kill_the_document_thread() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    parts.document_rev = u64::MAX;
    let d = tt_model::document::Document::from_parts(parts).unwrap();
    assert!(h.open_document(d, None, false).is_ok());
    let env = run(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"review":1}}}));
    assert!(env.is_error());
    assert!(h.snapshot().is_ok(), "overflow killed the document thread: {env:?}");
}

#[test]
fn diff_older_than_snapshot_window_replays_repeated_entry_batch() {
    let h = fixture();
    let before = rev(&h);
    assert!(
        batch(
            &h,
            json!({"label":"same entry twice", "steps":[
                {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"review":1}}}},
                {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"review":2}}}}
            ]})
        )
        .is_ok()
    );
    for n in 3..260 {
        patch(&h, "scene:#1", n);
    }
    let env = run(&h, "doc.diff", json!({"sinceRev":before}));
    assert!(env.is_ok(), "10 AG-09 retains at least 10,000 transactions; after only 258: {env:?}");
}

#[test]
fn session_idempotency_survives_more_than_ten_thousand_other_requests() {
    let h = fixture();
    let options = InvokeOptions { request_id: Some("original".into()), ..Default::default() };
    let original =
        with(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"review":1}}}), options.clone());
    assert!(original.is_ok());
    patch(&h, "scene:#1", 2);
    for n in 0..10_001 {
        assert!(
            with(
                &h,
                "window.fullScreen.set",
                json!({"enabled":false}),
                InvokeOptions { request_id: Some(format!("other-{n}")), ..Default::default() }
            )
            .is_ok()
        );
    }
    let before = rev(&h);
    let retry = with(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"review":1}}}), options);
    assert_eq!(retry, original, "11 7.2 guarantees the original result within the session");
    assert_eq!(rev(&h), before);
}

#[test]
fn clipboard_get_returns_the_specified_clipboard_directly() {
    let h = fixture();
    ok(&h, "edit.clipboard.set", clip());
    let result = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
    assert_eq!(result["kind"], "elements", "01 12.1/12.4 return Clipboard, not a wrapper: {result}");
}

#[test]
fn clipboard_set_validates_the_content_for_its_kind() {
    let h = fixture();
    let env = run(
        &h,
        "edit.clipboard.set",
        json!({"content":{"kind":"drawingObjects","itemCount":1,"items":[{"notAVectorObject":true}]}}),
    );
    assert!(env.is_error(), "01 12.1 promises validation against the content schema: {env:?}");
}

#[test]
fn registry_owners_follow_the_milestone_plan() {
    let r = tt_executor::registry().unwrap();
    assert_eq!(
        r.get("import.movie.run").unwrap().describe()["milestone"],
        "M8 C8.3",
        "plan 4 moved movie import with the decoder to M8"
    );
}

#[test]
fn every_registry_entry_has_a_gui_binding_or_agent_only_justification() {
    let r = tt_executor::registry().unwrap();
    let missing: Vec<_> = r
        .entries()
        .filter(|e| {
            let d = e.describe();
            !gui_reachable(&d)
        })
        .map(|e| e.id())
        .collect();
    assert!(missing.is_empty(), "11 14.1 reverse parity excludes {} inventory entries", missing.len());
}

#[test]
fn edit_commands_promised_for_c12_are_implemented() {
    let r = tt_executor::registry().unwrap();
    for id in [
        "edit.cut",
        "edit.copy",
        "edit.paste",
        "edit.delete",
        "edit.cutSpecial",
        "edit.pasteSpecial",
        "edit.pasteAsNewElement",
    ] {
        assert_eq!(
            r.get(id).unwrap().status(),
            tt_registry::Status::Implemented,
            "plan 4 C1.2 explicitly includes edit.*: {id}"
        );
    }
}

#[test]
fn setting_mode_activates_a_view_of_the_requested_type() {
    let h = fixture();
    ok(&h, "app.mode.set", json!({"mode":"drawing"}));
    let state = ok(&h, "app.state.get", json!({}));
    assert_eq!(
        state["activeView"]["type"], "drawing",
        "01 4 derives mode from active view, not an unrelated flag: {state}"
    );
}

#[test]
fn explicitly_saving_and_reselecting_current_workspace_keeps_layout() {
    let h = fixture();
    ok(&h, "window.panel.set", json!({"panelId":"history","visible":true}));
    let before = ok(&h, "workspace.get", json!({}));
    ok(&h, "workspace.select", json!({"name":"Default","saveCurrent":true}));
    assert_eq!(ok(&h, "workspace.get", json!({})), before, "01 5.5 saves first, then applies the chosen workspace");
}

#[test]
fn historical_diff_reports_the_revision_of_its_requested_end_state() {
    let h = fixture();
    let base = rev(&h);
    for n in 0..300 {
        patch(&h, "scene:#1", n);
    }
    let result = ok(&h, "doc.diff", json!({"sinceRev":base,"untilRev":base+1}));
    assert_eq!(result["toRev"], base + 1);
    assert_eq!(
        result["changes"][0]["rev"],
        base + 1,
        "10 REV-02: old content must carry its actual old revision: {result}"
    );
}

#[test]
fn diff_retains_the_last_save_even_after_ten_thousand_transactions() {
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(60) }).unwrap();
    assert!(
        h.open_document(tt_model::fixture::sample_document(1).unwrap(), Some("review.animpkg".into()), false).is_ok()
    );
    let saved = rev(&h);
    for n in 0..10_001 {
        patch(&h, "scene:#1", n);
    }
    let result = run(&h, "doc.diff", json!({"sinceRev":saved}));
    assert!(result.is_ok(), "10 AG-09 retains at least to the last save AND the last 10000 transactions: {result:?}");
}

fn incomplete_declaration(e: &tt_registry::Entry) -> bool {
    e.command.is_none()
        || e.describe()["examples"].as_array().unwrap().is_empty()
        || e.params_schema().get("$schema").is_none()
        || e.result_schema().get("$schema").is_none()
}

fn gui_reachable(d: &Value) -> bool {
    fn binds(value: &Value, id: &Value) -> bool {
        match value {
            Value::Object(m) => m.get("command") == Some(id) || m.values().any(|v| binds(v, id)),
            Value::Array(a) => a.iter().any(|v| binds(v, id)),
            _ => false,
        }
    }
    let manifest: Value = serde_json::from_str(include_str!("../../../desktop/src/ui-bindings/menu.json")).unwrap();
    binds(&manifest, &d["id"])
        || d["guiLocations"].as_str().is_some_and(|s| !s.is_empty())
        || d["agentOnly"].as_str().is_some_and(|s| !s.is_empty())
}
