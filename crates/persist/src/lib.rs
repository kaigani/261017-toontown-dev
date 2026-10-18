//! # tt-persist — persistence
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Project package (`.animpkg`, zip with ZIP64) and its folder form, manifest, atomic save, backups, autosave,
//! crash recovery, locks, schema migrations; `.animtpl`, `.animpal` and `.animlib` containers. Reads and writes go
//! through the model's single serializer (10 DM-1); loading produces documents, it never mutates an open one.
//!
//! Spec sections it will implement: 10 §6–§8; 01 §3 (projects, autosave, backups). Milestone: M1 (C1.3).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["10 §6–§8", "01 §3 (projects, autosave, backups)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.3)";
