//! Round-two scan of other command-owned persistence effects.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod c13_support;
mod common;

use c13_support::*;
use common::*;
use serde_json::json;

#[test]
fn discovery_token_file_does_not_change_when_atomic_batch_rolls_back() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    let made = ok(&h, "agent.token.regenerate", json!({}));
    let target = std::path::PathBuf::from(made["discoveryFile"].as_str().unwrap());
    let before = std::fs::read(&target).unwrap();
    let failed = batch(
        &h,
        json!({"label": "token rollback", "steps": [
            {"id": "agent.token.regenerate", "params": {}},
            {"id": "unknown.command", "params": {}}
        ]}),
    );
    assert!(failed.error.is_some(), "{failed:?}");
    let after = std::fs::read(&target).unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(after, before, "failed atomic batch replaced the discovery token file");
}
