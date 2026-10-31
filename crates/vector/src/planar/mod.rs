//! The planar map of a layer (02 §3.5, §3.12; 03 §8; ADR-0017): persistent noding of the stored strokes, the derived
//! graph with zones and sides, zone fills read from and written to stroke sides, fill carry-over across edits, gap
//! closing and point location. Pure functions over stroke geometry; the executor stages what they return.

pub mod fill;
pub mod flat;
pub mod graph;
pub mod node;

pub use graph::{EdgeKind, LayerMap, StrokeIn, Zone, build};
