//! Argument parsing and dispatch for `toontown-headless`.

use std::collections::BTreeSet;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use clap::{ArgGroup, Parser};
use serde_json::json;
use tt_engine::{CheckResult, Engine, EngineConfig, EngineError, RunMode, SelftestReport};
use tt_executor::agents::{Category, Policy, parse_scope};
use tt_executor::{
    ExecutorHandle, ExecutorOptions, Invocation, InvokeOptions, Origin, PersistOptions, Status, UserDirs,
};
use tt_mcp::ServerOptions;

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

    /// Serve MCP over stdio: newline-delimited JSON-RPC on stdin/stdout; diagnostics on stderr.
    #[arg(long)]
    pub mcp_stdio: bool,

    /// Execute a batch script (the `batch` tool's JSON) and exit: 0 when every step succeeded, 5 otherwise.
    #[arg(long, value_name = "BATCH_JSON")]
    pub run: Option<PathBuf>,

    /// Project to open first (with `--mcp-stdio` or `--run`).
    #[arg(long, value_name = "PATH", conflicts_with_all = ["version", "selftest"])]
    pub project: Option<PathBuf>,

    /// Write the batch log (JSON lines) here (only with `--run`).
    #[arg(long, value_name = "LOG_JSONL", conflicts_with_all = ["version", "selftest", "mcp_stdio"])]
    pub out: Option<PathBuf>,

    /// Permission policy of the agent session (`observe`, `assist`, `autonomous`; 11 §6.1). Default: `assist`.
    #[arg(long, value_name = "POLICY", conflicts_with_all = ["version", "selftest"])]
    pub policy: Option<String>,

    /// Scopes granted to the agent session, comma-separated (`read,edit,files.read,…`; 11 §6.2). Default: the
    /// policy's. `app` is never granted to an agent.
    #[arg(long, value_name = "SCOPES", value_delimiter = ',', conflicts_with_all = ["version", "selftest"])]
    pub grant: Vec<String>,

    /// A folder the agent may read and write besides the open project's (repeatable; 11 §6.2 roots).
    #[arg(long = "allow-path", value_name = "DIR", conflicts_with_all = ["version", "selftest"])]
    pub allow_path: Vec<PathBuf>,

    /// Confirmation categories pre-authorised for calls that pass `confirm: true`, comma-separated
    /// (`overwrite,destructive,discard,network,print,externalChange`; 11 §6.3).
    #[arg(long, value_name = "CATEGORIES", value_delimiter = ',', conflicts_with_all = ["version", "selftest"])]
    pub preauthorize: Vec<String>,

    /// Expose commands as individual MCP tools (`agent.typedTools.set`): all, or the given namespaces.
    #[arg(long = "typed-tools", value_name = "NAMESPACES", num_args = 0..=1, default_missing_value = "all",
          conflicts_with_all = ["version", "selftest", "run"])]
    pub typed_tools: Option<String>,

    /// Per-user folder (settings, recovery, backups) instead of the platform's (01 §11).
    #[arg(long = "user-dir", value_name = "DIR", conflicts_with_all = ["version", "selftest"])]
    pub user_dir: Option<PathBuf>,
}

/// Successful outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The requested action completed.
    Success,
    /// The self-test ran and at least one check failed.
    SelftestFailed,
    /// The batch script ran and a step failed (or needed a confirmation nobody could give).
    BatchFailed,
}

impl Outcome {
    /// Process exit status.
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::SelftestFailed => 1,
            Self::BatchFailed => 5,
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
    /// A flag's value or the batch script is not valid.
    #[error("{0}")]
    Usage(String),
    /// The executor, the project or the MCP server failed.
    #[error("{0}")]
    Failed(String),
}

impl CliError {
    /// Process exit status.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::NotYetAvailable { .. } => 3,
            Self::Engine(_) | Self::Json(_) | Self::Failed(_) => 1,
            Self::Usage(_) => 2,
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
        let opts = server_options(cli)?;
        let handle = start(cli)?;
        let stdin = BufReader::new(std::io::stdin());
        tt_mcp::serve(handle, opts, stdin, std::io::stdout()).map_err(CliError::Failed)?;
        return Ok(Outcome::Success);
    }
    if let Some(script) = &cli.run {
        let opts = server_options(cli)?;
        let req = tt_mcp::script::load(script).map_err(CliError::Usage)?;
        let handle = start(cli)?;
        let mut log = match &cli.out {
            Some(p) => Some(std::fs::File::create(p)?),
            None => None,
        };
        let outcome = tt_mcp::script::run(&handle, &opts, req, log.as_mut().map(|f| f as &mut dyn Write))
            .map_err(CliError::Failed)?;
        serde_json::to_writer_pretty(&mut *out, &outcome.envelope)?;
        writeln!(out)?;
        return Ok(if outcome.ok { Outcome::Success } else { Outcome::BatchFailed });
    }
    // Unreachable through clap (the `action` group is required), but kept typed rather than panicking.
    Err(CliError::NotYetAvailable { feature: "no action", milestone: "n/a" })
}

