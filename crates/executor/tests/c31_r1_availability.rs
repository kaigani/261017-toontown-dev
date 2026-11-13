//! Owner-section applicability takes precedence over generic drawing-bank metadata capability.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use tt_executor::Status;
#[test]
fn image_column_drawing_lock_refuses_per_availability_table() {
    let h = fixture();
    let image = get(&h, "scene:\"Intro\"/element:\"Background\"");
    let drawing = image["exposure"][0][0].as_str().unwrap();
    let r = rev(&h);
    let e = run(&h, "element.drawingLock.set", json!({"drawingIds":[drawing],"locked":true}));
    assert_eq!(e.status, Status::Error, "04 §13.20: image clear/lock drawing is unavailable: {e:?}");
    assert_eq!(e.error.unwrap().code, "cell.notApplicable");
    assert_eq!(rev(&h), r);
}
