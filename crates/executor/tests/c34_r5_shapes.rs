//! Independent round-5 declaration and nested-cause checks.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{BatchRequest, Origin};

#[test]
fn activity_reason_is_declared_for_consumers() {
    let registry = tt_executor::registry().unwrap();
    let schema = registry.get("agent.activity.list").unwrap().result_schema();
    eprintln!("activity result schema: {schema}");
    assert!(
        schema["items"]["properties"]["errorReason"].is_object(),
        "optional errorReason must be discoverable with its type in the declared activity entry: {schema}"
    );
}

#[test]
fn cancellation_does_not_relabel_an_argument_error() {
    let h = fixture();
    let request: BatchRequest =
        serde_json::from_value(json!({"atomic":false,"stopOnError":false,"label":"r5 cause","steps":[
            {"id":"element.create","params":{"type":"not-a-type","sceneId":"scene:#1","name":"bad"}},
            {"id":"element.create","params":{"type":"peg","sceneId":"scene:#1","name":"later"}}
        ]}))
        .unwrap();
    tt_vector::work::hooks::reset_asks();
    let env = h.batch_until(request, Origin::User, &|| tt_vector::work::hooks::asks() >= 3);
    let first = &env.steps.as_ref().unwrap()[0];
    let error = first.error.as_ref().unwrap();
    assert_ne!(error.code, "command.timeout");
    assert_eq!(error.details["reason"], Value::Null);
    let outer = env.error.as_ref().unwrap();
    assert_eq!(outer.details["error"]["code"], error.code);
    assert_eq!(outer.details["error"]["details"]["reason"], Value::Null);
}
#[test]
fn activity_entries_read_old_and_new_reason_shapes() {
    let old = json!({"seq":1,"time":"2026-11-12T00:00:00Z","sessionId":"s","client":"review","tool":"batch","commandId":"batch","params":{},"status":"error","errorCode":"command.timeout","undoStepId":null,"changedCount":0,"durationMs":1});
    let entry: tt_executor::agents::ActivityEntry = serde_json::from_value(old.clone()).unwrap();
    assert!(entry.error_reason.is_none());
    assert!(serde_json::to_value(entry).unwrap().get("errorReason").is_none());
    let mut new = old;
    new["errorReason"] = json!("cancelled");
    let entry: tt_executor::agents::ActivityEntry = serde_json::from_value(new).unwrap();
    assert_eq!(entry.error_reason.as_deref(), Some("cancelled"));
    assert_eq!(serde_json::to_value(entry).unwrap()["errorReason"], "cancelled");
}
