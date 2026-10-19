//! C11-R5-01: a patch is committed only through a staged copy whose full integrity check passes.
//!
//! Random RFC 6902 sequences (add with overwrite or `-` append, remove, replace, move, copy, test) are applied to a
//! random entry of a seeded sample document through `Document::with_patched_entry`. Values are drawn from a pool
//! that includes frame-limit edge cases (100 000 / 100 001 frames, huge counts, zero counts, long mouth strings),
//! invalid names, wrong types and ID changes. Whatever the outcome, the original document stays byte-identical, and
//! a committed document never has an error-severity issue.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers")]

use proptest::prelude::*;
use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey, Severity};
use tt_model::{fixture, pointer};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn all_pointers(v: &Value, at: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    out.push(at.clone());
    match v {
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                at.push(i.to_string());
                all_pointers(x, at, out);
                at.pop();
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                at.push(k.clone());
                all_pointers(x, at, out);
                at.pop();
            }
        }
        _ => {}
    }
}

fn value(rng: &mut Rng, current: Option<&Value>) -> Value {
    match rng.below(16) {
        0 => json!(100_000),
        1 => json!(100_001),
        2 => json!(50_001),
        3 => json!(0),
        4 => json!(2_147_483_648u64),
        5 => json!([["dr_staged", 100_001]]),
        6 => json!([[null, 1]]),
        7 => json!(["dr_staged", 1]),
        8 => json!("G".repeat(if rng.below(4) == 0 { 100_001 } else { 3 })),
        9 => json!("  Renamed  "),
        10 => json!("bad/name"),
        11 => Value::Null,
        12 => json!({"k": 1}),
        13 => json!(true),
        // Small perturbations of the current value keep many patches valid.
        14 => match current {
            Some(Value::Number(n)) if n.is_u64() => json!(n.as_u64().unwrap_or(0) + 1),
            Some(Value::String(s)) => json!(format!("{s}x")),
            Some(v) => v.clone(),
            None => json!(1),
        },
        _ => json!(rng.below(10)),
    }
}

/// A random patch of up to `n` operations that applies to `doc` as JSON (operations that do not apply are skipped).
fn random_patch(doc: &Value, rng: &mut Rng, n: usize) -> Vec<PatchOp> {
    let op = |kind: &'static str, path: String, from: Option<String>, value: Option<Value>, old: Option<Value>| {
        PatchOp { op: kind, path, from, value, old_value: old, range: None, elided: false }
    };
    let mut state = doc.clone();
    let mut patch = Vec::new();
    for _ in 0..n * 4 {
        if patch.len() == n {
            break;
        }
        let mut ptrs = Vec::new();
        all_pointers(&state, &mut Vec::new(), &mut ptrs);
        let p = ptrs[rng.below(ptrs.len())].clone();
        let current = pointer::get(&state, &p).cloned();
        let candidate = match rng.below(7) {
            // Replace (the most common edit), also the root's members.
            0 | 1 => Some(op("replace", pointer::join(&p), None, Some(value(rng, current.as_ref())), current.clone())),
            2 => match &current {
                Some(Value::Array(a)) => {
                    let mut q = p.clone();
                    q.push(if rng.below(2) == 0 { "-".into() } else { rng.below(a.len() + 1).to_string() });
                    Some(op("add", pointer::join(&q), None, Some(value(rng, None)), None))
                }
                Some(Value::Object(m)) => {
                    let mut q = p.clone();
                    let key = m.keys().nth(rng.below(m.len() + 1)).cloned().unwrap_or_else(|| "extra".into());
                    let old = m.get(&key).cloned();
                    q.push(key);
                    Some(op("add", pointer::join(&q), None, Some(value(rng, None)), old))
                }
                _ => None,
            },
            3 if !p.is_empty() => Some(op("remove", pointer::join(&p), None, None, current.clone())),
            4 if !p.is_empty() => {
                let to = ptrs[rng.below(ptrs.len())].clone();
                (!to.is_empty()).then(|| {
                    op("move", pointer::join(&to), Some(pointer::join(&p)), None, pointer::get(&state, &to).cloned())
                })
            }
            5 => {
                let to = ptrs[rng.below(ptrs.len())].clone();
                (!to.is_empty()).then(|| {
                    op("copy", pointer::join(&to), Some(pointer::join(&p)), None, pointer::get(&state, &to).cloned())
                })
            }
            _ => Some(op("test", pointer::join(&p), None, current.clone(), None)),
        };
        if let Some(o) = candidate
            && let Ok(s) = change::apply(&state, std::slice::from_ref(&o))
        {
            state = s;
            patch.push(o);
        }
    }
    patch
}

