//! Sound placements, mouth tracks, lip-sync mappings and media sequences (10 §4.14–§4.15, 07 §1).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::drawing::MouthShape;
use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::{MediaSequenceTag, SoundPlacementTag};
use crate::value::FrameRange;

/// A gain point of a placement envelope (07 §9.5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvelopePoint {
    /// Time within the source (seconds).
    #[schemars(range(min = 0.0), extend("x-unit" = "second"))]
    pub time: f64,
    /// Left gain 0–1.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub left: f64,
    /// Right gain 0–1.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub right: f64,
}

/// How a mouth track was produced (10 §4.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MouthTrackSource {
    /// Detected from audio.
    Detected,
    /// Aligned to a transcript.
    Transcript,
    /// From explicit phonemes.
    Phonemes,
    /// Set explicitly.
    Explicit,
}

/// The timing a mouth track was produced for (10 §4.14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProducedFor {
    /// Asset.
    #[schemars(extend("x-ref" = "asset"))]
    pub asset_id: Id,
    /// In-point (seconds).
    #[schemars(extend("x-unit" = "second"))]
    pub in_point: f64,
    /// Out-point (seconds).
    #[schemars(extend("x-unit" = "second"))]
    pub out_point: f64,
    /// Loop count.
    pub loops: u32,
    /// Stop frame.
    #[schemars(extend("x-unit" = "frame"))]
    pub stop_frame: u32,
    /// Project frame rate.
    pub frame_rate: f64,
}

/// A placement's lip-sync track (10 §4.14, 07 §10.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MouthTrack {
    /// One letter (`A`–`G`, `X`) per frame of the placement.
    #[schemars(regex(pattern = "^[A-GX]*$"))]
    pub mouths: String,
    /// Placement-relative frame ranges set by hand or explicitly.
    pub manual: Vec<FrameRange>,
    /// Optional display labels, keyed by placement-relative frame.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(extend("x-omitDefault" = true))]
    pub labels: BTreeMap<u32, String>,
    /// How the track was produced.
    pub source: MouthTrackSource,
    /// Timing snapshot.
    pub produced_for: ProducedFor,
}

model_struct! {
    /// One use of a sound asset in a sound element (10 §4.14, 07 §1.2).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "soundPlacement"))]
    pub struct SoundPlacement {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `soundPlacement`.
        #[serde(rename = "type")]
        ty: SoundPlacementTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// A `sound` asset, or a `movie` asset with audio.
        #[schemars(extend("x-ref" = "asset"))]
        asset_id: Id,
        /// First frame.
        #[schemars(range(min = 1), extend("x-unit" = "frame"))]
        start_frame: u32,
        /// Last frame; `startFrame ≤ stopFrame ≤` natural end.
        #[schemars(range(min = 1), extend("x-unit" = "frame"))]
        stop_frame: u32,
        /// In-point in the source (seconds).
        #[schemars(range(min = 0.0), extend("x-unit" = "second"))]
        in_point: f64,
        /// Out-point in the source (seconds); `inPoint < outPoint`.
        #[schemars(range(min = 0.0), extend("x-unit" = "second"))]
        out_point: f64,
        /// Repetitions of the trimmed segment.
        #[schemars(range(min = 1))]
        loops: u32,
        /// Global gain −50…+50.
        #[schemars(range(min = -50, max = 50))]
        volume: i32,
        /// Gain points, times ascending.
        envelope: Vec<EnvelopePoint>,
        /// The placement's lip-sync track.
        mouth_track: Option<MouthTrack>,
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

impl_object!(SoundPlacement, SoundPlacement);

/// Which drawing represents each mouth shape (07 §10.8).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MouthMapping {
    /// Drawing for A.
    #[serde(rename = "A")]
    pub a: Option<Id>,
    /// Drawing for B.
    #[serde(rename = "B")]
    pub b: Option<Id>,
    /// Drawing for C.
    #[serde(rename = "C")]
    pub c: Option<Id>,
    /// Drawing for D.
    #[serde(rename = "D")]
    pub d: Option<Id>,
    /// Drawing for E.
    #[serde(rename = "E")]
    pub e: Option<Id>,
    /// Drawing for F.
    #[serde(rename = "F")]
    pub f: Option<Id>,
    /// Drawing for G.
    #[serde(rename = "G")]
    pub g: Option<Id>,
    /// Drawing for X.
    #[serde(rename = "X")]
    pub x: Option<Id>,
}

impl MouthMapping {
    /// The mapped drawing of a shape.
    #[must_use]
    pub fn get(&self, shape: MouthShape) -> Option<&Id> {
        match shape {
            MouthShape::A => self.a.as_ref(),
            MouthShape::B => self.b.as_ref(),
            MouthShape::C => self.c.as_ref(),
            MouthShape::D => self.d.as_ref(),
            MouthShape::E => self.e.as_ref(),
            MouthShape::F => self.f.as_ref(),
            MouthShape::G => self.g.as_ref(),
            MouthShape::X => self.x.as_ref(),
        }
    }
}

/// A lip-sync mapping for one drawing element (10 §4.14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LipSyncMapping {
    /// The mapped drawing element.
    #[schemars(extend("x-ref" = "element"))]
    pub drawing_element_id: Id,
    /// Shape → drawing.
    pub mapping: MouthMapping,
    /// Re-apply automatically when the track changes.
    pub auto_reapply: bool,
}

model_struct! {
    /// A run of frames of a media element showing one linked source (10 §4.15).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "mediaSequence"))]
    pub struct MediaSequence {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `mediaSequence`.
        #[serde(rename = "type")]
        ty: MediaSequenceTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// A `linked` asset.
        #[schemars(extend("x-ref" = "asset"))]
        asset_id: Id,
        /// First frame.
        #[schemars(range(min = 1), extend("x-unit" = "frame"))]
        start_frame: u32,
        /// Last frame.
        #[schemars(range(min = 1), extend("x-unit" = "frame"))]
        stop_frame: u32,
        /// Frame of the source shown at `startFrame`, ≥ 1.
        #[schemars(range(min = 1), extend("x-unit" = "frame"))]
        link_offset: u32,
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

impl_object!(MediaSequence, MediaSequence);
