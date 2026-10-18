//! # tt-model — object model
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! The document object model shared by the project files, the in-memory document and the agent interface (10 DM-1):
//! object types and the common envelope (`id`, `type`, `rev`, `name`, `meta`, `ext`), ULID-style identifiers,
//! revisions and change sets, path addressing, canonical JSON, JSON Schemas (draft 2020-12, generated with
//! `schemars`) and validation.
//!
//! Boundary rule (plan §3.2): model types will expose **no public mutators**. Every document mutation goes through
//! `tt-executor`; M1 enforces this with crate visibility (mutation only through a transaction capability that only
//! the executor can construct) plus a CI lint.
//!
//! Spec sections it will implement: 10 §1–§5; 10 §6.5 (canonical JSON); 10 §2.8 (schemas); 11 §5 (addressing,
//! units, coordinate spaces). Milestone: M1 (C1.1).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["10 §1–§5", "10 §6.5 (canonical JSON)", "10 §2.8 (schemas)", "11 §5 (addressing, units, coordinate spaces)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.1)";

/// Project format / document schema version (`MAJOR.MINOR`, 10 §6.6 VER-01). `0.0` = pre-release, no format
/// defined yet; M1 sets the first real value. Reported in the MCP handshake (11 §4) and by `app.info.get` (01).
pub const FORMAT_VERSION: &str = "0.0";
