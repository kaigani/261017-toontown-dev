//! Evidence-only closure probes. Installed temporarily by run_probes.py.
#![allow(clippy::unwrap_used, clippy::panic, reason = "evidence-only assertions")]
use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey, Severity};
use tt_model::{fixture, types::ObjectType};

fn op(kind: &'static str, path: &str, value: Value) -> PatchOp {
    PatchOp { op: kind, path: path.into(), from: None, value: Some(value), old_value: None, range: None, elided: false }
}

fn bytes(value: &Value) -> Vec<u8> { serde_json::to_vec(value).unwrap() }
fn snapshot(doc: &Document) -> Vec<(String, Vec<u8>)> {
    doc.entry_keys().iter().map(|k| (format!("{k:?}"), bytes(&doc.entry_raw_json(k).unwrap()))).collect()
}

fn find(doc: &Document, field: &str) -> (EntryKey, Value) {
    doc.entry_keys().into_iter().filter(|k| matches!(k, EntryKey::Element(_)))
        .find_map(|k| {
            let v = doc.entry_raw_json(&k).unwrap();
            let yes = if field == "exposure" { v[field].as_array().is_some_and(|a| a.len() >= 3) }
                else { v.pointer("/placements/0/mouthTrack/mouths").is_some() };
            yes.then_some((k, v))
        }).unwrap()
}

fn check(label: &str, doc: &Document, key: &EntryKey, patch: &[PatchOp], require_restructure: bool) -> Value {
    let all_pre = snapshot(doc);
    let pre = doc.entry_raw_json(key).unwrap();
    let staged = change::apply(&pre, patch).unwrap();
    let (next, undo) = doc.with_patched_entry(key, patch).unwrap();
    let committed = next.entry_raw_json(key).unwrap();
    if require_restructure { assert_ne!(bytes(&staged), bytes(&committed), "{label}"); }
    assert!(!next.issues().iter().any(|i| i.severity == Severity::Error));
    assert_eq!(bytes(&change::apply(&committed, &undo).unwrap()), bytes(&pre), "raw undo {label}");
    let (back, redo) = next.with_patched_entry(key, &undo).unwrap();
    assert_eq!(snapshot(&back), all_pre, "full snapshot undo {label}");
    assert_eq!(bytes(&change::apply(&pre, &redo).unwrap()), bytes(&committed), "raw redo {label}");
    let (again, _) = back.with_patched_entry(key, &redo).unwrap();
    assert_eq!(snapshot(&again), snapshot(&next), "canonical redo {label}");
    let (replayed, _) = back.with_patched_entry(key, patch).unwrap();
    assert_eq!(snapshot(&replayed), snapshot(&next), "original forward replay {label}");
    let ty = ObjectType::parse(pre["type"].as_str().unwrap()).unwrap();
    let canonical_forward = change::diff_json(&pre, &committed, "", ty);
    assert_eq!(bytes(&change::apply(&pre, &canonical_forward).unwrap()), bytes(&committed), "canonical forward {label}");
    assert_eq!(snapshot(doc), all_pre, "immutable source {label}");
    println!("CASE {label}: staged_differs={} undo_ops={} redo_ops={} canonical_forward_ops={} raw_undo/exact_redo/original_replay/full_snapshot=true",
        staged != committed, undo.len(), redo.len(), canonical_forward.len());
    committed
}

#[test]
fn fresh_exposure_shapes_and_nested_canonicalization() {
    let fixture = fixture::sample_document(731927).unwrap();
    let (key, v) = find(&fixture, "exposure");
    let a = v["exposure"][0][0].clone();
    let b = v["exposure"][2][0].clone();
    let (doc, _) = fixture.with_patched_entry(&key, &[op("replace", "/exposure", json!([[a, 7], [b, 11]]))]).unwrap();
    check("merge-neighbors", &doc, &key, &[op("replace", "/exposure/0/0", b.clone())], true);
    check("split-run", &doc, &key, &[op("replace", "/exposure", json!([[a, 2], [b, 3], [a, 2], [b, 11]]))], false);
    let got = check("equivalent-noncanonical", &doc, &key, &[op("replace", "/exposure", json!([[a, 3], [a, 4], [b, 5], [b, 6], [null, 2]]))], true);
    assert_eq!(got["exposure"], json!([[a, 7], [b, 11]]));
    check("append-and-merge", &doc, &key, &[op("add", "/exposure/-", json!([b, 4]))], true);
    check("append-trailing-blank", &doc, &key, &[op("add", "/exposure/-", json!([null, 9]))], true);
    let pre = doc.entry_raw_json(&key).unwrap();
    let channel = pre["channels"].as_object().unwrap().iter().find(|(_, v)| v["keys"].is_array()).unwrap();
    let name_path = format!("/channels/{}/name", channel.0);
    let patch = [op("replace", "/exposure", json!([[b, 2], [b, 6], [null, 3]])),
        op("replace", "/name", json!("  Cafe\u{301}  ")),
        op("replace", &name_path, json!(format!("  {}  ", channel.0)))];
    let staged = change::apply(&pre, &patch).unwrap();
    let got = check("three-canonicalization-sites", &doc, &key, &patch, true);
    assert_ne!(got["exposure"], staged["exposure"]);
    assert_ne!(got["name"], staged["name"]);
    assert_ne!(got.pointer(&name_path), staged.pointer(&name_path));
    // A whole-entry write also runs normalization and the complete gate.
    check("whole-entry-nested", &doc, &key, &[op("replace", "", staged)], true);
}

