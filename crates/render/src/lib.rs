//! # tt-render — renderer
//!
//! **Status: M0 skeleton.** This crate has no functionality yet; it fixes the crate's place in the dependency
//! graph so later milestones add code without restructuring the workspace.
//!
//! The render-list compiler shared by interactive and offscreen paths (11 §2 Renderer; 06 §6), the deterministic
//! software rasteriser used for **all** offscreen renders (agent `render_frame`, previews, export; 11 §3 design
//! decision), mesh export for the web renderer and hit-test ids. Must run without display or GPU.
//!
//! C0.3 compares `vello_cpu` with an in-house exact-area-coverage rasteriser; the choice is recorded as an ADR in
//! C0.4. Spike code lives under `examples/` and is rewritten before it enters this crate.
//!
//! Spec sections it will implement: 06 §6 (render pipeline); 09 §1; 11 §3 (software rasteriser); 11 §7.5 (renders,
//! describe_frame, hit_test). Milestone: M3 (C3.3).

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["06 §6 (render pipeline)", "09 §1", "11 §3 (software rasteriser)", "11 §7.5 (renders, describe_frame, hit_test)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M3 (C3.3)";
