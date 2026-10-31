//! Worst-case fitting work (M2 C2.1 obligation 11, C21-R1-06): adversarial 100 000-sample inputs are fitted within
//! the product work budget, and every observation (wall time and sample visits per run) is kept in
//! `target/evidence/c21-fit/fit-worst-case-<os>.json`. Observations, not quantiles: the file lists every run.
#![allow(clippy::unwrap_used, missing_docs, reason = "test")]

use serde_json::{Value, json};
use tt_vector::fit::{FIT_WORK_BUDGET, Sample, clean, fit_bounded};
use tt_vector::geom::P;

/// A seeded linear congruential generator (no clock, no OS entropy).
struct Lcg(u64);

impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

const N: u32 = 100_000;

fn s(x: f64, y: f64, p: f64) -> Sample {
    Sample { p: P::new(x, y), pressure: p }
}

/// The adversarial corpus: (name, samples, closed).
fn corpus() -> Vec<(&'static str, Vec<Sample>, bool)> {
    let mut rng = Lcg(0x7474_6332_315f_7231);
    let mut walk = Vec::new();
    let (mut x, mut y, mut dx, mut dy) = (0.0, 0.0, 1.0, 0.0);
    for _ in 0..N {
        dx = f64::clamp(dx + (rng.unit() - 0.5) * 0.6, -3.0, 3.0);
        dy = f64::clamp(dy + (rng.unit() - 0.5) * 0.6, -3.0, 3.0);
        x += dx;
        y += dy;
        walk.push(s(x, y, rng.unit()));
    }
    let noise: Vec<Sample> = (0..N).map(|_| s(rng.unit() * 2000.0, rng.unit() * 2000.0, rng.unit())).collect();
    let tau = std::f64::consts::TAU;
    vec![
        ("dense-straight-closed", (0..N).map(|i| s(f64::from(i) * 1e-9, 0.0, 1.0)).collect(), true),
        (
            "dense-circle-closed",
            (0..N)
                .map(|i| {
                    let a = tau * f64::from(i) / f64::from(N);
                    s(1e-4 * a.cos(), 1e-4 * a.sin(), 1.0)
                })
                .collect(),
            true,
        ),
        ("straight-open", (0..N).map(|i| s(f64::from(i), 0.0, 1.0)).collect(), false),
        (
            "zigzag-above-tolerance",
            (0..N).map(|i| s(f64::from(i), if i % 2 == 0 { 0.0 } else { 3.0 }, 1.0)).collect(),
            false,
        ),
        (
            "wiggle-near-tolerance",
            (0..N).map(|i| s(f64::from(i) * 0.5, 0.3 * (f64::from(i) * 0.7).sin(), 1.0)).collect(),
            false,
        ),
        (
            "spiral-closed",
            (0..N)
                .map(|i| {
                    let a = f64::from(i) * 0.01;
                    s((10.0 + a) * a.cos(), (10.0 + a) * a.sin(), 0.5)
                })
                .collect(),
            true,
        ),
        ("random-walk-open", walk.clone(), false),
        ("random-walk-closed", walk, true),
        ("uniform-noise-open", noise.clone(), false),
        ("uniform-noise-closed", noise, true),
    ]
}

#[test]
#[ignore = "release-only measurement (about a minute); CI runs it in the headless job with `cargo test --release -p \
            tt-vector --test c21_worst_case -- --ignored --nocapture` and keeps the observations"]
fn adversarial_inputs_fit_within_the_work_budget() {
    let mut cases = Vec::new();
    let mut worst = (String::new(), 0u64, 0.0f64);
    for (name, raw, closed) in corpus() {
        let samples = clean(&raw, "/points").unwrap();
        let mut runs = Vec::new();
        for _ in 0..3 {
            let start = std::time::Instant::now();
            let (f, used) = fit_bounded(&samples, 3.0, closed, FIT_WORK_BUDGET).unwrap();
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            assert!(f.path.anchors.len() <= 100_000, "{name}");
            if ms > worst.2 {
                worst = (name.to_owned(), used, ms);
            }
            runs.push(json!({"wallMs": ms, "visits": used, "anchors": f.path.anchors.len()}));
        }
        eprintln!("{name}: {runs:?}");
        cases.push(json!({"name": name, "samples": samples.len(), "closed": closed, "runs": runs}));
    }
    let doc: Value = json!({
        "budgetVisits": FIT_WORK_BUDGET,
        "os": std::env::consts::OS,
        "cases": cases,
        "worst": {"name": worst.0, "visits": worst.1, "wallMs": worst.2},
    });
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c21-fit");
    std::fs::create_dir_all(&out).unwrap();
    let file = out.join(format!("fit-worst-case-{}.json", std::env::consts::OS));
    std::fs::write(&file, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    eprintln!("worst {worst:?}; observations in {}", file.display());
    // Every run finishes far inside the 30 s document watchdog.
    assert!(worst.2 < 10_000.0, "{worst:?}");
}

#[test]
fn input_needing_more_work_than_the_budget_is_refused_with_limit_exceeded() {
    let samples: Vec<Sample> = (0..2000).map(|i| s(f64::from(i), if i % 2 == 0 { 0.0 } else { 3.0 }, 1.0)).collect();
    let (_, used) = fit_bounded(&samples, 3.0, false, u64::MAX).unwrap();
    let e = fit_bounded(&samples, 3.0, false, used - 1).unwrap_err();
    assert_eq!(e.code, "limit.exceeded");
    assert_eq!(e.pointer, "/points");
    assert!(fit_bounded(&samples, 3.0, false, used).is_ok());
}
