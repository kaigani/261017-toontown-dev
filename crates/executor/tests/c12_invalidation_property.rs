//! Drawing invalidation follows content, not revision stamps (C12-R4-01): over random sequences of checkpoints, edits
//! and diffs — inside atomic batches (staged) and as single commands (committed) — `doc.diff` reports a drawing exactly
//! when content inside it differs between the two states compared. Stroke values are drawn from a small domain, so an
//! edit often rewrites the value a stroke already has or restores the one it had at the base: the drawing's revision
//! moves, its content does not, and no invalidation may appear.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::*;
use serde_json::{Value, json};

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

/// The oracle's view of the content: each chosen stroke's value and the scene's value.
#[derive(Clone, PartialEq)]
struct Model {
    strokes: BTreeMap<String, u64>,
    scene: u64,
}

/// What a diff from `then` to `now` must report: the strokes whose value differs, the drawings holding them, and the
/// scene if its value differs.
fn expected(then: &Model, now: &Model, scene: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (s, v) in &now.strokes {
        if then.strokes[s] != *v {
            out.insert(s.clone());
            out.insert(s.split('/').next().unwrap().to_owned());
        }
    }
    if then.scene != now.scene {
        out.insert(scene.to_owned());
    }
    out
}

fn reported(diff: &Value) -> BTreeSet<String> {
    diff["changes"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_owned()).collect()
}

fn drawings_only(ids: &BTreeSet<String>, drawings: &BTreeSet<String>) -> BTreeSet<String> {
    ids.iter().filter(|i| drawings.contains(*i)).cloned().collect()
}

/// A base a diff can start from.
#[derive(Clone)]
enum Base {
    Checkpoint(String),
    Rev(u64),
}

impl Base {
    fn params(&self) -> Value {
        match self {
            Self::Checkpoint(n) => json!({ "sinceCheckpoint": n, "limit": 1000 }),
            Self::Rev(r) => json!({ "sinceRev": r, "limit": 1000 }),
        }
    }
}

