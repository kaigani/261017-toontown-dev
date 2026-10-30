//! C11-R6-01: the inverse returned by a staged commit undoes the **committed canonical** entry.
//!
//! Random exposure edits (drawing changes that merge neighbouring runs, count changes, inserted and removed runs,
//! range replaces whose runs merge with or split their neighbours) and mouth-track edits (range and whole-string
//! replaces) are committed through `Document::with_patched_entry`. For every committed patch: the returned inverse
//! applies cleanly to the committed snapshot and restores the canonical pre-state byte for byte, its own inverse
//! restores the committed snapshot byte for byte, and neither is the whole-entry fallback.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers")]

use proptest::prelude::*;
use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey};
use tt_model::fixture;
use tt_model::value::FrameRange;

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

fn op(kind: &'static str, path: &str, value: Option<Value>) -> PatchOp {
    PatchOp { op: kind, path: path.into(), from: None, value, old_value: None, range: None, elided: false }
}

fn frames(runs: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for r in runs.as_array().unwrap() {
        for _ in 0..r[1].as_u64().unwrap() {
            out.push(r[0].clone());
        }
    }
    out
}

fn runs(frames: &[Value]) -> Value {
    let mut out: Vec<Value> = Vec::new();
    let mut i = 0;
    while i < frames.len() {
        let mut n = 1;
        while frames.get(i + n) == frames.get(i) {
            n += 1;
        }
        out.push(json!([frames[i], n]));
        i += n;
    }
    Value::Array(out)
}

/// A random exposure edit against the current entry JSON.
fn exposure_edit(state: &Value, ids: &[Value], rng: &mut Rng) -> Option<PatchOp> {
    let exposure = state.get("exposure")?.as_array()?.clone();
    let pick = |rng: &mut Rng| if rng.below(5) == 0 { Value::Null } else { ids[rng.below(ids.len())].clone() };
    let len = exposure.len();
    Some(match rng.below(6) {
        // Change a run's drawing; reusing a neighbour's drawing merges the runs on commit.
        0 if len > 0 => {
            let i = rng.below(len);
            let neighbour = exposure.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| exposure.get(j)));
            let d = match neighbour {
                Some(n) if rng.below(2) == 0 => n[0].clone(),
                _ => pick(rng),
            };
            op("replace", &format!("/exposure/{i}/0"), Some(d))
        }
        1 if len > 0 => op("replace", &format!("/exposure/{}/1", rng.below(len)), Some(json!(1 + rng.below(5)))),
        // Insert a run (often merging with a neighbour) or append one.
        2 => {
            let at = if rng.below(3) == 0 { "-".to_owned() } else { rng.below(len + 1).to_string() };
            op("add", &format!("/exposure/{at}"), Some(json!([pick(rng), 1 + rng.below(4)])))
        }
        3 if len > 0 => op("remove", &format!("/exposure/{}", rng.below(len)), None),
        // A range replace whose new runs split or merge with what surrounds them.
        _ => {
            let f = frames(&Value::Array(exposure));
            let start = rng.below(f.len() + 2);
            let old_len = rng.below(4);
            let mut old: Vec<Value> = f.iter().skip(start).take(old_len).cloned().collect();
            old.resize(old_len, Value::Null);
            let new: Vec<Value> = (0..rng.below(4)).map(|_| pick(rng)).collect();
            let span = old.len().max(new.len()).max(1);
            let from = u32::try_from(start + 1).unwrap();
            let to = from + u32::try_from(span).unwrap() - 1;
            PatchOp {
                range: Some(FrameRange { from, to }),
                old_value: Some(runs(&old)),
                ..op("replace", "/exposure", Some(runs(&new)))
            }
        }
    })
}

/// A random same-length mouth-track edit of placement `i`.
fn mouth_edit(state: &Value, i: usize, rng: &mut Rng) -> Option<PatchOp> {
    let path = format!("/placements/{i}/mouthTrack/mouths");
    let mouths = state.pointer(&path)?.as_str()?.to_owned();
    let letters: Vec<char> = "ABCDEFGX".chars().collect();
    let len = mouths.chars().count();
    if rng.below(3) == 0 {
        let s: String = (0..len).map(|_| letters[rng.below(letters.len())]).collect();
        return Some(op("replace", &path, Some(json!(s))));
    }
    if len == 0 {
        return None;
    }
    let start = rng.below(len);
    let n = 1 + rng.below(len - start);
    let old: String = mouths.chars().skip(start).take(n).collect();
    let new: String = (0..n).map(|_| letters[rng.below(letters.len())]).collect();
    let from = u32::try_from(start + 1).unwrap();
    Some(PatchOp {
        range: Some(FrameRange { from, to: from + u32::try_from(n).unwrap() - 1 }),
        old_value: Some(json!(old)),
        ..op("replace", &path, Some(json!(new)))
    })
}

