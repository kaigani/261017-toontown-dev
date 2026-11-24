//! M0 diagnostics: automated IPC latency benchmark (`toontown --ipc-bench <out.json>`).
//!
//! When started with `--ipc-bench`, the front end runs ping bursts at start-up, sends the measurements to the host
//! with `bench_report`, and the host writes them to the given file and exits. CI uses this to measure real
//! WebView2/WKWebView round trips on both runners. Not a spec command: it is a development diagnostic and is not
//! reachable from the menu or by agents.
//!
//! **Gating (C0.4):** the benchmark commands, the spikes and their commands are compiled only with the Cargo feature
//! `m0-diagnostics` (`tauri.diagnostics.conf.json` enables it together with the matching front-end build). The
//! default build still parses the flags, so it can refuse them with a clear [`crate::DesktopError`] instead of
//! "unknown argument", but it registers no diagnostic command.

use std::path::PathBuf;
#[cfg(feature = "m0-diagnostics")]
use std::{path::Path, time::Duration};

#[cfg(feature = "m0-diagnostics")]
use serde::Serialize;
#[cfg(feature = "m0-diagnostics")]
use tauri::{AppHandle, State};

/// Payload sizes measured by default (bytes).
#[cfg(feature = "m0-diagnostics")]
pub const DEFAULT_PAYLOADS: &[usize] = &[0, 1024, 65_536, 1_048_576];
/// Samples per payload size by default.
pub const DEFAULT_COUNT: u32 = 500;
/// The host gives up and exits with status 2 if no report arrives within this time.
#[cfg(feature = "m0-diagnostics")]
pub const TIMEOUT: Duration = Duration::from_secs(120);

/// Benchmark settings parsed from the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchSettings {
    /// Where to write the report.
    pub out: PathBuf,
    /// Samples per payload size.
    pub count: u32,
}

/// The Timeline's three-way check (`--timing-check <out> [--timing-check-external]`; M5 C5.1b, `timing_check.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingCheckSettings {
    /// Where the report is written.
    pub out: PathBuf,
    /// The gestures are made by real input from outside (the workstation script), not dispatched by the window.
    pub external: bool,
    /// Only the cases whose name contains this text (`--timing-check-only <text>`; a probe's subset).
    pub only: Option<String>,
    /// The window's budgets (plan §6.2) instead of the three-way rows (`--timing-check-budgets`).
    pub budgets: bool,
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
        "unknown argument `{0}` (usage: toontown [--ipc-bench <out.json> [--ipc-bench-count <n>]] [--timing-check <out.json> [--timing-check-external]] [--spike tablet|canvas [--spike-log <file>] [--spike-auto] [--spike-renderer auto|webgpu|webgl2]])"
    )]
    Unknown(String),
    /// `--spike` names a spike that does not exist.
    #[error("unknown spike `{0}` (available: tablet, canvas)")]
    UnknownSpike(String),
    /// `--spike-renderer` with a value other than `auto`, `webgpu`, `webgl2`.
    #[error("unknown renderer `{0}` (auto, webgpu, webgl2)")]
    BadRenderer(String),
    /// `--spike-renderer` without `--spike canvas`.
    #[error("--spike-renderer needs --spike canvas")]
    RendererWithoutCanvas,
    /// `--timing-check-external`, `--timing-check-only` or `--timing-check-budgets` without `--timing-check`.
    #[error("--timing-check-external, --timing-check-only and --timing-check-budgets need --timing-check <out.json>")]
    ExternalWithoutCheck,
    /// `--spike-log` without `--spike`.
    #[error("--spike-log needs --spike")]
    LogWithoutSpike,
    /// `--spike-auto` without `--spike` and `--spike-log`.
    #[error("--spike-auto needs --spike <name> and --spike-log <file>")]
    AutoWithoutLog,
    /// `--mcp-http` with something that is not a socket address.
    #[error("--mcp-http needs an address such as 127.0.0.1:0, got `{0}`")]
    BadAddress(String),
    /// `--mcp-http` with an address that is not a loopback address (ADR-0022 decision 2; C41-R1-02).
    #[error("--mcp-http refuses `{0}`: attached mode binds a loopback address only (127.0.0.1 or [::1])")]
    RemoteAddress(String),
}

