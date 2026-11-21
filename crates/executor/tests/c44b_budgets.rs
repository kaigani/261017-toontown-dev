//! M4 C4.4b, keystroke to glyph (ruling D-76 item 2): the engine's own work per keystroke, in release. Each keystroke
//! of a Text tool session is the work the window's glyph waits for: the input (`text_input`), the scene answer and the
//! current drawing's payload. `TT_MEASURE=1 cargo test --release -p tt-executor --test c44b_budgets -- --ignored
//! --nocapture` writes `target/evidence/c44b/keystroke-engine.json`.
//!
//! In CI (`measurements release`) the test asserts the shape only (ruling D-45: no wall-clock bound on a shared runner
//! without four times its headroom): 30 keystrokes, each one revision and one answer, n, p50, p95 and max computed from
//! the population, and the work not growing with the text (the last ten keystrokes' median at most four times the first
//! ten's, plus 1 ms). With `TT_ASSERT_BUDGET=1` (the workstation receipt) it also asserts D-76's bound, at most 5 ms p95.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::Instant;

use common::*;
use serde_json::json;
use tt_executor::text_edit::Input;
use tt_executor::{Origin, Status};

const BOB: &str = "scene:#1/element:\"Bob\"";
/// D-76 item 2: the engine's own work per keystroke, p95.
const BOUND_MS: f64 = 5.0;
const KEYS: &str = "the quick brown fox jumps over";

fn stat(v: &[f64]) -> (usize, f64, f64, f64) {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let at = |q: f64| s[((s.len() as f64 * q).ceil() as usize).max(1) - 1];
    (s.len(), at(0.5), at(0.95), *s.last().unwrap())
}

#[test]
#[ignore = "release-only measurement; CI runs it in `measurements release`"]
fn the_engines_work_per_keystroke() {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    // A Text tool click on empty canvas (the headless Drawing view: drawing unit -> view pixel is (x + 960, 540 - y)).
    let click = vec![
        json!({ "type": "down", "x": 260.0, "y": 940.0, "modifiers": [] }),
        json!({ "type": "up", "x": 260.0, "y": 940.0, "modifiers": [] }),
    ];
    let env = h.pointer(Origin::User, json!({ "view": "drawing", "tool": "text", "events": click }));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    // Warm the font database and the payload path once (the window's first glyph is not a keystroke of the run).
    let _ = h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
    let mut work = Vec::new();
    let mut parts = Vec::new();
    for c in KEYS.chars() {
        let rev0 = rev(&h);
        let t = Instant::now();
        let env = h.text_input(Origin::User, Input::Text(c.to_string()));
        let input = t.elapsed();
        let scene = h.canvas_scene("drawingView").unwrap();
        let after_scene = t.elapsed();
        let payload = h.canvas_drawing(None, 1, 0, Default::default()).unwrap();
        let total = t.elapsed();
        assert_eq!(env.status, Status::Ok, "{env:?}");
        assert_eq!(rev(&h), rev0 + 1, "one revision per keystroke");
        assert!(!payload.bytes.is_empty() && serde_json::to_value(&scene).is_ok());
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        work.push(ms(total));
        parts.push(json!({ "key": c.to_string(), "inputMs": ms(input), "sceneMs": ms(after_scene - input),
                           "payloadMs": ms(total - after_scene), "payloadBytes": payload.bytes.len() }));
    }
    let (n, p50, p95, max) = stat(&work);
    let first = stat(&work[..10]).1;
    let last = stat(&work[work.len() - 10..]).1;
    let record = json!({ "n": n, "p50": p50, "p95": p95, "max": max, "boundMs": BOUND_MS, "firstTenP50": first,
                         "lastTenP50": last, "keystrokes": parts });
    println!("{record}");
    if std::env::var_os("TT_MEASURE").is_some() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c44b");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("keystroke-engine.json"), serde_json::to_string_pretty(&record).unwrap()).unwrap();
    }
    // The shape (D-45).
    assert_eq!(n, KEYS.chars().count());
    assert!(work.iter().all(|w| w.is_finite() && *w > 0.0), "{work:?}");
    assert!(p50 <= p95 && p95 <= max, "{record}");
    assert!(last <= 4.0 * first + 1.0, "the work per keystroke grows with the text: {record}");
    // D-76's bound, on the workstation receipt.
    if std::env::var_os("TT_ASSERT_BUDGET").is_some() {
        assert!(p95 <= BOUND_MS, "the engine's work per keystroke {p95:.2} ms p95 > {BOUND_MS} ms: {record}");
    }
}
