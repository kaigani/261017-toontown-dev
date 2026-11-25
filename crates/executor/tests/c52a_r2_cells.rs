//! M5 C5.2a, ruling E-67: the cell commands 04 Ãƒâ€šÃ‚Â§13.20 gives media and sound columns.
//!
//! **Media (A).** A media column's cells derive from its sequences (10 Ãƒâ€šÃ‚Â§4.15: each frame of a sequence shows the next
//! frame of its linked source). The exposure commands work on those cells as on an image column's, a source frame
//! standing for a drawing, and the result is written back as sequences, one per run of consecutive source frames. The
//! expected cells are written from the spec as the source frame each frame shows; the sample's `Reference` shows its
//! source's frames 1 to 24 at frames 1 to 24.
//!
//! **Sound (B).** A sound column's cells are its placements (10 Ãƒâ€šÃ‚Â§4.14). Insert cells and insert blank move the
//! placements at or after the insertion later, delete blank moves the following ones earlier (a lip-sync track's
//! `producedFor.stopFrame` with them); an insertion strictly inside a placement would split the sound, which needs 07's
//! placement timing: refused with owner M8 C8.1 (SI-219), and not offered.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;
mod c13_support;

use common::*;
use serde_json::{Map, Value, json};
use tt_executor::ExecutorHandle;

const MEDIA: &str = "scene:\"Intro\"/element:\"Reference\"";
const SOUND: &str = "scene:\"Intro\"/element:\"Dialogue\"";

/// The stored sequences without their revisions (an undo restores content with a new revision, REV-03).
fn content(h: &ExecutorHandle) -> Vec<Value> {
    let mut v = sequences(h);
    for s in &mut v {
        s.as_object_mut().unwrap().remove("rev");
    }
    v
}

fn sequences(h: &ExecutorHandle) -> Vec<Value> {
    ok(h, "doc.object.get", json!({ "ref": MEDIA, "depth": -1 }))["object"]["sequences"].as_array().unwrap().clone()
}

/// The source frame each frame of the media column shows (`None`: blank), from its sequences.
fn cells(h: &ExecutorHandle) -> Vec<Option<u64>> {
    let mut out: Vec<Option<u64>> = Vec::new();
    let seqs = sequences(h);
    let sheet = ok(h, "xsheet.exposures.get", json!({"elementIds":[MEDIA],"from":1,"to":100000}));
    for r in sheet["columns"][0]["exposures"].as_array().unwrap() {
        let s = seqs.iter().find(|s| s["id"] == r["drawingId"]).unwrap();
        assert_eq!(s["startFrame"], r["from"]);
        assert_eq!(s["stopFrame"], r["to"]);
        let (start, stop, link) =
            (s["startFrame"].as_u64().unwrap(), s["stopFrame"].as_u64().unwrap(), s["linkOffset"].as_u64().unwrap());
        if out.len() < stop as usize {
            out.resize(stop as usize, None);
        }
        for f in start..=stop {
            assert!(out[f as usize - 1].is_none(), "overlapping sequences at frame {f}");
            out[f as usize - 1] = Some(link + f - start);
        }
    }
    out
}

fn src(range: std::ops::RangeInclusive<u64>) -> Vec<Option<u64>> {
    range.map(Some).collect()
}

fn cat(parts: &[Vec<Option<u64>>]) -> Vec<Option<u64>> {
    parts.concat()
}

/// Runs a command on a fresh sample scene and returns the media column's cells, then checks undo restores the stored
/// sequences exactly.
fn media_after(command: &str, params: Value) -> Vec<Option<u64>> {
    let h = fixture();
    let before = content(&h);
    assert_eq!(cells(&h), src(1..=24), "the sample's media column");
    ok(&h, command, params.clone());
    let after = cells(&h);
    println!("{}", ok(&h, "xsheet.exposures.get", json!({"elementIds":[MEDIA]})));
    let dir = c13_support::tmp();
    let path = dir.join("media.animpkg").to_string_lossy().into_owned();
    c13_support::job_ok(&h, "project.saveAs", json!({"path":path}));
    ok(&h, "project.close", json!({}));
    c13_support::job_ok(&h, "project.open", json!({"path":path}));
    assert_eq!(cells(&h), after);
    let h = fixture();
    ok(&h, command, params);

    ok(&h, "edit.undo", json!({}));
    assert_eq!(content(&h), before, "{command}: undo restores the sequences");
    after
}

