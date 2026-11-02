//! M2 C2.4 readiness item 4 (C22-T2, the bounded pass): where the 100 000-stroke Flatten's settling time goes, as raw
//! observations of its phases on the C22-T2 gate's layout (two layers of 50 000 short pencil strokes, `c24_measure.rs`):
//! noding the merged layer with the moved strokes marked changed; noding each source layer alone (the pairs an
//! incremental noding of settled layers would skip); building the merged map and each source map; the full settle.
//! The ceiling of what skipping already-noded pairs can save is the per-layer noding time.
//!
//! Ignored by default: `TT_MEASURE=1 cargo test --release -p tt-vector --test c24_flatten_phases -- --ignored`.
//! Output: `target/evidence/c24/flatten-phases-<os>.json`.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use std::collections::BTreeMap;
use std::time::Instant;

use serde_json::{Value, json};
use tt_vector::planar::EdgeKind;
use tt_vector::planar::build;
use tt_vector::planar::layer::{inputs, path_of, settle};
use tt_vector::planar::node::{NodeStroke, node};
use tt_vector::work::Work;

fn layer(k: usize, dy: f64) -> Vec<Value> {
    (0..50_000)
        .map(|n| {
            let (x, y) = (f64::from(u32::try_from(n % 250).unwrap()) * 10.0, f64::from(u32::try_from(n / 250).unwrap()) * 10.0 + dy);
            json!({ "id": format!("st_m{k}_{n}"), "type": "stroke", "rev": 1, "kind": "pencil", "closed": false,
                "points": [[x, y, 0, 0, 0, 0], [x + 5.0, y + 2.0, 0, 0, 0, 0]], "colourId": "col_ink", "fillMatrix": null,
                "lineStyle": {"thickness": 1, "lineType": "solid"}, "spine": null, "sides": {"left": null, "right": null} })
        })
        .collect()
}

fn node_input(strokes: &[Value], changed: impl Fn(usize) -> bool) -> Vec<NodeStroke> {
    strokes
        .iter()
        .enumerate()
        .map(|(i, s)| NodeStroke {
            id: s["id"].as_str().unwrap().to_owned(),
            kind: EdgeKind::Pencil,
            path: path_of(s),
            changed: changed(i),
        })
        .collect()
}

fn secs(f: impl FnOnce()) -> f64 {
    let t = Instant::now();
    f();
    t.elapsed().as_secs_f64()
}

#[test]
#[ignore = "measurement; run with TT_MEASURE=1 and --ignored in release"]
fn flatten_settle_phases_raw_observations() {
    let (a, b) = (layer(1, 0.0), layer(2, 5.0));
    let mut merged = a.clone();
    merged.extend(b.iter().cloned());
    let mut runs = Vec::new();
    for _ in 0..3 {
        let node_merged = secs(|| {
            node(&node_input(&merged, |i| i >= 50_000), &mut Work::unlimited("t")).unwrap();
        });
        let node_a = secs(|| {
            node(&node_input(&a, |_| false), &mut Work::unlimited("t")).unwrap();
        });
        let node_b = secs(|| {
            node(&node_input(&b, |_| true), &mut Work::unlimited("t")).unwrap();
        });
        let map_merged = secs(|| {
            build(&inputs(&merged), &mut Work::unlimited("t")).unwrap();
        });
        let map_a = secs(|| {
            build(&inputs(&a), &mut Work::unlimited("t")).unwrap();
        });
        let settle_all = secs(|| {
            let mut n = 0u64;
            let mut fresh = || -> Result<String, ()> {
                n += 1;
                Ok(format!("st_f{n}"))
            };
            settle(Some(&a), &merged, None, &BTreeMap::new(), &mut fresh, &mut Work::unlimited("t")).unwrap();
        });
        runs.push(json!({
            "nodeMergedSeconds": node_merged, "nodeLayer1Seconds": node_a, "nodeLayer2Seconds": node_b,
            "mapMergedSeconds": map_merged, "mapLayer1Seconds": map_a, "settleSeconds": settle_all,
            "skippableNodingCeilingSeconds": node_a + node_b,
        }));
    }
    let out = json!({ "os": std::env::consts::OS, "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "strokes": 100_000, "runs": runs });
    if std::env::var("TT_MEASURE").as_deref() == Ok("1") {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c24");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join(format!("flatten-phases-{}.json", std::env::consts::OS)),
            serde_json::to_string_pretty(&out).unwrap() + "\n",
        )
        .unwrap();
    }
    println!("{out}");
}
