//! Independent C3.1 review. Failing tests are intentional reproductions, never ignored.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{InvokeOptions, Status};
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";

#[test]
fn timeline_colour_dry_run_restores_session() {
    let h = fixture();
    let before = ok(&h, "element.typeColour.get", json!({}));
    let e = with(
        &h,
        "element.typeColour.set",
        json!({"category":"pegs","colour":"#123456"}),
        InvokeOptions { dry_run: true, ..Default::default() },
    );
    assert_eq!(e.status, Status::Ok, "{e:?}");
    assert_eq!(ok(&h, "element.typeColour.get", json!({})), before, "dry run must write nothing");
}
#[test]
fn timeline_colour_failing_atomic_batch_restores_session() {
    let h = fixture();
    let before = ok(&h, "element.typeColour.get", json!({}));
    let e = batch(
        &h,
        json!({"label":"Review rollback","atomic":true,"steps":[
            {"id":"element.typeColour.set","params":{"category":"pegs","colour":"#123456"}},
            {"id":"element.rename","params":{"elementId":BOB,"name":"bad.name"}}
        ]}),
    );
    assert_ne!(e.status, Status::Ok, "{e:?}");
    assert_eq!(ok(&h, "element.typeColour.get", json!({})), before, "failed atomic group leaked TimelineState");
}

