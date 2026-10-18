//! M0 diagnostics: automated IPC latency benchmark (`toontown --ipc-bench <out.json>`).
//!
//! When started with `--ipc-bench`, the front end runs ping bursts at start-up, sends the measurements to the host
//! with `bench_report`, and the host writes them to the given file and exits. CI uses this to measure real
//! WebView2/WKWebView round trips on both runners. Not a spec command: it is a development diagnostic and is not
//! reachable from the menu or by agents.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};

/// Payload sizes measured by default (bytes).
pub const DEFAULT_PAYLOADS: &[usize] = &[0, 1024, 65_536, 1_048_576];
/// Samples per payload size by default.
pub const DEFAULT_COUNT: u32 = 500;
/// The host gives up and exits with status 2 if no report arrives within this time.
pub const TIMEOUT: Duration = Duration::from_secs(120);

/// Benchmark settings parsed from the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchSettings {
    /// Where to write the report.
    pub out: PathBuf,
    /// Samples per payload size.
    pub count: u32,
}

/// Errors parsing the diagnostics arguments.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ArgsError {
    /// A flag is missing its value.
    #[error("{0} needs a value")]
    MissingValue(&'static str),
    /// `--ipc-bench-count` is not a positive integer.
    #[error("--ipc-bench-count must be a positive integer, got `{0}`")]
    BadCount(String),
    /// An argument this binary does not know.
    #[error(
        "unknown argument `{0}` (usage: toontown [--ipc-bench <out.json> [--ipc-bench-count <n>]] [--spike tablet [--spike-log <file>]])"
    )]
    Unknown(String),
    /// `--spike` names a spike that does not exist.
    #[error("unknown spike `{0}` (available: tablet)")]
    UnknownSpike(String),
    /// `--spike-log` without `--spike`.
    #[error("--spike-log needs --spike")]
    LogWithoutSpike,
    /// `--spike-auto` without `--spike` and `--spike-log`.
    #[error("--spike-auto needs --spike tablet and --spike-log <file>")]
    AutoWithoutLog,
}

/// Parses `toontown` arguments (without the program name). `Ok(None)` = normal interactive start.
///
/// # Errors
/// [`ArgsError`] for malformed or unknown arguments.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<LaunchOptions, ArgsError> {
    let mut out = None;
    let mut count = DEFAULT_COUNT;
    let mut spike = None;
    let mut spike_log = None;
    let mut spike_auto = false;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--ipc-bench" => out = Some(PathBuf::from(it.next().ok_or(ArgsError::MissingValue("--ipc-bench"))?)),
            "--ipc-bench-count" => {
                let v = it.next().ok_or(ArgsError::MissingValue("--ipc-bench-count"))?;
                count = v.parse().ok().filter(|n| *n > 0).ok_or(ArgsError::BadCount(v))?;
            }
            "--spike" => {
                let name = it.next().ok_or(ArgsError::MissingValue("--spike"))?;
                if name != "tablet" {
                    return Err(ArgsError::UnknownSpike(name));
                }
                spike = Some(name);
            }
            "--spike-log" => spike_log = Some(PathBuf::from(it.next().ok_or(ArgsError::MissingValue("--spike-log"))?)),
            "--spike-auto" => spike_auto = true,
            // macOS adds `-psn_…` when launched from Finder on older systems.
            other if other.starts_with("-psn_") => {}
            other => return Err(ArgsError::Unknown(other.to_owned())),
        }
    }
    if spike_log.is_some() && spike.is_none() {
        return Err(ArgsError::LogWithoutSpike);
    }
    if spike_auto && (spike.is_none() || spike_log.is_none()) {
        return Err(ArgsError::AutoWithoutLog);
    }
    Ok(LaunchOptions {
        bench: out.map(|out| BenchSettings { out, count }),
        spike: spike.map(|name| crate::spikes::tablet::SpikeSettings { name, log_path: spike_log, auto: spike_auto }),
    })
}

/// Parsed command line of the desktop binary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    /// `--ipc-bench`.
    pub bench: Option<BenchSettings>,
    /// `--spike tablet [--spike-log <file>]` (M0 spike, C0.2).
    pub spike: Option<crate::spikes::tablet::SpikeSettings>,
}

/// Managed state: the benchmark settings, if any.
#[derive(Debug, Default)]
pub struct Diagnostics {
    /// Settings of a requested benchmark run.
    pub bench: Option<BenchSettings>,
}

