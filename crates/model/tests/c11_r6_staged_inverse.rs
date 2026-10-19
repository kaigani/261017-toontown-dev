//! C11-R6-01: the staged commit's inverse must undo the canonical snapshot it returns.

use serde_json::{Value, json};
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryKey, Severity};
use tt_model::fixture;

fn write(kind: &'static str, path: &str, value: Value) -> PatchOp {
    PatchOp { op: kind, path: path.into(), from: None, value: Some(value), old_value: None, range: None, elided: false }
}

fn fixture_entry() -> Result<(Document, EntryKey, Value), String> {
    let doc = fixture::sample_document(1).map_err(|e| format!("fixture: {e:?}"))?;
    for key in doc.entry_keys() {
        if !matches!(key, EntryKey::Element(_)) {
            continue;
        }
        let entry = doc.entry_raw_json(&key).map_err(|e| e.to_string())?;
        if entry.get("exposure").and_then(Value::as_array).is_some_and(|a| a.len() >= 3) {
            return Ok((doc, key, entry));
        }
    }
    Err("fixture must contain an element with several exposure runs".into())
}

#[test]
fn staged_append_inverse_survives_run_merging() -> Result<(), String> {
    let (doc, key, entry) = fixture_entry()?;
    let runs = entry["exposure"].as_array().ok_or("exposure is not an array")?;
    let last = runs.last().ok_or("no exposure runs")?;
    let patch = [write("add", "/exposure/-", json!([last[0], 1]))];
    let (next, inverse) = doc.with_patched_entry(&key, &patch).map_err(|e| format!("forward: {e:?}"))?;
    assert!(!next.issues().iter().any(|i| i.severity == Severity::Error));
    let committed = next.entry_raw_json(&key).map_err(|e| e.to_string())?;
    assert_eq!(committed["exposure"].as_array().map(Vec::len), Some(runs.len()), "new run must merge");
    let (restored, _) = next.with_patched_entry(&key, &inverse).map_err(|e| {
        format!("C11-R6-01: inverse must target the canonical committed runs; inverse={inverse:?}; errors={e:?}")
    })?;
    assert_eq!(restored.entry_raw_json(&key).map_err(|e| e.to_string())?, entry);
    Ok(())
}

#[test]
fn staged_inverse_restores_frames_after_runs_merge() -> Result<(), String> {
    let (fixture, key, entry) = fixture_entry()?;
    let a = entry["exposure"][0][0].clone();
    let b = entry["exposure"][2][0].clone();
    assert!(a.is_string() && b.is_string() && a != b);
    let (doc, _) = fixture
        .with_patched_entry(&key, &[write("replace", "/exposure", json!([[a, 2], [b, 3]]))])
        .map_err(|e| format!("setup: {e:?}"))?;
    let before = doc.entry_raw_json(&key).map_err(|e| e.to_string())?;
    let (next, inverse) = doc
        .with_patched_entry(&key, &[write("replace", "/exposure/0/0", b.clone())])
        .map_err(|e| format!("forward: {e:?}"))?;
    assert_eq!(next.entry_raw_json(&key).map_err(|e| e.to_string())?["exposure"], json!([[b, 5]]));
    let (restored, _) = next.with_patched_entry(&key, &inverse).map_err(|e| format!("undo: {e:?}"))?;
    assert!(!restored.issues().iter().any(|i| i.severity == Severity::Error));
    let after = restored.entry_raw_json(&key).map_err(|e| e.to_string())?;
    assert_eq!(after["exposure"], before["exposure"], "C11-R6-01: undo must restore both original drawings and counts");
    assert_eq!(after, before);
    Ok(())
}