#[test]
fn the_exposure_commands_work_on_a_media_columns_cells() {
    // Set exposure 2 on frames 1-4: each of the four source frames held two frames; the rest follows.
    assert_eq!(
        media_after("cell.exposure.set", json!({ "elementIds": [MEDIA], "from": 1, "to": 4, "length": 2 })),
        cat(&[vec![Some(1), Some(1), Some(2), Some(2), Some(3), Some(3), Some(4), Some(4)], src(5..=24)])
    );
    // Add exposure on frame 1: the source frame 1 held one frame longer.
    assert_eq!(
        media_after("cell.exposure.add", json!({ "elementIds": [MEDIA], "from": 1, "to": 1 })),
        cat(&[vec![Some(1)], src(1..=24)])
    );
    // Remove exposure on frames 1-2 (each one frame long): their cells go, the rest moves up.
    assert_eq!(media_after("cell.exposure.remove", json!({ "elementIds": [MEDIA], "from": 1, "to": 2 })), src(3..=24));
    // Extend the cell of frame 1 to frame 3: it overwrites frames 2 and 3 (04 Ãƒâ€šÃ‚Â§13.21).
    assert_eq!(
        media_after("cell.fill.apply", json!({ "elementIds": [MEDIA], "frame": 1, "toFrame": 3 })),
        cat(&[vec![Some(1), Some(1), Some(1)], src(4..=24)])
    );
    // Insert two blank cells before frame 3.
    assert_eq!(
        media_after("cell.blank.add", json!({ "elementIds": [MEDIA], "frame": 3, "count": 2 })),
        cat(&[src(1..=2), vec![None, None], src(3..=24)])
    );
    // Insert cells (blank) after frame 2.
    assert_eq!(
        media_after("cell.range.add", json!({ "elementIds": [MEDIA], "frame": 2, "count": 1 })),
        cat(&[src(1..=2), vec![None], src(3..=24)])
    );
    // A cycle of frames 1-2, twice in all: the following cells move after it.
    assert_eq!(
        media_after("cell.cycle.create", json!({ "elementId": MEDIA, "from": 1, "to": 2, "cycles": 2 })),
        cat(&[vec![Some(1), Some(2), Some(1), Some(2)], src(3..=24)])
    );
    // Columnate the source's frames 5 to 7 from frame 1, overwriting: the drawings of a media column are its source's
    // frames, named by their number (E-67).
    assert_eq!(
        media_after(
            "cell.columnate.apply",
            json!({ "elementId": MEDIA, "frame": 1, "firstDrawing": "5", "lastDrawing": "7" })
        ),
        cat(&[src(5..=7), src(4..=24)])
    );
}


#[test]
fn q1_reset_natural_length() {
    let h = fixture();
    ok(&h, "element.mediaLength.set", json!({"elementId":MEDIA,"length":3}));
    ok(&h, "element.mediaLength.set", json!({"elementId":MEDIA}));
    assert_eq!(cells(&h), src(1..=24));
}

fn placement(h: &ExecutorHandle) -> (u64, u64, u64) {
    let p = ok(h, "doc.object.get", json!({ "ref": SOUND, "depth": -1 }))["object"]["placements"][0].clone();
    (
        p["startFrame"].as_u64().unwrap(),
        p["stopFrame"].as_u64().unwrap(),
        p["mouthTrack"]["producedFor"]["stopFrame"].as_u64().unwrap(),
    )
}