/// Parses `toontown` arguments (without the program name). `Ok(None)` = normal interactive start.
///
/// # Errors
/// [`ArgsError`] for malformed or unknown arguments.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<LaunchOptions, ArgsError> {
    let mut out = None;
    let mut timing_out = None;
    let mut timing_external = false;
    let mut timing_only = None;
    let mut timing_budgets = false;
    let mut count = DEFAULT_COUNT;
    let mut spike = None;
    let mut spike_log = None;
    let mut spike_auto = false;
    let mut renderer = None;
    let mut user_dir = None;
    let mut mcp_http = None;
    let mut mcp_discovery = None;
    let mut open = Vec::new();
    let mut canvas_metrics = false;
    let mut canvas_backend = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--ipc-bench" => out = Some(PathBuf::from(it.next().ok_or(ArgsError::MissingValue("--ipc-bench"))?)),
            "--timing-check" => {
                timing_out = Some(PathBuf::from(it.next().ok_or(ArgsError::MissingValue("--timing-check"))?));
            }
            "--timing-check-external" => timing_external = true,
            "--timing-check-budgets" => timing_budgets = true,
            "--timing-check-only" => {
                timing_only = Some(it.next().ok_or(ArgsError::MissingValue("--timing-check-only"))?);
            }
            "--ipc-bench-count" => {
                let v = it.next().ok_or(ArgsError::MissingValue("--ipc-bench-count"))?;
                count = v.parse().ok().filter(|n| *n > 0).ok_or(ArgsError::BadCount(v))?;
            }
            "--spike" => {
                let name = it.next().ok_or(ArgsError::MissingValue("--spike"))?;
                if name != "tablet" && name != "canvas" {
                    return Err(ArgsError::UnknownSpike(name));
                }
                spike = Some(name);
            }
            "--spike-log" => spike_log = Some(PathBuf::from(it.next().ok_or(ArgsError::MissingValue("--spike-log"))?)),
            "--spike-auto" => spike_auto = true,
            "--spike-renderer" => {
                let r = it.next().ok_or(ArgsError::MissingValue("--spike-renderer"))?;
                if !matches!(r.as_str(), "auto" | "webgpu" | "webgl2") {
                    return Err(ArgsError::BadRenderer(r));
                }
                renderer = Some(r);
            }
            // M4 C4.1 (ADR-0021, ADR-0022): the per-user folder, attached mode and its discovery file.
            "--user-dir" => user_dir = Some(PathBuf::from(it.next().ok_or(ArgsError::MissingValue("--user-dir"))?)),
            "--mcp-http" => {
                let v = it.next().ok_or(ArgsError::MissingValue("--mcp-http"))?;
                let addr = v.parse::<std::net::SocketAddr>().map_err(|_| ArgsError::BadAddress(v.clone()))?;
                if !tt_mcp::http::bind_allowed(&addr) {
                    return Err(ArgsError::RemoteAddress(v));
                }
                mcp_http = Some(addr);
            }
            "--mcp-discovery" => {
                mcp_discovery = Some(PathBuf::from(it.next().ok_or(ArgsError::MissingValue("--mcp-discovery"))?));
            }
            "--canvas-metrics" => canvas_metrics = true,
            "--canvas-backend" => {
                let v = it.next().ok_or(ArgsError::MissingValue("--canvas-backend"))?;
                if v != "webgpu" && v != "webgl2" {
                    return Err(ArgsError::Unknown(format!("--canvas-backend {v}")));
                }
                canvas_backend = Some(v);
            }
            // macOS adds `-psn_…` when launched from Finder on older systems.
            other if other.starts_with("-psn_") => {}
            // A document to open (01 §1.1: command line and OS open-document requests).
            other if !other.starts_with('-') => open.push(PathBuf::from(other)),
            other => return Err(ArgsError::Unknown(other.to_owned())),
        }
    }
    if (timing_external || timing_budgets || timing_only.is_some()) && timing_out.is_none() {
        return Err(ArgsError::ExternalWithoutCheck);
    }
    if spike_log.is_some() && spike.is_none() {
        return Err(ArgsError::LogWithoutSpike);
    }
    if spike_auto && (spike.is_none() || spike_log.is_none()) {
        return Err(ArgsError::AutoWithoutLog);
    }
    if renderer.is_some() && spike.as_deref() != Some("canvas") {
        return Err(ArgsError::RendererWithoutCanvas);
    }
    Ok(LaunchOptions {
        bench: out.map(|out| BenchSettings { out, count }),
        timing_check: timing_out.map(|out| TimingCheckSettings {
            out,
            external: timing_external,
            only: timing_only,
            budgets: timing_budgets,
        }),
        spike: spike.map(|name| SpikeSettings {
            name,
            log_path: spike_log,
            auto: spike_auto,
            renderer: renderer.filter(|r| r != "auto"),
        }),
        user_dir,
        mcp_http,
        mcp_discovery,
        open,
        canvas_metrics,
        canvas_backend,
    })
}

