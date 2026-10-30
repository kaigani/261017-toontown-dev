//! Effect parameters and particle blocks (10 §4.6, 06 §3, §7, §8.4).
//!
//! An effect element's `params` holds the static value of every parameter of its effect type (06 §3); animatable
//! parameters are additionally animated by channels named by the parameter id (10 §4.12). Each effect type's
//! parameter set is a typed struct here, so the published schema of `params` is exact per `elementType`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::ParticleBlockTag;
use crate::value::{Colour, Point2};

/// Colour Transform parameters (06 §3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ColourTransformParams {
    /// Red multiplier 0–1 (animatable).
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mult_red: f64,
    /// Green multiplier 0–1 (animatable).
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mult_green: f64,
    /// Blue multiplier 0–1 (animatable).
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mult_blue: f64,
    /// Alpha multiplier 0–1 (animatable).
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mult_alpha: f64,
    /// Red offset −255…255 (animatable).
    #[schemars(range(min = -255, max = 255))]
    pub add_red: i32,
    /// Green offset −255…255 (animatable).
    #[schemars(range(min = -255, max = 255))]
    pub add_green: i32,
    /// Blue offset −255…255 (animatable).
    #[schemars(range(min = -255, max = 255))]
    pub add_blue: i32,
    /// Alpha offset −255…255 (animatable).
    #[schemars(range(min = -255, max = 255))]
    pub add_alpha: i32,
    /// Composite the sub-tree first (static).
    pub flatten: bool,
}

impl Default for ColourTransformParams {
    fn default() -> Self {
        Self {
            mult_red: 1.0,
            mult_green: 1.0,
            mult_blue: 1.0,
            mult_alpha: 1.0,
            add_red: 0,
            add_green: 0,
            add_blue: 0,
            add_alpha: 0,
            flatten: false,
        }
    }
}

/// Clipping parameters (06 §3.3).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClippingParams {
    /// Draw children outside the mask instead of inside.
    pub inverted: bool,
}

/// How an inherited Colour Transform applies to a Drop Shadow (06 §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParentColourTransformMode {
    /// To the original drawings only (default).
    #[default]
    DrawingsOnly,
    /// To drawings and shadow.
    DrawingsAndShadow,
    /// To the shadow only.
    ShadowOnly,
}

/// A 2-D affine transform as axes and origin (06 §3.4 shadow transform).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AxesTransform {
    /// Image of the x axis.
    pub x_axis: Point2,
    /// Image of the y axis.
    pub y_axis: Point2,
    /// Origin (fields).
    #[schemars(extend("x-unit" = "field"))]
    pub origin: Point2,
}

/// Drop Shadow parameters (06 §3.4; all static).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DropShadowParams {
    /// How a parent Colour Transform applies.
    pub parent_colour_transform_mode: ParentColourTransformMode,
    /// Shadow colour (alpha ignored).
    pub colour: Colour,
    /// Opacity percent 0–100.
    #[schemars(range(min = 0, max = 100), extend("x-unit" = "percent"))]
    pub opacity: u32,
    /// Draw only the shadow.
    pub hide_elements: bool,
    /// Shadow transform in effect-local space.
    pub transform: AxesTransform,
}

impl Default for DropShadowParams {
    fn default() -> Self {
        Self {
            parent_colour_transform_mode: ParentColourTransformMode::DrawingsOnly,
            colour: Colour::BLACK,
            opacity: 50,
            hide_elements: false,
            transform: AxesTransform {
                x_axis: Point2 { x: 1.0, y: 0.0 },
                y_axis: Point2 { x: 0.0, y: 1.0 },
                origin: Point2 { x: 0.2, y: -0.2 },
            },
        }
    }
}

/// Blur parameters (06 §3.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BlurParams {
    /// Uniform (true) or directional (false) blur (static).
    pub radial: bool,
    /// Radius 0–100 output pixels (animatable).
    #[schemars(range(min = 0.0, max = 100.0), extend("x-unit" = "pixel"))]
    pub radius: f64,
    /// Motion-blur direction 0–360 degrees (animatable).
    #[schemars(range(min = 0.0, max = 360.0), extend("x-unit" = "degree"))]
    pub angle: f64,
}

impl Default for BlurParams {
    fn default() -> Self {
        Self { radial: true, radius: 4.0, angle: 0.0 }
    }
}

/// Shake axes (06 §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ShakeAxis {
    /// East–west.
    X,
    /// North–south.
    Y,
    /// Both in the plane (default).
    #[default]
    Xy,
    /// Front–back.
    Z,
    /// All three.
    Xyz,
}

/// Shake parameters (06 §3.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ShakeParams {
    /// Displacement axes (static).
    pub axis: ShakeAxis,
    /// Oscillation period 1–999 frames (static).
    #[schemars(range(min = 1, max = 999), extend("x-unit" = "frame"))]
    pub wave_time: u32,
    /// Half-life 0.1–999 frames (static).
    #[schemars(range(min = 0.1, max = 999.0), extend("x-unit" = "frame"))]
    pub decay_half_life_frames: f64,
    /// Peak displacement 0–100 fields (animatable).
    #[schemars(range(min = 0.0, max = 100.0), extend("x-unit" = "field"))]
    pub amplitude: f64,
}

impl Default for ShakeParams {
    fn default() -> Self {
        Self { axis: ShakeAxis::Xy, wave_time: 4, decay_half_life_frames: 6.0, amplitude: 0.5 }
    }
}

/// Particle effects have no effect-level parameters; their content is in `blocks` (06 §3.8).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParticleParams {}

