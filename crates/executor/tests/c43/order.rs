//! SI-157 (11 §6.4, lines 586-587): the window holds a gesture's modal state until the gesture's commit has answered.
//! Its commit of an agent gesture it played (arbitrated when the host routed it) is not arbitrated again, so it runs
//! under the modal state it holds; the agent's own `pointer` call and its other changes still wait for that state and
//! fail `ui.busy` when the wait runs out. Without the rule the window's commit would wait for itself.

use std::time::Duration;

use serde_json::json;
use tt_executor::Origin;
use tt_executor::host::ShellReport;

use crate::common::*;

fn report(text: &str) -> ShellReport {
    serde_json::from_str(text).unwrap()
}

fn agent() -> Origin {
    Origin::Agent { session: "s1".into(), name: "test".into(), scopes: tt_registry::Scope::ALL.into_iter().collect() }
}

#[test]
fn si157_the_windows_commit_runs_under_the_gesture_it_holds_and_agent_calls_wait_for_it() {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": crate::camera::BOB, "frame": 1 }));
    h.set_wait_for_user(Duration::ZERO);
    h.report_shell(report(r#"{"modal":{"kind":"gesture","title":"brush"}}"#));
    let req = json!({ "view": "drawing", "tool": "brush",
                      "events": [{ "type": "down", "x": 700, "y": 300 }, { "type": "up", "x": 760, "y": 320 }] });
    // The window's commit of a played agent gesture runs while its modal state is held.
    let env = h.pointer_in_window(agent(), req.clone());
    assert!(env.is_ok(), "the window's commit is not arbitrated again: {env:?}");
    // The agent's own call and its other changes meet user priority.
    let env = h.pointer(agent(), req);
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("ui.busy"), "{env:?}");
    let env = h.invoke(tt_executor::Invocation {
        command: "draw.selection.set".into(),
        params: json!({ "by": "all" }),
        options: tt_executor::InvokeOptions::default(),
        origin: agent(),
    });
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("ui.busy"), "{env:?}");
    h.report_shell(report(r#"{"modal":null}"#));
}
