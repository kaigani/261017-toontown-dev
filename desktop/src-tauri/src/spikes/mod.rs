//! **M0 spikes. Not product code.** Measurement prototypes for the go/no-go report (C0.4). Nothing here may move
//! into product code without being rewritten with tests (C0.1 brief, ADR-0002).
//!
//! Spikes are reachable only when the app is launched with `--spike <name>`; they are not in the menu, and every
//! spike command refuses with `spike.notEnabled` otherwise. No MCP server exists in M0, and none of these commands
//! may ever be exposed to agents.

pub mod canvas;
pub mod tablet;
