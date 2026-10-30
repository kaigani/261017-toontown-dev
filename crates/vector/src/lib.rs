//! # tt-vector — vector engine
//!
//! The drawing data model of 02 §3 as engine operations over the `tt-model` entities: Bézier paths in the persisted
//! (10 §4.9) and read-back (02 §13.4) forms, curve fitting independent of input timing (§3.3), pressure maps and brush
//! outlines swept from the pen (§3.2, §3.4), symmetry copies (§6.2), text content and layout (§3.7), the drawing
//! read-back JSON (§13.4) and the topology invariants of §3.12 as checkable properties. Deterministic: only
//! correctly rounded IEEE-754 operations in a fixed order, transcendental functions from the pure-software `libm`
//! port, unions on an integer grid (ADR-0004, ADR-0016).
//!
//! The planar map (noding, zones, sides, gap closing, paint by point) arrives with M2 C2.2; drawing operations and
//! rendering with C2.3; vectorization with C2.4.
//!
//! Spec sections: 02 §2–§3, §6 (pens, pressure, symmetry), §13.4; 10 §4.8–§4.11. Milestone: M2.

pub mod brush;
pub mod content;
pub mod drawing;
pub mod fit;
pub mod fonts;
pub mod geom;
pub mod num;
pub mod path;
pub mod stroke;
pub mod symmetry;
pub mod text;

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["02 §2–§3", "02 §6 (pens, pressure)", "02 §8", "02 §11 (vectorize)", "03 §8 (gap closing, paint by point)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M2";
