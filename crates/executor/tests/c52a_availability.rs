//! M5 C5.2a: the cell commands' availability by element type (04 §13.20, §12.8), against the kernel's prechecks and
//! handlers. For each command and each element type of the sample scene (drawing, image, media, sound, annotation,
//! peg), a dry run on one of its cells and the command's availability: where the table marks "—" the run answers
//! `cell.notApplicable` and availability is off; where it marks ✓ neither refuses for the element's type, and the
//! availability agrees with the run (prechecks = handlers).
//!
//! **Gaps reported to the orchestrator** (C5.2 plan §2 item 5: each gap a binding finds is reported before it is
//! changed). [`REPORTED`] lists the cells where the kernel differs from the table today; the test asserts they still
//! behave as reported, so a change made without its ruling fails here, and so does any other difference.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Map, Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions};

/// The sample scene's elements, one per type, in the table's column order.
const TYPES: [(&str, &str); 6] = [
    ("drawing", "Bob"),
    ("image", "Background"),
    ("media", "Reference"),
    ("sound", "Dialogue"),
    ("annotation", "Notes"),
    ("peg", "Bob-P"),
];

/// The cells of 04 §13.20 (✓ true, — false), in `TYPES` order; `None` where the table names another command for the
/// type (the peg's "Extend children exposure") or the type has no drawing to address.
type Row = (&'static str, [Option<bool>; 6]);

const TABLE: [Row; 15] = [
    ("cell.exposure.add", [Some(true), Some(true), Some(true), Some(false), Some(true), Some(true)]),
    ("cell.exposure.remove", [Some(true), Some(true), Some(true), Some(false), Some(true), Some(true)]),
    ("cell.exposure.set", [Some(true), Some(true), Some(true), Some(false), Some(true), Some(false)]),
    ("cell.fill.apply", [Some(true), Some(true), Some(true), Some(false), Some(true), None]),
    ("cell.range.add", [Some(true), Some(true), Some(true), Some(true), Some(true), Some(false)]),
    ("cell.blank.add", [Some(true), Some(true), Some(true), Some(true), Some(true), Some(false)]),
    ("cell.blank.remove", [Some(true), Some(true), Some(true), Some(true), Some(true), Some(false)]),
    ("cell.cycle.create", [Some(true), Some(true), Some(true), Some(false), Some(true), Some(false)]),
    ("cell.columnate.apply", [Some(true), Some(true), Some(true), Some(false), Some(true), Some(false)]),
    ("cell.drawing.duplicate", [Some(true), Some(false), Some(false), Some(false), Some(true), Some(false)]),
    ("element.drawingContent.delete", [Some(true), Some(false), None, None, Some(true), None]),
    ("element.drawingLock.set", [Some(true), Some(false), None, None, Some(true), None]),
    ("cell.label.set", [Some(true), Some(true), Some(false), Some(false), Some(true), Some(false)]),
    ("cell.note.set", [Some(true), Some(true), Some(false), Some(false), Some(true), Some(false)]),
    ("xsheet.lightTable.add", [Some(true), Some(true), Some(false), Some(false), Some(true), Some(false)]),
];

/// Where the kernel differs from 04 §13.20 today, as reported (command, type, what the kernel does).
const REPORTED: [(&str, &str, &str); 15] = [
    // A: media elements are refused by every cell command the table gives them.
    ("cell.exposure.add", "media", "notApplicable"),
    ("cell.exposure.remove", "media", "notApplicable"),
    ("cell.exposure.set", "media", "notApplicable"),
    ("cell.fill.apply", "media", "notApplicable"),
    ("cell.range.add", "media", "notApplicable"),
    ("cell.blank.add", "media", "notApplicable"),
    ("cell.blank.remove", "media", "notApplicable"),
    ("cell.cycle.create", "media", "notApplicable"),
    ("cell.columnate.apply", "media", "notApplicable"),
    // B: sound elements are refused by insert cells, insert blank and delete blank.
    ("cell.range.add", "sound", "notApplicable"),
    ("cell.blank.add", "sound", "notApplicable"),
    ("cell.blank.remove", "sound", "notApplicable"),
    // C, D, E: annotation elements are refused by extend exposure, change label and duplicate drawing.
    ("cell.fill.apply", "annotation", "notApplicable"),
    ("cell.label.set", "annotation", "notApplicable"),
    ("cell.drawing.duplicate", "annotation", "notApplicable"),
];

/// F: `cell.note.set` reads as available on media, sound and peg cells, whose run it refuses (its precheck does not
/// check the type as its handler does); so do `cell.fill.apply` and `cell.label.set` on an annotation cell (C, D).
const REPORTED_AVAILABILITY: [(&str, &str); 5] = [
    ("cell.note.set", "media"),
    ("cell.note.set", "sound"),
    ("cell.note.set", "peg"),
    ("cell.fill.apply", "annotation"),
    ("cell.label.set", "annotation"),
];

fn element(name: &str) -> String {
    format!("scene:\"Intro\"/element:\"{name}\"")
}

/// The first drawing the element exposes (the drawing-addressed commands' target), if any.
fn drawing_of(h: &ExecutorHandle, name: &str) -> Option<String> {
    let sheet = ok(h, "xsheet.exposures.get", json!({ "elementIds": [element(name)] }));
    sheet["columns"][0]["exposures"].as_array()?.first()?["drawingId"].as_str().map(str::to_owned)
}

/// A frame of the element that is blank (for `cell.blank.remove`), after its last exposure.
fn blank_frame(h: &ExecutorHandle, name: &str) -> u32 {
    let sheet = ok(h, "xsheet.exposures.get", json!({ "elementIds": [element(name)] }));
    let last = sheet["columns"][0]["exposures"]
        .as_array()
        .and_then(|runs| runs.iter().filter_map(|r| r["to"].as_u64()).max())
        .unwrap_or(0);
    u32::try_from(last).unwrap_or(0) + 1
}

fn params(h: &ExecutorHandle, command: &str, name: &str) -> Option<Value> {
    let e = element(name);
    Some(match command {
        "cell.exposure.add" | "cell.exposure.remove" => json!({ "elementIds": [e], "from": 1, "to": 1 }),
        "cell.exposure.set" => json!({ "elementIds": [e], "from": 1, "to": 2, "length": 2 }),
        "cell.fill.apply" => json!({ "elementIds": [e], "frame": 1, "toFrame": 4 }),
        "cell.range.add" => json!({ "elementIds": [e], "frame": 1, "count": 1 }),
        "cell.blank.add" => json!({ "elementIds": [e], "frame": 1 }),
        "cell.blank.remove" => json!({ "elementIds": [e], "frame": blank_frame(h, name) }),
        "cell.cycle.create" => json!({ "elementId": e, "from": 1, "to": 2, "cycles": 2 }),
        "cell.columnate.apply" => json!({ "elementId": e, "frame": 1, "firstDrawing": "1", "lastDrawing": "1" }),
        "cell.drawing.duplicate" => json!({ "elementId": e, "from": 1, "to": 1 }),
        "element.drawingContent.delete" => json!({ "drawingIds": [drawing_of(h, name)?] }),
        "element.drawingLock.set" => json!({ "drawingIds": [drawing_of(h, name)?], "locked": true }),
        "cell.label.set" => json!({ "elementIds": [e], "from": 1, "to": 1, "step": 1 }),
        "cell.note.set" => json!({ "elementId": e, "frame": 1, "text": "matrix" }),
        "xsheet.lightTable.add" => json!({ "items": [{ "elementId": e, "frame": 1 }] }),
        _ => return None,
    })
}

#[test]
fn the_cell_commands_follow_the_availability_table() {
    let h = fixture();
    let mut problems = Vec::new();
    for (command, cells) in TABLE {
        for (i, (ty, name)) in TYPES.iter().enumerate() {
            let Some(applicable) = cells[i] else { continue };
            let Some(p) = params(&h, command, name) else { continue };
            let env = with(&h, command, p.clone(), InvokeOptions { dry_run: true, ..Default::default() });
            let code = env.error.as_ref().map(|e| e.code.clone());
            let not_applicable = code.as_deref() == Some("cell.notApplicable");
            let m: Map<String, Value> = p.as_object().unwrap().clone();
            let available = serde_json::to_value(&h.availability(vec![(command.to_owned(), m)]).unwrap()[0]).unwrap();
            let on = available["enabled"] == true;
            let reported = REPORTED.iter().any(|(c, t, _)| *c == command && t == ty);
            let reported_on = REPORTED_AVAILABILITY.iter().any(|(c, t)| *c == command && t == ty);
            let at = format!("{command} on a {ty} cell");
            match (applicable, reported) {
                // ✓ and not reported: never refused for the type.
                (true, false) if not_applicable => {
                    problems.push(format!("{at}: ✓ in 04 §13.20, the run answers cell.notApplicable"))
                }
                // ✓ but reported: still refused as reported (a change needs its ruling).
                (true, true) if !not_applicable => {
                    problems.push(format!("{at}: reported as refused, now {code:?}: update REPORTED with its ruling"))
                }
                // —: refused with cell.notApplicable.
                (false, _) if !not_applicable => {
                    problems.push(format!("{at}: — in 04 §13.20, the run answers {code:?}"))
                }
                _ => {}
            }
            // Prechecks = handlers for the type: a cell the run refuses as not applicable is not offered (F aside).
            if not_applicable && on && !reported_on {
                problems.push(format!("{at}: available, but the run answers cell.notApplicable"));
            }
            if reported_on && !(not_applicable && on) {
                problems.push(format!(
                    "{at}: reported as available though refused, now {code:?} / {on}: update REPORTED_AVAILABILITY"
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{} differences:\n{}", problems.len(), problems.join("\n"));
}
