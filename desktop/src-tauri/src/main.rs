//! `toontown` — the desktop application binary.

// Release builds on Windows are GUI-subsystem executables (no console window).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    match tt_desktop::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "toontown: {e}");
            ExitCode::FAILURE
        }
    }
}