/// Spike launch settings (`--spike tablet|canvas [--spike-log <file>] [--spike-auto] [--spike-renderer ...]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpikeSettings {
    /// Spike name: `tablet` (C0.2) or `canvas` (C0.3).
    pub name: String,
    /// Where `spike_save_log` writes the JSON log.
    pub log_path: Option<PathBuf>,
    /// `--spike-auto`: the page runs its automated scenarios, saves the log and the app exits.
    pub auto: bool,
    /// `--spike-renderer auto|webgpu|webgl2` (canvas spike only; `None` = auto).
    pub renderer: Option<String>,
}

/// Parsed command line of the desktop binary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    /// `--ipc-bench`.
    pub bench: Option<BenchSettings>,
    /// `--timing-check` (M5 C5.1b).
    pub timing_check: Option<TimingCheckSettings>,
    /// `--spike tablet [--spike-log <file>]` (M0 spike, C0.2).
    pub spike: Option<SpikeSettings>,
    /// `--user-dir <dir>`: the per-user folder (settings, recovery, backups) instead of the platform's.
    pub user_dir: Option<PathBuf>,
    /// `--mcp-http <address>`: serve attached mode on this loopback address from start-up, whatever the stored
    /// preference says; after start-up the user's `agent.server.set` (or the preference) stops or restarts it, and a
    /// restart in the same run uses this address and the same token (ruling O-21).
    pub mcp_http: Option<std::net::SocketAddr>,
    /// `--mcp-discovery <file>`: also write the discovery file here.
    pub mcp_discovery: Option<PathBuf>,
    /// Documents to open.
    pub open: Vec<PathBuf>,
    /// `--canvas-metrics`: the interactive canvas publishes its frame, input and load timings in its UI-tree value
    /// (M4 C4.2b measurements; plan §6, ruling D-5).
    pub canvas_metrics: bool,
    /// `--canvas-backend webgpu|webgl2`: the canvas's back end, overriding the selection (ADR-0007 rule set step 1's
    /// user override, for measurements).
    pub canvas_backend: Option<String>,
}

impl LaunchOptions {
    /// The first M0 diagnostic flag this launch requests, if any (refused by a build without `m0-diagnostics`).
    #[must_use]
    pub fn requested_diagnostic(&self) -> Option<&'static str> {
        if self.bench.is_some() {
            Some("--ipc-bench")
        } else if self.timing_check.is_some() {
            Some("--timing-check")
        } else if self.spike.is_some() {
            Some("--spike")
        } else {
            None
        }
    }
}

/// Managed state: the benchmark settings, if any.
#[cfg(feature = "m0-diagnostics")]
#[derive(Debug, Default)]
pub struct Diagnostics {
    /// Settings of a requested benchmark run.
    pub bench: Option<BenchSettings>,
    /// Settings of a requested Timeline three-way check (M5 C5.1b).
    pub timing_check: Option<TimingCheckSettings>,
}

/// What the front end should measure.
#[cfg(feature = "m0-diagnostics")]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchPlan {
    /// Samples per payload size.
    pub count: u32,
    /// Payload sizes in bytes.
    pub payload_bytes: Vec<usize>,
}

/// Returns the benchmark plan when the app was started with `--ipc-bench`, otherwise `null`.
#[cfg(feature = "m0-diagnostics")]
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
#[cfg(feature = "m0-diagnostics")]
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
#[cfg(feature = "m0-diagnostics")]
pub fn arm_timeout(app: &AppHandle, settings: &BenchSettings) {
    let app = app.clone();
    let out = settings.out.clone();
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        let _ = write_json(&out, &serde_json::json!({ "ok": false, "error": "timeout waiting for bench_report" }));
        app.exit(2);
    });
}

#[cfg(feature = "m0-diagnostics")]
fn write_json(path: &Path, value: &serde_json::Value) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    std::fs::write(path, text + "\n")
}

