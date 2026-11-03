//! # tt-io — import and export
//!
//! **Status:** bounded bitmap decoding for import and vectorize ([`image`], M2 C2.4); the rest arrives in M10.
//!
//! Import of images, layered PSD, PDF/AI, SVG, sound and movies; export of image sequences
//! (PNG/JPEG/TGA/BMP/TIFF/GIF), PDF, print layout; the LGPL FFmpeg sidecar driver (process boundary only, never
//! linked; ADR-0003) for MP4/MOV/GIF and movie decode; stop-motion capture devices.
//!
//! Spec sections it will implement: 09 §4–§9; 07 §12 (movie import). Milestone: M10.

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["09 §4–§9", "07 §12 (movie import)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M10";

pub mod image;
