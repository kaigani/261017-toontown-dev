//! Independent atomic work-account refusal: both copies of the error keep its cancellation cause.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{BatchRequest, Origin};
use tt_vector::work::hooks as work;
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        work::set_spin_micros(0);
    }
}
#[test]
fn atomic_work_account_cancellation_labels_the_nested_error_as_well_as_the_step() {
    let _reset = Reset;
    let h = fixture();
    let before = rev(&h);
    let req: BatchRequest = serde_json::from_value(json!({"label":"atomic cancellation cause","atomic":true,"steps":[
        {"id":"render.frame.run","params":{"frame":1,"scale":0.25}},
        {"id":"element.create","params":{"type":"peg","name":"MustNotCommit","sceneId":"scene:#1"}}
    ]}))
    .unwrap();
    work::set_spin_micros(20000);
    let start = Instant::now();
    let env = h.batch_until(req, Origin::User, &|| start.elapsed() > Duration::from_millis(60));
    work::set_spin_micros(0);
    assert_eq!(rev(&h), before);
    let outer = env.error.expect("atomic work was cancelled");
    assert_eq!(outer.code, "batch.stepFailed", "{outer:?}");
    let inner = &outer.details["error"];
    assert_eq!(inner["code"], "command.timeout", "{outer:?}");
    let steps = env.steps.unwrap();
    let step = steps[0].error.as_ref().unwrap();
    assert_eq!(step.details["reason"], "cancelled", "the step copy is labelled: {step:?}");
    assert_eq!(
        inner["details"]["reason"], "cancelled",
        "C34-R4-01: the atomic wrapper must preserve the same cancellation cause: {outer:?}"
    );
}
