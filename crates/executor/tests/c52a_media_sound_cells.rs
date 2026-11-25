//! M5 C5.2a, ruling E-67: the cell commands 04 §13.20 gives media and sound columns.
//!
//! **Media (A).** A media column's cells derive from its sequences (10 §4.15: each frame of a sequence shows the next
//! frame of its linked source). The exposure commands work on those cells as on an image column's, a source frame
//! standing for a drawing, and the result is written back as sequences, one per run of consecutive source frames. The
//! expected cells are written from the spec as the source frame each frame shows; the sample's `Reference` shows its
//! source's frames 1 to 24 at frames 1 to 24.
//!
//! **Sound (B).** A sound column's cells are its placements (10 §4.14). Insert cells and insert blank move the
//! placements at or after the insertion later, delete blank moves the following ones earlier (a lip-sync track's
//! `producedFor.stopFrame` with them); an insertion strictly inside a placement would split the sound, which needs 07's
//! placement timing: refused with owner M8 C8.1 (SI-219), and not offered.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

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
    for s in sequences(h) {
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
    ok(&h, command, params);
    let after = cells(&h);
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
    // Extend the cell of frame 1 to frame 3: it overwrites frames 2 and 3 (04 §13.21).
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
fn deleting_a_blank_media_cell_and_the_sequences_written_back() {
    let h = fixture();
    let original = sequences(&h)[0]["id"].clone();
    ok(&h, "cell.blank.add", json!({ "elementIds": [MEDIA], "frame": 3, "count": 2 }));
    // Two runs of consecutive source frames: the first keeps the stored sequence (same start, same source frame).
    let seqs = sequences(&h);
    assert_eq!(seqs.len(), 2);
    assert_eq!(seqs[0]["id"], original);
    assert_eq!((seqs[0]["startFrame"].as_u64(), seqs[0]["stopFrame"].as_u64()), (Some(1), Some(2)));
    assert_eq!(
        (seqs[1]["startFrame"].as_u64(), seqs[1]["stopFrame"].as_u64(), seqs[1]["linkOffset"].as_u64()),
        (Some(5), Some(26), Some(3))
    );
    // A cell that is not blank is refused; the blank ones go and the runs join again.
    assert_eq!(err(&h, "cell.blank.remove", json!({ "elementIds": [MEDIA], "frame": 2 })).code, "cell.notBlank");
    ok(&h, "cell.blank.remove", json!({ "elementIds": [MEDIA], "frame": 3, "count": 2 }));
    assert_eq!(cells(&h), src(1..=24));
    assert_eq!(sequences(&h)[0]["id"], original);
}

#[test]
fn inserting_cells_in_all_elements_splits_a_spanning_media_sequence() {
    // Before E-67 a sequence spanning the insertion was left whole (SI-088); now the media column is a column.
    let h = fixture();
    ok(&h, "cell.range.add", json!({ "allElements": true, "frame": 4, "count": 2 }));
    assert_eq!(cells(&h), cat(&[src(1..=4), vec![None, None], src(5..=24)]));
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