#[test]
fn length_changing_mouth_tracks_and_inline_content() {
    let doc = fixture::sample_document(824731).unwrap();
    let (key, pre) = find(&doc, "mouths");
    for (label, mouths) in [("mouth-grow", "XABCDEFGXABX"), ("mouth-shrink", "XABX")] {
        let mut placement = pre["placements"][0].clone();
        placement["mouthTrack"]["mouths"] = json!(mouths);
        placement["stopFrame"] = json!(mouths.len());
        placement["mouthTrack"]["producedFor"]["stopFrame"] = json!(mouths.len());
        placement["mouthTrack"]["labels"]["1"] = json!("  Cafe\u{301}  ");
        check(label, &doc, &key, &[op("replace", "/placements/0", placement)], true);
    }
    let mut placement = pre["placements"][0].clone();
    placement["volume"] = json!(-7);
    placement["envelope"][1]["left"] = json!(0.7);
    placement["envelope"][1]["right"] = json!(0.8);
    placement["mouthTrack"]["labels"]["1"] = json!("e\u{301}");
    let got = check("inline-child-multiple-content-changes", &doc, &key,
        &[op("replace", "/placements/0", placement), op("replace", "/name", json!("  Voice\u{301}  "))], true);
    assert_eq!(got["placements"][0]["volume"], -7);
}

#[test]
fn full_gate_stays_atomic() {
    let doc = fixture::sample_document(912045).unwrap();
    let (key, _) = find(&doc, "exposure");
    let before = snapshot(&doc);
    for (label, patch) in [
        ("structural-limit", vec![op("replace", "/name", json!("changed")), op("replace", "/exposure/0/1", json!(100001))]),
        ("dangling-drawing", vec![op("replace", "/exposure/0/0", json!("missing_drawing"))]),
        ("immutable-id", vec![op("replace", "/id", json!("changed_entry_id"))]),
        ("failing-test", vec![op("replace", "/name", json!("changed")), op("test", "/name", json!("wrong"))]),
    ] {
        let errors = doc.with_patched_entry(&key, &patch).unwrap_err();
        assert!(!errors.is_empty());
        assert_eq!(snapshot(&doc), before);
        println!("REFUSAL {label}: {} immutable=true", errors[0]);
    }
}

// run_probes.py inserts the exact production self-check block in this function.
fn verified_candidate(before: Value, committed: Value, mut undo: Vec<PatchOp>) -> Vec<PatchOp> {
    // PRODUCTION_SELF_CHECK
    undo
}

#[test]
fn self_check_falls_back_for_wrong_and_unapplicable_diff() {
    let doc = fixture::sample_document(316971).unwrap();
    let (key, before) = find(&doc, "exposure");
    let (next, correct) = doc.with_patched_entry(&key, &[op("replace", "/name", json!("changed"))]).unwrap();
    let committed = next.entry_raw_json(&key).unwrap();
    assert_eq!(verified_candidate(before.clone(), committed.clone(), correct.clone()), correct);
    for (label, candidate) in [("wrong-success", vec![]),
        ("apply-error", vec![op("replace", "/does-not-exist", json!(1))])] {
        assert_ne!(change::apply(&committed, &candidate).ok().as_ref(), Some(&before));
        let undo = verified_candidate(before.clone(), committed.clone(), candidate);
        assert_eq!(undo.len(), 1);
        assert_eq!(undo[0].path, "");
        assert_eq!(undo[0].op, "replace");
        assert_eq!(undo[0].old_value.as_ref(), Some(&committed));
        assert_eq!(bytes(&change::apply(&committed, &undo).unwrap()), bytes(&before));
        let (back, redo) = next.with_patched_entry(&key, &undo).unwrap();
        assert_eq!(snapshot(&back), snapshot(&doc));
        assert_eq!(bytes(&change::apply(&before, &redo).unwrap()), bytes(&committed));
        println!("FALLBACK {label}: source-extracted production guard fired; root replace; raw and staged undo/redo exact; no panic/error surfaced");
    }
}
