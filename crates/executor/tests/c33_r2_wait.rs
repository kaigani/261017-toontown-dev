//! Waiting must leave the document queue available.
mod c13_support;
mod common;
use common::{fixture_with, ok};
use serde_json::json;
use std::time::{Duration, Instant};
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
    }
}
#[test]
fn waiting_caller_does_not_block_document_reads() -> Result<(), Box<dyn std::error::Error>> {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture_with(tt_executor::ExecutorConfig { budget: Duration::from_millis(500) });
    let dir = c13_support::tmp();
    let stale = dir.join(".cancel.png.tt-partial-abcdef123456");
    let inject_stale = stale.clone();
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let env = std::thread::scope(|scope| {
        let h = &h;
        let t = scope.spawn(move || {
            common::run(h, "render.range.run", json!({"from":1,"to":48,"width":32,"directory":dir,"wait":true}))
        });
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(&inject_stale, b"dead job staging").map_err(|_| "inject stale")?;
        let start = Instant::now();
        ok(h, "app.state.get", json!({}));
        assert!(start.elapsed() < Duration::from_millis(300));
        t.join().map_err(|_| "wait caller panicked")
    })?;
    let e = env.error.ok_or("expected timeout")?;
    assert_eq!(e.code, "job.waitTimeout");
    let job = e.details["jobId"].as_str().ok_or("job id")?;
    ok(&h, "job.cancel", json!({"jobId":job}));
    tt_vector::work::hooks::set_spin_micros(0);
    let record = h.jobs().wait(job, Duration::from_secs(60))?.0;
    assert_eq!(record.state, tt_executor::JobState::Cancelled);
    assert!(!stale.exists(), "cancel sweeps staging injected after the entry sweep");
    let receipt = serde_json::to_string(&record)?;
    assert!(
        receipt.contains("render.staleStagingRemoved") && receipt.contains(".cancel.png.tt-partial-abcdef123456"),
        "cancelled recovery must report swept files in warnings: {receipt}"
    );
    Ok(())
}

#[test]
fn dimension_preflight_precedes_slow_work() {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture_with(tt_executor::ExecutorConfig { budget: Duration::from_millis(100) });
    let _reset = Reset;
    let dir = c13_support::tmp();
    tt_vector::work::hooks::set_spin_micros(300_000);
    for (command, params, code) in [
        ("render.frame.run", json!({"width":2049,"height":1}), "render.inlineTooLarge"),
        (
            "inspect.contactSheet.render",
            json!({"frames":[1,2,3,4],"columns":2,"cellWidth":2048}),
            "render.inlineTooLarge",
        ),
        ("render.range.run", json!({"from":1,"to":2,"width":8192,"scale":2,"directory":dir}), "limit.exceeded"),
    ] {
        let start = Instant::now();
        assert_eq!(common::err(&h, command, params).code, code);
        assert!(start.elapsed() < Duration::from_millis(200));
    }
}

#[test]
fn encoded_byte_limit_is_checked_after_encoding() {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = common::fixture();
    ok(&h, "agent.policy.defaults.set", json!({"maxInlineBytes":1}));
    let e = common::err(&h, "render.frame.run", json!({"width":32}));
    assert_eq!(e.code, "render.inlineTooLarge");
    assert!(e.details["bytes"].as_u64().is_some_and(|n| n > 1), "{e:?}");
}
