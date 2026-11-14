//! M4 C4.1: user priority for agent calls (01 §12.1 rule 8, 11 §6.4), key dispatch through the keymap (01 §8.4),
//! sensor reports and the UI tree (`reviews/M4-design-busy-arbitration.md`). Builder-owned.
//!
//! No assertion depends on a sleep fitting a bound: waiting callers are observed through their cancellation probe
//! (the wait polls it), and refusals are asserted on their outcome, not on elapsed time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::common::*;
use serde_json::{Value, json};
use tt_executor::host::{KeyResolution, ShellReport};
use tt_executor::shell::ModalReport;
use tt_executor::{Invocation, InvokeOptions, Status};
use tt_registry::Scope;

/// Reports are read from the JSON text the front end sends (`desktop/src/shell/messages.tsx` through `report_shell`),
/// never built as structs: a struct skips the wire, where absent and `null` differ (C41-R1-01).
fn wire(text: &str) -> ShellReport {
    serde_json::from_str(text).unwrap()
}

fn modal(kind: &str) -> ShellReport {
    wire(&format!(r#"{{"modal":{{"kind":"{kind}","title":"Preferences"}}}}"#))
}

fn clear() -> ShellReport {
    wire(r#"{"modal":null}"#)
}

#[test]
fn modal_on_the_wire_distinguishes_absent_null_and_a_value() {
    assert_eq!(wire("{}").modal, None, "absent leaves the fact as it is");
    assert_eq!(wire(r#"{"modal":null}"#).modal, Some(None), "null ends the modal state");
    assert_eq!(
        wire(r#"{"modal":{"kind":"dialog","title":"Preferences"}}"#).modal,
        Some(Some(ModalReport { kind: "dialog".into(), title: "Preferences".into() }))
    );
    // The host's own writer round-trips the three states.
    for text in ["{}", r#"{"modal":null}"#, r#"{"modal":{"kind":"gesture","title":"Drag"}}"#] {
        assert_eq!(serde_json::to_string(&wire(text)).unwrap(), text);
    }
    // Over JSON, closing the dialog releases shortcuts and agents (the reviewer's end-to-end case is
    // c41_r1_review.rs).
    let h = fixture();
    h.report_shell(modal("dialog"));
    assert!(matches!(h.resolve_key("primary+s").unwrap(), KeyResolution::Suspended { .. }));
    h.report_shell(clear());
    assert!(matches!(h.resolve_key("primary+s").unwrap(), KeyResolution::Command { .. }));
    let env = h.invoke(agent_inv("doc.object.patch", json!({ "targets": ["@current"], "props": { "name": "After" } })));
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
}

fn agent_inv(command: &str, params: Value) -> Invocation {
    Invocation {
        command: command.into(),
        params,
        options: InvokeOptions::default(),
        origin: agent(&[Scope::Read, Scope::Edit, Scope::Ui]),
    }
}

/// Spins (bounded, 30 s, a 100× margin over the 20 ms poll) until `flag` is set.
fn until(flag: &AtomicBool) {
    let t = Instant::now();
    while !flag.load(Ordering::Acquire) {
        assert!(t.elapsed() < Duration::from_secs(30), "the agent caller never reached its wait");
        std::thread::yield_now();
    }
}

#[test]
fn an_agent_action_waits_for_the_user_and_then_runs() {
    let h = Arc::new(fixture());
    let before = rev(&h);
    h.report_shell(modal("gesture"));
    let waiting = Arc::new(AtomicBool::new(false));
    let (h2, w2) = (h.clone(), waiting.clone());
    let caller = std::thread::spawn(move || {
        let probe = move || {
            w2.store(true, Ordering::Release);
            false
        };
        h2.invoke_served_until(
            agent_inv("doc.object.patch", json!({ "targets": ["@current"], "props": { "name": "Moved" } })),
            &probe,
        )
        .0
    });
    until(&waiting);
    assert_eq!(rev(&h), before, "nothing ran while the user was busy");
    h.report_shell(clear());
    let env = caller.join().unwrap();
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
    assert_eq!(rev(&h), before + 1);
}

#[test]
fn an_agent_action_gives_up_with_ui_busy_and_changes_nothing() {
    let h = fixture();
    let before = rev(&h);
    h.set_wait_for_user(Duration::from_millis(50));
    h.report_shell(modal("dialog"));
    let env = h.invoke(agent_inv("doc.object.patch", json!({ "targets": ["@current"], "props": { "name": "X" } })));
    let e = env.error.unwrap();
    assert_eq!(e.code, "ui.busy");
    assert_eq!(e.details["blockedBy"]["kind"], "dialog");
    assert_eq!(rev(&h), before);
    // 0 fails at once; a batch is held the same way.
    h.set_wait_for_user(Duration::ZERO);
    let env = h.invoke(agent_inv("doc.object.patch", json!({ "targets": ["@current"], "props": { "name": "X" } })));
    assert_eq!(env.error.unwrap().code, "ui.busy");
    let req = serde_json::from_value(json!({ "label": "b", "steps": [{ "id": "app.state.get" }] })).unwrap();
    let env = h.batch(req, agent(&[Scope::Read]));
    assert_eq!(env.error.unwrap().code, "ui.busy");
}

#[test]
fn queries_user_calls_and_gestures_are_not_blocked() {
    let h = fixture();
    h.set_wait_for_user(Duration::ZERO);
    h.report_shell(modal("dialog"));
    // Queries run for agents; every user call runs.
    assert_eq!(h.invoke(agent_inv("app.state.get", json!({}))).status, Status::Ok);
    let st = ok(&h, "app.state.get", json!({}));
    assert_eq!(st["modal"]["kind"], "dialog", "app.state.get reports the modal state");
    ok(&h, "doc.object.patch", json!({ "targets": ["@current"], "props": { "name": "User edit" } }));
    // A focus-changing agent ui command under a user dialog is refused at once; under a gesture every agent command
    // that writes session state waits and then answers `ui.busy` (M4 C4.2, ruling D-12); queries still run.
    let env = h.invoke(agent_inv("window.panel.set", json!({ "panelId": "history", "visible": true })));
    assert_eq!(env.error.unwrap().code, "ui.modalOpen");
    h.report_shell(modal("gesture"));
    let env = h.invoke(agent_inv("window.panel.set", json!({ "panelId": "history", "visible": true })));
    assert_eq!(env.error.map(|e| e.code), Some("ui.busy".to_owned()));
    assert_eq!(h.invoke(agent_inv("app.state.get", json!({}))).status, Status::Ok);
}

#[test]
fn a_cancelled_waiting_caller_is_answered_as_cancelled_and_queues_nothing() {
    let h = fixture();
    let before = rev(&h);
    h.report_shell(modal("dialog"));
    let polls = AtomicUsize::new(0);
    let probe = || polls.fetch_add(1, Ordering::AcqRel) >= 2;
    let (env, _) = h.invoke_served_until(
        agent_inv("doc.object.patch", json!({ "targets": ["@current"], "props": { "name": "X" } })),
        &probe,
    );
    let e = env.error.unwrap();
    assert_eq!(e.code, "command.timeout");
    assert_eq!(e.details["reason"], "cancelled");
    assert_eq!(rev(&h), before);
}

#[test]
fn keys_resolve_through_the_keymap() {
    let h = fixture();
    match h.resolve_key("primary+s").unwrap() {
        KeyResolution::Command { command_id, keys, enabled, .. } => {
            assert_eq!((command_id.as_str(), keys.as_str(), enabled), ("project.save", "Primary+S", true));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(h.resolve_key("F9").unwrap(), KeyResolution::Unbound { .. }));
    assert_eq!(h.resolve_key("Hyper+Q").unwrap_err().code, "keymap.invalidSequence");
    // A user binding wins at once (the dispatch reads the accepted keymap).
    // Round 2 (C41-R2-09; declared change): a key bound to a command that cannot run now answers that command's
    // `command.disabled` with the reason, no longer a success with `enabled: false`.
    ok(&h, "keymap.set", json!({ "commandId": "edit.undo", "keys": "F9" }));
    let e = h.resolve_key("F9").unwrap_err();
    assert_eq!((e.code.as_str(), e.details["commandId"].as_str()), ("command.disabled", Some("edit.undo")));
    assert_eq!(e.details["reason"], "there is nothing to undo");
    // A toggle the binding leaves open gets its next state.
    let fs = if cfg!(target_os = "macos") { "Primary+Ctrl+F" } else { "F11" };
    match h.resolve_key(fs).unwrap() {
        KeyResolution::Command { command_id, params, dialog, .. } => {
            assert_eq!(command_id, "window.fullScreen.set");
            assert_eq!(params, json!({ "enabled": true }));
            assert!(!dialog);
        }
        other => panic!("{other:?}"),
    }
    // A panel toggle: hidden → show.
    match h.resolve_key("Primary+Shift+H").unwrap() {
        KeyResolution::Command { params, .. } => assert_eq!(params, json!({ "panelId": "history", "visible": true })),
        other => panic!("{other:?}"),
    }
    // Missing required parameters: the GUI opens a dialog (01 §0).
    match h.resolve_key("Primary+Shift+S").unwrap() {
        KeyResolution::Command { command_id, dialog, .. } => {
            assert_eq!((command_id.as_str(), dialog), ("project.saveAs", true))
        }
        other => panic!("{other:?}"),
    }
    // A text field keeps unmodified keys; shortcuts with Primary still work; a dialog suspends everything.
    h.report_shell(ShellReport { text_focus: Some(true), ..Default::default() });
    assert!(
        matches!(h.resolve_key("Delete").unwrap(), KeyResolution::Suspended { ref reason, .. } if reason == "textField")
    );
    assert!(matches!(h.resolve_key("Primary+S").unwrap(), KeyResolution::Command { .. }));
    h.report_shell(ShellReport { text_focus: Some(false), ..modal("dialog") });
    assert!(
        matches!(h.resolve_key("Primary+S").unwrap(), KeyResolution::Suspended { ref reason, .. } if reason == "modal")
    );
}

#[test]
fn the_ui_tree_is_virtual_headless_and_reported_when_attached() {
    let h = fixture();
    let t = ok(&h, "inspect.ui.tree", json!({}));
    assert_eq!(t["source"], "virtual");
    let panels: Vec<&Value> =
        t["root"]["children"].as_array().unwrap().iter().filter(|n| n["role"] == "panel").collect();
    assert_eq!(panels.len(), 21);
    let camera = panels.iter().find(|p| p["id"] == "cameraView").unwrap();
    assert_eq!(camera["view"]["type"], "camera");
    let shallow = ok(&h, "inspect.ui.tree", json!({ "depth": 1 }));
    assert!(shallow["root"]["children"][0].get("children").is_none());
    assert_eq!(err(&h, "inspect.ui.tree", json!({ "target": "nope" })).code, "window.panelNotFound");
    // Attached: the reported tree, with the freshness wait keyed to broadcast events.
    assert!(h.await_rendered(Duration::from_millis(1)), "headless never waits");
    h.report_shell(ShellReport { attached: Some(true), ..Default::default() });
    ok(&h, "window.panel.set", json!({ "panelId": "history", "visible": true }));
    assert!(!h.await_rendered(Duration::from_millis(30)), "nothing rendered the latest events yet");
    let seq = h.events_broadcast();
    h.report_shell(ShellReport {
        ui_tree: Some(json!({ "role": "window", "id": "main", "children": [{ "role": "panel", "id": "history", "label": "History" }] })),
        rendered_seq: Some(seq),
        ..Default::default()
    });
    assert!(h.await_rendered(Duration::from_secs(5)));
    let t = ok(&h, "inspect.ui.tree", json!({ "target": "history" }));
    assert_eq!((t["source"].as_str(), t["root"]["label"].as_str()), (Some("gui"), Some("History")));
}

#[test]
fn a_shown_message_report_emits_ui_message_shown_and_sensors_feed_the_status_bar() {
    let h = fixture();
    let events = h.subscribe();
    h.report_shell(ShellReport {
        description: Some("Undo the last step".into()),
        pointer: Some("x 1.0 f, y 2.0 f".into()),
        message_shown: Some(json!({ "messageId": "project.saveFailed", "severity": "error", "text": "Save failed" })),
        ..Default::default()
    });
    let sb = ok(&h, "ui.statusBar.get", json!({}));
    assert_eq!(sb["description"], "Undo the last step");
    assert_eq!(sb["pointer"], "x 1.0 f, y 2.0 f");
    let shown: Vec<_> = events.try_iter().filter(|e| e.name == "ui.message.shown").collect();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].payload["source"], "user");
    assert_eq!(shown[0].payload["suppressed"], false);
}

#[test]
fn a_tree_is_fresh_only_with_its_own_sequence() {
    let h = fixture();
    h.report_shell(wire(r#"{"attached":true}"#));
    ok(&h, "prefs.set", json!({ "values": { "interface.theme": "dark" } }));
    let now = h.events_broadcast();
    let label = |h: &tt_executor::ExecutorHandle| {
        ok(h, "inspect.ui.tree", json!({}))["root"]["label"].as_str().map(str::to_owned)
    };
    // From the future: refused, nothing stored.
    h.report_shell(wire(&format!(r#"{{"uiTree":{{"id":"main","label":"future"}},"renderedSeq":{}}}"#, now + 5)));
    assert!(!h.await_rendered(Duration::ZERO));
    assert_ne!(label(&h).as_deref(), Some("future"));
    // Current: fresh.
    h.report_shell(wire(&format!(r#"{{"uiTree":{{"id":"main","label":"current"}},"renderedSeq":{now}}}"#)));
    assert!(h.await_rendered(Duration::ZERO));
    assert_eq!(label(&h).as_deref(), Some("current"));
    // Older: does not replace.
    h.report_shell(wire(r#"{"uiTree":{"id":"main","label":"older"},"renderedSeq":0}"#));
    assert_eq!(label(&h).as_deref(), Some("current"));
    assert!(h.await_rendered(Duration::ZERO));
    // A sequence without a tree vouches for nothing; a tree without a sequence is kept but never fresh.
    h.report_shell(wire(r#"{"uiTree":{"id":"main","label":"unvouched"}}"#));
    assert_eq!(label(&h).as_deref(), Some("unvouched"));
    assert!(!h.await_rendered(Duration::ZERO));
    h.report_shell(wire(&format!(r#"{{"renderedSeq":{now}}}"#)));
    assert!(!h.await_rendered(Duration::ZERO));
}

#[test]
fn availability_is_the_pipelines_own_answer_for_every_surface() {
    let none = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    let refs = |ids: &[(&str, Value)]| -> Vec<(String, serde_json::Map<String, Value>)> {
        ids.iter().map(|(c, p)| ((*c).to_owned(), p.as_object().cloned().unwrap_or_default())).collect()
    };
    let a = none
        .availability(refs(&[
            ("edit.undo", json!({})),
            ("project.create", json!({})),
            ("ui.statusBar.set", json!({})),
            ("window.panel.set", json!({ "panelId": "timeline" })),
            ("draw.feather.apply", json!({})),
        ]))
        .unwrap();
    assert!(!a[0].enabled && a[0].reason.is_some(), "no project: nothing to undo {:?}", a[0]);
    assert!(a[1].enabled, "{:?}", a[1]);
    assert_eq!((a[2].checked, a[2].params.clone()), (Some(true), json!({ "visible": false })), "a toggle's check mark");
    assert_eq!(a[3].checked, Some(true), "the timeline panel is shown");
    assert_eq!(a[4].reason.as_deref(), Some("no project is open"), "the pipeline's order: project first");
    // After an edit, Undo is enabled and labelled with the step; the key dispatch reads the same answer.
    let h = fixture();
    ok(&h, "doc.object.patch", json!({ "targets": ["@current"], "props": { "name": "Renamed" } }));
    let later = &h.availability(refs(&[("draw.feather.apply", json!({}))])).unwrap()[0];
    assert!(!later.enabled && later.reason.as_deref().is_some_and(|r| r.contains("arrives with")), "{later:?}");
    let undo = &h.availability(refs(&[("edit.undo", json!({}))])).unwrap()[0];
    assert!(undo.enabled && undo.label.as_deref().is_some_and(|l| l.starts_with("Undo ")), "{undo:?}");
    match h.resolve_key("primary+z").unwrap() {
        KeyResolution::Command { command_id, enabled, .. } => {
            assert_eq!((command_id.as_str(), enabled), ("edit.undo", true))
        }
        other => panic!("{other:?}"),
    }
    // The key dispatch refuses what the menu disables, with the menu's reason (C41-R2-09).
    let e = none.resolve_key("primary+z").unwrap_err();
    assert_eq!(e.code, "command.disabled");
    assert_eq!(e.details["reason"].as_str(), a[0].reason.as_deref());
}

/// C41-R2-08: the selection commands are enabled exactly when their handler would act: without `objectIds` they need
/// a selection in their context. Both directions, availability against a dry run of the same reference.
#[test]
fn selection_commands_are_available_exactly_when_they_run() {
    const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
    let refs = |ids: &[(&str, Value)]| -> Vec<(String, serde_json::Map<String, Value>)> {
        ids.iter().map(|(c, p)| ((*c).to_owned(), p.as_object().cloned().unwrap_or_default())).collect()
    };
    let h = fixture();
    let ids = ["edit.copy", "edit.cut", "edit.cutSpecial", "edit.delete"];
    let check = |selected: bool| {
        for id in ids {
            let params = json!({ "context": "staging" });
            let a = h.availability(refs(&[(id, params.clone())])).unwrap().remove(0);
            let e = with(&h, id, params, InvokeOptions { dry_run: true, ..Default::default() });
            assert_eq!(a.enabled, selected, "{id}: {a:?}");
            assert_eq!(e.is_ok(), selected, "{id}: {e:?}");
            if !selected {
                assert_eq!(e.error.unwrap().code, "command.disabled");
                assert!(a.reason.is_some_and(|r| r.contains("nothing is selected")));
            }
        }
        // An explicit target needs no selection.
        let a = h.availability(refs(&[("edit.copy", json!({ "context": "staging", "objectIds": [BOB] }))])).unwrap();
        assert!(a[0].enabled, "{:?}", a[0]);
    };
    check(false);
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
    check(true);
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [] }));
    check(false);
}

/// C41-R3-06: from the moment `window.panel.set {floating: true}` is accepted, a tree without that float's node is
/// unvouched (the expected windows come from the kernel's arrangement), so a wait answers stale and `renderedSeq` is
/// null; once the float's node is there, the tree is fresh again.
#[test]
fn a_float_the_kernel_shows_is_expected_from_the_moment_it_is_accepted() {
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    let report = |tree: Value| {
        let seq = h.events_broadcast();
        h.report_shell(
            serde_json::from_value(json!({ "attached": true, "uiTree": tree, "renderedSeq": seq })).unwrap(),
        );
    };
    report(json!({ "id": "main" }));
    assert!(h.await_rendered(Duration::ZERO));
    ok(&h, "window.panel.set", json!({ "panelId": "history", "floating": true, "visible": true }));
    report(json!({ "id": "main", "children": [] }));
    assert!(!h.await_rendered(Duration::ZERO), "the float the kernel shows has not reported");
    assert_eq!(ok(&h, "inspect.ui.tree", json!({}))["renderedSeq"], Value::Null);
    report(json!({ "id": "main", "children": [{ "id": "float-history" }] }));
    assert!(h.await_rendered(Duration::ZERO));
    // Docked again: the float is no longer expected.
    ok(&h, "window.panel.set", json!({ "panelId": "history", "floating": false }));
    report(json!({ "id": "main" }));
    assert!(h.await_rendered(Duration::ZERO));
}