/// The agent session's options from the flags.
fn server_options(cli: &Cli) -> Result<ServerOptions, CliError> {
    let policy = match &cli.policy {
        Some(p) => Some(
            Policy::parse(p)
                .ok_or_else(|| CliError::Usage(format!("unknown policy `{p}` (observe, assist, autonomous)")))?,
        ),
        None => None,
    };
    let scopes = if cli.grant.is_empty() {
        None
    } else {
        let mut set = BTreeSet::new();
        for g in cli.grant.iter().map(|g| g.trim()).filter(|g| !g.is_empty()) {
            if g == "app" {
                return Err(CliError::Usage("the `app` scope is never granted to an agent (11 §6.2)".to_owned()));
            }
            set.insert(parse_scope(g).ok_or_else(|| CliError::Usage(format!("unknown scope `{g}`")))?);
        }
        Some(set)
    };
    let mut preauthorized = BTreeSet::new();
    for c in cli.preauthorize.iter().map(|c| c.trim()).filter(|c| !c.is_empty()) {
        preauthorized
            .insert(Category::parse(c).ok_or_else(|| CliError::Usage(format!("unknown confirmation category `{c}`")))?);
    }
    let roots = cli.allow_path.iter().map(|p| absolute(p)).collect::<Result<Vec<_>, _>>()?;
    Ok(ServerOptions { policy, scopes, roots, preauthorized, ..ServerOptions::default() })
}

fn absolute(p: &Path) -> Result<PathBuf, CliError> {
    Ok(std::path::absolute(p)?)
}

/// Starts the executor with the user folders, opens `--project` and applies `--typed-tools`.
fn start(cli: &Cli) -> Result<ExecutorHandle, CliError> {
    let user_dirs = match &cli.user_dir {
        Some(d) => Some(UserDirs::under(&absolute(d)?)),
        None => UserDirs::platform(),
    };
    let options = ExecutorOptions {
        persist: PersistOptions { user_dirs, ..PersistOptions::default() },
        ..ExecutorOptions::default()
    };
    let handle = ExecutorHandle::spawn_with_options(options).map_err(|e| CliError::Failed(e.message))?;
    let user = |command: &str, params: serde_json::Value| {
        handle.invoke(Invocation {
            command: command.into(),
            params,
            options: InvokeOptions::default(),
            origin: Origin::User,
        })
    };
    if let Some(project) = &cli.project {
        let path = absolute(project)?;
        let env = user("project.open", json!({ "path": path.to_string_lossy() }));
        let fail = |why: String| CliError::Failed(format!("cannot open {}: {why}", path.display()));
        if env.status != Status::Ok {
            return Err(fail(
                env.error.map_or_else(|| "needs confirmation".to_owned(), |e| format!("{}: {}", e.code, e.message)),
            ));
        }
        // Opening is a job (10 §8): serve only once the project is installed.
        if let Some(job) = env.result.as_ref().and_then(|r| r.get("jobId")).cloned() {
            let waited = user("job.wait", json!({ "jobId": job, "timeoutMs": 600_000 }));
            let record = waited.result.unwrap_or_default();
            let record = record.get("record").cloned().unwrap_or(record);
            if record.get("state").and_then(serde_json::Value::as_str) != Some("succeeded") {
                return Err(fail(record.get("error").map_or_else(|| record.to_string(), ToString::to_string)));
            }
        }
    }
    if let Some(ns) = &cli.typed_tools {
        let namespaces: Vec<&str> =
            if ns == "all" { Vec::new() } else { ns.split(',').map(str::trim).filter(|n| !n.is_empty()).collect() };
        let env = user("agent.typedTools.set", json!({ "enabled": true, "namespaces": namespaces }));
        if let Some(e) = env.error {
            return Err(CliError::Usage(format!("--typed-tools: {}", e.message)));
        }
    }
    Ok(handle)
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
        assert!(out.contains("format 1.0"), "{out}");
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
    fn agent_flags_are_validated() {
        assert!(
            server_options(&parse(&["--mcp-stdio", "--policy", "assist", "--grant", "read,edit"]).unwrap()).is_ok()
        );
        let e = server_options(&parse(&["--mcp-stdio", "--policy", "root"]).unwrap()).unwrap_err();
        assert_eq!(e.exit_code(), 2);
        let e = server_options(&parse(&["--mcp-stdio", "--grant", "read,app"]).unwrap()).unwrap_err();
        assert!(e.to_string().contains("app"), "{e}");
        assert!(server_options(&parse(&["--mcp-stdio", "--preauthorize", "overwrite,sometimes"]).unwrap()).is_err());
        assert!(parse(&["--selftest", "--grant", "read"]).is_err());
        assert!(parse(&["--run", "b.json", "--typed-tools"]).is_err());
    }

    #[test]
    fn a_missing_batch_script_is_a_usage_error() {
        let (r, _) = run_to_string(&["--run", "no-such-batch.json"]);
        assert_eq!(r.unwrap_err().exit_code(), 2);
    }
}
