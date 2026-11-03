//! # tt-anim — animation engine
//!
//! **Status: M3 C3.1.** Exposure algebra ([`exposure`]: frame arrays, canonical runs and the cell operations of
//! 04 §13) and the scene hierarchy ([`hierarchy`]: Timeline order, parenting, 04 §9). Transform maths, motion paths,
//! channels, cameras and per-frame evaluation follow in C3.2.
//!
//! Per-frame evaluation: exposures → drawing, peg hierarchy → world matrices (normative transform maths), TCB
//! motion paths with arc-length parameterisation, velocity, channels and easing, bones with linear-blend skinning,
//! cameras and projection with multiplane depth.
//!
//! Spec sections it will implement: 00 §4.1 (per-frame evaluation); 04 §13 (exposure operations); 05 §2 (transform
//! mathematics); 05 §7–§10; 05 §12–§13. Milestone: M3 (maths), M6 (complete).

pub mod exposure;
pub mod hierarchy;

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &[
    "00 §4.1 (per-frame evaluation)",
    "04 §13 (exposure operations)",
    "05 §2 (transform mathematics)",
    "05 §7–§10",
    "05 §12–§13",
];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M3 (maths), M6 (complete)";
