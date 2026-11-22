//! Ruling D-46 item 4 (D-45): the timing tests' figures, written where CI uploads them. With `TT_TIMING_DIR` set, each
//! call appends one JSON line to `<dir>/<binary>.jsonl` (the figure, its bound, the place and the OS); without it,
//! nothing. The figures let a round judge each bound against the slowest runner (a passing test's output is captured).

use std::io::Write as _;
use std::time::Duration;

/// Records one timing figure and the bound its assertion holds it to.
pub fn record(place: &str, figure: Duration, bound: Duration) {
    let Some(dir) = std::env::var_os("TT_TIMING_DIR") else { return };
    let dir = std::path::PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&dir);
    let binary = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().split('-').next().unwrap_or("tests").to_owned()))
        .unwrap_or_else(|| "tests".to_owned());
    let line = serde_json::json!({
        "place": place,
        "ms": figure.as_secs_f64() * 1000.0,
        "boundMs": bound.as_secs_f64() * 1000.0,
        "os": std::env::consts::OS,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{binary}.jsonl"))) {
        let _ = writeln!(f, "{line}");
    }
}