/// What the front end should measure.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchPlan {
    /// Samples per payload size.
    pub count: u32,
    /// Payload sizes in bytes.
    pub payload_bytes: Vec<usize>,
}

/// Returns the benchmark plan when the app was started with `--ipc-bench`, otherwise `null`.
#[tauri::command]
#[must_use]
pub fn bench_plan(diagnostics: State<'_, Diagnostics>) -> Option<BenchPlan> {
    diagnostics.bench.as_ref().map(|b| BenchPlan { count: b.count, payload_bytes: DEFAULT_PAYLOADS.to_vec() })
}

/// Receives the front end's measurements, writes them next to host facts, and exits the app.
///
/// # Errors
/// `{code: "diagnostics.notRequested"}` when no benchmark was requested; `fs.unwritable` when the file cannot be
/// written (the app then exits with status 3).
#[tauri::command]
pub fn bench_report(
    app: AppHandle,
    diagnostics: State<'_, Diagnostics>,
    engine: State<'_, tt_engine::Engine>,
    report: serde_json::Value,
) -> Result<(), tt_engine::ErrorInfo> {
    let Some(bench) = diagnostics.bench.as_ref() else {
        return Err(error("diagnostics.notRequested", "the app was not started with --ipc-bench".into()));
    };
    let full = serde_json::json!({ "host": engine.info(), "frontend": report });
    match write_json(&bench.out, &full) {
        Ok(()) => {
            app.exit(0);
            Ok(())
        }
        Err(e) => {
            app.exit(3);
            Err(error("fs.unwritable", format!("cannot write {}: {e}", bench.out.display())))
        }
    }
}

/// Exits with status 2 if the front end never reports (e.g. the webview failed to load).
pub fn arm_timeout(app: &AppHandle, settings: &BenchSettings) {
    let app = app.clone();
    let out = settings.out.clone();
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        let _ = write_json(&out, &serde_json::json!({ "ok": false, "error": "timeout waiting for bench_report" }));
        app.exit(2);
    });
}

fn write_json(path: &Path, value: &serde_json::Value) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    std::fs::write(path, text + "\n")
}

fn error(code: &str, message: String) -> tt_engine::ErrorInfo {
    tt_engine::ErrorInfo { code: code.to_owned(), message, details: serde_json::json!({}) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn no_arguments_is_interactive() {
        assert_eq!(parse_args(args(&[])), Ok(LaunchOptions::default()));
    }

    #[test]
    fn bench_with_count() {
        let s = parse_args(args(&["--ipc-bench", "o.json", "--ipc-bench-count", "50"])).unwrap().bench.unwrap();
        assert_eq!(s, BenchSettings { out: "o.json".into(), count: 50 });
        let s = parse_args(args(&["--ipc-bench", "o.json"])).unwrap().bench.unwrap();
        assert_eq!(s.count, DEFAULT_COUNT);
    }

    #[test]
    fn spike_flags() {
        let o = parse_args(args(&["--spike", "tablet", "--spike-log", "t.json"])).unwrap();
        let s = o.spike.unwrap();
        assert_eq!((s.name.as_str(), s.log_path), ("tablet", Some(PathBuf::from("t.json"))));
        assert_eq!(parse_args(args(&["--spike", "canvas"])), Err(ArgsError::UnknownSpike("canvas".into())));
        assert_eq!(parse_args(args(&["--spike-log", "t.json"])), Err(ArgsError::LogWithoutSpike));
        assert_eq!(parse_args(args(&["--spike"])), Err(ArgsError::MissingValue("--spike")));
        assert_eq!(parse_args(args(&["--spike", "tablet", "--spike-auto"])), Err(ArgsError::AutoWithoutLog));
        let auto = parse_args(args(&["--spike", "tablet", "--spike-log", "t.json", "--spike-auto"])).unwrap();
        assert!(auto.spike.unwrap().auto);
    }

    #[test]
    fn errors() {
        assert_eq!(parse_args(args(&["--ipc-bench"])), Err(ArgsError::MissingValue("--ipc-bench")));
        assert_eq!(
            parse_args(args(&["--ipc-bench", "o", "--ipc-bench-count", "0"])),
            Err(ArgsError::BadCount("0".into()))
        );
        assert_eq!(parse_args(args(&["--nope"])), Err(ArgsError::Unknown("--nope".into())));
        assert_eq!(parse_args(args(&["-psn_0_12345"])), Ok(LaunchOptions::default()));
    }
}