fn bytes(doc: &Document, key: &EntryKey) -> Vec<u8> {
    serde_json::to_vec(&doc.entry_raw_json(key).unwrap()).unwrap()
}

#[derive(Default)]
struct Outcome {
    committed: usize,
    refused: usize,
    restructured: usize,
}

/// Runs one random case.
fn case(seed: u64, out: &mut Outcome) {
    let mut rng = Rng(seed | 1);
    let doc = fixture::sample_document(1 + rng.next() % 4).unwrap();
    // Candidate entries: elements with an exposure (with their bank's drawings) or with mouth tracks.
    let mut candidates = Vec::new();
    for key in doc.entry_keys() {
        let EntryKey::Element(id) = &key else { continue };
        let v = doc.entry_raw_json(&key).unwrap();
        let element = doc.element(id).unwrap();
        let ids: Vec<Value> = element
            .drawing_bank_id()
            .as_ref()
            .and_then(|b| doc.bank(b))
            .map(|b| b.drawings().ids().iter().map(|d| json!(d.as_str())).collect())
            .unwrap_or_default();
        let mouths: Vec<usize> = v
            .get("placements")
            .and_then(Value::as_array)
            .map(|p| (0..p.len()).filter(|i| p[*i].get("mouthTrack").is_some_and(Value::is_object)).collect())
            .unwrap_or_default();
        if (v.get("exposure").is_some() && !ids.is_empty()) || !mouths.is_empty() {
            candidates.push((key, ids, mouths));
        }
    }
    let (key, ids, mouths) = candidates[rng.below(candidates.len())].clone();
    let pre = bytes(&doc, &key);
    let mut state = doc.entry_raw_json(&key).unwrap();
    let mut patch = Vec::new();
    for _ in 0..1 + rng.below(3) {
        let edit = if !ids.is_empty() && (mouths.is_empty() || rng.below(2) == 0) {
            exposure_edit(&state, &ids, &mut rng)
        } else {
            mouth_edit(&state, mouths[rng.below(mouths.len())], &mut rng)
        };
        if let Some(o) = edit
            && let Ok(s) = change::apply(&state, std::slice::from_ref(&o))
        {
            state = s;
            patch.push(o);
        }
    }
    let Ok((next, inverse)) = doc.with_patched_entry(&key, &patch) else {
        out.refused += 1;
        return;
    };
    out.committed += 1;
    let committed = next.entry_raw_json(&key).unwrap();
    if committed.get("exposure") != state.get("exposure") {
        out.restructured += 1; // canonicalisation merged, split or trimmed runs of the staged value
    }
    assert_eq!(bytes(&doc, &key), pre, "the pre-state is immutable");
    assert!(
        !(inverse.len() == 1 && inverse[0].path.is_empty()),
        "the inverse is a diff, not the whole-entry fallback: {patch:?}"
    );
    let (back, redo) = next.with_patched_entry(&key, &inverse).unwrap_or_else(|e| {
        panic!("inverse must apply to the committed state: {e:?}\npatch {patch:?}\ninverse {inverse:?}")
    });
    assert_eq!(bytes(&back, &key), pre, "undo restores the canonical pre-state byte for byte\npatch {patch:?}");
    let (again, _) = back.with_patched_entry(&key, &redo).unwrap_or_else(|e| panic!("redo must apply: {e:?}"));
    assert_eq!(bytes(&again, &key), bytes(&next, &key), "redo restores the committed snapshot byte for byte");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn staged_exposure_and_mouth_edits_undo_and_redo_exactly(seed in any::<u64>()) {
        case(seed, &mut Outcome::default());
    }
}

/// Deterministic sweep: commits occur, and many of them are restructured by canonicalisation (the C11-R6-01 case).
#[test]
fn staged_inverse_sweep_covers_merging_and_splitting_runs() {
    let mut out = Outcome::default();
    for seed in 0..1000u64 {
        case(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), &mut out);
    }
    println!(
        "staged inverse sweep: {} committed ({} restructured by canonicalisation), {} refused",
        out.committed, out.restructured, out.refused
    );
    assert!(
        out.committed > 300 && out.restructured > 50,
        "{} committed, {} restructured",
        out.committed,
        out.restructured
    );
}
