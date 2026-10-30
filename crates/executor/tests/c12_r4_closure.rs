//! Scoped C1.2 closure regressions introduced by the round-3 fixes (C12-R4-01/02).
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent review regressions")]

mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

fn fresh() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(0xc124_2026).unwrap(), None, false).is_ok());
    h
}

#[test]
fn immediately_querying_a_staged_checkpoint_is_empty() {
    let h = fresh();
    let stroke = first(&h, "stroke");
    let e = batch(
        &h,
        json!({"label":"closure immediate staged checkpoint","steps":[
            {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"closure":891}}}},
            {"id":"checkpoint.create","params":{"name":"closure-staged"}},
            {"id":"doc.diff","params":{"sinceCheckpoint":"closure-staged"}}
        ]}),
    );
    assert!(e.is_ok(), "{e:?}");
    let steps = e.steps.unwrap();
    let diff = steps[2].result.as_ref().unwrap();
    assert_eq!(diff["fromRev"], diff["toRev"]);
    assert_eq!(diff["changes"], json!([]), "C12-R4-01: no content changed after the checkpoint");
}

#[test]
fn nested_checkpoints_do_not_reinvalidate_unchanged_drawing_content() {
    let h = fresh();
    let stroke = first(&h, "stroke");
    let e = batch(
        &h,
        json!({"label":"closure nested checkpoint sequence","steps":[
            {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"closure":384}}}},
            {"id":"checkpoint.create","params":{"name":"outer"}},
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"later":283}}}},
            {"id":"checkpoint.create","params":{"name":"inner"}},
            {"id":"doc.diff","params":{"sinceCheckpoint":"outer","types":["drawing"]}},
            {"id":"doc.diff","params":{"sinceCheckpoint":"inner"}}
        ]}),
    );
    assert!(e.is_ok(), "{e:?}");
    // The installed state is coherent; the discrepancy exists only in the still-staged query path.
    assert_eq!(ok(&h, "doc.diff", json!({"sinceCheckpoint":"inner"}))["changes"], json!([]));
    assert_eq!(ok(&h, "doc.diff", json!({"sinceCheckpoint":"outer","types":["drawing"]}))["changes"], json!([]));
    let steps = e.steps.unwrap();
    assert_eq!(steps[4].result.as_ref().unwrap()["changes"], json!([]), "C12-R4-01: only the scene changed");
    assert_eq!(steps[5].result.as_ref().unwrap()["changes"], json!([]), "C12-R4-01: nothing changed");
}

fn accepts_result(id: &str, value: Value) {
    let schema = tt_executor::registry().unwrap().get(id).unwrap().result_schema();
    let violations = tt_registry::schema_violations(&schema, &value, 10).unwrap();
    assert!(violations.is_empty(), "C12-R4-02: {id} rejects an owning-section value: {violations:?}");
}

#[test]
fn configured_publishing_services_can_be_connected() {
    // 09 §4.9 and §12.3: sign-in status is boolean; account.close's false does not type service.list.
    accepts_result(
        "export.publish.service.list",
        json!({"services":[{
            "id":"closure-service","name":"Review Service","formats":["mp4"],
            "limits":{"minSec":0.5,"maxSec":90,"maxBytes":1000000},
            "fields":[{"name":"title","required":true}],"visibilities":["private","public"],"connected":true
        }]}),
    );
}

#[test]
fn storyline_playback_changed_can_announce_a_stop() {
    // 08's event explicitly covers started/stopped; play.start's true is not the event's type.
    accepts_result("storyline.play.changed", json!({"playing":false,"globalFrame":17,"sceneId":"closure-scene"}));
}