#[test]
fn drawing_invalidation_appears_exactly_when_drawing_content_differs() {
    let seed = 0xc12_4001_u64;
    let mut rng = Rng(seed);
    let h = fixture();
    let scene = first(&h, "scene");
    // The fixture draws in one drawing: copy its layer and strokes into two new elements, then take two strokes from
    // each of the three drawings.
    let mut layer = vec![json!(first(&h, "drawingNode"))];
    layer.extend(
        ok(&h, "doc.object.find", json!({ "type": "stroke", "limit": 500 }))["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].clone()),
    );
    ok(&h, "edit.copy", json!({ "context": "drawing", "objectIds": layer }));
    ok(&h, "edit.pasteAsNewElement", json!({ "frame": 1 }));
    ok(&h, "edit.pasteAsNewElement", json!({ "frame": 1 }));
    let found = ok(&h, "doc.object.find", json!({ "type": "stroke", "limit": 500 }));
    let mut per_drawing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in found["items"].as_array().unwrap() {
        let id = item["id"].as_str().unwrap().to_owned();
        let d = id.split('/').next().unwrap().to_owned();
        let list = per_drawing.entry(d).or_default();
        if list.len() < 2 {
            list.push(id);
        }
    }
    let strokes: Vec<String> = per_drawing.values().take(3).flatten().cloned().collect();
    let drawings: BTreeSet<String> = strokes.iter().map(|s| s.split('/').next().unwrap().to_owned()).collect();
    assert!(drawings.len() >= 3, "strokes in at least three drawings: {per_drawing:?}");
    // Start from known values.
    let mut model = Model { strokes: strokes.iter().map(|s| (s.clone(), 0)).collect(), scene: 0 };
    let mut setup =
        vec![json!({ "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "p": 0 } } } })];
    for s in &strokes {
        setup.push(json!({ "id": "doc.object.patch", "params": { "targets": [s], "props": { "meta": { "p": 0 } } } }));
    }
    assert!(batch(&h, json!({ "label": "setup", "steps": setup })).is_ok());

    // The model at each checkpoint, and at each committed revision.
    let mut at_checkpoint: BTreeMap<String, Model> = BTreeMap::new();
    let mut at_rev: BTreeMap<u64, Model> = BTreeMap::from([(rev(&h), model.clone())]);
    let (mut diffs, mut phantom_chances, mut real_changes) = (0usize, 0usize, 0usize);
    let pick_base = |rng: &mut Rng, cps: &BTreeMap<String, Model>, revs: &BTreeMap<u64, Model>| -> (Base, Model) {
        if !cps.is_empty() && rng.below(3) != 0 {
            let (n, m) = cps.iter().nth(rng.below(cps.len() as u64) as usize).unwrap();
            (Base::Checkpoint(n.clone()), m.clone())
        } else {
            let (r, m) = revs.iter().nth(rng.below(revs.len() as u64) as usize).unwrap();
            (Base::Rev(*r), m.clone())
        }
    };

    for round in 0..150 {
        let ctx = |what: &str| format!("seed {seed:#x} round {round}: {what}");
        if rng.below(4) == 0 {
            // Single committed commands.
            match rng.below(3) {
                0 if at_checkpoint.len() < 80 => {
                    let name = format!("cp-{round}");
                    ok(&h, "checkpoint.create", json!({ "name": name }));
                    at_checkpoint.insert(name, model.clone());
                }
                1 => {
                    let s = strokes[rng.below(strokes.len() as u64) as usize].clone();
                    let v = rng.below(3);
                    ok(&h, "doc.object.patch", json!({ "targets": [s], "props": { "meta": { "p": v } } }));
                    model.strokes.insert(s, v);
                }
                _ => {
                    let (base, then) = pick_base(&mut rng, &at_checkpoint, &at_rev);
                    let d = ok(&h, "doc.diff", base.params());
                    let want = expected(&then, &model, &scene);
                    assert_eq!(reported(&d), want, "{}", ctx(&format!("committed diff {:?}: {d}", base.params())));
                    diffs += 1;
                }
            }
            at_rev.insert(rev(&h), model.clone());
            continue;
        }
        // An atomic batch: every query in it sees the staged state (11 §7.3).
        let mut steps = Vec::new();
        let mut queries: Vec<(usize, Base, BTreeSet<String>)> = Vec::new();
        let mut staged = model.clone();
        let mut cps_here: BTreeMap<String, Model> = at_checkpoint.clone();
        let mut new_cps = Vec::new();
        for _ in 0..(2 + rng.below(6)) {
            match rng.below(5) {
                0 if cps_here.len() < 80 => {
                    let name = format!("cp-{round}-{}", steps.len());
                    steps.push(json!({ "id": "checkpoint.create", "params": { "name": name } }));
                    cps_here.insert(name.clone(), staged.clone());
                    new_cps.push(name);
                }
                1 => {
                    let v = rng.below(3);
                    steps.push(json!({ "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "p": v } } } }));
                    staged.scene = v;
                }
                2 => {
                    let (base, then) = pick_base(&mut rng, &cps_here, &at_rev);
                    queries.push((steps.len(), base.clone(), expected(&then, &staged, &scene)));
                    steps.push(json!({ "id": "doc.diff", "params": base.params() }));
                }
                _ => {
                    let s = strokes[rng.below(strokes.len() as u64) as usize].clone();
                    let v = rng.below(3);
                    if staged.strokes[&s] == v {
                        phantom_chances += 1;
                    } else {
                        real_changes += 1;
                    }
                    steps.push(json!({ "id": "doc.object.patch", "params": { "targets": [s], "props": { "meta": { "p": v } } } }));
                    staged.strokes.insert(s, v);
                }
            }
        }
        let env = batch(&h, json!({ "label": format!("round {round}"), "steps": steps }));
        assert!(env.is_ok(), "{}", ctx(&format!("{env:?}")));
        let results = env.steps.unwrap();
        for (i, base, want) in queries {
            let d = results[i].result.as_ref().unwrap();
            let got = reported(d);
            assert_eq!(
                drawings_only(&got, &drawings),
                drawings_only(&want, &drawings),
                "{}",
                ctx(&format!("staged drawing invalidation since {:?}: {d}", base.params()))
            );
            assert_eq!(got, want, "{}", ctx(&format!("staged diff since {:?}: {d}", base.params())));
            diffs += 1;
        }
        model = staged;
        for n in new_cps {
            at_checkpoint.insert(n.clone(), cps_here[&n].clone());
        }
        at_rev.insert(rev(&h), model.clone());
    }
    // After the run, every base agrees with the oracle on the committed state.
    for (name, then) in &at_checkpoint {
        let d = ok(&h, "doc.diff", json!({ "sinceCheckpoint": name, "limit": 1000 }));
        assert_eq!(reported(&d), expected(then, &model, &scene), "seed {seed:#x}: final diff since `{name}`: {d}");
    }
    for (r, then) in &at_rev {
        let d = ok(&h, "doc.diff", json!({ "sinceRev": r, "limit": 1000 }));
        assert_eq!(reported(&d), expected(then, &model, &scene), "seed {seed:#x}: final diff since rev {r}: {d}");
    }
    assert!(diffs > 100 && phantom_chances > 20 && real_changes > 50, "{diffs} {phantom_chances} {real_changes}");
}
