//! Finding probe: known oversized inline dimensions must be refused before evaluation work.
mod common;

use serde_json::json;
use std::time::Duration;
use tt_executor::ExecutorConfig;

static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
    }
}

#[test]
fn oversized_inline_frame_is_refused_before_any_slow_work() {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = common::fixture_with(ExecutorConfig { budget: Duration::from_millis(100) });
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(300_000);
    let error = common::err(&h, "render.frame.run", json!({"width":2049,"height":1}));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(error.code, "render.inlineTooLarge", "ADR-0020 decision 8: dimensions are known before evaluation");
}

#[test]
fn oversized_contact_sheet_is_refused_before_any_slow_work() {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = common::fixture_with(ExecutorConfig { budget: Duration::from_millis(100) });
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(300_000);
    let error =
        common::err(&h, "inspect.contactSheet.render", json!({"frames":[1,2,3,4],"columns":2,"cellWidth":2048}));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(error.code, "render.inlineTooLarge", "ADR-0020 decision 8: sheet dimensions are known before rendering");
}
