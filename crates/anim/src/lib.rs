//! # tt-anim — animation engine
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Per-frame evaluation: exposures → drawing, peg hierarchy → world matrices (normative transform maths), TCB
//! motion paths with arc-length parameterisation, velocity, channels and easing, bones with linear-blend skinning,
//! cameras and projection with multiplane depth.
//!
//! Spec sections it will implement: 00 §4.1 (per-frame evaluation); 05 §2 (transform mathematics); 05 §7–§10; 05
//! §12–§13. Milestone: M3 (maths), M6 (complete).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["00 §4.1 (per-frame evaluation)", "05 §2 (transform mathematics)", "05 §7–§10", "05 §12–§13"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M3 (maths), M6 (complete)";
