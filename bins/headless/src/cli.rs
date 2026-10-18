//! Argument parsing and dispatch for `toontown-headless`.

use std::io::Write;
use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use tt_engine::{CheckResult, Engine, EngineConfig, EngineError, RunMode, SelftestReport};

/// Command line of the headless binary (11 §3).
#[derive(Debug, Parser)]
#[command(
    name = "toontown-headless",
    about = "Toontown engine without a window: self-test, MCP over stdio, batch scripts",
    disable_version_flag = true,
    group(ArgGroup::new("action").required(true).args(["version", "selftest", "mcp_stdio", "run"]))
)]
pub struct Cli {
    /// Print version information and exit.
    #[arg(long)]
    pub version: bool,

    /// Run the engine self-test (no display, window system or GPU needed). Exit status 1 if any check fails.
    #[arg(long)]
    pub selftest: bool,

    /// Print the self-test report as JSON (only with `--selftest`).
    // `conflicts_with_all` rather than `requires`: clap treats a bool flag's implicit `false` as satisfying `requires`.
    #[arg(long, conflicts_with_all = ["version", "mcp_stdio", "run"])]
    pub json: bool,

    /// Accepted for compatibility with the spec's `app --headless …` form (11 §3); this binary is always headless.
    #[arg(long)]
    pub headless: bool,

    /// Serve MCP over stdio (available from M1, C1.4).
    #[arg(long)]
    pub mcp_stdio: bool,

    /// Execute a batch script and exit (available from M1, C1.4).
    #[arg(long, value_name = "BATCH_JSON")]
    pub run: Option<PathBuf>,

    /// Project to open first (with `--mcp-stdio` or `--run`).
    #[arg(long, value_name = "PATH", conflicts_with_all = ["version", "selftest"])]
    pub project: Option<PathBuf>,

    /// Write the batch log (JSON lines) here (only with `--run`).
    #[arg(long, value_name = "LOG_JSONL", conflicts_with_all = ["version", "selftest", "mcp_stdio"])]
    pub out: Option<PathBuf>,
}

/// Successful outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The requested action completed.
    Success,
    /// The self-test ran and at least one check failed.
    SelftestFailed,
}

impl Outcome {
    /// Process exit status.
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::SelftestFailed => 1,
        }
    }
}

/// Errors of the headless binary.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// The requested run mode exists in the spec but not yet in this build.
    #[error("{feature} is not available in this build (planned for {milestone})")]
    NotYetAvailable {
        /// The flag that was requested.
        feature: &'static str,
        /// Milestone that delivers it.
        milestone: &'static str,
    },
    /// The engine failed.
    #[error("engine error {code}: {0}", code = .0.code())]
    Engine(#[from] EngineError),
    /// Writing output failed.
    #[error("cannot write output: {0}")]
    Io(#[from] std::io::Error),
    /// Serializing the report failed.
    #[error("cannot serialize report: {0}")]
    Json(#[from] serde_json::Error),
}

impl CliError {
    /// Process exit status.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::NotYetAvailable { .. } => 3,
            Self::Engine(_) | Self::Json(_) => 1,
            Self::Io(_) => 4,
        }
    }
}

/// Runs the parsed command line, writing normal output to `out`.
pub fn run(cli: &Cli, out: &mut impl Write) -> Result<Outcome, CliError> {
    if cli.version {
        let engine = Engine::new(EngineConfig { run_mode: RunMode::Headless })?;
        let info = engine.info();
        writeln!(
            out,
            "toontown-headless {} ({} {}; format {}; registry {}; {}-{})",
            info.version, info.name, info.version, info.format_version, info.registry_version, info.platform, info.arch
        )?;
        return Ok(Outcome::Success);
    }
    if cli.selftest {
        let engine = Engine::new(EngineConfig { run_mode: RunMode::Headless })?;
        let report = selftest(&engine);
        if cli.json {
            serde_json::to_writer_pretty(&mut *out, &report)?;
            writeln!(out)?;
        } else {
            write_human(&report, out)?;
        }
        return Ok(if report.ok { Outcome::Success } else { Outcome::SelftestFailed });
    }
    if cli.mcp_stdio {
        return Err(CliError::NotYetAvailable { feature: "--mcp-stdio", milestone: "M1 (C1.4)" });
    }
    if cli.run.is_some() {
        return Err(CliError::NotYetAvailable { feature: "--run", milestone: "M1 (C1.4)" });
    }
    // Unreachable through clap (the `action` group is required), but kept typed rather than panicking.
    Err(CliError::NotYetAvailable { feature: "no action", milestone: "n/a" })
}

