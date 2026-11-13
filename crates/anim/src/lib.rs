//! # tt-anim — animation engine
//!
//! **Status: M3 C3.2.** Exposure algebra ([`exposure`]: frame arrays, canonical runs and the cell operations of
//! 04 §13), the scene hierarchy ([`hierarchy`]: Timeline order, parenting, 04 §9), and the evaluator (ADR-0019):
//! [`matrix`] (05 §2), [`tcb`] (05 §9, §10.5), [`function`] (05 §10.3, §10.4), [`timing`] (05 §2.10, §8.4), [`node`]
//! (05 §1.5, §2.2, §2.3, §8.3), [`eval`] (05 §2.4, §2.9) and [`camera`] (05 §2.6). Bones and skinning follow in M6 C6.3.
//!
//! Per-frame evaluation: exposures → drawing, peg hierarchy → world matrices (normative transform maths), TCB
//! motion paths with arc-length parameterisation, velocity, channels and easing, bones with linear-blend skinning,
//! cameras and projection with multiplane depth.
//!
//! Spec sections it will implement: 00 §4.1 (per-frame evaluation); 04 §13 (exposure operations); 05 §2 (transform
//! mathematics); 05 §7–§10; 05 §12–§13. Milestone: M3 (maths), M6 (complete).

pub mod camera;
pub mod eval;
pub mod exposure;
pub mod function;
pub mod hierarchy;
pub mod matrix;
pub mod node;
pub mod num;
pub mod tcb;
pub mod timing;

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
