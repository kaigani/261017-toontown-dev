//! Worst-case planar-map work (M2 C2.2 readiness items 3 and 9): adversarial layers are settled — persistent noding,
//! the derived graph, fill carry-over — within the planar work budget, or refused with `limit.exceeded` well inside
//! the 30 s document watchdog. Every observation (wall time, units used, zones, outcome) is kept in
//! `target/evidence/c22-planar/planar-worst-case-<os>.json`. Observations, not quantiles: the file lists every run.
#![allow(clippy::unwrap_used, missing_docs, reason = "test")]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tt_vector::planar::layer::settle;
use tt_vector::work::{PLANAR_WORK_BUDGET, Work};

fn line(id: &str, kind: &str, pts: &[(f64, f64)], closed: bool) -> Value {
    let points: Vec<Value> = pts.iter().map(|&(x, y)| json!([x, y, 0.0, 0.0, 0.0, 0.0])).collect();
    json!({
        "id": id, "type": "stroke", "rev": 1, "kind": kind, "closed": closed, "points": points,
        "colourId": if kind == "invisible" { Value::Null } else { json!("col_ink") }, "fillMatrix": null,
        "lineStyle": if kind == "pencil" { json!({"thickness": 1, "lineType": "solid"}) } else { Value::Null },
        "spine": null, "sides": {"left": null, "right": null}
    })
}

struct Lcg(u64);

impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A corpus case: name, layer before (none for a new layer), layer after.
type Case = (&'static str, Option<Vec<Value>>, Vec<Value>);

/// The corpus.
fn corpus() -> Vec<Case> {
    let mut out = Vec::new();
    // 100 000 small separate curves (the C2.1 measurement layer) with one stroke added.
    let small: Vec<Value> = (0..100_000u32)
        .map(|n| {
            let (x, y) = (f64::from(n % 1000), f64::from(n / 1000));
            let mut v = line(&format!("s{n}"), "pencil", &[(x, y), (x + 0.9, y + 0.5)], false);
            v["points"] = json!([[x, y, 0, 0, 0.3, 0.2], [x + 0.9, y + 0.5, -0.3, 0, 0, 0]]);
            v
        })
        .collect();
    let mut added = small.clone();
    added.push(line("new", "pencil", &[(0.0, -5.0), (0.0, -6.0), (1.0, -6.0), (1.0, -5.0)], false));
    out.push(("100000-small-curves-plus-one", Some(small), added));
    for n in [200u32, 600] {
        let grid: Vec<Value> = (0..n)
            .map(|i| line(&format!("h{i}"), "pencil", &[(-1.0, f64::from(i)), (f64::from(n), f64::from(i))], false))
            .chain((0..n).map(|i| {
                line(&format!("v{i}"), "invisible", &[(f64::from(i), -1.0), (f64::from(i), f64::from(n))], false)
            }))
            .collect();
        out.push((if n == 200 { "grid-200x200-crossing-lines" } else { "grid-600x600-crossing-lines" }, None, grid));
    }
    let nested: Vec<Value> = (1..=10_000u32)
        .map(|i| {
            let r = f64::from(i) * 0.5;
            line(&format!("q{i}"), "pencil", &[(-r, -r), (r, -r), (r, r), (-r, r)], true)
        })
        .collect();
    out.push(("10000-concentric-squares", None, nested));
    let separate: Vec<Value> = (0..20_000u32)
        .map(|i| {
            let (x, y) = (f64::from(i % 200) * 5.0, f64::from(i / 200) * 5.0);
            line(&format!("c{i}"), "pencil", &[(x, y), (x + 3.0, y), (x + 3.0, y + 3.0), (x, y + 3.0)], true)
        })
        .collect();
    out.push(("20000-separate-closed-squares", None, separate));
    let mut rng = Lcg(0x7474_6332_325f_7763);
    let random: Vec<Value> = (0..1_500u32)
        .map(|i| {
            let (a, b) = ((rng.unit() * 1000.0, rng.unit() * 1000.0), (rng.unit() * 1000.0, rng.unit() * 1000.0));
            line(&format!("r{i}"), "pencil", &[a, b], false)
        })
        .collect();
    out.push(("1500-random-crossing-lines", None, random));
    let scribble: Vec<(f64, f64)> = (0..5_000).map(|_| (rng.unit() * 200.0, rng.unit() * 200.0)).collect();
    out.push(("5000-point-random-scribble", None, vec![line("sc", "pencil", &scribble, false)]));
    let zig: Vec<(f64, f64)> = (0..100_000).map(|i| (f64::from(i % 2) * 10.0, f64::from(i % 997) * 0.01)).collect();
    out.push(("100000-point-dense-zigzag", None, vec![line("z", "pencil", &zig, false)]));
    let square = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
    let coincident: Vec<Value> = (0..100_000u32).map(|i| line(&format!("b{i}"), "brush", &square, true)).collect();
    let mut plus = coincident.clone();
    plus.push(line("bnew", "brush", &square, true));
    out.push(("100000-coincident-outlines-plus-one", Some(coincident), plus));
    out
}

#[test]
#[ignore = "release-only measurement (a few minutes); CI runs it in the headless job with `cargo test --release -p \
            tt-vector --test c22_worst_case -- --ignored --nocapture` and keeps the observations"]
fn adversarial_layers_settle_or_are_refused_within_the_planar_budget() {
    let mut cases = Vec::new();
    let mut worst = (String::new(), 0.0f64);
    for (name, before, after) in
        corpus().into_iter().filter(|c| std::env::var("CASE").map_or(true, |k| c.0.contains(&k)))
    {
        let mut runs = Vec::new();
        for _ in 0..2 {
            let mut n = 0u64;
            let mut fresh = || -> Result<String, ()> {
                n += 1;
                Ok(format!("st_piece{n}"))
            };
            let mut work = Work::new(PLANAR_WORK_BUDGET, "the planar map");
            let start = std::time::Instant::now();
            let r = settle(before.as_deref(), &after, None, &BTreeMap::new(), &mut fresh, &mut work);
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            let (outcome, zones, strokes) = match &r {
                Ok(s) => ("ok".to_owned(), s.map.zones.len(), s.strokes.len()),
                Err(tt_vector::planar::layer::SettleError::Geom(e)) => {
                    assert_eq!(e.code, "limit.exceeded", "{name}: {e:?}");
                    (format!("limit.exceeded: {}", e.message), 0, 0)
                }
                Err(tt_vector::planar::layer::SettleError::Id(())) => ("id".to_owned(), 0, 0),
            };
            if ms > worst.1 {
                worst = (name.to_owned(), ms);
            }
            runs.push(
                json!({ "wallMs": ms, "units": work.used(), "outcome": outcome, "zones": zones, "strokes": strokes }),
            );
        }
        eprintln!("{name}: {runs:?}");
        cases.push(json!({ "name": name, "inputStrokes": after.len(), "runs": runs }));
    }
    let doc = json!({ "budgetUnits": PLANAR_WORK_BUDGET, "os": std::env::consts::OS, "cases": cases,
                      "worst": { "name": worst.0, "wallMs": worst.1 } });
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c22-planar");
    std::fs::create_dir_all(&out).unwrap();
    let file = out.join(format!("planar-worst-case-{}.json", std::env::consts::OS));
    std::fs::write(&file, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    eprintln!("worst {worst:?}; observations in {}", file.display());
    assert!(worst.1 < 10_000.0, "{worst:?}");
}
