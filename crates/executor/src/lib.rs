//! # tt-executor — serialized command executor
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! The single document thread and transaction queue through which **every** document mutation flows, from the GUI
//! and from agents alike (11 P2): validate → enablement → permission/confirmation → execute as transaction → record
//! undo step → emit change events. Undo/redo, copy-on-write checkpoints, atomic batches, `dryRun`, `expectRev`,
//! `requestId` idempotency.
//!
//! Spec sections it will implement: 11 §2 (executor); 11 §7.2–§7.3; 11 §7.7; 10 §2.5–§2.6 (REV-01…06, change sets);
//! 01 §6.1 (undo/redo). Milestone: M1 (C1.2).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["11 §2 (executor)", "11 §7.2–§7.3", "11 §7.7", "10 §2.5–§2.6 (REV-01…06, change sets)", "01 §6.1 (undo/redo)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.2)";
