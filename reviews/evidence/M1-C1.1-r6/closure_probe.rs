//! Evidence-only scoped probes. run_probe.py installs and removes this target.
#![allow(clippy::unwrap_used, clippy::panic, reason = "temporary evidence helpers")]

use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey, Severity};
use tt_model::{canonical, fixture};

fn op(kind: &'static str, path: &str, value: Value) -> PatchOp {
    PatchOp { op: kind, path: path.into(), value: Some(value), old_value: None, from: None, range: None, elided: false }
}

fn transfer(kind: &'static str, from: &str, path: &str) -> PatchOp {
    PatchOp { from: Some(from.into()), value: None, ..op(kind, path, Value::Null) }
}

fn canonical_bytes(v: &Value) -> Vec<u8> {
    canonical::to_canonical_string(v).unwrap().into_bytes()
}

fn record(name: &str, before: &[u8], after: &[u8]) {
    assert_eq!(before, after, "rollback changed canonical JSON: {name}");
    let dir = std::path::PathBuf::from(std::env::var("C11_R6_SNAPSHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.before.json")), before).unwrap();
    std::fs::write(dir.join(format!("{name}.after.json")), after).unwrap();
}

fn refuse(name: &str, v: &Value, patch: &[PatchOp], pointer: &str) {
    let before = canonical_bytes(v);
    let error = change::apply(v, patch).err().expect("oversized write was accepted");
    assert_eq!(error.code(), "doc.limitExceeded", "{name}: {error:?}");
    assert_eq!(error.details()["pointer"], pointer, "{name}");
    assert_eq!(error.details()["maxFrames"], 100_000, "{name}");
    assert!(change::apply_with_inverse(v, patch).is_err(), "{name}");
    assert!(change::invert_against(v, patch).is_err(), "{name}");
    record(name, &before, &canonical_bytes(v));
    println!("refused {name}: {} {}", error.code(), error.details());
}

fn round_trip(v: &Value, patch: &[PatchOp]) {
    let (next, inverse) = change::apply_with_inverse(v, patch).unwrap();
    assert_eq!(change::apply(&next, &inverse).unwrap(), *v);
}

fn element(n: u32, mouth: bool) -> Value {
    if mouth {
        json!({"placements": [{"mouthTrack": {"mouths": "G".repeat(n as usize)}}]})
    } else {
        json!({"exposure": [["dr_A", n]]})
    }
}

#[test]
fn ancestor_replacements_and_root_add_cannot_bypass() {
    for mouth in [false, true] {
        let field = if mouth { "mouths" } else { "exposure" };
        let before = json!({"scenes": [{"elements": [element(2, mouth)]}]});
        for (name, path) in [("element", "/scenes/0/elements/0"), ("list", "/scenes/0/elements"), ("root", "")] {
            let value = |n| match name {
                "element" => element(n, mouth),
                "list" => json!([element(n, mouth)]),
                _ => json!({"scenes": [{"elements": [element(n, mouth)]}]}),
            };
            for kind in ["replace", "add"] {
                round_trip(&before, &[op(kind, path, value(100_000))]);
                refuse(&format!("{field}-{name}-{kind}"), &before, &[op(kind, path, value(100_001))], path);
            }
        }
    }
}

#[test]
fn copy_move_shrink_grow_and_parent_add() {
    let before = json!({"scenes": [{"elements": [element(50_000, false), element(50_001, false)]}, {"elements": []}]});
    refuse("cross-element-copy-run", &before, &[transfer("copy", "/scenes/0/elements/0/exposure/0", "/scenes/0/elements/1/exposure/-")], "/scenes/0/elements/1/exposure/-");
    round_trip(&before, &[transfer("copy", "/scenes/0/elements/0/exposure", "/scenes/0/elements/1/exposure")]);
    round_trip(&before, &[transfer("move", "/scenes/0/elements/0", "/scenes/1/elements/-")]);
    let over = json!({"scenes": [{"elements": [element(100_001, false)]}, {"elements": []}]});
    // An invalid raw source cannot migrate unnoticed; valid Document creation would already refuse it.
    refuse("cross-scene-move-over", &over, &[transfer("move", "/scenes/0/elements/0", "/scenes/1/elements/-")], "/scenes/1/elements/-");
    let before = element(100_000, false);
    refuse("shrink-then-grow", &before, &[op("replace", "/exposure/0/1", json!(99_999)), op("add", "/exposure/-", json!(["dr_B", 2]))], "/exposure/-");
    let before = json!({"mouthTrack": {"mouths": "AB"}, "marker": 0});
    round_trip(&before, &[op("add", "/mouthTrack", json!({"mouths": "G".repeat(100_000)}))]);
    refuse("repeated-mouth-parent-add", &before, &[op("replace", "/marker", json!(1)), op("add", "/mouthTrack", json!({"mouths": "G".repeat(100_000)})), op("add", "/mouthTrack", json!({"mouths": "G".repeat(100_001)}))], "/mouthTrack");
    refuse("failed-op-before-test", &before, &[op("replace", "/marker", json!(1)), op("add", "/mouthTrack", json!({"mouths": "G".repeat(100_001)})), op("test", "/marker", json!(999))], "/mouthTrack");
}

