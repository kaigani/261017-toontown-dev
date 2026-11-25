//! M5 C5.2a, ruling E-67: the cell commands 04 Ã‚Â§13.20 gives media and sound columns.
//!
//! **Media (A).** A media column's cells derive from its sequences (10 Ã‚Â§4.15: each frame of a sequence shows the next
//! frame of its linked source). The exposure commands work on those cells as on an image column's, a source frame
//! standing for a drawing, and the result is written back as sequences, one per run of consecutive source frames. The
//! expected cells are written from the spec as the source frame each frame shows; the sample's `Reference` shows its
//! source's frames 1 to 24 at frames 1 to 24.
//!
//! **Sound (B).** A sound column's cells are its placements (10 Ã‚Â§4.14). Insert cells and insert blank move the
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
    // Extend the cell of frame 1 to frame 3: it overwrites frames 2 and 3 (04 Ã‚Â§13.21).
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
