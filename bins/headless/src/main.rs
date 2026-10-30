//! `toontown-headless` — the engine without a window.
//!
//! Implements the command line of the headless and batch-script run modes (11 §3):
//!
//! ```text
//! toontown-headless --version
//! toontown-headless --selftest [--json]
//! toontown-headless [--headless] --mcp-stdio [--project <path>] [agent options]
//! toontown-headless [--headless] --run <batch.json> [--project <path>] [--out <log>] [agent options]
//!
//! agent options: --policy <p> --grant <scopes> --allow-path <dir>... --preauthorize <categories>
//!                --typed-tools [<namespaces>] (MCP only) --user-dir <dir>
//! ```
//!
//! The binary links no GUI, windowing or GPU library; CI proves it by running `--selftest` on a runner without a
//! window system and by checking the binary's linked libraries (`scripts/check-headless-deps.py`).
//!
//! Exit status: 0 success, 1 self-test or engine failure, 2 usage error (including an invalid batch script), 3 feature
//! not available in this build, 4 I/O error, 5 a batch-script step failed.

mod cli;

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // clap prints help/usage itself and knows whether this is an error (exit 2) or `--help` (exit 0).
            let _ = e.print();
            return ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(2));
        }
    };
    // Not locked: in MCP mode the server writes from its own threads.
    let mut stdout = std::io::stdout();
    match cli::run(&cli, &mut stdout) {
        Ok(outcome) => ExitCode::from(outcome.exit_code()),
        Err(e) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "toontown-headless: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
