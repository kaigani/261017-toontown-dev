//! Checkpoints hold coherent, stamped content (C12-R3-02): over random sequences of atomic batches mixing edits and
//! checkpoints, `doc.diff {sinceCheckpoint}` reports exactly the objects edited after the checkpoint's step, every
//! reported change is newer than or equal to the checkpoint's advertised revision, and a checkpoint taken after the
//! batch's last edit reports nothing until something changes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::*;
use serde_json::json;

/// SplitMix64: a deterministic stream (the seed is printed on failure).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn random_edit_checkpoint_diff_sequences_stay_coherent() {
    let seed = 0xc12_3002_u64;
    let mut rng = Rng(seed);
    let h = fixture();
    let targets = [first(&h, "scene"), first(&h, "element"), first(&h, "stroke"), first(&h, "palette")];
    // For each checkpoint: the targets edited after its step.
    let mut edited_after: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut counter = 0u64;
    let mut checkpoints = 0usize;
    for round in 0..120 {
        let mut steps = Vec::new();
        let mut created_here: Vec<String> = Vec::new();
        let mut last_is_checkpoint = false;
        for _ in 0..(1 + rng.below(4)) {
            if rng.below(3) == 0 && checkpoints < 90 {
                let name = format!("cp-{round}-{}", steps.len());
                steps.push(json!({ "id": "checkpoint.create", "params": { "name": name } }));
                created_here.push(name.clone());
                edited_after.insert(name, BTreeSet::new());
                checkpoints += 1;
                last_is_checkpoint = true;
            } else {
                let t = targets[rng.below(targets.len() as u64) as usize].clone();
                counter += 1;
                steps.push(json!({ "id": "doc.object.patch", "params": { "targets": [t], "props": { "meta": { "n": counter } } } }));
                for set in edited_after.values_mut() {
                    set.insert(t.clone());
                }
                last_is_checkpoint = false;
            }
        }
        let env = batch(&h, json!({ "label": format!("round {round}"), "steps": steps }));
        assert!(env.is_ok(), "seed {seed:#x} round {round}: {env:?}");
        let list = ok(&h, "checkpoint.list", json!({}));
        let rev_of = |name: &str| -> u64 {
            list.as_array().unwrap().iter().find(|c| c["name"] == name).unwrap()["documentRev"].as_u64().unwrap()
        };
        if last_is_checkpoint {
            let name = created_here.last().unwrap();
            assert_eq!(rev_of(name), rev(&h), "seed {seed:#x} round {round}: advertised revision is the content's");
            let d = ok(&h, "doc.diff", json!({ "sinceCheckpoint": name }));
            assert_eq!(d["changes"], json!([]), "seed {seed:#x} round {round}: nothing changed since `{name}`: {d}");
        }
        // Every few rounds, check every live checkpoint against the oracle.
        if round % 10 == 9 {
            for (name, edited) in &edited_after {
                let d = ok(&h, "doc.diff", json!({ "sinceCheckpoint": name, "limit": 500 }));
                let base = rev_of(name);
                let mut ids: BTreeSet<String> = BTreeSet::new();
                for c in d["changes"].as_array().unwrap() {
                    assert!(c["rev"].as_u64().unwrap() >= base, "seed {seed:#x}: change older than `{name}`: {c}");
                    if c["type"] != "drawing" {
                        ids.insert(c["id"].as_str().unwrap().to_owned());
                    }
                }
                assert_eq!(&ids, edited, "seed {seed:#x} round {round}: changes since `{name}`");
            }
        }
    }
}
