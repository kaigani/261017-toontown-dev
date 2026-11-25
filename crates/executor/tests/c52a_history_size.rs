//! M5 C5.2a, rulings E-70 and E-73: the undo history's count (what its memory bound adds up: since E-73 an estimate of
//! the heap a step's values hold, `crates/executor/src/heap.rs`) against the memory the steps really hold. The counting
//! allocator wraps the system allocator for this test binary only (`stats_alloc`, as `c21_measurements`); the count
//! comes from the dev-only `hooks::history_counted`, and the history is taken out with `hooks::history_take` so that
//! dropping it shows what it held.
//!
//! - **Patch steps** of small (5-character notes), medium (5,000-character notes) and large values (strokes of 300
//!   points): the count is within 1.5 times of what dropping the history frees (E-73's check).
//! - **A snapshot step** (a checkpoint restore) counts only the entries it changed; the entries it shares with the live
//!   document are not counted. After the crowded drawing it shares is edited and later changes have pushed that drawing
//!   out of the revision log's window, the snapshot step alone holds it. Two runs, one with the restore and one without,
//!   the same calls otherwise: their difference is what the snapshot step holds uncounted (M11 C11.1's ticket, E-73
//!   item 2; printed, not asserted).
#![cfg(feature = "test-hooks")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "test")]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_wrap, reason = "statistics")]

mod common;

use std::alloc::System;
use std::time::Duration;

use common::*;
use serde_json::json;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use tt_executor::{ExecutorConfig, ExecutorHandle};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

/// The crowded drawing's strokes: 1,000 in CI (the response quotes a local run with 20,000, which holds about 3 GB).
const STROKES: usize = 1_000;

fn net(r: &Region<'_, System>) -> i64 {
    let c = r.change();
    c.bytes_allocated as i64 - c.bytes_deallocated as i64
}

fn note(i: usize) -> String {
    let head = format!("note {i:04} ");
    let fill = char::from(b'a' + (i % 26) as u8);
    let mut s = head.clone();
    s.extend(std::iter::repeat_n(fill, 5_000 - head.len()));
    s
}

/// The sample document with `n` simple strokes in drawing `D` (as `c23_r4_closure`'s crowded drawing), open.
fn crowded(n: usize) -> ExecutorHandle {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let index = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut drawing = serde_json::to_value(&parts.drawings[index]).unwrap();
    let template = drawing["nodes"][0]["strokes"][1].clone();
    let mut layer = drawing["nodes"][0].clone();
    layer["strokes"] = json!(
        (0..n)
            .map(|i| {
                let mut s = template.clone();
                s["id"] = json!(format!("st_e70_{i}"));
                s["points"] = json!([[i * 3, 2000, 0, 0, 0, 0], [i * 3 + 1, 2001, 0, 0, 0, 0]]);
                s
            })
            .collect::<Vec<_>>()
    );
    drawing["nodes"] = json!([layer]);
    parts.drawings[index] = tt_model::object::from_json(&drawing).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(120) }).unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    h
}

/// The memory the process holds after editing the crowded drawing and 260 changes after it (past the 256-revision window), with or without a
/// snapshot step made before; with the counted bytes of the history.
fn snapshot_run(with_restore: bool) -> (i64, usize) {
    let h = crowded(STROKES);
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    ok(&h, "checkpoint.create", json!({ "name": "e70" }));
    ok(&h, "element.note.set", json!({ "elementId": bob, "text": "changed" }));
    if with_restore {
        ok(&h, "checkpoint.restore", json!({ "name": "e70" }));
    } else {
        ok(&h, "element.note.set", json!({ "elementId": bob, "text": "" }));
    }
    let r = Region::new(GLOBAL);
    ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D, "kind": "pencil", "points": [{ "x": 0, "y": 0 }, { "x": 9, "y": 9 }] }),
    );
    for i in 0..260 {
        ok(&h, "element.note.set", json!({ "elementId": bob, "text": format!("n{i}") }));
    }
    let held = net(&r);
    let counted = tt_executor::hooks::history_counted(&h).unwrap().1;
    (held, counted)
}

/// The memory the history's steps hold against their count, for a kind of change (`change(i)` makes the i-th): after
/// 200 changes, the history is taken out of the project (`hooks::history_take`) and the counting allocator measures
/// what dropping it frees: the memory the steps really held, capacities included. Returns the ratio of the freed bytes
/// to the count.
fn ratio(label: &str, h: &ExecutorHandle, change: &dyn Fn(usize)) -> f64 {
    for i in 0..200 {
        change(i);
    }
    let (steps, counted) = tt_executor::hooks::history_counted(h).unwrap();
    let taken = tt_executor::hooks::history_take(h).unwrap();
    let r = Region::new(GLOBAL);
    drop(taken);
    let held = -net(&r);
    let ratio = held as f64 / counted as f64;
    println!(
        "{label}: {steps} steps counted {counted} B ({:.0} B per step), held {held} B ({ratio:.2} x counted)",
        counted as f64 / steps as f64
    );
    ratio
}

fn opened() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(120) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    h
}

#[test]
fn the_counted_size_against_the_memory_held() {
    // Ruling E-73: the count is within 1.5 times of what the allocator measures, for small, medium and large values.
    let h = opened();
    let element = ok(&h, "element.create", json!({ "type": "drawing", "name": "Noted" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let small = ratio("small values (5-character notes)", &h, &|i| {
        ok(&h, "element.note.set", json!({ "elementId": element, "text": format!("n{i:04}") }));
    });
    let medium = ratio("medium values (5,000-character notes)", &h, &|i| {
        ok(&h, "element.note.set", json!({ "elementId": element, "text": note(i) }));
    });
    drop(h);
    let h = opened();
    let drawing = ok(
        &h,
        "element.drawing.create",
        json!({ "elementId": "scene:\"Intro\"/element:\"Bob\"", "name": "big" }),
    )["drawingId"]
        .as_str()
        .unwrap()
        .to_owned();
    let large = ratio("large values (strokes of 300 points)", &h, &|i| {
        let points: Vec<serde_json::Value> =
            (0..300).map(|k| json!({ "x": (i * 7 + k) as f64 * 0.5, "y": ((k * 13) % 97) as f64 })).collect();
        ok(&h, "draw.stroke.add", json!({ "drawingId": drawing, "kind": "pencil", "points": points }));
    });
    drop(h);
    for (label, r) in [("small", small), ("medium", medium), ("large", large)] {
        assert!((1.0 / 1.5..=1.5).contains(&r), "{label} values: the history holds {r:.2} times its count");
    }

    // A snapshot step's uncounted shared entries.
    let (with, counted_with) = snapshot_run(true);
    let (without, counted_without) = snapshot_run(false);
    println!(
        "snapshot step ({STROKES} strokes): held after the crowded drawing's edit and 260 changes: {with} B with the restore, {without} B \
         without; the snapshot step holds {} B more, of which the bound counted {} B",
        with - without,
        counted_with as i64 - counted_without as i64
    );
    assert!(with != 0 && without != 0);
}