#[test]
fn fill_target_above_spec_maximum_refuses() {
    let h = fixture();
    let e = run(&h, "cell.fill.apply", json!({"elementIds":[BOB],"frame":1,"toFrame":10000}));
    assert_eq!(
        e.error.as_ref().map(|e| e.code.as_str()),
        Some("cell.frameOutOfRange"),
        "04 §13.5 and §17.6 cap fill targets at 9999: {e:?}"
    );
}
#[test]
fn delegated_hide_matches_command_current_position_and_event() {
    let h = fixture();
    ok(&h, "element.current.set", json!({"elementId":BOB}));
    let rx = h.subscribe();
    ok(&h, "doc.object.patch", json!({"targets":[BOB],"props":{"visibleInXsheet":false}}));
    let current = ok(&h, "cell.selection.get", json!({}))["currentElementId"].clone();
    assert_ne!(current, get(&h, BOB)["id"], "04 §10.2 / AG-02 must advance current element");
    assert_eq!(
        rx.try_iter().filter(|e| e.name == "element.state.changed").count(),
        1,
        "owning command event exactly once"
    );
}
#[test]
fn delegated_note_emits_owning_event() {
    let h = fixture();
    let rx = h.subscribe();
    ok(&h, "doc.object.patch", json!({"targets":[BOB],"props":{"note":"review"}}));
    assert_eq!(
        rx.try_iter().filter(|e| e.name == "element.state.changed").count(),
        1,
        "AG-02 owning note command announces properties"
    );
}
#[test]
fn delegated_colour_normalizes_like_command() {
    let direct = fixture();
    let patch = fixture();
    ok(&direct, "element.colour.set", json!({"elementIds":[BOB],"marker":"#aabbcc"}));
    ok(&patch, "doc.object.patch", json!({"targets":[BOB],"props":{"markerColour":"#aabbcc"}}));
    assert_eq!(get(&direct, BOB)["markerColour"], get(&patch, BOB)["markerColour"]);
}
#[test]
fn clipboard_survives_deleted_source_element() {
    let h = fixture();
    ok(&h, "cell.clipboard.set", json!({"elementIds":[BOB],"from":1,"to":6}));
    ok(&h, "element.delete", json!({"elementIds":[BOB]}));
    let dest = ok(&h, "element.create", json!({"type":"drawing","name":"PasteTarget"}))["elementIds"][0].clone();
    let e = run(&h, "cell.clipboard.apply", json!({"elementId":dest,"frame":1}));
    assert_eq!(e.status, Status::Ok, "copied artwork must survive source deletion: {e:?}");
}
#[test]
fn clipboard_cross_project_copies_artwork() {
    let h = fixture();
    ok(&h, "cell.clipboard.set", json!({"elementIds":[BOB],"from":1,"to":6}));
    let doc = tt_model::fixture::sample_document(981).unwrap();
    let e = h.open_document_with(
        doc,
        tt_executor::OpenOptions { on_unsaved: tt_executor::UnsavedPolicy::Discard, ..Default::default() },
        tt_executor::Origin::User,
    );
    assert!(e.is_ok(), "{e:?}");
    let target = first(&h, "element");
    // Use the fresh project's Bob path so the target is a compatible drawing column.
    let _ = target;
    let e = run(&h, "cell.clipboard.apply", json!({"elementId":BOB,"frame":50}));
    assert_eq!(e.status, Status::Ok, "cross-project clipboard must carry drawing payload: {e:?}");
}
#[test]
fn xsheet_missing_name_on_two_clones_is_created_once() {
    let h = fixture();
    let clone = ok(&h, "element.clone", json!({"elementId":BOB}))["elementId"].clone();
    let e = run(
        &h,
        "xsheet.exposures.set",
        json!({"createMissingDrawings":true,"columns":[
            {"elementId":BOB,"from":1,"to":1,"exposures":[{"drawing":" shared ","from":1,"to":1}]},
            {"elementId":clone,"from":1,"to":1,"exposures":[{"drawing":"shared","from":1,"to":1}]}
        ]}),
    );
    assert_eq!(e.status, Status::Ok, "normalised names share one bank entry: {e:?}");
}
#[test]
fn multi_column_overlap_copies_original_source_values() {
    let h = fixture();
    let a=ok(&h,"element.create",json!({"type":"drawing","count":3,"name":"Shift","sceneId":"scene:\"Intro\""}))["elementIds"].as_array().unwrap().clone();
    for (i, e) in a.iter().enumerate() {
        ok(
            &h,
            "cell.label.set",
            json!({"elementIds":[e],"from":1,"to":1,"drawing":format!("D{i}"),"createMissing":true}),
        );
    }
    let before = get(&h, a[1].as_str().unwrap())["exposure"].clone();
    ok(
        &h,
        "cell.range.move",
        json!({"sourceElementIds":[a[0],a[1]],"from":1,"to":1,"targetElementId":a[1],"targetFrame":1,"mode":"copy","options":{"exposure":false}}),
    );
    let new = get(&h, a[2].as_str().unwrap())["exposure"][0][0].as_str().unwrap().to_owned();
    let expected = get(&h, before[0][0].as_str().unwrap())["name"].clone();
    assert_eq!(
        get(&h, &new)["name"],
        expected,
        "overlapping columns must read original source, not earlier destination"
    );
}
#[test]
fn columnate_exact_total_with_short_final_hold() {
    let h = fixture();
    let e = ok(
        &h,
        "cell.columnate.apply",
        json!({"elementId":BOB,"frame":1,"firstDrawing":"1","lastDrawing":"10","exposeEach":4,"exposeLast":1,"loopMode":"continuous","totalFrames":12}),
    );
    assert_eq!(e["totalFrames"], 12, "requested total frame count must be satisfied");
}
#[test]
fn cycle_invalid_counts_refuse_and_one_is_noop() {
    let h = fixture();
    let r = rev(&h);
    for n in [0, 1000, 1_000_000] {
        assert_eq!(
            err(&h, "cell.cycle.create", json!({"elementId":BOB,"from":1,"to":6,"cycles":n})).code,
            "arg.invalid"
        );
    }
    ok(&h, "cell.cycle.create", json!({"elementId":BOB,"from":1,"to":6,"cycles":1}));
    assert_eq!(rev(&h), r);
}
#[test]
fn hierarchy_cycles_parentability_and_undo_are_atomic() {
    let h = fixture();
    let tree = get(&h, "scene:\"Intro\"")["tree"].clone();
    assert_eq!(err(&h, "element.parent.set", json!({"elementIds":[PEG],"parentId":BOB})).code, "element.notParentable");
    assert_eq!(
        err(&h, "element.parent.set", json!({"elementIds":[PEG],"parentId":PEG})).code,
        "element.hierarchyCycle"
    );
    ok(&h, "element.parent.set", json!({"elementIds":[BG],"parentId":PEG}));
    ok(&h, "element.delete", json!({"elementIds":[PEG]}));
    assert_eq!(ok(&h, "element.hierarchy.get", json!({"elementId":BG}))["parentId"], Value::Null);
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h, "scene:\"Intro\"")["tree"], tree);
}

#[test]
fn copied_cells_keep_notes() {
    let h = fixture();
    ok(&h, "cell.note.set", json!({"elementId":BOB,"frame":1,"text":"copied note"}));
    ok(&h, "cell.clipboard.set", json!({"elementIds":[BOB],"from":1,"to":3}));
    ok(&h, "cell.clipboard.apply", json!({"elementId":BOB,"frame":50,"options":{"exposure":false}}));
    assert_eq!(get(&h, BOB)["cellNotes"]["50"], "copied note", "04 §13.16 copies cells with properties");
}

