//! Round-two scan of other command-owned persistence effects.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod c13_support;
mod common;

use c13_support::*;
use common::*;
use serde_json::json;

#[test]
fn restart_does_not_publish_a_handcrafted_uncommitted_stage() {
    let dir = tmp();
    let target = dir.join("activity.jsonl");
    std::fs::write(&target, b"old").unwrap();
    let staged = tt_persist::fsx::temp_sibling(&target);
    std::fs::write(&staged, b"uncommitted").unwrap();
    let h = spawn(&dir.join("user"));
    assert_eq!(std::fs::read(&target).unwrap(), b"old");
    drop(h);
    assert_eq!(std::fs::read(&target).unwrap(), b"old");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn media_export_is_not_published_by_a_failed_atomic_batch() {
    use std::sync::Arc;
    use tt_persist::container::{BlobData, Blobs, Form};
    let dir = tmp();
    let doc = tt_model::fixture::sample_document(901).unwrap();
    let mut blobs = Blobs::default();
    for (name, _, _, _) in tt_persist::load::blob_refs(&doc) {
        blobs.insert(name, BlobData::Bytes(Arc::new(vec![7; 64])));
    }
    let package = dir.join("Media.animpkg");
    tt_persist::save::save(
        &package,
        Form::Package,
        &tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-30T00:00:00.000Z".into(),
        },
        None,
        false,
    )
    .unwrap();
    let h = spawn(&dir.join("user"));
    job_ok(&h, "project.open", json!({"path": package.to_string_lossy()}));
    let asset = ok(&h, "doc.asset.list", json!({}))["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["storage"] == "embedded")
        .unwrap()["id"]
        .clone();
    let target = dir.join("media.bin");
    let failed = batch(
        &h,
        json!({"label": "media rollback", "steps": [
            {"id": "doc.asset.export", "params": {"assetId": asset, "path": target.to_string_lossy()}},
            {"id": "unknown.command", "params": {}}
        ]}),
    );
    assert!(failed.error.is_some(), "{failed:?}");
    let published = target.exists();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    assert!(!published, "failed atomic batch published doc.asset.export");
}

#[test]
fn json_export_is_not_published_by_a_failed_atomic_batch() {
    let dir = tmp();
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({"name": "Export"}));
    let target = dir.join("bundle.json");
    let failed = batch(
        &h,
        json!({"label": "export rollback", "steps": [
            {"id": "doc.export.json", "params": {"scope": "project", "path": target.to_string_lossy()}},
            {"id": "unknown.command", "params": {}}
        ]}),
    );
    assert!(failed.error.is_some(), "{failed:?}");
    let published = target.exists();
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!published, "failed atomic batch published doc.export.json");
}

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
