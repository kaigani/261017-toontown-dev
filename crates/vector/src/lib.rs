//! # tt-vector — vector engine
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Vector geometry for drawings: curve fitting independent of input timing, brush outline sweep from pressure,
//! pencil lines, planar map (noding, zones, sides), gap closing, flatten/optimize/smooth, centre-line extraction,
//! text outlines, bitmap tracing (vectorize). All maths deterministic (fixed operation order, no platform libm in
//! persisted geometry; ADR-0004).
//!
//! Spec sections it will implement: 02 §2–§3; 02 §6 (pens, pressure); 02 §8; 02 §11 (vectorize); 03 §8 (gap
//! closing, paint by point). Milestone: M2.

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["02 §2–§3", "02 §6 (pens, pressure)", "02 §8", "02 §11 (vectorize)", "03 §8 (gap closing, paint by point)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M2";