/// Animatable parameter ids per effect `elementType` (06 §3; the channel names an effect element may carry).
#[must_use]
pub fn animatable_params(element_type: &str) -> &'static [&'static str] {
    match element_type {
        "colourTransform" => {
            &["multRed", "multGreen", "multBlue", "multAlpha", "addRed", "addGreen", "addBlue", "addAlpha"]
        }
        "blur" => &["radius", "angle"],
        "shake" => &["amplitude"],
        _ => &[],
    }
}

/// Particle types (06 §3.8.1). Persisted as `particleType` because `type` is the envelope field (SI-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParticleType {
    /// Nature: rain.
    Rain,
    /// Nature: snow.
    Snow,
    /// Nature: hail.
    Hail,
    /// Nature: fog.
    Fog,
    /// Nature: cloud.
    Cloud,
    /// Nature: eruption.
    Eruption,
    /// Action: shatter.
    Shatter,
    /// Action: magic.
    Magic,
    /// Action: explosion.
    Explosion,
    /// Action: splash.
    Splash,
    /// Action: fire.
    Fire,
    /// Action: celestial.
    Celestial,
}

impl ParticleType {
    /// Whether the type belongs to the natural-phenomena effect (`particleNature`); otherwise action/impact.
    #[must_use]
    pub const fn is_nature(self) -> bool {
        matches!(self, Self::Rain | Self::Snow | Self::Hail | Self::Fog | Self::Cloud | Self::Eruption)
    }
}

/// Background of a particle block (06 §3.8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParticleBackground {
    /// None (default).
    #[default]
    Transparent,
    /// Dark blue sky gradient.
    DarkSkyGradient,
    /// Light blue sky gradient.
    LightSkyGradient,
}

/// Static parameters of a particle block (06 §3.8.3). Frames are absolute scene frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BlockStatic {
    /// Background drawn behind the particles.
    pub background: ParticleBackground,
    /// First simulated frame (≤ firstFrame, ≥ firstFrame − 999; may be < 1 for pre-roll).
    #[schemars(extend("x-unit" = "frame"))]
    pub simulation_start: i32,
    /// First displayed frame.
    #[schemars(range(min = 1), extend("x-unit" = "frame"))]
    pub first_frame: u32,
    /// Frames the block lasts, ≥ 1.
    #[schemars(range(min = 1), extend("x-unit" = "frame"))]
    pub duration: u32,
    /// Particle cap (≤ 3000; 50 for fog and cloud).
    #[schemars(range(min = 0, max = 3000))]
    pub max_particles: u32,
    /// Lifetime in frames: −1 (unlimited) or 0–512.
    #[schemars(range(min = -1, max = 512), extend("x-unit" = "frame"))]
    pub lifetime_frames: i32,
    /// Fade particles out.
    pub fade: bool,
    /// Region width 0–1999 particle units.
    #[schemars(range(min = 0.0, max = 1999.0))]
    pub area_width: f64,
    /// Region height 0–1999.
    #[schemars(range(min = 0.0, max = 1999.0))]
    pub area_height: f64,
    /// Region depth 0–1999.
    #[schemars(range(min = 0.0, max = 1999.0))]
    pub area_depth: f64,
    /// Floor height 0–1999.
    #[schemars(range(min = 0.0, max = 1999.0))]
    pub floor_level: f64,
}

/// A `{frame, value}` keyframe of a dynamic block parameter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameValue {
    /// Absolute scene frame.
    #[schemars(range(min = 1), extend("x-unit" = "frame"))]
    pub frame: u32,
    /// Value.
    pub value: f64,
}

/// A dynamic block parameter: a constant `{value}` or `{keyframes}` (06 §8.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DynamicParam {
    /// Constant value.
    Value {
        /// The value.
        value: f64,
    },
    /// Keyframed value.
    Keyframes {
        /// Keyframes sorted by frame, at least one.
        #[schemars(length(min = 1))]
        keyframes: Vec<FrameValue>,
    },
}

/// The dynamic parameter ids of a particle block with their ranges (06 §3.8.4).
pub const DYNAMIC_PARAMS: &[(&str, f64, f64)] = &[
    ("intensity", 0.0, 999.0),
    ("scale", 0.0, 199.0),
    ("bounciness", 0.0, 99.0),
    ("speedMin", 0.0, 199.0),
    ("speedMax", 0.0, 199.0),
    ("directionEW", -1000.0, 1000.0),
    ("directionNS", -1000.0, 1000.0),
    ("directionFB", -1000.0, 1000.0),
];

model_struct! {
    /// A simulation block of a particle effect (10 §4.6, 06 §3.8.2, §8.4).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "particleBlock"))]
    pub struct ParticleBlock {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `particleBlock`.
        #[serde(rename = "type")]
        ty: ParticleBlockTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Particle type (06 Block `type`; SI-004).
        particle_type: ParticleType,
        /// Sub-behaviour; `null` = type default.
        variant: Option<String>,
        /// Last applied preset (informational).
        preset_id: Option<String>,
        /// Seed of all random choices, 0–2³²−1 (06 §3.8.3, §4.6: a full unsigned 32-bit range, stated explicitly).
        #[schemars(extend("maximum" = 4_294_967_295_u32))]
        seed: u32,
        /// Built-in sprite set.
        sprite_set_id: Option<String>,
        /// Static parameters.
        #[serde(rename = "static")]
        statics: BlockStatic,
        /// Dynamic parameters by id (exactly the ids of 06 §3.8.4).
        dynamic: BTreeMap<String, DynamicParam>,
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

impl_object!(ParticleBlock, ParticleBlock);
