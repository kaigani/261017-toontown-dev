//! # tt-mcp — MCP agent server
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! The MCP server (`rmcp`; stdio for headless, Streamable HTTP on 127.0.0.1 for attached mode): generic tools,
//! resources and subscriptions, prompts, sessions, permission scopes, the confirmation protocol, the audit log and
//! the discovery file. It is a client of `tt-engine` (and through it of the registry and executor) exactly like the
//! GUI; it never touches the document directly (11 P2).
//!
//! Spec sections it will implement: 11 §4; 11 §6–§7; 11 §9–§13; 11 §15. Milestone: M1 (C1.4).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["11 §4", "11 §6–§7", "11 §9–§13", "11 §15"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.4)";
