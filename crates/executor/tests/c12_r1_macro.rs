//! Independent attribute-macro boundary probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
use serde_json::{Value, json};
use tt_registry::{CommandError, Registry, command};

#[command(id="app.state.get",kind=query,label="Review A",scopes=[read],example(title="valid",params="{}"))]
fn review_a(_cx: &mut (), _params: Value) -> Result<Value, CommandError> {
    Ok(json!({}))
}

#[command(id="app.state.get",kind=query,label="Review B",scopes=[read],example(title="valid",params="{}"))]
fn review_b(_cx: &mut (), _params: Value) -> Result<Value, CommandError> {
    Ok(json!({}))
}

#[command(id="app.state.get",kind=action,label="Wrong kind",scopes=[read],example(title="valid",params="{}"))]
fn wrong_kind(_cx: &mut (), _params: Value) -> Result<Value, CommandError> {
    Ok(json!({}))
}

#[test]
fn duplicate_ids_and_inventory_kind_drift_are_rejected() {
    assert!(Registry::assemble(&[&REVIEW_A, &REVIEW_B]).is_err());
    assert!(Registry::assemble(&[&WRONG_KIND]).is_err());
}

#[test]
fn missing_example_is_rejected_at_compile_time() {
    trybuild::TestCases::new().compile_fail("tests/c12-r1-ui/*.rs");
}
