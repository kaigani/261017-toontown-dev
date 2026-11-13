//! Node timing (05 §2.10, §8.4): scene frame → local frame → cycle frame. Keys are stored in local frames.
//!
//! **Cycle length (SI-089).** A `count` loop's cycle is `⌊duration ÷ loops⌋` (at least 1); a remainder plays as a
//! partial cycle. A `fill` loop keeps its cycle in `cycleLength`, because `duration` and `loops` cannot express it when
//! the last cycle is partial. Cell-based elements have no bar: their local frame is the scene frame, over the scene.

use tt_model::entities::element::{Element, LoopType};

/// A node's bar as the evaluator reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    /// Scene frame of local frame 1.
    pub start: f64,
    /// Length in frames (at least 1).
    pub duration: f64,
    /// The cycle length `L` of a looped node.
    pub cycle: Option<f64>,
}

/// The cycle length of an element: see the module documentation. Elements without a bar have a cycle of their whole
/// `span`.
#[must_use]
pub fn cycle_length(e: &Element, span: u32) -> u32 {
    let Some(d) = *e.duration() else { return span.max(1) };
    let loops = (*e.loops()).max(1);
    match (e.loop_type(), e.cycle_length()) {
        (LoopType::Fill, Some(l)) => (*l).max(1),
        _ => (d / loops).max(1),
    }
}

impl Bar {
    /// The bar of `e`; `scene_length` is the span of an element without a bar.
    #[must_use]
    pub fn of(e: &Element, scene_length: u64) -> Self {
        let span = u32::try_from(scene_length).unwrap_or(u32::MAX).max(1);
        let (start, duration) = match (*e.start(), *e.duration()) {
            (Some(s), Some(d)) => (s, d.max(1)),
            _ => (1, span),
        };
        let l = cycle_length(e, span);
        let looped = e.duration().is_some() && l < duration;
        Self { start: f64::from(start), duration: f64::from(duration), cycle: looped.then_some(f64::from(l)) }
    }

    /// The local frame of scene frame `frame`, held at the nearest end of the bar (05 §2.10).
    #[must_use]
    pub fn local(&self, frame: f64) -> f64 {
        (frame - self.start + 1.0).clamp(1.0, self.duration)
    }

    /// The scene frame of local frame `local`.
    #[must_use]
    pub fn scene(&self, local: f64) -> f64 {
        local + self.start - 1.0
    }

    /// `(cycle frame, cycle index)` of a local frame: ((f − 1) mod L) + 1 (05 §2.10).
    #[must_use]
    pub fn cycle_frame(&self, local: f64) -> (f64, f64) {
        match self.cycle {
            Some(l) if local > l => {
                let k = libm::floor((local - 1.0) / l);
                (local - k * l, k)
            }
            _ => (local, 0.0),
        }
    }

    /// Whether a local frame can be keyed: inside the bar and in the first cycle (05 §19.1.4).
    #[must_use]
    pub fn keyable(&self, local: f64) -> bool {
        local >= 1.0 && local <= self.cycle.unwrap_or(self.duration).min(self.duration)
    }
}
