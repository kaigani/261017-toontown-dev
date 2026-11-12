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
    assert!(schema["items"]["properties"]["errorReason"].is_object(),
        "optional errorReason must be discoverable with its type in the declared activity entry: {schema}");
}

#[test]
fn cancellation_does_not_relabel_an_argument_error() {
    let h = fixture();
    let request: BatchRequest = serde_json::from_value(json!({"atomic":false,"stopOnError":false,"label":"r5 cause","steps":[
        {"id":"element.create","params":{"type":"not-a-type","sceneId":"scene:#1","name":"bad"}},
        {"id":"element.create","params":{"type":"peg","sceneId":"scene:#1","name":"later"}}
    ]})).unwrap();
    let env = h.batch_until(request, Origin::User, &|| false);
    let first = &env.steps.as_ref().unwrap()[0];
    let error = first.error.as_ref().unwrap();
    assert_ne!(error.code, "command.timeout");
    assert_eq!(error.details["reason"], Value::Null);
}
