//! C31-R2-04: an insertion cannot silently discard a note-only tail at the legal frame boundary.
//! Deliberately failing finding reproduction; no expected-panic or skip marker.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use tt_executor::Status;

#[test]
fn inserting_before_a_note_at_the_frame_cap_refuses_instead_of_deleting_the_note() {
    let h = fixture();
    let element = ok(&h, "element.create", json!({"type":"drawing","name":"Boundary731"}))["elementIds"][0].clone();
    ok(&h, "cell.note.set", json!({"elementId":element,"frame":100000,"text":"tail731"}));
    let before = get(&h, element.as_str().unwrap());
    let revision = rev(&h);
    let result = run(&h, "cell.range.add", json!({"elementIds":[element],"frame":1,"position":"before","count":1}));
    let after = get(&h, element.as_str().unwrap());
    assert_eq!(
        result.status,
        Status::Error,
        "SI-093 requires notes to follow inserted cells. The 100000-frame domain requires atomic refusal, not silent deletion: before={} after={} result={result:?}",
        before["cellNotes"],
        after["cellNotes"]
    );
    assert_eq!(rev(&h), revision);
    assert_eq!(after, before, "a refused insertion retains the boundary note");
}
