//! # tt-mcp — MCP agent server
//!
//! The one agent surface (plan §3.2: one command layer). Every tool is a generic wrapper over the registered
//! commands (`run_command`, `batch`, `get_object`, …); nothing is bespoke. The server speaks MCP (revision
//! 2025-06-18, newline-delimited JSON-RPC 2.0) over stdio for the headless binary: tools, resources and
//! subscriptions, events, prompts, completions, logging, progress, cancellation and elicitation. Sessions, scopes,
//! policies, the confirmation protocol and the audit log live in the executor's agent hub (`tt_executor::agents`),
//! so `agent.*` are ordinary registry commands. Streamable HTTP for attached mode arrives with M4 (ADR-0015).
//!
//! Spec sections: 11 §4; 11 §6–§7; 11 §9–§13; 11 §15. Milestone: M1 (C1.4).

pub mod docs;
pub mod jsonrpc;
pub mod prompts;
pub mod resources;
pub mod script;
pub mod server;
pub mod tools;

pub use server::{DEFAULT_PROTOCOL, PROTOCOL_VERSIONS, ServerOptions, serve};

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["11 §4", "11 §6–§7", "11 §9–§13", "11 §15"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.4)";
