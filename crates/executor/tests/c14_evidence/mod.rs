//! Measurement evidence that keeps every observation (C14-R1-07): each series stores its samples (`tMs` from the
//! recorder's time origin, the measured `value`, an optional `label`) and a summary derived from them by the same
//! nearest-rank rule as `scripts/analyse-measurements.py`, which recomputes and checks it.
#![allow(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    dead_code,
    missing_docs,
    reason = "test helper"
)]

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

/// Milliseconds of a duration.
#[must_use]
pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// The nearest-rank summary of values: `n`, `p50`, `p90`, `p99`, `max` (index `round((n - 1) × q)` of the sorted values).
#[must_use]
pub fn summary(values: &[f64]) -> Value {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        return json!({ "n": 0 });
    }
    let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    json!({ "n": v.len(), "p50": at(0.5), "p90": at(0.9), "p99": at(0.99), "max": v[v.len() - 1] })
}

/// Collects series of observations against one time origin.
pub struct Recorder {
    origin: Instant,
    started_at: String,
    series: BTreeMap<String, (String, Vec<Value>)>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    #[must_use]
    pub fn new() -> Self {
        Self { origin: Instant::now(), started_at: tt_persist::time::now_iso(), series: BTreeMap::new() }
    }

    /// Milliseconds since the time origin.
    #[must_use]
    pub fn t(&self, at: Instant) -> f64 {
        ms(at.saturating_duration_since(self.origin))
    }

    /// The time origin (for samples taken on other threads).
    #[must_use]
    pub fn origin(&self) -> Instant {
        self.origin
    }

    /// Records one observation taken at `at`.
    pub fn sample(&mut self, series: &str, unit: &str, at: Instant, value: f64, label: Option<&str>) {
        let t = self.t(at);
        let entry = self.series.entry(series.to_owned()).or_insert_with(|| (unit.to_owned(), Vec::new()));
        let mut s = json!({ "tMs": t, "value": value });
        if let (Some(l), Some(m)) = (label, s.as_object_mut()) {
            m.insert("label".into(), json!(l));
        }
        entry.1.push(s);
    }

    /// Records observations taken elsewhere: `(tMs, value)` pairs on this recorder's time origin.
    pub fn extend(&mut self, series: &str, unit: &str, samples: impl IntoIterator<Item = (f64, f64)>) {
        let entry = self.series.entry(series.to_owned()).or_insert_with(|| (unit.to_owned(), Vec::new()));
        entry.1.extend(samples.into_iter().map(|(t, v)| json!({ "tMs": t, "value": v })));
    }

    /// The values of a series.
    #[must_use]
    pub fn values(&self, series: &str) -> Vec<f64> {
        self.series.get(series).map(|(_, s)| s.iter().filter_map(|x| x["value"].as_f64()).collect()).unwrap_or_default()
    }

    /// The report: environment, time origin, every series with its samples and derived summary, and `facts`.
    #[must_use]
    pub fn report(&self, obligation: &str, fixture: &str, facts: Value) -> Value {
        let mut series = Map::new();
        for (name, (unit, samples)) in &self.series {
            let values: Vec<f64> = samples.iter().filter_map(|x| x["value"].as_f64()).collect();
            series.insert(name.clone(), json!({ "unit": unit, "samples": samples, "summary": summary(&values) }));
        }
        json!({
            "obligation": obligation,
            "fixture": fixture,
            "environment": {
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "cpus": std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get),
                "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
                "startedAt": self.started_at,
                "timeOrigin": "tMs: milliseconds since startedAt on a monotonic clock",
                "summaryRule": "nearest rank: sorted[round((n - 1) * q)]",
            },
            "series": series,
            "facts": facts,
        })
    }
}

/// Writes a report to `target/evidence/c14/<name>-<os>.json`.
pub fn write(name: &str, report: &Value) {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c14");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join(format!("{name}-{}.json", std::env::consts::OS)),
        serde_json::to_vec_pretty(report).unwrap(),
    )
    .unwrap();
}
