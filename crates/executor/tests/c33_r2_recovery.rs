//! Independent round-two recovery and evaluator probes.
mod c13_support;
mod common;
use common::{fixture, ok};
use serde_json::json;
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn recovery_preserves_non_product_marker_files() -> Result<(), Box<dyn std::error::Error>> {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    let dir = c13_support::tmp();
    let name = ".artist-notes.tt-partial-draft";
    std::fs::write(dir.join(name), b"user working material")?;
    c13_support::job_ok(&h, "render.range.run", json!({"from":1,"to":1,"width":32,"directory":dir.to_string_lossy()}));
    assert_eq!(std::fs::read(dir.join(name))?, b"user working material");
    Ok(())
}

#[test]
fn contact_sheet_evaluates_each_node_once_per_frame() {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    tt_anim::eval::hooks::start();
    ok(&h, "inspect.contactSheet.render", json!({"frames":[1,3,5],"cellWidth":32}));
    let mut counts = std::collections::BTreeMap::new();
    for key in tt_anim::eval::hooks::take() {
        *counts.entry((key.0, key.1.to_bits())).or_insert(0usize) += 1;
    }
    assert!(!counts.is_empty());
    assert!(counts.values().all(|n| *n == 1), "{counts:?}");
}

#[test]
fn stale_stage_is_reported_and_orphan_backup_restored() -> Result<(), Box<dyn std::error::Error>> {
    let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    let dir = c13_support::tmp();
    let partial = ".dead.png.tt-partial-012345abcdef";
    let backup = ".original.png.tt-backup-abcdef012345";
    dead_sidecar(&dir, "012345abcdef", "dead.png")?;
    dead_sidecar(&dir, "abcdef012345", "original.png")?;
    std::fs::write(dir.join(partial), b"never publish")?;
    std::fs::write(dir.join(backup), b"original bytes")?;
    std::fs::write(dir.join("artist.txt"), b"untouched")?;
    let m = c13_support::job_ok(&h, "render.range.run", json!({"from":1,"to":1,"width":32,"directory":dir}));
    assert_eq!(std::fs::read(dir.join("original.png"))?, b"original bytes");
    assert_eq!(std::fs::read(dir.join("artist.txt"))?, b"untouched");
    assert!(!dir.join(partial).exists() && !dir.join(backup).exists() && !dir.join("dead.png").exists());
    let warnings = m["warnings"].as_array().ok_or("warnings")?;
    let swept = warnings.iter().find(|w| w["code"] == "render.staleStagingRemoved").ok_or("sweep warning")?;
    let files = swept["files"].as_array().ok_or("files")?;
    assert!(files.contains(&json!(partial)) && files.contains(&json!(backup)));
    Ok(())
}

fn dead_sidecar(dir: &std::path::Path, token: &str, target: &str) -> std::io::Result<()> {
    let body = json!({"kind":"toontown.render.range.staging","version":1,"token":token,
        "jobId":"job-dead-review","head":"unknown","productVersion":"0.0.1",
        "host":tt_persist::host::host(),"pid":std::process::id(),"published":false,
        "targets":[target],"staged":[format!(".{target}.tt-partial-{token}")],
        "backups":[format!(".{target}.tt-backup-{token}")]});
    std::fs::write(dir.join(format!(".tt-partial-{token}.job")), body.to_string())
}
