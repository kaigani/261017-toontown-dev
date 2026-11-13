//! M4 C4.1: user priority for agent calls (01 §12.1 rule 8, 11 §6.4), key dispatch through the keymap (01 §8.4),
//! sensor reports and the UI tree (`reviews/M4-design-busy-arbitration.md`). Builder-owned.
//!
//! No assertion depends on a sleep fitting a bound: waiting callers are observed through their cancellation probe
//! (the wait polls it), and refusals are asserted on their outcome, not on elapsed time.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::host::{KeyResolution, ShellReport};
use tt_executor::shell::ModalReport;
use tt_executor::{Invocation, InvokeOptions, Status};
use tt_registry::Scope;

fn modal(kind: &str) -> ShellReport {
    ShellReport {
        modal: Some(Some(ModalReport { kind: kind.into(), title: "Preferences".into() })),
        ..Default::default()
    }
}

fn clear() -> ShellReport {
    ShellReport { modal: Some(None), ..Default::default() }
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
    // A focus-changing agent ui command under a user dialog is refused at once; under a gesture it runs.
    let env = h.invoke(agent_inv("window.panel.set", json!({ "panelId": "history", "visible": true })));
    assert_eq!(env.error.unwrap().code, "ui.modalOpen");
    h.report_shell(modal("gesture"));
    let env = h.invoke(agent_inv("window.panel.set", json!({ "panelId": "history", "visible": true })));
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
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
    ok(&h, "keymap.set", json!({ "commandId": "edit.undo", "keys": "F9" }));
    assert!(
        matches!(h.resolve_key("F9").unwrap(), KeyResolution::Command { ref command_id, .. } if command_id == "edit.undo")
    );
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