#[test]
fn opaque_data_is_not_range_checked_and_failed_test_rolls_back() {
    let before = json!({"meta": {}, "ext": {"a/b~c": {}}, "marker": 0});
    let lookalike = json!({"exposure": [["dr_x", 100_001]], "mouthTrack": {"mouths": "G".repeat(100_001)}});
    round_trip(&before, &[op("add", "/meta", lookalike.clone()), op("add", "/ext/a~1b~0c", lookalike)]);
    let saved = canonical_bytes(&before);
    let err = change::apply(&before, &[op("replace", "/marker", json!(1)), op("test", "/marker", json!(999))]).unwrap_err();
    assert_eq!(err.code(), "doc.patchTestFailed");
    assert_eq!(err.details()["pointer"], "/marker");
    record("failed-test-after-write", &saved, &canonical_bytes(&before));
}

fn document_bytes(doc: &Document) -> Vec<u8> {
    canonical_bytes(&Value::Array(doc.entry_keys().iter().map(|k| json!({"key": format!("{k:?}"), "value": doc.entry_raw_json(k).unwrap()})).collect()))
}

fn document_refuse(name: &str, doc: &Document, key: &EntryKey, patch: &[PatchOp], code: &str) {
    let before = document_bytes(doc);
    let errors = doc.with_patched_entry(key, patch).err().expect("invalid document committed");
    assert!(errors.iter().any(|e| e.code() == code), "{name}: {errors:?}");
    assert!(errors.iter().all(|e| e.details().get("pointer").is_some()), "{name}: {errors:?}");
    record(name, &before, &document_bytes(doc));
    println!("refused {name}: {errors:?}");
}

#[test]
fn staged_document_checks_full_integrity_and_non_exposure_scene_limits() {
    let doc = fixture::sample_document(1).unwrap();
    assert!(!doc.issues().iter().any(|i| i.severity == Severity::Error));
    document_refuse("project-dangling-ref", &doc, &EntryKey::Project, &[op("replace", "/scenes/0/id", json!("sc_missing"))], "ref.dangling");
    document_refuse("future-revision", &doc, &EntryKey::Project, &[op("replace", "/rev", json!(999_999))], "arg.invalid");
    document_refuse("id-immutable", &doc, &EntryKey::Project, &[op("replace", "/id", json!("pr_changed"))], "arg.invalid");
    let (key, entry) = doc.entry_keys().into_iter().filter(|k| matches!(k, EntryKey::Element(_)))
        .map(|k| { let v=doc.entry_raw_json(&k).unwrap(); (k,v) })
        .find(|(_,v)| v.get("start").is_some() && v.get("duration").is_some()).unwrap();
    let at = op("replace", "/duration", json!(100_001 - entry["start"].as_u64().unwrap()));
    let (valid, _) = doc.with_patched_entry(&key, &[at]).unwrap();
    assert!(!valid.issues().iter().any(|i| i.severity == Severity::Error));
    document_refuse("bar-duration-limit", &doc, &key, &[op("replace", "/duration", json!(100_002 - entry["start"].as_u64().unwrap()))], "arg.invalid");
    for key in doc.entry_keys().into_iter().filter(|k| matches!(k, EntryKey::Element(_))) {
        let entry = doc.entry_raw_json(&key).unwrap();
        if entry.get("exposure").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
            let mut over = entry.clone();
            over["exposure"][0][1] = json!(100_001);
            document_refuse("real-element-root-replace", &doc, &key, &[op("replace", "", over)], "doc.limitExceeded");
            break;
        }
    }
    let (next, _) = doc.with_patched_entry(&EntryKey::Project, &[op("add", "/meta", json!({"exposure": [["x",100_001]], "mouthTrack": {"mouths":"G".repeat(100_001)}}))]).unwrap();
    assert!(!next.issues().iter().any(|i| i.severity == Severity::Error));
}

#[test]
fn observe_inverse_after_normalization() {
    let doc = fixture::sample_document(1).unwrap();
    for name in ["Renamed", "  Renamed  ", "Cafe\u{301}"] {
        let (next, inverse) = doc.with_patched_entry(&EntryKey::Project, &[op("replace", "/name", json!(name))]).unwrap();
        let undo = next.with_patched_entry(&EntryKey::Project, &inverse);
        println!("inverse normalization observation: input={name:?}, committed={:?}, inverse={inverse:?}, undo_errors={:?}", next.entry_raw_json(&EntryKey::Project).unwrap()["name"], undo.as_ref().err());
        if name == "Renamed" {
            assert_eq!(document_bytes(&undo.unwrap().0), document_bytes(&doc));
        }
    }
}

#[test]
fn observe_inverse_after_exposure_canonicalization() {
    let doc = fixture::sample_document(1).unwrap();
    for key in doc.entry_keys().into_iter().filter(|k| matches!(k, EntryKey::Element(_))) {
        let entry=doc.entry_raw_json(&key).unwrap();
        let Some(runs)=entry.get("exposure").and_then(Value::as_array).filter(|a| !a.is_empty()) else { continue };
        let last=runs.last().unwrap().clone();
        let (next,inverse)=doc.with_patched_entry(&key, &[op("add", "/exposure/-", json!([last[0],1]))]).unwrap();
        let saved=document_bytes(&next);
        let undo=next.with_patched_entry(&key,&inverse);
        if undo.is_err() { record("inverse-after-merge-refusal", &saved, &document_bytes(&next)); }
        println!("exposure inverse observation: before={:?}; committed={:?}; inverse={inverse:?}; undo_errors={:?}", entry["exposure"],next.entry_raw_json(&key).unwrap()["exposure"],undo.as_ref().err());
        break;
    }
}
