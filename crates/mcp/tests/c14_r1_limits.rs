//! Session limits must apply to batch scripts as well as protocol wrappers (11 §6.5).
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod support;
use serde_json::json;
use tt_executor::{Invocation, InvokeOptions, Origin};

#[test]
fn configured_batch_limit_applies_to_batch_script() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    let settings = h.invoke(Invocation {
        command: "agent.policy.defaults.set".into(),
        params: json!({ "maxBatchSteps": 1 }),
        options: InvokeOptions::default(),
        origin: Origin::User,
    });
    assert!(settings.is_ok());
    let req = serde_json::from_value(json!({ "label": "too many", "steps": [
        { "id": "app.info.get", "params": {} },
        { "id": "app.info.get", "params": {} }
    ] }))
    .unwrap();
    let outcome = tt_mcp::script::run(&h, &support::opts(&dir), req, None).unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(
        outcome.envelope.error.as_ref().map(|e| e.code.as_str()),
        Some("limit.exceeded"),
        "the script ignored the configured session limit: {:?}",
        outcome.envelope
    );
}

#[test]
fn saved_observe_policy_and_limits_survive_restart() {
    let dir = support::tmp();
    let user_dir = dir.join("user");
    let h = support::executor(&user_dir);
    let settings = h.invoke(Invocation {
        command: "agent.policy.defaults.set".into(),
        params: json!({ "policy": "observe", "maxBatchSteps": 1, "maxJobs": 1 }),
        options: InvokeOptions::default(),
        origin: Origin::User,
    });
    assert!(settings.is_ok());
    drop(h);
    let h = support::executor(&user_dir);
    let session = h.connect_agent(tt_executor::agents::AgentConnect::default()).unwrap();
    let status = h.agent_status(&session.session_id).unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(
        (session.policy, status.limits.max_batch_steps, status.limits.max_jobs),
        (tt_executor::agents::Policy::Observe, 1, 1),
        "saved policy and resource limits were lost on restart"
    );
}