/// The engine self-test plus the checks only this binary can make.
fn selftest(engine: &Engine) -> SelftestReport {
    let mut report = engine.selftest();
    report.push(CheckResult {
        id: "headless.mcpLinked",
        ok: tt_mcp::NAME == "tt-mcp" && !tt_mcp::SPEC_SECTIONS.is_empty(),
        detail: format!("{} linked ({})", tt_mcp::NAME, tt_mcp::MILESTONE),
    });
    report
}

fn write_human(report: &SelftestReport, out: &mut impl Write) -> std::io::Result<()> {
    let e = &report.engine;
    writeln!(out, "{} {} self-test ({}-{}, run mode headless)", e.name, e.version, e.platform, e.arch)?;
    for c in &report.checks {
        writeln!(out, "  [{}] {:<22} {}", if c.ok { "ok" } else { "FAIL" }, c.id, c.detail)?;
    }
    writeln!(out, "{}", if report.ok { "self-test passed" } else { "self-test FAILED" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("toontown-headless").chain(args.iter().copied()))
    }

    fn run_to_string(args: &[&str]) -> (Result<Outcome, CliError>, String) {
        let cli = parse(args).unwrap();
        let mut buf = Vec::new();
        let r = run(&cli, &mut buf);
        (r, String::from_utf8(buf).unwrap())
    }

    #[test]
    fn an_action_is_required() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["--headless"]).is_err());
    }

    #[test]
    fn actions_are_exclusive() {
        assert!(parse(&["--version", "--selftest"]).is_err());
    }

    #[test]
    fn option_combinations() {
        assert!(parse(&["--version", "--json"]).is_err());
        assert!(parse(&["--selftest", "--json"]).is_ok());
        assert!(parse(&["--selftest", "--project", "p.animpkg"]).is_err());
        assert!(parse(&["--mcp-stdio", "--project", "p.animpkg"]).is_ok());
        assert!(parse(&["--mcp-stdio", "--out", "x.jsonl"]).is_err());
        assert!(parse(&["--run", "b.json", "--out", "x.jsonl"]).is_ok());
    }

    #[test]
    fn version_line() {
        let (r, out) = run_to_string(&["--version"]);
        assert_eq!(r.unwrap(), Outcome::Success);
        assert!(out.starts_with(&format!("toontown-headless {}", env!("CARGO_PKG_VERSION"))), "{out}");
        assert!(out.contains("format 0.0"));
    }

    #[test]
    fn selftest_json_passes() {
        let (r, out) = run_to_string(&["--selftest", "--json"]);
        assert_eq!(r.unwrap(), Outcome::Success);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["engine"]["runMode"], "headless");
        assert!(v["checks"].as_array().unwrap().iter().any(|c| c["id"] == "headless.mcpLinked"));
    }

    #[test]
    fn selftest_human_passes() {
        let (r, out) = run_to_string(&["--selftest"]);
        assert_eq!(r.unwrap(), Outcome::Success);
        assert!(out.trim_end().ends_with("self-test passed"), "{out}");
    }

    #[test]
    fn future_modes_are_typed_errors() {
        let (r, _) = run_to_string(&["--headless", "--mcp-stdio"]);
        let e = r.unwrap_err();
        assert!(matches!(e, CliError::NotYetAvailable { feature: "--mcp-stdio", .. }));
        assert_eq!(e.exit_code(), 3);
        let (r, _) = run_to_string(&["--run", "batch.json", "--project", "p.animpkg"]);
        assert!(matches!(r.unwrap_err(), CliError::NotYetAvailable { feature: "--run", .. }));
    }
}
