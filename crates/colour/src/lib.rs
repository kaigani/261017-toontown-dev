//! # tt-colour — colour engine
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Palettes, styles and swatches (solid, linear/radial gradient, texture), colour IDs, colour recovery, tint
//! offset/blend, gradient and texture mapping, `.animpal` content.
//!
//! Spec sections it will implement: 03 §1; 03 §3–§5; 03 §7; 03 §9–§12. Milestone: M2 (model and paint), M7
//! (complete).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["03 §1", "03 §3–§5", "03 §7", "03 §9–§12"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M2 (model and paint), M7 (complete)";
