//! R3 reruns the builder's RFC 6902 generator/property at 10,000 cases.
//! The generator helpers are copied unchanged from properties.rs at eb54b55; the explicit case count is necessary
//! because that file hard-codes 128 and ignores PROPTEST_CASES. No previous reviewer/builder test is edited.
#![allow(clippy::unwrap_used, clippy::panic, reason = "property regression assertions")]
use proptest::prelude::*;
use serde_json::{Number, Value, json};
use tt_model::change::apply;
fn any_double() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>().prop_filter("finite", |x| x.is_finite()),
        (-1_000_000i64..1_000_000).prop_map(|i| i as f64),
        (-1000.0f64..1000.0),
        Just(-0.0),
        Just(5e-324),
        Just(f64::MAX),
        Just(1e21),
        Just(1e-7),
    ]
}

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|i| Value::Number(i.into())),
        any::<u64>().prop_map(|u| Value::Number(u.into())),
        any_double().prop_map(|f| Number::from_f64(f).map_or(Value::Null, Value::Number)),
        ".{0,12}".prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec(("[a-z~/]{1,6}", inner), 0..6).prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ]
    })
}

/// Pointers to every value of `v` (the root is `""`).
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

/// A random valid RFC 6902 patch of up to `n` operations against `doc` (deterministic in `seed`). Operations that
/// would not apply to the current state are skipped; `oldValue` is set exactly where an operation overwrites.
fn random_patch(doc: &Value, seed: u64, n: usize) -> Vec<tt_model::change::PatchOp> {
    use tt_model::change::PatchOp;
    use tt_model::pointer;
    let mut rng = seed | 1;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut state = doc.clone();
    let mut patch = Vec::new();
    for _ in 0..n * 4 {
        if patch.len() == n {
            break;
        }
        let mut ptrs = Vec::new();
        all_pointers(&state, &mut Vec::new(), &mut ptrs);
        let pick = |r: u64| ptrs[(r % ptrs.len() as u64) as usize].clone();
        let value = |r: u64| match r % 4 {
            0 => json!(r % 100),
            1 => json!(format!("v{}", r % 7)),
            2 => json!([r % 3, [r % 5]]),
            _ => json!({"k": r % 9}),
        };
        let containers: Vec<Vec<String>> = ptrs
            .iter()
            .filter(|p| pointer::get(&state, p).is_some_and(|v| v.is_array() || v.is_object()))
            .cloned()
            .collect();
        // A destination inside a container of `s`: an existing or new member, an index or `-`.
        let dest = |s: &Value, r: u64| -> Option<(String, Option<Value>)> {
            let mut cs = Vec::new();
            all_pointers(s, &mut Vec::new(), &mut cs);
            cs.retain(|p| pointer::get(s, p).is_some_and(|v| v.is_array() || v.is_object()));
            let c = cs.get((r % cs.len().max(1) as u64) as usize)?.clone();
            let mut p = c.clone();
            match pointer::get(s, &c)? {
                Value::Array(a) => {
                    p.push(if r.is_multiple_of(3) {
                        "-".into()
                    } else {
                        ((r >> 8) % (a.len() as u64 + 1)).to_string()
                    });
                    Some((pointer::join(&p), None))
                }
                Value::Object(m) => {
                    let existing: Vec<&String> = m.keys().collect();
                    if r.is_multiple_of(2) && !existing.is_empty() {
                        let k = existing[((r >> 8) % existing.len() as u64) as usize].clone();
                        let old = m.get(&k).cloned();
                        p.push(k);
                        Some((pointer::join(&p), old))
                    } else {
                        p.push(format!("n{}/~{}", r % 5, r % 3));
                        let old = m.get(p.last()?).cloned();
                        Some((pointer::join(&p), old))
                    }
                }
                _ => None,
            }
        };
        let r = next();
        let op = |kind: &'static str, path: String, from: Option<String>, value: Option<Value>, old: Option<Value>| {
            PatchOp { op: kind, path, from, value, old_value: old, range: None, elided: false }
        };
        let candidate = match r % 6 {
            0 if !containers.is_empty() => {
                dest(&state, next()).map(|(p, old)| op("add", p, None, Some(value(next())), old))
            }
            1 => Some(pick(next())).filter(|p| !p.is_empty()).map(|p| {
                let old = pointer::get(&state, &p).cloned();
                op("remove", pointer::join(&p), None, None, old)
            }),
            2 => Some(pick(next())).map(|p| {
                let old = pointer::get(&state, &p).cloned();
                op("replace", pointer::join(&p), None, Some(value(next())), old)
            }),
            3 => {
                let f = pick(next());
                if f.is_empty() {
                    None
                } else {
                    let from = pointer::join(&f);
                    let removed = apply(&state, &[op("remove", from.clone(), None, None, None)]).ok();
                    removed.and_then(|s1| dest(&s1, next())).map(|(p, old)| op("move", p, Some(from), None, old))
                }
            }
            4 if !containers.is_empty() => {
                let f = pointer::join(pick(next()));
                dest(&state, next()).map(|(p, old)| op("copy", p, Some(f), None, old))
            }
            _ => Some(pick(next())).map(|p| {
                let current = pointer::get(&state, &p).cloned();
                op("test", pointer::join(&p), None, current, None)
            }),
        };
        if let Some(o) = candidate
            && let Ok(s) = apply(&state, std::slice::from_ref(&o))
        {
            state = s;
            patch.push(o);
        }
    }
    patch
}

#[test]
fn ten_thousand_rfc6902_sequences_apply_and_invert() {
    let config = ProptestConfig { cases: 10_000, failure_persistence: None, ..ProptestConfig::default() };
    let mut runner = proptest::test_runner::TestRunner::new(config);
    let completed = std::cell::Cell::new(0u32);
    let counts: [std::cell::Cell<u32>; 6] = std::array::from_fn(|_| std::cell::Cell::new(0));
    runner
        .run(&(arb_json(), any::<u64>(), 1usize..10), |(doc, seed, n)| {
            let patch = random_patch(&doc, seed, n);
            let after = apply(&doc, &patch).unwrap();
            let inverse = tt_model::change::invert_against(&doc, &patch).unwrap();
            prop_assert_eq!(&apply(&after, &inverse).unwrap(), &doc, "patch {:?} inverse {:?}", patch, inverse);
            for op in &inverse {
                for p in std::iter::once(&op.path).chain(op.from.as_ref()) {
                    prop_assert!(
                        !tt_model::pointer::parse(p).unwrap().iter().any(|t| t == "-"),
                        "append in inverse {:?}",
                        op
                    );
                }
            }
            let again = tt_model::change::invert_against(&after, &inverse).unwrap();
            prop_assert_eq!(&apply(&doc, &again).unwrap(), &after);
            for op in patch {
                let i = ["add", "remove", "replace", "move", "copy", "test"].iter().position(|k| *k == op.op).unwrap();
                counts[i].set(counts[i].get() + 1);
            }
            completed.set(completed.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(completed.get(), 10_000);
    println!(
        "seed={:?}; successful cases={}; add/remove/replace/move/copy/test={:?}",
        std::env::var("PROPTEST_RNG_SEED"),
        completed.get(),
        counts.map(|c| c.get())
    );
}
