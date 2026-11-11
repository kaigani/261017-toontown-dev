//! Independent colour refusal and atomic-job boundary probes.
mod common;
use common::{fixture, ok, run};
use serde_json::json;

fn canonical_document(h: &tt_executor::ExecutorHandle) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut exported = ok(h, "doc.export.json", json!({"scope":"project","binaries":"omit"}));
    // Export time is receipt metadata, not document content. Preserve every other field.
    exported["bundle"].as_object_mut().ok_or("bundle object required")?.remove("exportedAt");
    Ok(exported)
}

#[test]
fn sequential_wait_timeout_retains_job_identity() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            tt_vector::work::hooks::set_spin_micros(0);
        }
    }
    let h = common::fixture_with(tt_executor::ExecutorConfig { budget: Duration::from_millis(500) });
    let dir = std::env::temp_dir().join(format!("c34-r1-wait-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let batch = serde_json::from_value(
        json!({"label":"bounded wait","atomic":false,"steps":[{"id":"render.range.run","params":{"from":1,"to":48,"width":32,"directory":dir,"wait":true}}]}),
    )?;
    let env = h.batch(batch, tt_executor::Origin::User);
    let steps = env.steps.ok_or("step envelopes required")?;
    let error = steps.first().and_then(|s| s.error.as_ref()).ok_or("typed step timeout required")?;
    assert_eq!(error.code, "job.waitTimeout");
    let job = error.details["jobId"].as_str().ok_or("job identity required")?;
    assert!(error.details["timeoutMs"].as_u64().is_some_and(|n| n <= 450));
    h.jobs().cancel(job)?;
    tt_vector::work::hooks::set_spin_micros(0);
    let record = h.jobs().wait(job, Duration::from_secs(60))?.0;
    assert_eq!(record.state, tt_executor::JobState::Cancelled);
    Ok(())
}

#[test]
fn swatch_refusals_preserve_canonical_document_and_atomic_jobs_are_refused() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let before = canonical_document(&h)?;
    for command in ["draw.ellipse.add", "colour.paint.apply"] {
        let params = if command == "draw.ellipse.add" {
            json!({"drawingId":"dr_01K742SG0KKMCAP9X1E6Z5P4SW", "from":{"x":0,"y":0},"to":{"x":20,"y":20},"colourId":"palette:#1/swatch:#99999"})
        } else {
            json!({"drawingId":"dr_01K742SG0KKMCAP9X1E6Z5P4SW", "points":[{"x":0,"y":0}],"colourId":"palette:#1/swatch:#99999"})
        };
        let env = run(&h, command, params);
        assert_eq!(env.error.ok_or("expected typed refusal")?.code, "colour.notFound");
        assert_eq!(canonical_document(&h)?, before);
    }
    let batch = serde_json::from_value(
        json!({"label":"atomic job boundary","atomic":true,"steps":[{"id":"render.range.run","params":{"from":1,"to":2,"directory":"unused","wait":true}}]}),
    )?;
    let env = h.batch(batch, tt_executor::Origin::User);
    assert!(serde_json::to_string(&env)?.contains("batch.jobNotAllowed"));
    assert_eq!(canonical_document(&h)?, before);
    Ok(())
}