#[test]
fn insert_blank_insert_cells_and_delete_blank_move_a_sound_columns_placements() {
    let h = fixture();
    assert_eq!(placement(&h), (1, 8, 8), "the sample's sound: frames 1-8");
    ok(&h, "cell.blank.add", json!({ "elementIds": [SOUND], "frame": 1, "count": 2 }));
    assert_eq!(placement(&h), (3, 10, 10), "moved later with its lip-sync timing");
    ok(&h, "cell.range.add", json!({ "elementIds": [SOUND], "frame": 3, "count": 1, "position": "before" }));
    assert_eq!(placement(&h), (4, 11, 11));
    // Delete blank: frames 1-3 are blank now.
    ok(&h, "cell.blank.remove", json!({ "elementIds": [SOUND], "frame": 1, "count": 3 }));
    assert_eq!(placement(&h), (1, 8, 8));
    assert_eq!(err(&h, "cell.blank.remove", json!({ "elementIds": [SOUND], "frame": 8 })).code, "cell.notBlank");
    // After the sound nothing moves; the call is accepted and changes nothing.
    let r = rev(&h);
    ok(&h, "cell.blank.add", json!({ "elementIds": [SOUND], "frame": 20 }));
    assert_eq!(rev(&h), r);
    // Undo walks back through the moves.
    ok(&h, "edit.undo", json!({ "steps": 3 }));
    assert_eq!(placement(&h), (1, 8, 8));
}

#[test]
fn an_insertion_inside_a_sound_is_refused_with_its_owner_and_not_offered() {
    let h = fixture();
    for (command, params) in [
        ("cell.blank.add", json!({ "elementIds": [SOUND], "frame": 4 })),
        ("cell.range.add", json!({ "elementIds": [SOUND], "frame": 3, "count": 2 })),
    ] {
        let e = err(&h, command, params.clone());
        assert_eq!(e.code, "cell.notApplicable", "{command}");
        assert_eq!((e.details["reason"].as_str(), e.details["owner"].as_str()), (Some("splitsSound"), Some("M8 C8.1")));
        assert_eq!(e.details["specIssue"], "SI-219");
        let m: Map<String, Value> = params.as_object().unwrap().clone();
        let a = h.availability(vec![(command.to_owned(), m)]).unwrap().remove(0);
        assert!(!a.enabled, "{command}: offered though refused");
    }
    assert_eq!(placement(&h), (1, 8, 8));
}