#[test]
fn inserted_cells_shift_existing_notes() {
    let h = fixture();
    ok(&h, "cell.note.set", json!({"elementId":BOB,"frame":9,"text":"drawing two"}));
    ok(&h, "cell.blank.add", json!({"elementIds":[BOB],"frame":1,"count":2}));
    assert_eq!(
        get(&h, BOB)["cellNotes"]["11"],
        "drawing two",
        "notes are per-cell properties and follow shifted cells"
    );
}

#[test]
fn duplicate_clipping_preserves_required_parameter_row() {
    let h = fixture();
    let orig = ok(&h, "element.create", json!({"type":"clipping","name":"ClipReview"}))["elementIds"][0].clone();
    let copy = ok(&h, "element.duplicate", json!({"elementIds":[orig]}))["elementIds"][0].clone();
    assert!(
        !ok(&h, "element.hierarchy.get", json!({"elementId":copy}))["childIds"].as_array().unwrap().is_empty(),
        "04 §2.9, 10 §4.6 required mask parameter row must accompany copied effect"
    );
}

#[test]
fn patch_unlock_ops_matches_command() {
    let h = fixture();
    let d = "scene:\"Intro\"/element:\"Bob\"/drawing:\"2\"";
    let e = run(&h, "doc.object.patch", json!({"targets":[d],"ops":[{"op":"replace","path":"/locked","value":false}]}));
    assert_eq!(e.status, Status::Ok, "AG-02 patch ops and props both delegate drawing unlock: {e:?}");
}

#[test]
fn event_created_ids_match_created_element_ids() {
    let h = fixture();
    let rx = h.subscribe();
    let r = rev(&h);
    let e = run(&h, "element.create", json!({"type":"drawing","name":"EventReview"}));
    assert_eq!(e.status, Status::Ok);
    assert_eq!(rev(&h), r + 1);
    let events: Vec<_> = rx.try_iter().collect();
    assert_eq!(events.iter().filter(|e| e.name == "doc.changed").count(), 1);
    assert_eq!(events.iter().filter(|e| e.name == "element.state.changed").count(), 1);
    let created = e.result.unwrap()["elementIds"].clone();
    assert_eq!(events.iter().find(|e| e.name == "element.state.changed").unwrap().payload["elementIds"], created);
}

#[test]
fn locked_drawing_overwrite_refuses_and_dry_run_leaves_document() {
    let h = fixture();
    let before = get(&h, BOB);
    let r = rev(&h);
    ok(&h, "cell.clipboard.set", json!({"elementIds":[BOB],"from":1,"to":1}));
    assert_eq!(
        err(&h, "cell.clipboard.apply", json!({"elementId":BOB,"frame":9,"options":{"exposure":false}})).code,
        "drawing.locked"
    );
    let e = with(
        &h,
        "cell.blank.add",
        json!({"elementIds":[BOB],"frame":1}),
        InvokeOptions { dry_run: true, ..Default::default() },
    );
    assert_eq!(e.status, Status::Ok);
    assert_eq!(rev(&h), r);
    assert_eq!(get(&h, BOB), before);
}

#[test]
fn copied_drawing_survives_later_metadata_settle_pass() {
    let h = fixture();
    let d=ok(&h,"element.drawing.duplicate",json!({"drawingId":"scene:\"Intro\"/element:\"Bob\"/drawing:\"1\""}))["drawingId"].as_str().unwrap().to_owned();
    let nodes = get(&h, &d)["nodes"].clone();
    let pixels = ok(&h, "draw.drawing.render", json!({"drawingId":d,"width":64}))["pixelsSha256"].clone();
    ok(&h, "doc.object.patch", json!({"targets":[d],"props":{"meta":{"review":"settle control"}}}));
    assert_eq!(get(&h, &d)["nodes"], nodes, "metadata must not re-node copied content");
    assert_eq!(ok(&h, "draw.drawing.render", json!({"drawingId":d,"width":64}))["pixelsSha256"], pixels);
}

#[test]
fn extreme_timing_parameters_do_not_stop_document_worker() {
    for field in ["start", "startDelta", "duration", "loops"] {
        let h = fixture();
        let r = rev(&h);
        let mut p = json!({"elementIds":[PEG]});
        p[field] = json!(i64::MAX);
        let e = run(&h, "element.timing.set", p);
        assert_eq!(e.status, Status::Error, "{field}: {e:?}");
        assert!(
            matches!(
                e.error.as_ref().map(|e| e.code.as_str()),
                Some("arg.invalid" | "cell.frameOutOfRange" | "doc.limitExceeded")
            ),
            "{field} must refuse before arithmetic overflow: {e:?}"
        );
        assert_eq!(rev(&h), r);
        assert!(run(&h, "doc.state.get", json!({})).is_ok());
    }
}