/// Every entry's raw JSON, as bytes.
fn snapshot_bytes(doc: &Document) -> Vec<(EntryKey, Vec<u8>)> {
    doc.entry_keys()
        .into_iter()
        .map(|k| {
            let bytes = serde_json::to_vec(&doc.entry_raw_json(&k).unwrap()).unwrap();
            (k, bytes)
        })
        .collect()
}

/// Runs one random case; returns whether the patch was committed.
fn case(seed: u64) -> bool {
    let mut rng = Rng(seed | 1);
    let doc = fixture::sample_document(1 + rng.next() % 4).unwrap();
    let keys = doc.entry_keys();
    let key = keys[rng.below(keys.len())].clone();
    let entry = doc.entry_raw_json(&key).unwrap();
    let n = 1 + rng.below(6);
    let patch = random_patch(&entry, &mut rng, n);
    let before = snapshot_bytes(&doc);
    let result = doc.with_patched_entry(&key, &patch);
    assert_eq!(snapshot_bytes(&doc), before, "the original document must stay byte-identical");
    match result {
        Ok((next, inverse)) => {
            let errors: Vec<_> = next.issues().into_iter().filter(|i| i.severity == Severity::Error).collect();
            assert!(errors.is_empty(), "committed a document with errors: {errors:?}\npatch {patch:?}");
            // The returned inverse is concrete (computed against the staged pre-state).
            assert!(inverse.iter().all(|o| !o.path.ends_with("/-")));
            true
        }
        Err(errors) => {
            assert!(!errors.is_empty());
            for e in &errors {
                assert!(e.details().get("pointer").is_some(), "error without details.pointer: {e:?}");
            }
            false
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn random_patches_never_commit_a_document_with_integrity_errors(seed in any::<u64>()) {
        case(seed);
    }
}

/// The deterministic sweep shows both outcomes occur: random patches are committed and refused.
#[test]
fn staged_commit_sweep_commits_and_refuses() {
    let (mut committed, mut refused) = (0, 0);
    for seed in 0..1500u64 {
        if case(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)) {
            committed += 1;
        } else {
            refused += 1;
        }
    }
    println!("staged commit sweep: {committed} committed, {refused} refused");
    assert!(committed > 50 && refused > 50, "{committed} committed, {refused} refused");
}

/// The four reviewer shapes also hold at the document level: an element's exposure, a placement's mouth string.
#[test]
fn document_level_frame_writes_are_refused_and_leave_the_document_unchanged() {
    let doc = fixture::sample_document(1).unwrap();
    let mut checked = std::collections::BTreeSet::new();
    for key in doc.entry_keys() {
        let EntryKey::Element(_) = &key else { continue };
        let v = doc.entry_raw_json(&key).unwrap();
        let before = snapshot_bytes(&doc);
        let mut patches = Vec::new();
        if v.get("exposure").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
            patches.push(PatchOp {
                op: "replace",
                path: "/exposure/0/1".into(),
                from: None,
                value: Some(json!(100_001)),
                old_value: None,
                range: None,
                elided: false,
            });
        }
        if let Some(placements) = v.get("placements").and_then(Value::as_array) {
            for (i, placement) in placements.iter().enumerate() {
                if placement.get("mouthTrack").is_some_and(Value::is_object) {
                    patches.push(PatchOp {
                        op: "replace",
                        path: format!("/placements/{i}/mouthTrack/mouths"),
                        from: None,
                        value: Some(json!("G".repeat(100_001))),
                        old_value: None,
                        range: None,
                        elided: false,
                    });
                }
            }
        }
        for p in patches {
            let errors = doc.with_patched_entry(&key, std::slice::from_ref(&p)).unwrap_err();
            assert_eq!(errors[0].code(), "doc.limitExceeded", "{errors:?}");
            assert_eq!(errors[0].details()["pointer"], json!(p.path));
            assert_eq!(snapshot_bytes(&doc), before);
            checked.insert(if p.path.ends_with("mouths") { "mouths" } else { "exposure" });
        }
    }
    assert_eq!(checked.len(), 2, "the fixture must exercise both fields: {checked:?}");
}
