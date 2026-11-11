//! Independent terminal job diagnostics across the MCP transport.
mod support;
use serde_json::json;
#[test]
fn cancelled_error_is_visible_through_job_status() -> Result<(), Box<dyn std::error::Error>> {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).map_err(|e| format!("{e:?}"))?, None, false).is_ok());
    std::fs::create_dir(dir.join("out"))?;
    let (mut client, _) = support::Client::connect(&h, support::opts(&dir), json!({}));
    let started = client.run("render.range.run", json!({"from":1,"to":48,"width":512,"directory":dir.join("out")}));
    let id = started["result"]["jobId"].as_str().ok_or_else(|| format!("job id: {started}"))?;
    h.jobs().cancel(id)?;
    let rec = h.jobs().wait(id, std::time::Duration::from_secs(60))?.0;
    assert_eq!(rec.state, tt_executor::JobState::Cancelled);
    let answer = client.call("job_status", json!({"jobId":id}));
    client.close()?;
    let receipt = answer["structuredContent"]["result"].to_string();
    assert!(receipt.contains("render.stagingDiscarded"), "{answer}");
    let result = &answer["structuredContent"]["result"];
    let row = result.get("record").unwrap_or(result);
    let row = if row.is_array() { &row[0] } else { row };
    assert_eq!(row["state"], "cancelled", "{answer}");
    assert_eq!(row["error"]["code"], "job.cancelled", "{answer}");
    Ok(())
}
