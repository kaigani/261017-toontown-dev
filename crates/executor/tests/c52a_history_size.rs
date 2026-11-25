//! M5 C5.2a response, ruling E-70 (the history, from the static report's observations): how far the undo history's
//! counted size (what its memory bound adds up: a patch's serialised size, a snapshot step's changed entries) is from the
//! memory a step really holds. A receipt, not a gate: it prints the numbers the response quotes and asserts only that
//! they were measured. The counting allocator wraps the system allocator for this test binary only (`stats_alloc`, as
//! `c21_measurements`); the counted size comes from the dev-only `hooks::history_counted`.
//!
//! - **Patch steps.** 200 note changes of 5,000 characters, then 200 more with a bound so small that the history stays at
//!   its 200-step floor: the second half's growth is the revision log's (each change's back patches, kept for 10,000
//!   revisions), so the history's own memory per step is the first half's growth less the log's share.
//! - **A snapshot step** (a checkpoint restore) counts only the entries it changed; the entries it shares with the live
//!   document are not counted. After the crowded drawing it shares is edited, and enough later changes have pushed that
//!   drawing out of the revision log's snapshot window (256 revisions), the snapshot step alone holds it. Two runs, one
//!   with the restore and one without, the same calls otherwise: their difference is what the snapshot step holds that
//!   the bound never counted.
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

#[test]
fn the_counted_size_against_the_memory_held() {
    // Patch steps.
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(120) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let element = ok(&h, "element.create", json!({ "type": "drawing", "name": "Noted" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    for i in 0..5 {
        ok(&h, "element.note.set", json!({ "elementId": element, "text": note(i) }));
    }
    let (n0, c0) = tt_executor::hooks::history_counted(&h).unwrap();
    let r = Region::new(GLOBAL);
    for i in 5..205 {
        ok(&h, "element.note.set", json!({ "elementId": element, "text": note(i) }));
    }
    let held1 = net(&r);
    let (n1, c1) = tt_executor::hooks::history_counted(&h).unwrap();
    tt_executor::hooks::set_history_bound(1);
    for i in 205..405 {
        ok(&h, "element.note.set", json!({ "elementId": element, "text": note(i) }));
    }
    tt_executor::hooks::set_history_bound(0);
    let held2 = net(&r);
    let (n2, _) = tt_executor::hooks::history_counted(&h).unwrap();
    assert_eq!(n2, 200, "the history stayed at its 200-step floor in the second half");
    let counted_per_step = (c1 - c0) as f64 / (n1 - n0) as f64;
    let log_per_change = (held2 - held1) as f64 / 200.0;
    let history_per_step = held1 as f64 / 200.0 - log_per_change;
    println!(
        "patch steps: counted {counted_per_step:.0} B per step; held per change {:.0} B, of which the revision log \
         {log_per_change:.0} B and the history {history_per_step:.0} B ({:.2} x counted)",
        held1 as f64 / 200.0,
        history_per_step / counted_per_step
    );
    assert!(counted_per_step > 10_000.0 && held1 > 0);
    drop(h);

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
