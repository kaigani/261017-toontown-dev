//! M5 C5.1b (builder), ruling E-40: an effect-parameter row the setting hides (04 line 74, §2.9) is left out of
//! `timeline.rows.list` and of the rows the timing-gesture interpreter hit-tests; it stays readable through
//! `element.list` and `element.get`; the setting's change raises the host-only `shell.timelineView` the window reads
//! the rows again on.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

/// A clipping effect (created with its `mask` parameter row) and a second effect; their ids and the mask rows' ids.
fn effects(h: &ExecutorHandle) -> (String, String, String, String) {
    let clip = ok(h, "element.create", json!({"type": "clipping", "name": "E40Clip"}))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let other = ok(h, "element.create", json!({"type": "clipping", "name": "E40Clip2"}))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let mask_of = |parent: &str| -> String {
        rows(h)
            .iter()
            .find(|r| r["type"] == "effectParameter" && r["parentId"] == parent)
            .unwrap_or_else(|| panic!("the mask row of {parent}"))["elementId"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (m1, m2) = (mask_of(&clip), mask_of(&other));
    (clip, other, m1, m2)
}

fn rows(h: &ExecutorHandle) -> Vec<Value> {
    ok(h, "timeline.rows.list", json!({}))["rows"].as_array().cloned().unwrap()
}

fn listed(h: &ExecutorHandle, id: &str) -> bool {
    rows(h).iter().any(|r| r["elementId"] == id)
}

fn set(h: &ExecutorHandle, params: Value) {
    ok(h, "timeline.effectParameterRows.set", params);
}

#[test]
fn the_setting_leaves_hidden_parameter_rows_out_per_effect_and_for_all() {
    let h = fixture();
    let (clip, other, m1, m2) = effects(&h);
    assert!(listed(&h, &m1) && listed(&h, &m2), "shown by default");
    // Per effect.
    set(&h, json!({"elementId": clip, "visible": false}));
    assert!(!listed(&h, &m1) && listed(&h, &m2));
    assert!(listed(&h, &clip), "the effect's own row stays");
    // For all: every effect's flag and the default.
    set(&h, json!({"visible": false}));
    assert!(!listed(&h, &m1) && !listed(&h, &m2));
    // One effect shown against the default for all.
    set(&h, json!({"elementId": other, "visible": true}));
    assert!(!listed(&h, &m1) && listed(&h, &m2));
    // Show all clears the per-effect flags.
    set(&h, json!({"elementId": clip, "visible": false}));
    set(&h, json!({"visible": true}));
    assert!(listed(&h, &m1) && listed(&h, &m2));
    // Hidden rows stay readable.
    set(&h, json!({"visible": false}));
    assert_eq!(ok(&h, "element.get", json!({"elementId": m1}))["id"], m1);
    let all = ok(&h, "element.list", json!({}))["elements"].as_array().cloned().unwrap();
    assert!(all.iter().any(|e| e["id"] == m1), "element.list still lists the hidden row");
    // The windowed read follows the same rule.
    let windowed = ok(&h, "timeline.rows.list", json!({"from": 1, "to": 5}))["rows"].as_array().cloned().unwrap();
    assert!(!windowed.iter().any(|r| r["elementId"] == m1));
}

/// The row index the Timeline shows a listed element at (no ancestor collapsed, from `depth` and `collapsed`).
fn shown_index(h: &ExecutorHandle, id: &str) -> Option<usize> {
    let mut hidden_under: Option<u64> = None;
    rows(h)
        .iter()
        .filter(|r| {
            let depth = r["depth"].as_u64().unwrap();
            if hidden_under.is_some_and(|d| depth > d) {
                return false;
            }
            hidden_under = (r["collapsed"] == true).then_some(depth);
            true
        })
        .position(|r| r["elementId"] == id)
}

/// What a click at frame 1 of a shown row commits, as text (the element ids it names).
fn click(h: &ExecutorHandle, row: usize) -> String {
    let layout = json!({"surface": "tracks", "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"});
    let at = json!({"x": 8.0, "y": row as f64 * 22.0 + 16.0, "modifiers": []});
    let out = h.timing_replay(Origin::User, layout, json!([at, at]), false).unwrap();
    out["plan"].to_string()
}

#[test]
fn a_gesture_on_the_row_below_a_hidden_row_hits_that_element() {
    let h = fixture();
    let (clip, _other, m1, _m2) = effects(&h);
    // The row listed right below the clipping effect's mask row.
    let all = rows(&h);
    let at = all.iter().position(|r| r["elementId"] == m1).unwrap();
    let below = all.get(at + 1).unwrap_or_else(|| panic!("a row below the mask row: {all:?}"))["elementId"]
        .as_str()
        .unwrap()
        .to_owned();
    let mask_row = shown_index(&h, &m1).unwrap();
    assert_eq!(shown_index(&h, &below), Some(mask_row + 1));
    // Shown: that pixel row is the mask's.
    let plan = click(&h, mask_row);
    assert!(plan.contains(&m1) && !plan.contains(&below), "{plan}");
    // Hidden: the row below moves up and the same pixel row hits it, never the hidden row.
    set(&h, json!({"elementId": clip, "visible": false}));
    assert_eq!(shown_index(&h, &below), Some(mask_row));
    let plan = click(&h, mask_row);
    assert!(plan.contains(&below) && !plan.contains(&m1), "{plan}");
}

#[test]
fn changing_the_setting_raises_the_view_notice_the_window_reads_rows_on() {
    let h = fixture();
    let (clip, ..) = effects(&h);
    let rx = h.subscribe();
    set(&h, json!({"elementId": clip, "visible": false}));
    let ev: Vec<(String, Value)> = rx.try_iter().map(|e| (e.name, e.payload)).collect();
    let notice = ev.iter().find(|(n, _)| n == "shell.timelineView").expect("the window follows an agent's change");
    assert_eq!(notice.1["effectParameterRows"][&clip], false);
    let rx = h.subscribe();
    set(&h, json!({"visible": false}));
    let ev: Vec<(String, Value)> = rx.try_iter().map(|e| (e.name, e.payload)).collect();
    let notice = ev.iter().find(|(n, _)| n == "shell.timelineView").expect("and to the setting for all");
    assert_eq!(notice.1["effectParameterRowsHidden"], true);
    // No change, no notice.
    let rx = h.subscribe();
    set(&h, json!({"visible": false}));
    assert!(!rx.try_iter().any(|e| e.name == "shell.timelineView"));
}

/// Ruling E-71 (the Timeline's half): a row dragged within its level never displaces a row the Timeline does not show.
/// A clipping effect's children ordered drawing A, the hidden mask row, drawing B: dragging B before A gives B, the
/// mask (where it was among the level's rows), A.
#[test]
fn a_row_drop_keeps_a_hidden_parameter_row_in_place() {
    let h = fixture();
    let (clip, _other, mask, _m2) = effects(&h);
    let made = ok(&h, "element.create", json!({"type": "drawing", "count": 2, "name": "E71"}))["elementIds"].clone();
    let (a, b) = (made[0].as_str().unwrap().to_owned(), made[1].as_str().unwrap().to_owned());
    ok(&h, "element.parent.set", json!({"elementIds": [a, b], "parentId": clip}));
    ok(&h, "element.order.set", json!({"order": "timeline", "parentId": clip, "elementIds": [a, mask, b]}));
    set(&h, json!({"elementId": clip, "visible": false}));
    // B alone is dragged (element.create selected both new elements).
    ok(&h, "element.select", json!({"elementIds": [b], "mode": "replace"}));
    let (ra, rb) = (shown_index(&h, &a).unwrap(), shown_index(&h, &b).unwrap());
    assert_eq!(rb, ra + 1, "the hidden mask row between them is not shown");
    let layout = json!({"surface": "layers", "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"});
    let press = json!({"x": 40.0, "y": rb as f64 * 22.0 + 11.0, "modifiers": []});
    let drop = json!({"x": 40.0, "y": ra as f64 * 22.0 + 2.0, "modifiers": []});
    let out = h.timing_replay(Origin::User, layout, json!([press, drop]), true).unwrap();
    assert_eq!(
        out["plan"]["commands"][0],
        json!(["element.order.set", {"order": "timeline", "elementIds": [b, mask, a], "parentId": clip}]),
        "{out}"
    );
    // Committed: the level's order, the mask where it was.
    assert_eq!(ok(&h, "element.get", json!({"elementId": clip}))["childIds"], json!([b, mask, a]));
}
