//! Independent final-round ordered patch variants. Failing tests are retained findings.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use tt_executor::Status;
const B: &str = "scene:\"Intro\"/element:\"Bob\"";

#[test]
fn copy_reads_a_plain_source_written_immediately_before_it() {
    let h = fixture();
    ok(&h, "doc.object.patch", json!({"targets":[B],"props":{"meta":{"r3":"old-928"}}}));
    let r = run(
        &h,
        "doc.object.patch",
        json!({"targets":[B],"ops":[
            {"op":"replace","path":"/meta/r3","value":"new-928"},
            {"op":"copy","from":"/meta/r3","path":"/note"}
        ]}),
    );
    assert_eq!(r.status, Status::Ok, "{r:?}");
    assert_eq!(get(&h, B)["note"], "new-928", "RFC copy reads the preceding ordinary write");
}

#[test]
fn move_reads_a_plain_source_created_immediately_before_it() {
    let h = fixture();
    let r = run(
        &h,
        "doc.object.patch",
        json!({"targets":[B],"props":{"note":"props-928"},"ops":[
            {"op":"add","path":"/meta/freshR3","value":"moved-928"},
            {"op":"move","from":"/meta/freshR3","path":"/note"}
        ]}),
    );
    assert_eq!(r.status, Status::Ok, "RFC move reads the newly added source: {r:?}");
    assert_eq!(get(&h, B)["note"], "moved-928");
    assert!(get(&h, B)["meta"].get("freshR3").is_none());
}

#[test]
fn copy_of_a_source_removed_immediately_before_it_refuses_atomically() {
    let h = fixture();
    ok(&h, "doc.object.patch", json!({"targets":[B],"props":{"meta":{"removedR3":"stale-928"}}}));
    let before = get(&h, B);
    let revision = rev(&h);
    let r = run(
        &h,
        "doc.object.patch",
        json!({"targets":[B],"ops":[
            {"op":"remove","path":"/meta/removedR3"},
            {"op":"copy","from":"/meta/removedR3","path":"/note"}
        ]}),
    );
    assert_ne!(r.status, Status::Ok, "copy of a removed source must fail: {r:?}");
    assert_eq!(get(&h, B), before);
    assert_eq!(rev(&h), revision);
}

#[test]
fn mixed_orders_and_failed_tests_restore_everything_in_one_undo_step() {
    for delegated_first in [false, true] {
        for failing_at in [None, Some(1), Some(3)] {
            let h = fixture();
            let before = get(&h, B);
            let revision = rev(&h);
            let mut ops = if delegated_first {
                vec![
                    json!({"op":"replace","path":"/note","value":"after-928"}),
                    json!({"op":"add","path":"/meta/r3","value":928}),
                    json!({"op":"replace","path":"/locked","value":true}),
                ]
            } else {
                vec![
                    json!({"op":"add","path":"/meta/r3","value":928}),
                    json!({"op":"replace","path":"/locked","value":true}),
                    json!({"op":"replace","path":"/note","value":"after-928"}),
                ]
            };
            if let Some(at) = failing_at {
                ops.insert(at, json!({"op":"test","path":"/note","value":"impossible-928"}));
            }
            let r = run(&h, "doc.object.patch", json!({"targets":[B],"props":{"note":"props-928"},"ops":ops}));
            if failing_at.is_some() {
                assert_eq!(r.error.as_ref().unwrap().code, "doc.patchTestFailed");
                assert_eq!(get(&h, B), before);
                assert_eq!(rev(&h), revision);
            } else {
                assert_eq!(r.status, Status::Ok, "{r:?}");
                assert_eq!(rev(&h), revision + 1);
                ok(&h, "edit.undo", json!({}));
                let restored = get(&h, B);
                for field in ["note", "locked", "meta"] {
                    assert_eq!(restored[field], before[field], "{field}");
                }
            }
        }
    }
}
