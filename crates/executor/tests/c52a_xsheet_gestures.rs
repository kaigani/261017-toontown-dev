//! M5 C5.2a: the Exposure Sheet's gestures in the timing-gesture interpreter (`crates/executor/src/xsheet_gesture.rs`;
//! 04 §12.5, §12.9; rulings E-7, E-10). Each gesture's plan is the expectation written from the spec (D-23); the window's
//! press and release commit it; the direct commands give the same state (D-32). The geometry follows the columns'
//! widths and the thumbnails option; a change under a drag cancels the gesture (E-10).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

const ROW: f64 = 20.0;
const COL: f64 = 80.0;

fn layout(surface: &str) -> Value {
    json!({"surface": surface, "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "scrollLeft": 0, "platform": "windows"})
}

fn s(x: f64, y: f64) -> Value {
    json!({"x": x, "y": y, "modifiers": []})
}

fn s_with(x: f64, y: f64, modifiers: &[&str]) -> Value {
    json!({"x": x, "y": y, "modifiers": modifiers})
}

/// The point in the middle of a cell (column `c`, frame `f`).
fn cell(c: usize, f: u32) -> (f64, f64) {
    (c as f64 * COL + COL / 2.0, f64::from(f - 1) * ROW + ROW / 2.0)
}

/// The sheet's columns (element IDs, left to right) and each one's exposures (`[drawingId, from, to]`).
/// A column: its element and its exposures (`[drawingId, from, to]`).
type Column = (String, Vec<(String, u32, u32)>);

fn columns(h: &ExecutorHandle) -> Vec<Column> {
    ok(h, "xsheet.exposures.get", json!({}))["columns"]
        .as_array()
        .unwrap()
        .iter()
        // Every column the sheet shows (a sound column has clips, no exposures: its runs are empty).
        .filter(|c| c["visible"] == true)
        .map(|c| {
            let runs = c["exposures"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| {
                    (
                        r["drawingId"].as_str().unwrap().to_owned(),
                        r["from"].as_u64().unwrap() as u32,
                        r["to"].as_u64().unwrap() as u32,
                    )
                })
                .collect();
            (c["elementId"].as_str().unwrap().to_owned(), runs)
        })
        .collect()
}

fn state(h: &ExecutorHandle) -> Value {
    json!({
        "sheet": ok(h, "xsheet.exposures.get", json!({})),
        "selection": ok(h, "cell.selection.get", json!({})),
        "lightTable": ok(h, "xsheet.lightTable.list", json!({})),
        "app": ok(h, "app.state.get", json!({})),
        "rev": rev(h),
    })
}

/// The plan equals the expectation; the window's press and release commit it; the direct commands give the same state.
fn three_way(setup: &dyn Fn(&ExecutorHandle), surface: &str, samples: Value, expected: Value) {
    three_way_in(setup, layout(surface), samples, expected);
}

/// [`three_way`] with a given layout.
fn three_way_in(setup: &dyn Fn(&ExecutorHandle), layout: Value, samples: Value, expected: Value) {
    let (hw, hd) = (fixture(), fixture());
    setup(&hw);
    setup(&hd);
    let plan = hw.timing_replay(Origin::User, layout.clone(), samples.clone(), false).unwrap()["plan"].clone();
    assert_eq!(plan, expected, "the interpreter's plan");
    let press =
        hw.timing_gesture(Origin::User, json!({"phase": "press", "layout": layout, "sample": samples[0]})).unwrap();
    let out = hw
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["plan"], expected, "the window's commit: {out}");
    for env in out["envelopes"].as_array().unwrap() {
        assert_eq!(env["status"], "ok", "{env}");
    }
    for c in expected["commands"].as_array().unwrap() {
        ok(&hd, c[0].as_str().unwrap(), c[1].clone());
    }
    assert_eq!(state(&hw), state(&hd), "the window's path and the direct commands");
}

fn none(_: &ExecutorHandle) {}

#[test]
fn a_click_selects_the_cell_and_makes_it_current() {
    let cols = columns(&fixture());
    let (x, y) = cell(0, 3);
    three_way(
        &none,
        "xsheetGrid",
        json!([s(x, y), s(x, y)]),
        json!({"kind": "commands", "commands": [
            ["cell.select", {"elementIds": [cols[0].0], "from": 3, "to": 3}],
            ["xsheet.currentCell.set", {"elementId": cols[0].0, "frame": 3}],
        ]}),
    );
}

#[test]
fn a_drag_from_an_unselected_cell_selects_the_rectangle() {
    let cols = columns(&fixture());
    assert!(cols.len() >= 2, "the fixture shows two columns at least");
    let (x0, y0) = cell(0, 2);
    let (x1, y1) = cell(1, 4);
    three_way(
        &none,
        "xsheetGrid",
        json!([s(x0, y0), s(x1, y1)]),
        json!({"kind": "commands", "commands": [
            ["cell.select", {"elementIds": [cols[0].0, cols[1].0], "from": 2, "to": 4}],
            ["xsheet.currentCell.set", {"elementId": cols[0].0, "frame": 2}],
        ]}),
    );
}

#[test]
fn dragging_the_selected_cells_moves_them_down() {
    let h = fixture();
    let cols = columns(&h);
    let bob = cols[0].0.clone();
    let (_, from, to) = cols[0].1[0].clone();
    let select = move |h: &ExecutorHandle| {
        ok(h, "cell.select", json!({"elementIds": [bob], "from": from, "to": to}));
    };
    let options = ok(&h, "timeline.transferOptions.get", json!({"operation": "dragDrop"}))["options"].clone();
    let (x, y0) = cell(0, from);
    three_way(
        &select,
        "xsheetGrid",
        json!([s(x, y0), s(x, y0 + 2.0 * ROW)]),
        json!({"kind": "commands", "commands": [
            ["cell.range.move", {"sourceElementIds": [cols[0].0], "from": from, "to": to, "targetFrame": from + 2,
                                 "mode": "move", "options": options}],
        ]}),
    );
    // With the copy modifier the same drag copies.
    let bob = cols[0].0.clone();
    let select = move |h: &ExecutorHandle| {
        ok(h, "cell.select", json!({"elementIds": [bob], "from": from, "to": to}));
    };
    three_way(
        &select,
        "xsheetGrid",
        json!([s(x, y0), s_with(x, y0 + 2.0 * ROW, &["ctrl"])]),
        json!({"kind": "commands", "commands": [
            ["cell.range.move", {"sourceElementIds": [cols[0].0], "from": from, "to": to, "targetFrame": from + 2,
                                 "mode": "copy", "options": options}],
        ]}),
    );
}

#[test]
fn dragging_the_selected_cells_onto_the_light_table_panel_sends_their_drawings() {
    let h = fixture();
    let sheet = ok(&h, "xsheet.exposures.get", json!({}));
    let shown: Vec<Value> =
        sheet["columns"].as_array().unwrap().iter().filter(|c| c["visible"] == true).cloned().collect();
    let ids: Vec<String> = shown.iter().map(|c| c["elementId"].as_str().unwrap().to_owned()).collect();
    assert!(shown.iter().any(|c| c["elementType"] == "media"), "the selection holds a column the table does not take");
    // Expected from the spec (04 §15.1, §13.20): the distinct drawings of the drawing, image and annotation columns in
    // frames 1 to 4, in column then frame order.
    let mut items: Vec<Value> = Vec::new();
    for c in &shown {
        if !["drawing", "image", "annotation"].contains(&c["elementType"].as_str().unwrap()) {
            continue;
        }
        let mut runs: Vec<&Value> = c["exposures"].as_array().into_iter().flatten().collect();
        runs.sort_by_key(|r| r["from"].as_u64());
        for r in runs.iter().filter(|r| r["from"].as_u64().unwrap() <= 4) {
            let item = json!({"elementId": c["elementId"], "drawingId": r["drawingId"]});
            if !items.contains(&item) {
                items.push(item);
            }
        }
    }
    assert!(!items.is_empty());
    let ids2 = ids.clone();
    let select = move |h: &ExecutorHandle| {
        ok(h, "cell.select", json!({"elementIds": ids2, "from": 1, "to": 4}));
    };
    let mut with_panel = layout("xsheetGrid");
    with_panel["lightTable"] = json!({"x": 0.0, "y": 600.0, "width": 400.0, "height": 200.0});
    let (x, y0) = cell(0, 2);
    three_way_in(
        &select,
        with_panel.clone(),
        json!([s(x, y0), s(x + 30.0, 700.0)]),
        json!({"kind": "commands", "commands": [["xsheet.lightTable.add", {"items": items}]]}),
    );
    // The same drawings as the command without items sends for the selection (one rule).
    let (hd, hs) = (fixture(), fixture());
    select(&hd);
    select(&hs);
    ok(&hd, "xsheet.lightTable.add", json!({"items": items}));
    ok(&hs, "xsheet.lightTable.add", json!({}));
    assert_eq!(ok(&hd, "xsheet.lightTable.list", json!({})), ok(&hs, "xsheet.lightTable.list", json!({})));
    // Released outside the panel, the same drag moves the cells as before.
    select(&h);
    let plan = h.timing_replay(Origin::User, with_panel, json!([s(x, y0), s(x, y0 + 2.0 * ROW)]), false).unwrap();
    assert_eq!(plan["plan"]["commands"][0][0], "cell.range.move", "{plan}");
}

#[test]
fn dragging_an_exposures_bottom_edge_lengthens_and_shortens_it() {
    let cols = columns(&fixture());
    let (_, from, to) = cols[0].1[0].clone();
    let x = COL / 2.0;
    let edge = f64::from(to) * ROW - 1.0;
    three_way(
        &none,
        "xsheetGrid",
        json!([s(x, edge), s(x, edge + 2.0 * ROW)]),
        json!({"kind": "commands", "commands": [
            ["cell.exposure.add", {"elementIds": [cols[0].0], "from": to, "to": to, "frames": 2}],
        ]}),
    );
    if to > from {
        three_way(
            &none,
            "xsheetGrid",
            json!([s(x, edge), s(x, edge - ROW)]),
            json!({"kind": "commands", "commands": [
                ["cell.exposure.remove", {"elementIds": [cols[0].0], "from": to, "to": to, "frames": 1}],
            ]}),
        );
    }
}

#[test]
fn a_header_click_selects_the_column_and_a_header_drag_reorders() {
    let h = fixture();
    let cols = columns(&h);
    three_way(
        &none,
        "xsheetHeaders",
        json!([s(COL / 2.0, 10.0), s(COL / 2.0, 10.0)]),
        json!({"kind": "commands", "commands": [
            ["element.select", {"elementIds": [cols[0].0], "mode": "replace"}],
            ["element.current.set", {"elementId": cols[0].0}],
        ]}),
    );
    // The first column dropped after the second: the complete order with the first moved there.
    let order: Vec<String> = ok(&h, "xsheet.exposures.get", json!({}))["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["elementId"].as_str().unwrap().to_owned())
        .collect();
    // Ruling E-71 (the design note §3): the shown columns in their new order (the first after the second), each
    // hidden column at the same position among all columns as before.
    let mut shown: Vec<String> = cols.iter().map(|c| c.0.clone()).collect();
    let first = shown.remove(0);
    shown.insert(1, first);
    let mut next = shown.into_iter();
    let expected: Vec<String> =
        order.iter().map(|e| if cols.iter().any(|c| c.0 == *e) { next.next().unwrap() } else { e.clone() }).collect();
    assert!(expected.len() > cols.len(), "the sample shows a hidden column (the annotation element)");
    three_way(
        &none,
        "xsheetHeaders",
        json!([s(COL / 2.0, 10.0), s(COL * 1.9, 10.0)]),
        json!({"kind": "commands", "commands": [["element.order.set", {"order": "xsheet", "elementIds": expected}]]}),
    );
}

#[test]
fn the_geometry_follows_the_column_widths_and_the_thumbnails_option() {
    let h = fixture();
    let cols = columns(&h);
    let replay = |h: &ExecutorHandle, x: f64, y: f64| {
        h.timing_replay(Origin::User, layout("xsheetGrid"), json!([s(x, y), s(x, y)]), false).unwrap()["plan"]
            ["commands"][1][1]
            .clone()
    };
    // The first column 200 pixels wide: x = 150 is still in it.
    assert_eq!(replay(&h, 150.0, 5.0)["elementId"], json!(cols[1].0));
    ok(&h, "xsheet.columnWidth.set", json!({"elementId": cols[0].0, "width": 200}));
    assert_eq!(replay(&h, 150.0, 5.0)["elementId"], json!(cols[0].0));
    // Thumbnails make the rows taller: y = 60 is frame 4, then frame 2.
    assert_eq!(replay(&h, 10.0, 60.0)["frame"], 4);
    ok(&h, "xsheet.display.set", json!({"thumbnails": true}));
    assert_eq!(replay(&h, 10.0, 60.0)["frame"], 2);
    // Scrolled: the layout's scroll offsets count.
    let scrolled = json!({"surface": "xsheetGrid", "zoomLevel": 6, "firstFrame": 1, "scrollTop": 104, "scrollLeft": 0,
                          "platform": "windows"});
    let plan =
        h.timing_replay(Origin::User, scrolled, json!([s(10.0, 0.0), s(10.0, 0.0)]), false).unwrap()["plan"].clone();
    assert_eq!(plan["commands"][1][1]["frame"], 3, "104 px scrolled at 52 px rows: frame 3 at the top");
    // Right of the last column: nothing.
    let far = h
        .timing_replay(Origin::User, layout("xsheetGrid"), json!([s(100_000.0, 5.0), s(100_000.0, 5.0)]), false)
        .unwrap()["plan"]
        .clone();
    assert_eq!(far, json!({"kind": "nothing"}));
}

#[test]
fn a_change_under_a_drag_cancels_the_gesture() {
    let h = fixture();
    let cols = columns(&h);
    let (_, from, to) = cols[0].1[0].clone();
    ok(&h, "cell.select", json!({"elementIds": [cols[0].0], "from": from, "to": to}));
    let (x, y0) = cell(0, from);
    let samples = json!([s(x, y0), s(x, y0 + 2.0 * ROW)]);
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout("xsheetGrid"), "sample": samples[0]}))
        .unwrap();
    let before = rev(&h);
    // An agent changes the dragged cells while the person drags: a blank cell inserted at the first of them.
    ok(&h, "cell.blank.add", json!({"elementIds": [cols[0].0], "frame": from}));
    let changed = rev(&h);
    assert!(changed > before);
    // Answered as the Timeline's: cancelled, with the reason the status line shows (E-10, E-23).
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["cancelled"], true, "{out}");
    assert_eq!(out["error"]["code"], "timeline.targetChanged");
    assert_eq!(rev(&h), changed, "nothing of the gesture was committed");
}

