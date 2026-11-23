//! M5 C5.1b (builder): the Timeline's rows of the gesture table (`crates/executor/src/gesture-table.json`, view
//! `timeline`, the gesture's id as `tool`) are the timing-gesture interpreter's, both ways: every command the
//! interpreter's source names is in a row, and a battery of gestures through `timing_replay` produces exactly each row's
//! commands. The rows bind those commands in the parity manifest's gesture section (ruling D-35).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tt_executor::{ExecutorHandle, Origin};
use tt_registry::Kind;

const TABLE: &str = include_str!("../src/gesture-table.json");
const INTERPRETER: &str = include_str!("../src/timing_gesture.rs");
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

/// The rows the Timeline shows (no ancestor collapsed), in order.
fn visible(h: &ExecutorHandle) -> Vec<Value> {
    let mut hidden_under: Option<u64> = None;
    ok(h, "timeline.rows.list", json!({}))["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            let d = r["depth"].as_u64().unwrap();
            if hidden_under.is_some_and(|u| d > u) {
                return false;
            }
            hidden_under = (r["collapsed"] == true).then_some(d);
            true
        })
        .cloned()
        .collect()
}

fn row_of(h: &ExecutorHandle, name: &str) -> usize {
    visible(h).iter().position(|r| r["name"] == name).unwrap_or_else(|| panic!("no row {name}"))
}

fn layout(surface: &str) -> Value {
    json!({"surface": surface, "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"})
}

/// A sample at frame `f`'s cell centre, at `frac` of row `r`'s height.
fn at(r: usize, f: u32, frac: f64, mods: &[&str]) -> Value {
    json!({"x": f64::from(f - 1) * 16.0 + 8.0, "y": (r as f64 + frac) * 22.0, "modifiers": mods})
}

/// The command ids of a replayed gesture's plan (commands or a batch's steps).
fn commands_of(h: &ExecutorHandle, surface: &str, samples: Value) -> Vec<String> {
    let plan = h.timing_replay(Origin::User, layout(surface), samples.clone(), false).unwrap()["plan"].clone();
    let list = plan["commands"].as_array().or_else(|| plan["steps"].as_array()).cloned().unwrap_or_default();
    assert!(!list.is_empty(), "{surface} {samples}: no command ({plan})");
    list.iter().map(|c| c[0].as_str().unwrap().to_owned()).collect()
}

#[test]
fn the_timeline_gesture_table_is_the_interpreters_both_ways() {
    let table: Value = serde_json::from_str(TABLE).unwrap();
    let rows: BTreeMap<String, BTreeSet<String>> = table["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["view"] == "timeline")
        .map(|r| {
            let cmds = r["commands"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_owned()).collect();
            (r["tool"].as_str().unwrap().to_owned(), cmds)
        })
        .collect();
    // The interpreter can end only in the commands its source names: each is in a row.
    let r = tt_executor::registry().unwrap();
    let product = INTERPRETER.split("#[cfg(test)]").next().unwrap();
    let named: BTreeSet<String> = product
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|p| p.contains('.') && r.get(p).is_some_and(|e| e.spec.kind != Kind::Event))
        .map(str::to_owned)
        .collect();
    let listed: BTreeSet<String> = rows.values().flatten().cloned().collect();
    assert_eq!(named, listed, "the interpreter's commands and the table's");

    // Each row is what its gestures produce, exactly.
    let mut produced: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut add = |row: &'static str, cmds: Vec<String>| produced.entry(row).or_default().extend(cmds);
    let h = fixture();
    // The ruler: a click; a free range's start marker dragged.
    add("ruler:scrub", commands_of(&h, "ruler", json!([at(0, 12, 0.5, &[])])));
    let free = fixture();
    ok(&free, "timeline.playbackRange.set", json!({"start": 5, "end": 20}));
    add("ruler:rangeMarker", commands_of(&free, "ruler", json!([at(0, 5, 0.5, &[]), at(0, 8, 0.5, &[])])));
    // The layer list: a click on a name; a drag between rows, onto a peg, into a level, to the root from a deeper level.
    let bob = row_of(&h, "Bob");
    let peg = row_of(&h, "Bob-P");
    let background = row_of(&h, "Background");
    add("layers:select", commands_of(&h, "layers", json!([at(bob, 1, 0.5, &[])])));
    add("layers:select", commands_of(&h, "layers", json!([at(bob, 1, 0.5, &["ctrl"])])));
    add("layers:drag", commands_of(&h, "layers", json!([at(background, 1, 0.5, &[]), at(peg, 1, 0.1, &[])])));
    add("layers:drag", commands_of(&h, "layers", json!([at(background, 1, 0.5, &[]), at(peg, 1, 0.5, &[])])));
    let rows_now = visible(&h).len();
    add("layers:drag", commands_of(&h, "layers", json!([at(bob, 1, 0.5, &[]), at(rows_now + 1, 1, 0.5, &[])])));
    // The tracks: a bar's body; a keyframe; a cell click; the selection dragged.
    let peg_bar = visible(&h)[peg]["bar"].clone();
    let mid = u32::try_from((peg_bar["start"].as_u64().unwrap() + peg_bar["end"].as_u64().unwrap()) / 2).unwrap();
    add("tracks:bar", commands_of(&h, "tracks", json!([at(peg, mid, 0.7, &[]), at(peg, mid + 4, 0.7, &[])])));
    let (key_row, key) = visible(&h)
        .iter()
        .enumerate()
        .find_map(|(i, r)| r["keyframes"].as_array().and_then(|k| k.first()).map(|k| (i, k["frame"].as_u64().unwrap())))
        .expect("a row with a keyframe in the fixture");
    let key = u32::try_from(key).unwrap();
    add(
        "tracks:keyframe",
        commands_of(&h, "tracks", json!([at(key_row, key, 0.2, &[]), at(key_row, key + 3, 0.2, &[])])),
    );
    add("tracks:cells", commands_of(&h, "tracks", json!([at(bob, 4, 0.7, &[])])));
    let sel = fixture();
    ok(&sel, "cell.select", json!({"elementIds": [BOB], "from": 1, "to": 2}));
    add("tracks:moveCells", commands_of(&sel, "tracks", json!([at(bob, 1, 0.7, &[]), at(bob, 31, 0.7, &[])])));

    let seen: BTreeSet<&str> = produced.keys().copied().collect();
    let listed_rows: BTreeSet<&str> = rows.keys().map(String::as_str).collect();
    assert_eq!(seen, listed_rows, "the battery covers the table's rows");
    for (row, cmds) in &rows {
        assert_eq!(&produced[row.as_str()], cmds, "row {row}: the commands its gestures produced");
    }
}
