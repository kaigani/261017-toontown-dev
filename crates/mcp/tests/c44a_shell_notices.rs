//! Ruling D-57: the host-only `shell.*` notices (the Activity panel's `shell.agentActivity`, the sessions strip's
//! `shell.agentSession`, Regenerate token's `shell.agentToken`, the indicator's `shell.agentActing`) never reach an MCP
//! client: neither as an event in `app://events`, nor in a log notification, nor in anything else the client receives.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod support;
use serde_json::{Value, json};
use std::time::Duration;
use support::*;

#[test]
fn shell_notices_never_reach_an_mcp_client() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    c.request("resources/subscribe", json!({"uri": "app://events"}));
    // Every event as a log notification too.
    c.request("logging/setLevel", json!({"level": "debug"}));
    let rx = h.subscribe();
    // An agent call: recorded (shell.agentActivity) and announced as acting (shell.agentActing).
    let r = c.run("draw.pivot.set", json!({"drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW", "point": {"x": 71, "y": 28}}));
    assert_eq!(r["status"], "ok", "{r}");
    // The person's grant (shell.agentSession) and token regeneration (shell.agentToken).
    let status = h.invoke(tt_executor::Invocation::user("agent.server.status", json!({})));
    let sid = status.result.unwrap()["sessions"][0]["sessionId"].as_str().unwrap().to_owned();
    let g = h.invoke(tt_executor::Invocation::user(
        "agent.session.grant",
        json!({"sessionId": sid, "policy": "autonomous"}),
    ));
    assert!(g.is_ok(), "{g:?}");
    let t = h.invoke(tt_executor::Invocation::user("agent.token.regenerate", json!({})));
    assert!(t.is_ok(), "{t:?}");
    c.drain(Duration::from_millis(400));
    // The notices were emitted (the window gets them) ...
    let names: Vec<String> = rx.try_iter().map(|e| e.name).collect();
    for n in ["shell.agentActivity", "shell.agentSession", "shell.agentToken"] {
        assert!(names.iter().any(|x| x == n), "{n} not emitted: {names:?}");
    }
    // ... and none reached the client.
    let resource = c.request("resources/read", json!({"uri": "app://events?after=0"}));
    let body: Value = serde_json::from_str(resource["contents"][0]["text"].as_str().unwrap()).unwrap();
    let types: Vec<&str> = body["events"].as_array().unwrap().iter().filter_map(|e| e["type"].as_str()).collect();
    assert!(!types.is_empty(), "{body}");
    assert!(types.iter().all(|t| !t.starts_with("shell.")), "{types:?}");
    let all = serde_json::to_string(&c.notes_of("notifications/message")).unwrap();
    assert!(!all.contains("\"shell."), "{all}");
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
