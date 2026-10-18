//! # tt-fx — effects and particles
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Effect elements and their scopes over hierarchy sub-trees, effect parameters, and the particle engine. Particle
//! randomness uses an in-house seeded PRNG whose seed is stored in the document (11 P7), so simulations are
//! byte-identical across runs, restarts and machines.
//!
//! Spec sections it will implement: 06 §2–§4; 06 §7. Milestone: M7.

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["06 §2–§4", "06 §7"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M7";
