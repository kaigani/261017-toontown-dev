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

/// The allocator's counters and the hooks are process-wide: one test at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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

/// Ruling E-73 item 3: the revision log weighs each kept snapshot by the memory it holds alone (the entries the change
/// replaced and its own rebuilt index), within a budget, the oldest dropped first. On a 2,000-stroke drawing a small
/// change keeps a snapshot of about a megabyte: the estimate is within 1.5 times of what the allocator measures; with a
/// small budget the kept snapshots stay within it, and a revision whose snapshot was dropped still diffs (from its
/// back-patches: only slower).
#[test]
fn the_revision_log_weighs_its_snapshots_by_memory_and_keeps_them_within_the_budget() {
    let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = crowded(STROKES2);
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    ok(&h, "element.note.set", json!({ "elementId": bob, "text": "warm" }));
    let (_, kept0, bytes0) = tt_executor::hooks::log_counted(&h).unwrap();
    let (_, hist0) = tt_executor::hooks::history_counted(&h).unwrap();
    let r = Region::new(GLOBAL);
    for i in 0..40 {
        ok(&h, "element.note.set", json!({ "elementId": bob, "text": format!("n{i}") }));
    }
    let held = net(&r);
    let (_, kept1, bytes1) = tt_executor::hooks::log_counted(&h).unwrap();
    let (_, hist1) = tt_executor::hooks::history_counted(&h).unwrap();
    assert_eq!(kept1 - kept0, 40, "every snapshot fits the product's budget here");
    let estimate = (bytes1 - bytes0) + (hist1 - hist0);
    let ratio = held as f64 / estimate as f64;
    println!(
        "revision log: 40 changes on {STROKES2} strokes hold {held} B; the estimate {estimate} B ({} B per snapshot); \
         {ratio:.2} x",
        (bytes1 - bytes0) / 40
    );
    assert!((1.0 / 1.5..=1.5).contains(&ratio), "the snapshots hold {ratio:.2} times their estimate");
    let rev_early = ok(&h, "doc.state.get", json!({}))["documentRev"].as_u64().unwrap() - 30;
    // A small budget: the oldest snapshots go first; the kept ones stay within it.
    let per = (bytes1 - bytes0) / 40;
    tt_executor::hooks::set_snapshot_budget((8 * per) as u64);
    for i in 0..20 {
        ok(&h, "element.note.set", json!({ "elementId": bob, "text": format!("m{i}") }));
    }
    let (records, kept, bytes) = tt_executor::hooks::log_counted(&h).unwrap();
    tt_executor::hooks::set_snapshot_budget(0);
    assert!(kept <= 8 && bytes <= 8 * per + per / 2, "{kept} snapshots, {bytes} B against {}", 8 * per);
    assert!(records > 60, "the records are kept (10 AG-09): {records}");
    // A revision whose snapshot was dropped diffs from its back-patches.
    let diff = ok(&h, "doc.diff", json!({ "sinceRev": rev_early, "detail": "summary" }));
    assert!(diff["changes"].as_array().is_some_and(|c| !c.is_empty()), "{diff}");
}

/// The crowded drawing's strokes for the revision log's test.
const STROKES2: usize = 2_000;

/// Ruling E-77 (C52A-R3S-01): the reviewer's sequence. Two checkpoints of a crowded drawing, differing in a note,
/// restored in turn 300 times: every restore's record keeps the earlier document in `back`. The log weighs it and, past
/// the budget, releases the oldest records whole (the newest always kept whole). After the sequence, with the undo
/// history taken out (its snapshot steps hold the same documents: M11 C11.1's ticket, E-73 item 2), what the process
/// still holds of the sequence stays within the budget, the log's own count within it, and a diff from a released base
/// answers `doc.diffUnavailable` with the oldest kept revision.
#[test]
fn restores_in_turn_stay_within_the_logs_budget_and_a_released_base_is_unavailable() {
    let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = crowded(STROKES2);
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let base = ok(&h, "doc.state.get", json!({}))["documentRev"].as_u64().unwrap();
    ok(&h, "checkpoint.create", json!({ "name": "one" }));
    ok(&h, "element.note.set", json!({ "elementId": bob, "text": "two" }));
    ok(&h, "checkpoint.create", json!({ "name": "two" }));
    // The memory one earlier document holds, from the log's own estimate.
    let (_, kept_a, bytes_a) = tt_executor::hooks::log_counted(&h).unwrap();
    ok(&h, "checkpoint.restore", json!({ "name": "one" }));
    let (_, kept_b, bytes_b) = tt_executor::hooks::log_counted(&h).unwrap();
    assert!(kept_b >= kept_a);
    let per = bytes_b.saturating_sub(bytes_a).max(1);
    let budget = 12 * per;
    tt_executor::hooks::set_snapshot_budget(budget as u64);
    let r = Region::new(GLOBAL);
    for i in 0..300 {
        let name = if i % 2 == 0 { "two" } else { "one" };
        ok(&h, "checkpoint.restore", json!({ "name": name }));
    }
    let (records, kept, bytes) = tt_executor::hooks::log_counted(&h).unwrap();
    // The undo history's snapshot steps hold the same documents (M11's ticket): taken out, so what remains is the log's.
    drop(tt_executor::hooks::history_take(&h).unwrap());
    let held = net(&r);
    tt_executor::hooks::set_snapshot_budget(0);
    println!(
        "restores in turn: {records} records kept, {kept} with their document, counted {bytes} B against {budget} B \
         ({per} B per document); the process holds {held} B more after the 300 restores"
    );
    assert!(bytes <= budget + per, "the log counts {bytes} B against {budget} B");
    assert!(records < 300, "the oldest records were released: {records}");
    assert!(held <= (budget + 2 * per) as i64, "the process holds {held} B, over the budget {budget} B");
    // A base released with its record: unavailable, with the oldest kept revision.
    let e = err(&h, "doc.diff", json!({ "sinceRev": base, "detail": "summary" }));
    assert_eq!(e.code, "doc.diffUnavailable", "{e:?}");
    assert!(e.details["oldestRev"].as_u64().is_some_and(|o| o > base), "{e:?}");
    // The newest record is kept whole: a diff from the revision before the last restore answers.
    let now = ok(&h, "doc.state.get", json!({}))["documentRev"].as_u64().unwrap();
    assert!(ok(&h, "doc.diff", json!({ "sinceRev": now - 1, "detail": "summary" }))["changes"].is_array());
}
