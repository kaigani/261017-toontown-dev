//! Storyline transitions (10 §4.4, 08 §13.1, §16.2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::TransitionTag;
use crate::value::Colour;

/// Transition type (08 §13.1). Persisted as `transitionType` because `type` is the envelope field (SI-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TransitionType {
    /// No transition (new edit points).
    #[default]
    None,
    /// Fade through a colour.
    Fade,
    /// Cross dissolve.
    CrossDissolve,
    /// Wipe.
    Wipe,
    /// Slide.
    Slide,
    /// Clock wipe.
    ClockWipe,
    /// Checkerboard.
    Checker,
}

/// Transition direction (08 §13.1; type-dependent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TransitionDirection {
    /// Slide in.
    In,
    /// Slide out.
    Out,
    /// Clock wipe clockwise.
    Clockwise,
    /// Clock wipe counter-clockwise.
    Counterclockwise,
}

model_struct! {
    /// A transition at an edit point: `project.startTransition` or a scene's `outTransition` (10 §4.4). Fields not
    /// used by the current type are kept so that switching back restores them.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "transition"))]
    pub struct Transition {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `transition`.
        #[serde(rename = "type")]
        ty: TransitionTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// The transition type (08 `type`; SI-004).
        #[schemars(extend("x-command" = "storyline.transition.set"))]
        transition_type: TransitionType,
        /// Duration / left duration, 0–1000 frames (default 12).
        #[schemars(range(max = 1000), extend("x-unit" = "frame", "x-command" = "storyline.transition.set"))]
        duration_left: u32,
        /// Blank duration (fade only), 0–1000 frames (default 0).
        #[schemars(range(max = 1000), extend("x-unit" = "frame", "x-command" = "storyline.transition.set"))]
        blank_duration: u32,
        /// Right duration (fade only), 0–1000 frames (default 12).
        #[schemars(range(max = 1000), extend("x-unit" = "frame", "x-command" = "storyline.transition.set"))]
        duration_right: u32,
        /// Fade colour (default opaque black).
        #[schemars(extend("x-command" = "storyline.transition.set"))]
        colour: Colour,
        /// Angle for wipe, slide and clock wipe, 0–360 degrees (default 0).
        #[schemars(range(min = 0.0, max = 360.0), extend("x-unit" = "degree", "x-command" = "storyline.transition.set"))]
        angle: f64,
        /// Direction for slide and clock wipe; `null` until a type that uses it is chosen.
        #[schemars(extend("x-command" = "storyline.transition.set"))]
        direction: Option<TransitionDirection>,
        /// Checker size, 5–100 in steps of 5 (default 25).
        #[schemars(range(min = 5, max = 100), extend("multipleOf" = 5, "x-command" = "storyline.transition.set"))]
        size: u32,
        /// Free-form metadata.
        #[serde(default, skip_serializing_if = "meta_is_empty")]
        #[schemars(extend("x-omitDefault" = true))]
        meta: Meta,
        /// Extension data.
        #[serde(default, skip_serializing_if = "ext_is_empty")]
        #[schemars(extend("x-omitDefault" = true))]
        ext: Ext,
    }
}

impl_object!(Transition, Transition);

impl Transition {
    /// A transition of type `none` with the defaults of 08 §16.2.
    #[must_use]
    pub fn none(id: Id) -> Self {
        Self {
            id,
            ty: TransitionTag,
            rev: 0,
            transition_type: TransitionType::None,
            duration_left: 12,
            blank_duration: 0,
            duration_right: 12,
            colour: Colour::BLACK,
            angle: 0.0,
            direction: None,
            size: 25,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}