#[cfg(feature = "m0-diagnostics")]
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
    fn canvas_measurement_flags() {
        let o = parse_args(args(&["--canvas-metrics", "--canvas-backend", "webgpu"])).unwrap();
        assert!(o.canvas_metrics);
        assert_eq!(o.canvas_backend.as_deref(), Some("webgpu"));
        assert_eq!(o.requested_diagnostic(), None, "product flags, allowed in every build");
        assert!(parse_args(args(&["--canvas-backend", "vulkan"])).is_err());
        assert_eq!(parse_args(args(&["--canvas-backend"])), Err(ArgsError::MissingValue("--canvas-backend")));
    }

    #[test]
    fn bench_with_count() {
        let s = parse_args(args(&["--ipc-bench", "o.json", "--ipc-bench-count", "50"])).unwrap().bench.unwrap();
        assert_eq!(s, BenchSettings { out: "o.json".into(), count: 50 });
        let s = parse_args(args(&["--ipc-bench", "o.json"])).unwrap().bench.unwrap();
        assert_eq!(s.count, DEFAULT_COUNT);
    }

    #[test]
    fn timing_check_flags() {
        let s = parse_args(args(&["--timing-check", "t.json"])).unwrap();
        assert_eq!(
            s.timing_check,
            Some(TimingCheckSettings { out: "t.json".into(), external: false, only: None, budgets: false })
        );
        assert_eq!(s.requested_diagnostic(), Some("--timing-check"));
        let s =
            parse_args(args(&["--timing-check", "t.json", "--timing-check-external", "--timing-check-only", "row"]))
                .unwrap()
                .timing_check
                .unwrap();
        assert!(s.external);
        assert_eq!(s.only.as_deref(), Some("row"));
        assert_eq!(parse_args(args(&["--timing-check"])), Err(ArgsError::MissingValue("--timing-check")));
        assert_eq!(parse_args(args(&["--timing-check-external"])), Err(ArgsError::ExternalWithoutCheck));
        assert_eq!(parse_args(args(&["--timing-check-only", "x"])), Err(ArgsError::ExternalWithoutCheck));
    }

    #[test]
    fn spike_flags() {
        let o = parse_args(args(&["--spike", "tablet", "--spike-log", "t.json"])).unwrap();
        let s = o.spike.unwrap();
        assert_eq!((s.name.as_str(), s.log_path), ("tablet", Some(PathBuf::from("t.json"))));
        assert_eq!(parse_args(args(&["--spike", "paint"])), Err(ArgsError::UnknownSpike("paint".into())));
        let canvas = parse_args(args(&["--spike", "canvas", "--spike-renderer", "webgl2"])).unwrap().spike.unwrap();
        assert_eq!((canvas.name.as_str(), canvas.renderer.as_deref()), ("canvas", Some("webgl2")));
        let auto = parse_args(args(&["--spike", "canvas", "--spike-renderer", "auto"])).unwrap().spike.unwrap();
        assert_eq!(auto.renderer, None);
        assert_eq!(
            parse_args(args(&["--spike", "canvas", "--spike-renderer", "gl"])),
            Err(ArgsError::BadRenderer("gl".into()))
        );
        assert_eq!(
            parse_args(args(&["--spike", "tablet", "--spike-renderer", "webgl2"])),
            Err(ArgsError::RendererWithoutCanvas)
        );
        assert_eq!(parse_args(args(&["--spike-log", "t.json"])), Err(ArgsError::LogWithoutSpike));
        assert_eq!(parse_args(args(&["--spike"])), Err(ArgsError::MissingValue("--spike")));
        assert_eq!(parse_args(args(&["--spike", "tablet", "--spike-auto"])), Err(ArgsError::AutoWithoutLog));
        let auto = parse_args(args(&["--spike", "tablet", "--spike-log", "t.json", "--spike-auto"])).unwrap();
        assert!(auto.spike.unwrap().auto);
    }

    #[test]
    fn requested_diagnostic_names_the_flag() {
        assert_eq!(parse_args(args(&[])).unwrap().requested_diagnostic(), None);
        assert_eq!(parse_args(args(&["--ipc-bench", "o"])).unwrap().requested_diagnostic(), Some("--ipc-bench"));
        assert_eq!(parse_args(args(&["--spike", "canvas"])).unwrap().requested_diagnostic(), Some("--spike"));
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

    #[test]
    fn mcp_http_accepts_loopback_only() {
        for good in ["127.0.0.1:0", "[::1]:47215", "127.0.0.2:5"] {
            assert!(parse_args(args(&["--mcp-http", good])).is_ok(), "{good}");
        }
        for bad in ["0.0.0.0:0", "[::]:0", "192.168.1.2:47215", "[fe80::1]:1"] {
            assert_eq!(parse_args(args(&["--mcp-http", bad])), Err(ArgsError::RemoteAddress(bad.to_owned())), "{bad}");
        }
    }
}
