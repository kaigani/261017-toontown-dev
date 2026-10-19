//! C11-R4-01: R3-03 closure requires exposure bounds at the patch boundary, before expansion/splicing.

use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::value::FrameRange;

fn range(value: Value, old: Value, from: u32, to: u32) -> PatchOp {
    PatchOp {
        op: "replace",
        path: "/exposure".into(),
        from: None,
        value: Some(value),
        old_value: Some(old),
        range: Some(FrameRange { from, to }),
        elided: false,
    }
}

#[test]
fn exposure_range_rejects_total_above_scene_limit() {
    let before = json!({"exposure": [["dr_original", 4]]});
    let limit = tt_model::document::MAX_SCENE_LENGTH as u32;
    let control = range(json!([["dr_new", limit]]), json!([["dr_original", 4]]), 1, limit);
    assert!(change::apply(&before, &[control]).is_ok());

    let bad = range(json!([["dr_new", limit + 1]]), json!([["dr_original", 4]]), 1, limit + 1);
    let result = change::apply(&before, &[bad]);
    assert!(
        matches!(&result, Err(error) if matches!(error.code(), "arg.invalid" | "doc.limitExceeded") && error.details()["pointer"] == "/exposure"),
        "R3-03: patch boundary must reject exposure totals beyond the scene limit before expansion: {result:?}"
    );
}

#[test]
fn exposure_range_rejects_window_past_scene_limit() {
    let before = json!({"exposure": [["dr_original", 4]]});
    let limit = tt_model::document::MAX_SCENE_LENGTH as u32;
    let control = range(json!([["dr_new", 1]]), json!([[null, 1]]), limit, limit);
    assert!(change::apply(&before, &[control]).is_ok());

    // Each payload is only one frame; bounding individual run totals alone cannot bound this window.
    let bad = range(json!([["dr_new", 1]]), json!([[null, 1]]), limit + 1, limit + 1);
    let result = change::apply(&before, &[bad]);
    assert!(
        matches!(&result, Err(error) if matches!(error.code(), "arg.invalid" | "doc.limitExceeded") && error.details()["pointer"] == "/exposure"),
        "R3-03: patch boundary must reject exposure windows beyond the scene limit before resizing: {result:?}"
    );
}
