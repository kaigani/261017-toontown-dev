//! C11-R5-01: every declared frame-bearing write must obey the R4-01 patch-boundary limit.
//! These are field writes, not the deferred C1.2 whole-object replacement policy.

use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};

fn write(kind: &'static str, path: &str, value: Value) -> PatchOp {
    PatchOp { op: kind, path: path.into(), from: None, value: Some(value), old_value: None, range: None, elided: false }
}

fn require_limit(before: &Value, patch: &[PatchOp], pointer: &str) -> Result<(), serde_json::Error> {
    let bytes = serde_json::to_vec(before)?;
    let result = change::apply(before, patch);
    assert_eq!(serde_json::to_vec(before)?, bytes, "a failed batch must leave the input byte-identical");
    // Do not dump a 100,001-character accepted document in a regression failure.
    let error = result.err();
    assert!(
        matches!(&error, Some(e) if e.code() == "doc.limitExceeded"
            && e.details()["pointer"] == pointer && e.details()["maxFrames"] == 100_000),
        "C11-R5-01: oversized frame-bearing write at {pointer} must return doc.limitExceeded; error={error:?}"
    );
    Ok(())
}

#[test]
fn whole_mouth_string_writes_obey_scene_limit() -> Result<(), serde_json::Error> {
    let before = json!({"mouthTrack": {"mouths": "AB"}});
    for kind in ["add", "replace"] {
        let at = write(kind, "/mouthTrack/mouths", json!("G".repeat(100_000)));
        assert!(change::apply(&before, &[at]).is_ok(), "exactly 100,000 mouth frames is valid");
        let over = write(kind, "/mouthTrack/mouths", json!("G".repeat(100_001)));
        require_limit(&before, &[over], "/mouthTrack/mouths")?;
    }
    Ok(())
}

#[test]
fn nested_exposure_count_write_obeys_total_limit() -> Result<(), serde_json::Error> {
    let before = json!({"exposure": [["dr_first", 49_999], ["dr_second", 50_000]]});
    assert!(change::apply(&before, &[write("replace", "/exposure/0/1", json!(50_000))]).is_ok());
    require_limit(&before, &[write("replace", "/exposure/0/1", json!(50_001))], "/exposure/0/1")
}

#[test]
fn exposure_append_batch_cannot_cross_limit() -> Result<(), serde_json::Error> {
    let before = json!({"exposure": [["dr_first", 99_999]], "marker": 0});
    let first = write("add", "/exposure/-", json!(["dr_second", 1]));
    assert!(change::apply(&before, std::slice::from_ref(&first)).is_ok());
    let patch = [write("replace", "/marker", json!(1)), first, write("add", "/exposure/-", json!(["dr_third", 1]))];
    require_limit(&before, &patch, "/exposure/-")
}

#[test]
fn copy_and_move_into_exposure_obey_scene_limit() -> Result<(), serde_json::Error> {
    for kind in ["copy", "move"] {
        let mut patch = write(kind, "/exposure", Value::Null);
        patch.value = None;
        patch.from = Some("/meta/saved".into());
        let at = json!({"exposure": [["dr_original", 1]], "meta": {"saved": [["dr_new", 100_000]]}});
        assert!(change::apply(&at, std::slice::from_ref(&patch)).is_ok());
        // Opaque metadata may hold this value. Writing it into a declared exposure must validate the destination.
        let over = json!({"exposure": [["dr_original", 1]], "meta": {"saved": [["dr_new", 100_001]]}});
        require_limit(&over, &[patch], "/exposure")?;
    }
    Ok(())
}
