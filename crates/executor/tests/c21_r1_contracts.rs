//! C2.1 review contract regressions through the serialized executor.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use tt_executor::Status;

const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

#[test]
fn object_setters_accept_common_cell_addressing() {
    let h = fixture();
    let cases = [
        (
            "draw.stroke.set",
            json!({"elementId":BOB,"frame":1,"strokeId":"st_01K742SG0R61SYK9GG0HGYVNBT","thickness":3}),
        ),
        ("draw.text.set", json!({"elementId":BOB,"frame":1,"textId":"nd_01K742SG0WFAQJFKAZKV4CD56C","text":"review"})),
        (
            "draw.textFormat.set",
            json!({"elementId":BOB,"frame":1,"textId":"nd_01K742SG0WFAQJFKAZKV4CD56C","bold":true}),
        ),
    ];
    let mut failures = Vec::new();
    for (id, p) in cases {
        let e = run(&h, id, p);
        if e.status != Status::Ok {
            failures.push(format!("{id}: {:?}", e.error));
        }
    }
    assert!(failures.is_empty(), "02 13.1(4): {failures:?}");
}

#[test]
fn direct_outline_edit_discards_spine_when_pressure_also_changes() {
    let h = fixture();
    let made = ok(&h, "draw.stroke.add", json!({"drawingId":D,"points":[{"x":0,"y":0},{"x":20,"y":0}]}));
    let id = made["strokeIds"][0].as_str().unwrap();
    let key = format!("{D}/{id}");
    // Orchestrator ruling on C21-R1-04: one patch editing both the outline and the spine is refused with a typed
    // error naming both pointers; an outline edit alone discards the spine.
    let e = run(
        &h,
        "doc.object.patch",
        json!({"targets":[key],"ops":[
            {"op":"replace","path":"/points/0/0","value":1000},
            {"op":"replace","path":"/spine/pressure","value":[[0,0.2],[1,0.2]]}
        ]}),
    );
    assert_eq!(e.status, Status::Error, "{e:?}");
    assert_eq!(e.error.as_ref().unwrap().details["pointers"], json!(["/points", "/spine"]));
    ok(&h, "doc.object.patch", json!({"targets":[key],"ops":[{"op":"replace","path":"/points/0/0","value":1000}]}));
    let stroke = get(&h, &key);
    assert!(stroke["spine"].is_null(), "02 3.2: directly edited outline is authoritative: {stroke}");
}

#[test]
fn generic_patch_enforces_one_paragraph_per_line() {
    let h = fixture();
    let made = ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":"one\ntwo"}));
    let id = made["textId"].as_str().unwrap();
    let key = format!("{D}/{id}");
    let e =
        run(&h, "doc.object.patch", json!({"targets":[key],"ops":[{"op":"replace","path":"/paragraphs","value":[]}]}));
    // Either reject malformed content atomically, or normalize it to valid paragraphs.
    if e.status == Status::Ok {
        assert_eq!(get(&h, &key)["paragraphs"].as_array().unwrap().len(), 2, "02 3.7 / 13.6");
    }
}

#[test]
fn content_changes_emit_drawing_event_including_undo_redo() {
    let h = fixture();
    let rx = h.subscribe();
    let mut failures = Vec::new();
    for (id, p) in [
        ("draw.pivot.set", json!({"drawingId":D,"point":{"x":5,"y":6}})),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
    ] {
        ok(&h, id, p);
        let events: Vec<_> = rx.try_iter().collect();
        if !events.iter().any(|e| e.name == "draw.drawing.changed" && e.payload["drawingId"] == D) {
            failures.push(format!("{id}: {:?}", events.iter().map(|e| &e.name).collect::<Vec<_>>()));
        }
    }
    assert!(failures.is_empty(), "02 13.6 requires a drawing event on every content change: {failures:?}");
}

#[test]
fn edit_results_report_the_resolved_drawing() {
    let h = fixture();
    let r = ok(&h, "draw.stroke.set", json!({"drawingId":D,"strokeId":"st_01K742SG0R61SYK9GG0HGYVNBT","thickness":3}));
    assert_eq!(r["drawingId"], D, "02 13.1(11): edit result includes drawingId and rev");
}

#[test]
fn decomposed_unicode_text_is_accepted_and_ranges_follow_normalization() {
    let h = fixture();
    let created = run(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":"e\u{301}"}));
    let base = ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":"e"}));
    let inserted =
        run(&h, "draw.text.set", json!({"drawingId":D,"textId":base["textId"],"insert":{"at":1,"text":"\u{301}"}}));
    assert!(
        created.is_ok() && inserted.is_ok(),
        "02 3.7 / 10 1.1 NFC text must keep valid codepoint ranges; add {:?}, insert {:?}",
        created.error,
        inserted.error
    );
}
