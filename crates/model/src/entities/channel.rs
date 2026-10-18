//! Channels, keys and motion points (10 §4.12, 05 §1.6, §9, §10).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::{ChannelTag, KeyTag, MotionPointTag};
use crate::value::Point3;

/// The channel id of the motion path.
pub const POSITION: &str = "position";

/// Channel ids of transformable nodes (05 §1.6). `position.x/y/z` are projections and are not stored.
pub const TRANSFORM_CHANNELS: &[&str] =
    &["position", "velocity", "pivot.x", "pivot.y", "rotation", "scale.x", "scale.y", "skew"];

/// Curve type of a 1-D channel (05 §10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CurveType {
    /// Cubic Bézier with free handles.
    Spline,
    /// Bézier with an easing preset per segment.
    Interpolation,
    /// Progress follows the velocity channel.
    VelocityBased,
}

/// Easing presets (05 §10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Easing {
    /// Hold, then step.
    Constant,
    /// Straight line.
    Linear,
    /// Slow start, slow end.
    EaseInOut,
    /// Fast start, fast end.
    FastInOut,
    /// Strong slow start and end.
    ExtremeEaseInOut,
    /// Strong fast start and end.
    ExtremeFastInOut,
    /// Ease in.
    SlowAcceleration,
    /// Ease out.
    SlowDeceleration,
}

/// A handle offset `{dt, dv}` (05 §10.3).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HandleOffset {
    /// Frame offset.
    #[schemars(extend("x-unit" = "frame"))]
    pub dt: f64,
    /// Value offset.
    pub dv: f64,
}

/// Handle mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HandleMode {
    /// Collinear handles (default).
    #[default]
    Smooth,
    /// Independent handles.
    Broken,
}

/// Handles of a key.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Handles {
    /// Left handle.
    pub left: HandleOffset,
    /// Right handle.
    pub right: HandleOffset,
    /// Smooth or broken.
    pub mode: HandleMode,
}

model_struct! {
    /// A key of a 1-D channel (10 §4.12). Addressed as `key:"<frame>"` or `key:@<id>`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "key"))]
    pub struct Key {
        /// ID, unique within the channel (ID-03).
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `key`.
        #[serde(rename = "type")]
        ty: KeyTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Integer frame, unique in the channel.
        #[schemars(range(min = 1), extend("x-unit" = "frame"))]
        frame: u32,
        /// Value.
        value: f64,
        /// The segment starting here holds the value.
        constant_segment: bool,
        /// Spline handles.
        handles: Handles,
        /// Easing preset of the segment starting here (interpolation curves).
        easing: Option<Easing>,
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

impl_object!(Key, Key);

impl Key {
    /// A key with default handles.
    #[must_use]
    pub fn new(id: Id, frame: u32, value: f64) -> Self {
        Self {
            id,
            ty: KeyTag,
            rev: 0,
            frame,
            value,
            constant_segment: false,
            handles: Handles::default(),
            easing: None,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}

model_struct! {
    /// A point of a motion path (10 §4.12): a keyframe (`frame` set) or a control point (`frame: null`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "motionPoint"))]
    pub struct MotionPoint {
        /// ID, unique within the channel (ID-03).
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `motionPoint`.
        #[serde(rename = "type")]
        ty: MotionPointTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Keyframe frame (integer, unique); `null` = control point.
        #[schemars(range(min = 1), extend("x-unit" = "frame"))]
        frame: Option<u32>,
        /// Position in parent space (fields).
        #[schemars(extend("x-unit" = "field"))]
        position: Point3,
        /// TCB tension, −1…1.
        #[schemars(range(min = -1.0, max = 1.0))]
        tension: f64,
        /// TCB continuity, −1…1.
        #[schemars(range(min = -1.0, max = 1.0))]
        continuity: f64,
        /// TCB bias, −1…1.
        #[schemars(range(min = -1.0, max = 1.0))]
        bias: f64,
        /// The segment starting at this keyframe holds the position.
        constant_segment: bool,
        /// Constant Z for the segment starting here (05 §9.3).
        lock_depth: bool,
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

impl_object!(MotionPoint, MotionPoint);

model_struct! {
    /// An animation channel (10 §4.12). The `name` is the channel id (`rotation`, `scale.x`, …, or an effect
    /// parameter id) and equals its key in the element's `channels` map. A 1-D channel has `curveType` and `keys`;
    /// the motion-path channel `position` has `points` instead.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "channel"))]
    pub struct Channel {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `channel`.
        #[serde(rename = "type")]
        ty: ChannelTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// The channel id.
        #[schemars(extend("readOnly" = true))]
        name: String,
        /// 1-D channels: curve type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        curve_type: Option<CurveType>,
        /// 1-D channels: keys sorted by frame, at least one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        keys: Option<Vec<Key>>,
        /// Motion path: points in path order; first and last are keyframes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        points: Option<Vec<MotionPoint>>,
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

impl_object!(Channel, Channel, name);

impl Channel {
    /// A 1-D channel.
    #[must_use]
    pub fn one_d(id: Id, name: String, curve_type: CurveType, keys: Vec<Key>) -> Self {
        Self {
            id,
            ty: ChannelTag,
            rev: 0,
            name,
            curve_type: Some(curve_type),
            keys: Some(keys),
            points: None,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}

/// Clamps spline handle frame offsets as 05 §10.3 requires (10 §4.12): the right handle of key `k` to
/// `0 ≤ dt ≤ t(k+1) − t(k)`, the left handle of key `k` to `−(t(k) − t(k−1)) ≤ dt ≤ 0`; the outer handles of the first
/// and last key keep only the sign rule. Keys out of frame order are left alone (validation rejects them).
pub(crate) fn clamp_handles(keys: &mut [Key]) {
    if !keys.windows(2).all(|w| w[0].frame < w[1].frame) {
        return;
    }
    let frames: Vec<f64> = keys.iter().map(|k| f64::from(k.frame)).collect();
    for (i, k) in keys.iter_mut().enumerate() {
        let t = f64::from(k.frame);
        let next = frames.get(i + 1).map_or(f64::INFINITY, |n| n - t);
        let prev = i.checked_sub(1).and_then(|j| frames.get(j)).map_or(f64::INFINITY, |p| t - p);
        k.handles.right.dt = k.handles.right.dt.clamp(0.0, next);
        k.handles.left.dt = k.handles.left.dt.clamp(-prev, 0.0);
    }
}
