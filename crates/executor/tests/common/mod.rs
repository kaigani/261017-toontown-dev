//! Shared helpers of the executor tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "test helpers")]

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::{Value, json};
use tt_executor::{BatchRequest, Envelope, ExecutorConfig, ExecutorHandle, Invocation, InvokeOptions, Origin, Status};
use tt_registry::Scope;

/// An executor with the fixture project open (sample document, seed 1).
pub fn fixture() -> ExecutorHandle {
    fixture_with(ExecutorConfig { budget: Duration::from_secs(60) })
}

pub fn fixture_with(config: ExecutorConfig) -> ExecutorHandle {
    let h = ExecutorHandle::spawn(config).unwrap();
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let env = h.open_document(doc, None, false);
    assert!(env.is_ok(), "{env:?}");
    h
}

pub fn run(h: &ExecutorHandle, id: &str, params: Value) -> Envelope {
    h.invoke(Invocation::user(id, params))
}

pub fn ok(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    let env = run(h, id, params.clone());
    assert_eq!(env.status, Status::Ok, "{id} {params}: {:?}", env.error);
    env.result.clone().unwrap_or(Value::Null)
}

pub fn err(h: &ExecutorHandle, id: &str, params: Value) -> tt_registry::CommandError {
    let env = run(h, id, params.clone());
    assert_eq!(env.status, Status::Error, "{id} {params} unexpectedly succeeded: {:?}", env.result);
    env.error.unwrap()
}

pub fn with(h: &ExecutorHandle, id: &str, params: Value, options: InvokeOptions) -> Envelope {
    h.invoke(Invocation { command: id.to_owned(), params, options, origin: Origin::User })
}

pub fn batch(h: &ExecutorHandle, req: Value) -> Envelope {
    let req: BatchRequest = serde_json::from_value(req).unwrap();
    h.batch(req, Origin::User)
}

pub fn agent(scopes: &[Scope]) -> Origin {
    Origin::Agent {
        session: "s1".into(),
        name: "Test agent".into(),
        scopes: scopes.iter().copied().collect::<BTreeSet<_>>(),
    }
}

pub fn rev(h: &ExecutorHandle) -> u64 {
    h.snapshot().unwrap().unwrap().document_rev()
}

/// `rev` of an object in the live document.
pub fn obj_rev(h: &ExecutorHandle, key: &str) -> u64 {
    h.snapshot().unwrap().unwrap().locate(key).unwrap().rev
}

pub fn get(h: &ExecutorHandle, r: &str) -> Value {
    ok(h, "doc.object.get", json!({ "ref": r }))["object"].clone()
}

/// The first object of a type, by `doc.object.find`.
pub fn first(h: &ExecutorHandle, ty: &str) -> String {
    ok(h, "doc.object.find", json!({ "type": ty, "limit": 1 }))["items"][0]["id"].as_str().unwrap().to_owned()
}