/// Ruling E-70 (C52A-R1S-03; E-23, E-28): the exposure-edge drag compares the exposure grabbed and the cells after the
/// shift between the press and the present. Replacing the grabbed exposure's drawing before the release cancels the
/// drag instead of lengthening the new drawing; an unrelated change elsewhere does not.
#[test]
fn an_edge_drag_whose_exposure_changed_is_cancelled() {
    let h = fixture();
    let cols = columns(&h);
    let bob = cols[0].0.clone();
    let (_, _, to) = cols[0].1[0].clone();
    let other = cols[0].1.iter().map(|r| r.0.clone()).find(|d| *d != cols[0].1[0].0).expect("a second drawing of Bob");
    let x = COL / 2.0;
    let edge = f64::from(to) * ROW - 1.0;
    let samples = json!([s(x, edge), s(x, edge + 2.0 * ROW)]);
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout("xsheetGrid"), "sample": samples[0]}))
        .unwrap();
    assert_eq!(press["target"]["kind"], "exposureEdge", "{press}");
    // The last frame of the grabbed exposure now shows another drawing.
    ok(&h, "cell.label.set", json!({"elementIds": [bob], "from": to, "to": to, "drawingId": other}));
    let changed = rev(&h);
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["cancelled"], true, "{out}");
    assert_eq!(out["error"]["code"], "timeline.targetChanged");
    assert_eq!(rev(&h), changed, "nothing of the gesture was committed");
    // A change of another column under the same drag leaves the grabbed exposure and its outcome as they were.
    let h = fixture();
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout("xsheetGrid"), "sample": samples[0]}))
        .unwrap();
    ok(&h, "cell.blank.add", json!({"elementIds": [cols[1].0], "frame": 1}));
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert!(out["cancelled"] != true, "{out}");
}

#[test]
fn the_bundled_layout_parses() {
    let l = tt_executor::xsheet_gesture::parse_bundled().unwrap();
    assert_eq!(&l, tt_executor::xsheet_gesture::xsheet_layout());
    assert_eq!((l.row_height, l.column_width), (20, 80), "the constants the tests above use");
}
