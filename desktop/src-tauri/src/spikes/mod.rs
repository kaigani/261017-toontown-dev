//! **M0 spikes. Not product code.** Measurement prototypes for the go/no-go report (C0.4). Nothing here may move
//! into product code without being rewritten with tests (C0.1 brief, ADR-0002).
//!
//! Compiled only with the `m0-diagnostics` Cargo feature (and in unit tests); the default build has no spike code
//! and registers no spike command (C0.4). Spikes are reachable only when the app is launched with `--spike <name>`;
//! they are not in the menu, and every spike command refuses with `spike.notEnabled` otherwise. No MCP server exists in M0, and none of these commands
//! may ever be exposed to agents.

pub mod canvas;
pub mod tablet;
