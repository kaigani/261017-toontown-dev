//! Independent committed cancellation and refused rollback over MCP.
mod support;
use serde_json::json;
#[test]
fn committed_cancel_refused_restore_through_mcp() -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        path::Path,
        sync::{Arc, Mutex},
    };
    use tt_persist::lock::{Boundary, hooks};
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            hooks::set(None);
        }
    }
    let _reset = Reset;
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).map_err(|e| format!("{e:?}"))?, None, false).is_ok());
    let out = dir.join("out");
    std::fs::create_dir(&out)?;
    std::fs::write(out.join("r-1.png"), b"MCP original")?;
    let hook_h = h.clone();
    let answers = Arc::new(Mutex::new(Vec::new()));
    let hook_answers = answers.clone();
    hooks::set(Some(Arc::new(move |at: Boundary, path: &Path| {
        if at == Boundary::RangePublish && path.file_name().is_some_and(|n| n == "r-2.png") {
            for record in hook_h.jobs().list(None, None) {
                if record.command_id == "render.range.run"
                    && let Ok(answer) = hook_h.jobs().cancel(&record.job_id)
                    && let Ok(mut answers) = hook_answers.lock()
                {
                    answers.push(answer.state);
                }
            }
            let _ = std::fs::create_dir(path);
        }
        if at == Boundary::RangeRestore && path.file_name().is_some_and(|n| n == "r-1.png") {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::create_dir(path);
        }
    })));
    let (mut client, _) = support::Client::connect(&h, support::opts(&dir), json!({}));
    let params = json!({"from":1,"to":2,"width":32,"directory":out,"namePattern":"r-{frame}","onConflict":"overwrite"});
    let asked = client.run("render.range.run", params.clone());
    let token = asked["result"]["confirmToken"].as_str().ok_or_else(|| format!("confirmation: {asked}"))?;
    let started = client.call("run_command", json!({"id":"render.range.run","params":params,"confirmToken":token}))["structuredContent"].clone();
    let id = started["result"]["jobId"].as_str().ok_or_else(|| format!("job id: {started}"))?;
    h.jobs().wait(id, std::time::Duration::from_secs(60))?;
    let answer = client.call("job_status", json!({"jobId":id}));
    client.close()?;
    hooks::set(None);
    assert_eq!(*answers.lock().map_err(|_| "answers poisoned")?, vec![tt_executor::JobState::Running]);
    let result = &answer["structuredContent"]["result"];
    let row = result.get("record").unwrap_or(result);
    let row = if row.is_array() { &row[0] } else { row };
    assert_eq!(row["state"], "failed", "{answer}");
    assert_eq!(row["error"]["code"], "export.rollbackIncomplete", "{answer}");
    assert!(row["error"]["details"]["unrestored"].to_string().contains("r-1.png"));
    assert!(row["warnings"].to_string().contains("render.stagingDiscarded"));
    Ok(())
}
