//! # tt-lipsync — lip-sync detection
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Automatic mouth-shape detection (A–G, X): ONNX CTC phoneme model (downloaded on first use, plan §6.4) with
//! transcript-assisted forced alignment, the always-available acoustic fallback classifier, phoneme-to-mouth
//! mapping, smoothing and frame quantisation. Deterministic on CPU.
//!
//! Spec sections it will implement: 07 §10. Milestone: M8 (C8.2).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["07 §10"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M8 (C8.2)";
