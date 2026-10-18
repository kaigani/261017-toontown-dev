//! # tt-audio — audio
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! Sound import and decode (`symphonia`), waveform peaks cache, mixer with limiter, audio output (`cpal`) and
//! scrubbing, synchronised with playback.
//!
//! Spec sections it will implement: 07 §1–§4; 07 §8–§9; 07 §13. Milestone: M8 (C8.1).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["07 §1–§4", "07 §8–§9", "07 §13"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M8 (C8.1)";