#[test]
fn q3_drawing_fill_frame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 48, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_frame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 100000, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_frame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1000000000, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_frame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 9223372036854775807, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_frame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 18446744073709551615, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_toFrame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "toFrame": 48}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_toFrame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "toFrame": 100000}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_toFrame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "toFrame": 1000000000}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_toFrame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "toFrame": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_fill_toFrame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "toFrame": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_frame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 48, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_frame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 100000, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_frame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1000000000, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_frame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 9223372036854775807, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_frame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 18446744073709551615, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_count_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "count": 48}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_count_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "count": 100000}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_count_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "count": 1000000000}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_count_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "count": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_range_count_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "count": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_from_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 48, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_from_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 100000, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_from_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1000000000, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_from_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 9223372036854775807, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_from_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 18446744073709551615, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_to_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 48, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_to_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 100000, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_to_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 1000000000, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_to_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 9223372036854775807, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_to_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 18446744073709551615, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_length_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 2, "length": 48}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_length_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 2, "length": 100000}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_length_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 2, "length": 1000000000}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_length_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 2, "length": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_exposure_length_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 2, "length": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_from_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 48, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_from_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 100000, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_from_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1000000000, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_from_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 9223372036854775807, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_from_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 18446744073709551615, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_to_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 48, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_to_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 100000, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_to_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 1000000000, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_to_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 9223372036854775807, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_to_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 18446744073709551615, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_cycles_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 2, "cycles": 48}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_cycles_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 2, "cycles": 100000}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_cycles_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 2, "cycles": 1000000000}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_cycles_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 2, "cycles": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_cycle_cycles_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 2, "cycles": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_frame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 48, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_frame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 100000, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_frame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1000000000, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_frame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 9223372036854775807, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_frame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 18446744073709551615, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeEach_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 48, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeEach_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 100000, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeEach_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1000000000, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeEach_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 9223372036854775807, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeEach_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 18446744073709551615, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeLast_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 48}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeLast_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 100000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeLast_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 1000000000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeLast_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_exposeLast_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_loops_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 48}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_loops_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 100000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_loops_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1000000000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_loops_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_loops_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_totalFrames_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 48}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_totalFrames_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 100000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_totalFrames_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 1000000000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_totalFrames_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_drawing_columnate_totalFrames_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_frame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 48, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_frame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 100000, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_frame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1000000000, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_frame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 9223372036854775807, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_frame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 18446744073709551615, "toFrame": 3}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_toFrame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "toFrame": 48}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_toFrame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "toFrame": 100000}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_toFrame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "toFrame": 1000000000}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_toFrame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "toFrame": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_fill_toFrame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "toFrame": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.fill.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_frame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 48, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_frame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 100000, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_frame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1000000000, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_frame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 9223372036854775807, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_frame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 18446744073709551615, "count": 1}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_count_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "count": 48}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_count_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "count": 100000}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_count_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "count": 1000000000}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_count_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "count": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_range_count_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "frame": 1, "count": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.range.add",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_from_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 48, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_from_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 100000, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_from_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1000000000, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_from_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 9223372036854775807, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_from_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 18446744073709551615, "to": 2, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_to_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 48, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_to_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 100000, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_to_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 1000000000, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_to_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 9223372036854775807, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_to_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 18446744073709551615, "length": 2}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_length_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 2, "length": 48}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_length_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 2, "length": 100000}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_length_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 2, "length": 1000000000}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_length_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 2, "length": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_exposure_length_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementIds": ["scene:\"Intro\"/element:\"Reference\""], "from": 1, "to": 2, "length": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.exposure.set",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_from_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 48, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_from_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 100000, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_from_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1000000000, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_from_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 9223372036854775807, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_from_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 18446744073709551615, "to": 2, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_to_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 48, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_to_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 100000, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_to_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 1000000000, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_to_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 9223372036854775807, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_to_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 18446744073709551615, "cycles": 2}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_cycles_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 2, "cycles": 48}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_cycles_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 2, "cycles": 100000}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_cycles_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 2, "cycles": 1000000000}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_cycles_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 2, "cycles": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_cycle_cycles_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "from": 1, "to": 2, "cycles": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.cycle.create",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_frame_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 48, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_frame_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 100000, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_frame_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1000000000, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_frame_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 9223372036854775807, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_frame_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 18446744073709551615, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeEach_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 48, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeEach_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 100000, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeEach_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1000000000, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeEach_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 9223372036854775807, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeEach_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 18446744073709551615, "loopMode": "continuous", "loops": 1}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeLast_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 48}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeLast_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 100000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeLast_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 1000000000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeLast_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_exposeLast_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "exposeLast": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_loops_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 48}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_loops_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 100000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_loops_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1000000000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_loops_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_loops_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_totalFrames_48() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 48}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_totalFrames_100000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 100000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_totalFrames_1000000000() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 1000000000}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_totalFrames_9223372036854775807() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 9223372036854775807}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}

#[test]
fn q3_media_columnate_totalFrames_18446744073709551615() {
 let h=fixture();
 let p: Value=serde_json::from_str(r##"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "1", "exposeEach": 1, "loopMode": "continuous", "loops": 1, "totalFrames": 18446744073709551615}"##).unwrap();
 let e=run(&h,"cell.columnate.apply",p);
 assert!(e.result.is_some() || e.error.is_some(), "{e:?}");
 ok(&h,"doc.state.get",json!({}));
}
