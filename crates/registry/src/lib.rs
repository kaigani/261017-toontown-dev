//! # tt-registry — command registry
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Declarations of every command and event: id, kind (`action`, `toggle`, `query`, `job`, `ui`, `event`), label,
//! parameter and result JSON Schemas, enablement rule with reason, undo label, permission scopes, confirmation flag
//! and examples; the registry version (semver) reported to agents.
//!
//! Commands are declared next to their implementation with an attribute macro (M1) so the registry, the parity test
//! (11 §14.1) and the schema/example tests (11 §14.2) are generated from one source.
//!
//! Spec sections it will implement: 11 §2 (registry); 11 §7.2; 11 §8 (kinds); 11 §14.1–§14.2; A (command index).
//! Milestone: M1 (C1.2).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["11 §2 (registry)", "11 §7.2", "11 §8 (kinds)", "11 §14.1–§14.2", "A (command index)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.2)";

/// Command-registry version (semver; minor = additive, major = breaking; 11 §4). `0.0.0` until M1 registers
/// commands.
pub const REGISTRY_VERSION: &str = "0.0.0";
