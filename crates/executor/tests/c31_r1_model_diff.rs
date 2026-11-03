//! Differential receipt for the declared M1 edit. Old string expansion reconstructed from 0b8fd0b change.rs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_model::change::{self, PatchOp};
use tt_model::value::FrameRange;
const IDS: [&str; 3] =
    ["dr_01K742SG0KKMCAP9X1E6Z5P4SW", "dr_01K742SG0M5J3BAJSW8241WA87", "dr_01K742SG0NG5HTFZRD7238FZXJ"];
fn old_expand(v: &Value) -> Vec<Value> {
    v.as_array()
        .unwrap()
        .iter()
        .flat_map(|r| std::iter::repeat_n(r[0].clone(), r[1].as_u64().unwrap() as usize))
        .collect()
}
fn old_runs(v: &[Value]) -> Value {
    let mut out = vec![];
    let mut i = 0;
    while i < v.len() {
        let mut n = 1;
        while v.get(i + n) == Some(&v[i]) {
            n += 1;
        }
        out.push(json!([v[i], n]));
        i += n;
    }
    json!(out)
}
fn old_patch(a: &Value, b: &Value) -> Vec<PatchOp> {
    if a == b {
        return vec![];
    }
    let mut fa = old_expand(a);
    let mut fb = old_expand(b);
    let n = fa.len().max(fb.len());
    fa.resize(n, Value::Null);
    fb.resize(n, Value::Null);
    if fa == fb {
        // range path declines equal arrays; ordinary JSON array differencing is outside this oracle corpus.
        return vec![];
    }
    let p = fa.iter().zip(&fb).take_while(|(a, b)| a == b).count();
    let q = fa.iter().rev().zip(fb.iter().rev()).take(n - p).take_while(|(a, b)| a == b).count();
    vec![PatchOp {
        op: "replace",
        path: "/exposure".into(),
        from: None,
        range: Some(FrameRange { from: (p + 1) as u32, to: (n - q) as u32 }),
        value: Some(old_runs(&fb[p..n - q])),
        old_value: Some(old_runs(&fa[p..n - q])),
        elided: false,
    }]
}
fn next(r: &mut u64, n: usize) -> usize {
    *r ^= *r << 13;
    *r ^= *r >> 7;
    *r ^= *r << 17;
    *r as usize % n
}
fn random_runs(r: &mut u64) -> Value {
    let n = next(r, 35);
    let frames: Vec<Value> = (0..n)
        .map(|_| {
            let k = next(r, 4);
            if k == 3 { Value::Null } else { json!(IDS[k]) }
        })
        .collect();
    let mut frames = frames;
    while frames.last() == Some(&Value::Null) {
        frames.pop();
    }
    old_runs(&frames)
}
#[test]
fn old_string_path_matches_interned_diff_and_range_application_4096() {
    let mut seed = 0xc31d1ff;
    for case in 0..4096 {
        let a = random_runs(&mut seed);
        let b = random_runs(&mut seed);
        let expected = old_patch(&a, &b);
        let observed = change::diff_json(&a, &b, "/exposure", ObjectType::Element);
        assert_eq!(observed, expected, "case {case}: {a} -> {b}");
        let before = json!({"type":"element","exposure":a});
        let (after, inverse) = change::apply_with_inverse(&before, &observed).unwrap();
        assert_eq!(after["exposure"], b, "apply case {case}");
        assert_eq!(change::apply(&after, &inverse).unwrap(), before, "inverse case {case}");
    }
}
#[test]
fn old_string_path_matches_full_length_column() {
    let a = json!([[IDS[0], 100000]]);
    let b = json!([[IDS[0], 99999], [IDS[1], 1]]);
    assert_eq!(change::diff_json(&a, &b, "/exposure", ObjectType::Element), old_patch(&a, &b));
}
