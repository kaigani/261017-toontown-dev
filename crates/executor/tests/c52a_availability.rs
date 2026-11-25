//! M5 C5.2a: the cell commands' availability by element type (04 §13.20, §12.8), against the kernel's prechecks and
//! handlers (rulings E-67, E-70). Every row of the table and every element type of the sample scene (drawing, image,
//! media, sound, annotation, peg): a dry run on one of its cells and the command's availability for the same
//! parameters. Where the table marks "—" the run answers `cell.notApplicable` and availability is off; where it marks ✓
//! the run never refuses for the element's type. In every cell the availability agrees with the run: a run that is
//! accepted is offered, and a run refused as not applicable is not (prechecks = handlers).
//!
//! The table holds no exception (E-67): media columns take the exposure commands through their sequences, sound
//! columns take insert cells, insert blank and delete blank through their placements. Rename drawing addresses a
//! drawing by its ID, and media, sound and peg elements have no drawing bank (10 §4.15, §4.14): there is nothing of
//! theirs to name, so those three cells are not callable. Clear and lock are called both on a drawing and on the
//! current cell (their form without drawings), so their "—" cells are reached for every type.
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

/// A row of 04 §13.20 (✓ true, — false) in `TYPES` order; `None` where the table names another command for the type
/// (the peg's "Extend children exposure") or nothing of the type can be named.
type Row = (&'static str, [Option<bool>; 6]);

const Y: Option<bool> = Some(true);
const N: Option<bool> = Some(false);

/// Every row of the table, each command of a row on its own line. `element.drawingContent.delete` and
/// `element.drawingLock.set` appear twice: by drawing (`:drawing`) and on the current cell (`:current`).
const TABLE: [Row; 19] = [
    ("cell.exposure.add", [Y, Y, Y, N, Y, Y]),
    ("cell.exposure.remove", [Y, Y, Y, N, Y, Y]),
    ("cell.exposure.set", [Y, Y, Y, N, Y, N]),
    ("cell.fill.apply", [Y, Y, Y, N, Y, None]),
    ("cell.range.add", [Y, Y, Y, Y, Y, N]),
    ("cell.blank.add", [Y, Y, Y, Y, Y, N]),
    ("cell.blank.remove", [Y, Y, Y, Y, Y, N]),
    ("cell.cycle.create", [Y, Y, Y, N, Y, N]),
    ("cell.columnate.apply", [Y, Y, Y, N, Y, N]),
    ("cell.drawing.duplicate", [Y, N, N, N, Y, N]),
    ("element.drawingContent.delete:drawing", [Y, N, None, None, Y, None]),
    ("element.drawingContent.delete:current", [Y, N, N, N, Y, N]),
    ("element.drawingLock.set:drawing", [Y, N, None, None, Y, None]),
    ("element.drawingLock.set:current", [Y, N, N, N, Y, N]),
    ("cell.label.set", [Y, Y, N, N, Y, N]),
    ("element.drawing.rename", [Y, Y, None, None, Y, None]),
    ("cell.clipboard.apply", [Y, Y, N, N, Y, N]),
    ("cell.note.set", [Y, Y, N, N, Y, N]),
    ("xsheet.lightTable.add", [Y, Y, N, N, Y, N]),
];

fn element(name: &str) -> String {
    format!("scene:\"Intro\"/element:\"{name}\"")
}

/// The sample scene with a cell at frame 1 in every column the table's ✓ cells address: the annotation element (blank
/// in the sample) exposes two drawings over frames 1 to 8.
fn scene() -> ExecutorHandle {
    let h = fixture();
    ok(
        &h,
        "xsheet.exposures.set",
        json!({ "columns": [{ "elementId": element("Notes"), "from": 1, "to": 8,
                 "exposures": [{ "drawing": "n1", "from": 1, "to": 4 }, { "drawing": "n2", "from": 5, "to": 8 }] }],
                "createMissingDrawings": true }),
    );
    h
}

/// The first drawing the element exposes (the drawing-addressed commands' target), if any.
fn drawing_of(h: &ExecutorHandle, name: &str) -> Option<String> {
    let sheet = ok(h, "xsheet.exposures.get", json!({ "elementIds": [element(name)] }));
    let ty = sheet["columns"][0]["elementType"].as_str()?.to_owned();
    if !["drawing", "image", "annotation"].contains(&ty.as_str()) {
        return None;
    }
    sheet["columns"][0]["exposures"].as_array()?.first()?["drawingId"].as_str().map(str::to_owned)
}

/// The first drawing's name in the element's bank (columnate's first and last drawing); a media column's source frame
/// `1` (E-67).
fn first_name(h: &ExecutorHandle, name: &str) -> String {
    let sheet = ok(h, "xsheet.exposures.get", json!({ "elementIds": [element(name)] }));
    match sheet["columns"][0]["elementType"].as_str() {
        Some("drawing" | "image" | "annotation") => {
            sheet["columns"][0]["exposures"][0]["drawing"].as_str().unwrap_or("1").to_owned()
        }
        _ => "1".to_owned(),
    }
}

/// A frame of the element after its last cell (blank, for `cell.blank.remove`).
fn blank_frame(h: &ExecutorHandle, name: &str) -> u32 {
    let sheet = ok(h, "xsheet.exposures.get", json!({ "elementIds": [element(name)] }));
    let c = &sheet["columns"][0];
    let last = c["exposures"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(c["clips"].as_array().into_iter().flatten())
        .filter_map(|r| r["to"].as_u64().or_else(|| r["stopFrame"].as_u64()))
        .max()
        .unwrap_or(0);
    u32::try_from(last).unwrap_or(0) + 1
}

/// Prepares what a command reads besides its parameters, and its parameters (`None`: nothing to address).
fn prepare(h: &ExecutorHandle, command: &str, name: &str) -> Option<Value> {
    let e = element(name);
    Some(match command {
        "cell.exposure.add" | "cell.exposure.remove" => json!({ "elementIds": [e], "from": 1, "to": 1 }),
        "cell.exposure.set" => json!({ "elementIds": [e], "from": 1, "to": 2, "length": 2 }),
        "cell.fill.apply" => json!({ "elementIds": [e], "frame": 1, "toFrame": 4 }),
        "cell.range.add" => json!({ "elementIds": [e], "frame": 1, "count": 1, "position": "before" }),
        "cell.blank.add" => json!({ "elementIds": [e], "frame": 1 }),
        "cell.blank.remove" => json!({ "elementIds": [e], "frame": blank_frame(h, name) }),
        "cell.cycle.create" => json!({ "elementId": e, "from": 1, "to": 2, "cycles": 2 }),
        "cell.columnate.apply" => {
            let first = first_name(h, name);
            json!({ "elementId": e, "frame": 1, "firstDrawing": first, "lastDrawing": first })
        }
        "cell.drawing.duplicate" => json!({ "elementId": e, "from": 1, "to": 1 }),
        "element.drawingContent.delete:drawing" => json!({ "drawingIds": [drawing_of(h, name)?] }),
        "element.drawingLock.set:drawing" => json!({ "drawingIds": [drawing_of(h, name)?], "locked": true }),
        "element.drawingContent.delete:current" | "element.drawingLock.set:current" => {
            ok(h, "element.current.set", json!({ "elementId": e }));
            ok(h, "app.frame.set", json!({ "frame": 1 }));
            if command.starts_with("element.drawingLock") { json!({ "locked": true }) } else { json!({}) }
        }
        "cell.label.set" => json!({ "elementIds": [e], "from": 1, "to": 1, "step": 1 }),
        "element.drawing.rename" => json!({ "drawingId": drawing_of(h, name)?, "name": "renamed" }),
        "cell.clipboard.apply" => {
            // Cells of the same element where the table allows it (paste as new drawings of its own type), else
            // the drawing element's: the "—" cells refuse whatever the clipboard holds.
            let source = if ["Bob", "Background", "Notes"].contains(&name) { e.clone() } else { element("Bob") };
            ok(h, "cell.clipboard.set", json!({ "elementIds": [source], "from": 1, "to": 1 }));
            json!({ "elementId": e, "frame": 1, "mode": "newDrawings" })
        }
        "cell.note.set" => json!({ "elementId": e, "frame": 1, "text": "matrix" }),
        "xsheet.lightTable.add" => json!({ "items": [{ "elementId": e, "frame": 1 }] }),
        _ => return None,
    })
}

#[test]
fn every_cell_command_follows_the_availability_table_and_its_precheck_agrees() {
    let mut problems = Vec::new();
    let mut checked = 0;
    for (row, cells) in TABLE {
        let command = row.split(':').next().unwrap();
        for (i, (ty, name)) in TYPES.iter().enumerate() {
            let Some(applicable) = cells[i] else { continue };
            // A fresh scene per cell: a prepared clipboard or current cell never leaks into the next.
            let h = scene();
            let Some(p) = prepare(&h, row, name) else {
                problems.push(format!("{row} on a {ty} cell: the table gives it, but nothing could be addressed"));
                continue;
            };
            checked += 1;
            let env = with(&h, command, p.clone(), InvokeOptions { dry_run: true, ..Default::default() });
            let code = env.error.as_ref().map(|e| e.code.clone());
            let not_applicable = code.as_deref() == Some("cell.notApplicable");
            let m: Map<String, Value> = p.as_object().unwrap().clone();
            let available = h.availability(vec![(command.to_owned(), m)]).unwrap().remove(0);
            let at = format!("{row} on a {ty} cell");
            if applicable && not_applicable {
                problems.push(format!("{at}: ✓ in 04 §13.20, the run answers cell.notApplicable: {:?}", env.error));
            }
            if !applicable && !not_applicable {
                problems.push(format!("{at}: — in 04 §13.20, the run answers {code:?}"));
            }
            // Prechecks = handlers: an accepted run is offered; a run refused as not applicable is not.
            if code.is_none() && !available.enabled {
                problems.push(format!("{at}: the run is accepted but not offered ({:?})", available.reason));
            }
            if not_applicable && available.enabled {
                problems.push(format!("{at}: offered, but the run answers cell.notApplicable"));
            }
        }
    }
    assert!(checked >= 100, "only {checked} cells were run");
    assert!(problems.is_empty(), "{} differences:\n{}", problems.len(), problems.join("\n"));
}
