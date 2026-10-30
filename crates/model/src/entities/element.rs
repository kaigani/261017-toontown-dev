//! Elements (10 §4.5–§4.7, 04 §2, 05 §8, 06, 07).
//!
//! One struct carries the common fields and every type-specific block; type-specific blocks that do not apply to
//! the `elementType` are absent (10 §4.6). [`crate::validate`] enforces the presence matrix and the published schema
//! expresses it with one `if/then` rule per `elementType`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::channel::Channel;
use super::effect::ParticleBlock;
use super::sound::{LipSyncMapping, MediaSequence, SoundPlacement};
use super::{Ext, Meta, ext_is_empty, meta_is_empty, str_is_empty};
use crate::id::Id;
use crate::types::ElementTag;
use crate::value::{Colour, Point2, Point3, Timestamp};

/// Element types (10 §4.5). The 04 read model reports this value as `type`; the object model uses `elementType`
/// (design decision D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ElementType {
    /// Vector drawing element.
    Drawing,
    /// Annotation (note drawing) element.
    Annotation,
    /// Bitmap image element.
    Image,
    /// Sound element.
    Sound,
    /// Media element (linked sources).
    Media,
    /// Peg.
    Peg,
    /// Camera.
    Camera,
    /// Skeleton effect.
    Skeleton,
    /// Bone.
    Bone,
    /// Effect parameter element (mask, sprites, bones, skin slot).
    EffectParameter,
    /// Colour transform effect.
    ColourTransform,
    /// Clipping effect.
    Clipping,
    /// Drop shadow effect.
    DropShadow,
    /// Blur effect.
    Blur,
    /// Shake effect.
    Shake,
    /// Natural-phenomena particle effect.
    ParticleNature,
    /// Action/impact particle effect.
    ParticleImpact,
}

impl ElementType {
    /// All element types.
    pub const ALL: [ElementType; 17] = [
        Self::Drawing,
        Self::Annotation,
        Self::Image,
        Self::Sound,
        Self::Media,
        Self::Peg,
        Self::Camera,
        Self::Skeleton,
        Self::Bone,
        Self::EffectParameter,
        Self::ColourTransform,
        Self::Clipping,
        Self::DropShadow,
        Self::Blur,
        Self::Shake,
        Self::ParticleNature,
        Self::ParticleImpact,
    ];

    /// The string form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Drawing => "drawing",
            Self::Annotation => "annotation",
            Self::Image => "image",
            Self::Sound => "sound",
            Self::Media => "media",
            Self::Peg => "peg",
            Self::Camera => "camera",
            Self::Skeleton => "skeleton",
            Self::Bone => "bone",
            Self::EffectParameter => "effectParameter",
            Self::ColourTransform => "colourTransform",
            Self::Clipping => "clipping",
            Self::DropShadow => "dropShadow",
            Self::Blur => "blur",
            Self::Shake => "shake",
            Self::ParticleNature => "particleNature",
            Self::ParticleImpact => "particleImpact",
        }
    }

    /// Cell-based elements have an Exposure Sheet column and derived `start`/`duration` (10 §4.5–§4.6).
    #[must_use]
    pub const fn is_cell_based(self) -> bool {
        matches!(self, Self::Drawing | Self::Annotation | Self::Image | Self::Sound | Self::Media)
    }

    /// Elements whose timing is stored as exposure runs (10 §4.7).
    #[must_use]
    pub const fn has_exposure(self) -> bool {
        matches!(self, Self::Drawing | Self::Annotation | Self::Image)
    }

    /// Whether elements of this type may have children in the scene tree (10 §4.6).
    #[must_use]
    pub const fn can_have_children(self) -> bool {
        !matches!(self, Self::Sound | Self::Media)
    }

    /// Effect element types (06).
    #[must_use]
    pub const fn is_effect(self) -> bool {
        matches!(
            self,
            Self::ColourTransform
                | Self::Clipping
                | Self::DropShadow
                | Self::Blur
                | Self::Shake
                | Self::ParticleNature
                | Self::ParticleImpact
        )
    }

    /// The type-specific properties of this element type (10 §4.6). Every other type-specific property is absent.
    #[must_use]
    pub const fn specific_properties(self) -> &'static [&'static str] {
        match self {
            Self::Drawing | Self::Annotation => &["drawingBankId", "exposure", "cellNotes", "transform", "channels"],
            Self::Image => &["drawingBankId", "exposure", "cellNotes", "importOptions", "transform", "channels"],
            Self::Sound => &["streamed", "showMouthShapes", "placements", "lipSyncMappings"],
            Self::Media => &["sequences", "transform", "channels"],
            Self::Peg => &["transform", "exportGroup", "channels"],
            Self::Camera => &["camera", "transform", "channels"],
            Self::Skeleton => &["skeleton", "transform", "channels"],
            Self::Bone => &["bone", "channels"],
            Self::EffectParameter => &["slot"],
            Self::ColourTransform | Self::Clipping | Self::DropShadow | Self::Blur | Self::Shake => {
                &["params", "channels"]
            }
            Self::ParticleNature | Self::ParticleImpact => &["params", "blocks"],
        }
    }

    /// The channel ids an element of this type may carry (05 §1.6, 06 §3).
    #[must_use]
    pub fn allowed_channels(self) -> &'static [&'static str] {
        match self {
            Self::Drawing | Self::Annotation | Self::Image | Self::Media | Self::Peg | Self::Skeleton => {
                super::channel::TRANSFORM_CHANNELS
            }
            Self::Camera => &["position", "velocity", "rotation", "fov"],
            Self::Bone => &["rotation", "bone.x", "bone.y", "bone.length"],
            other => super::effect::animatable_params(other.as_str()),
        }
    }
}

/// All type-specific property names (10 §4.6).
pub const SPECIFIC_PROPERTIES: &[&str] = &[
    "drawingBankId",
    "exposure",
    "cellNotes",
    "importOptions",
    "streamed",
    "showMouthShapes",
    "placements",
    "lipSyncMappings",
    "sequences",
    "transform",
    "exportGroup",
    "camera",
    "skeleton",
    "bone",
    "slot",
    "params",
    "blocks",
    "channels",
];

/// Loop type (05 §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LoopType {
    /// The duration becomes `loops × L`.
    #[default]
    Count,
    /// The cycle length is kept; cycles follow the duration.
    Fill,
}

/// Position looping mode (05 §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PositionLoop {
    /// Every cycle restarts from the first position.
    #[default]
    Cycle,
    /// Cycles advance by the first cycle's displacement.
    Offset,
}

/// Composite role (04 §6.1, 05 §8.1; SI-003: the superset of 04/05 and 10 §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// Normal compositing.
    #[default]
    Normal,
    /// In front of all normal elements.
    Foreground,
    /// Behind all normal elements; auto-extends to the scene length.
    Background,
    /// Marks a bone-deformation skin.
    Envelope,
}

/// Pivot application of pegs and skeletons (05 §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ApplyPivotTo {
    /// Rotation only (default).
    #[default]
    Rotation,
    /// Scaling only.
    Scaling,
    /// Both.
    Both,
}

/// A 2-D scale `{x, y}`; `|s| ≥ 0.001`, negative values flip.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scale2 {
    /// Horizontal factor.
    #[schemars(extend("not" = {"exclusiveMinimum": -0.001, "exclusiveMaximum": 0.001}))]
    pub x: f64,
    /// Vertical factor.
    #[schemars(extend("not" = {"exclusiveMinimum": -0.001, "exclusiveMaximum": 0.001}))]
    pub y: f64,
}

impl Default for Scale2 {
    fn default() -> Self {
        Self { x: 1.0, y: 1.0 }
    }
}

/// The static transform block (10 §4.6, 05 §1.5, §8). Pivot fields depend on the element type: drawing, annotation,
/// image and media elements have `rotationPivot`, `scalePivot`, `rotateAboutPivot`; pegs have `pivot`,
/// `applyPivotTo`, `useChildDrawingPivot`; skeletons have `pivot`, `applyPivotTo`; cameras have none (ADR-0010).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Transform {
    /// Static position (fields). Camera default (0, 0, 16).
    #[schemars(extend("x-unit" = "field", "x-command" = "anim.transform.set"))]
    pub offset: Point3,
    /// Static rotation (degrees; camera: roll).
    #[schemars(extend("x-unit" = "degree", "x-command" = "anim.transform.set"))]
    pub rotation: f64,
    /// Static skew, −89…89 degrees.
    #[schemars(range(min = -89.0, max = 89.0), extend("x-unit" = "degree", "x-command" = "anim.transform.set"))]
    pub skew: f64,
    /// Static scale.
    #[schemars(extend("x-command" = "anim.transform.set"))]
    pub scale: Scale2,
    /// Keep the scale ratio when editing one value.
    pub keep_proportions: bool,
    /// Elements: rotation pivot (fields).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "field", "x-kindSpecific" = true))]
    pub rotation_pivot: Option<Point2>,
    /// Elements: scale pivot (fields).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "field", "x-kindSpecific" = true))]
    pub scale_pivot: Option<Point2>,
    /// Elements: rotate about the rotation pivot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-kindSpecific" = true))]
    pub rotate_about_pivot: Option<bool>,
    /// Pegs and skeletons: pivot (fields).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "field", "x-kindSpecific" = true))]
    pub pivot: Option<Point2>,
    /// Pegs and skeletons: what the pivot applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-kindSpecific" = true))]
    pub apply_pivot_to: Option<ApplyPivotTo>,
    /// Pegs: use the child drawing's pivot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-kindSpecific" = true))]
    pub use_child_drawing_pivot: Option<bool>,
    /// Constant Z for nodes with a motion path (05 §9.3).
    pub lock_depth: bool,
    /// Rotation follows the motion-path tangent (05 §8.3).
    pub orient_to_path: bool,
}

/// Which pivot fields a transform carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PivotSet {
    /// `rotationPivot`, `scalePivot`, `rotateAboutPivot`.
    Element,
    /// `pivot`, `applyPivotTo`, `useChildDrawingPivot`.
    Peg,
    /// `pivot`, `applyPivotTo`.
    Skeleton,
    /// None.
    Camera,
}

impl PivotSet {
    /// The pivot set of an element type, if it has a transform.
    #[must_use]
    pub const fn of(t: ElementType) -> Option<PivotSet> {
        match t {
            ElementType::Drawing | ElementType::Annotation | ElementType::Image | ElementType::Media => {
                Some(Self::Element)
            }
            ElementType::Peg => Some(Self::Peg),
            ElementType::Skeleton => Some(Self::Skeleton),
            ElementType::Camera => Some(Self::Camera),
            _ => None,
        }
    }

    /// The transform properties present for this set (in schema order).
    #[must_use]
    pub const fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Element => &["rotationPivot", "scalePivot", "rotateAboutPivot"],
            Self::Peg => &["pivot", "applyPivotTo", "useChildDrawingPivot"],
            Self::Skeleton => &["pivot", "applyPivotTo"],
            Self::Camera => &[],
        }
    }
}

impl Transform {
    /// The default transform of a pivot set (all zero/identity; camera offset (0, 0, 16)).
    #[must_use]
    pub fn default_for(set: PivotSet) -> Self {
        let zero = Some(Point2::default());
        let mut t = Transform {
            offset: Point3::default(),
            rotation: 0.0,
            skew: 0.0,
            scale: Scale2::default(),
            keep_proportions: false,
            rotation_pivot: None,
            scale_pivot: None,
            rotate_about_pivot: None,
            pivot: None,
            apply_pivot_to: None,
            use_child_drawing_pivot: None,
            lock_depth: false,
            orient_to_path: false,
        };
        match set {
            PivotSet::Element => {
                t.rotation_pivot = zero;
                t.scale_pivot = zero;
                t.rotate_about_pivot = Some(false);
            }
            PivotSet::Peg => {
                t.pivot = zero;
                t.apply_pivot_to = Some(ApplyPivotTo::Rotation);
                t.use_child_drawing_pivot = Some(false);
            }
            PivotSet::Skeleton => {
                t.pivot = zero;
                t.apply_pivot_to = Some(ApplyPivotTo::Rotation);
            }
            PivotSet::Camera => t.offset = Point3 { x: 0.0, y: 0.0, z: 16.0 },
        }
        t
    }
}

/// Camera FOV mode (05 §12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FovMode {
    /// A single FOV value.
    #[default]
    Static,
    /// The `fov` channel.
    Dynamic,
}

/// Camera block (10 §4.6, 05 §12.3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CameraBlock {
    /// Static or dynamic FOV.
    #[schemars(extend("x-command" = "anim.camera.set"))]
    pub fov_mode: FovMode,
    /// FOV 1–179 degrees (default 41.1121).
    #[schemars(range(min = 1.0, max = 179.0), extend("x-unit" = "degree", "x-command" = "anim.camera.set"))]
    pub fov: f64,
}

impl Default for CameraBlock {
    fn default() -> Self {
        Self { fov_mode: FovMode::Static, fov: 41.1121 }
    }
}

/// Export-group block of pegs (05 §8.2, 09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExportGroup {
    /// Group the subtree on vector export.
    pub group_on_vector_export: bool,
    /// Add a composite for the export group.
    pub add_composite: bool,
}

/// Skeleton rendering quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum VectorQuality {
    /// Low.
    Low,
    /// Medium (default).
    #[default]
    Medium,
    /// High.
    High,
    /// Highest.
    Highest,
}

/// Skeleton block (05 §13.6).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkeletonBlock {
    /// Bone colour; `null` = preference.
    pub bone_colour: Option<Colour>,
    /// Show the track.
    pub show_track: bool,
    /// Show the root.
    pub show_root: bool,
    /// Show bones.
    pub show_bones: bool,
    /// Show influence.
    pub show_influence: bool,
    /// Deformation quality.
    pub vector_quality: VectorQuality,
    /// Deformation on.
    pub deformation_enabled: bool,
}

impl Default for SkeletonBlock {
    fn default() -> Self {
        Self {
            bone_colour: None,
            show_track: false,
            show_root: true,
            show_bones: true,
            show_influence: false,
            vector_quality: VectorQuality::Medium,
            deformation_enabled: true,
        }
    }
}

/// Rest pose of a bone (05 §13.1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoneRest {
    /// X (fields).
    #[schemars(extend("x-unit" = "field"))]
    pub x: f64,
    /// Y (fields).
    #[schemars(extend("x-unit" = "field"))]
    pub y: f64,
    /// Rotation (degrees).
    #[schemars(extend("x-unit" = "degree"))]
    pub rotation: f64,
    /// Length (fields), > 0.01.
    #[schemars(range(min = 0.01), extend("exclusiveMinimum" = 0.01, "x-unit" = "field"))]
    pub length: f64,
}

/// Influence type of a bone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InfluenceType {
    /// Unlimited (default).
    #[default]
    Infinite,
    /// Elliptical zone.
    Elliptical,
}

/// Zone of influence of a bone (05 §13.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BoneInfluence {
    /// Infinite or elliptical.
    #[serde(rename = "type")]
    pub influence_type: InfluenceType,
    /// Length radius (fields).
    #[schemars(range(min = 0.0), extend("x-unit" = "field"))]
    pub length_radius: f64,
    /// Width radius (fields).
    #[schemars(range(min = 0.0), extend("x-unit" = "field"))]
    pub width_radius: f64,
    /// Explicit region polygon, or `null`.
    pub region: Option<Vec<Point2>>,
}

/// Bone block (10 §4.6, 05 §13.1–§13.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoneBlock {
    /// Rest pose.
    pub rest: BoneRest,
    /// Zone of influence.
    pub influence: BoneInfluence,
}

/// Slot of an effect-parameter element (06 §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParamSlot {
    /// Clipping mask.
    Mask,
    /// Particle sprites.
    Sprites,
    /// Skeleton bones.
    Bones,
    /// Skeleton skin.
    Skin,
}

/// One exposure run `[drawingId | null, count]` (10 §4.7).
pub type ExposureRun = (Option<Id>, u32);

model_struct! {
    /// An element of a scene (10 §4.5–§4.7). Stored in `scenes/<sceneId>/elements/<elementId>.json`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "element"))]
    pub struct Element {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `element`.
        #[serde(rename = "type")]
        ty: ElementTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Unique in the scene.
        #[schemars(extend("x-command" = "element.rename"))]
        name: String,
        /// Element type; read-only except through conversion commands.
        #[schemars(extend("readOnly" = true))]
        element_type: ElementType,
        /// Non-cell elements: first frame of the bar (absent for cell-based elements: derived).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(range(min = 1), extend("x-unit" = "frame", "x-kindSpecific" = true))]
        start: Option<u32>,
        /// Non-cell elements: length of the bar (absent for cell-based elements: derived).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-unit" = "frame", "x-kindSpecific" = true))]
        duration: Option<u32>,
        /// Loop count ≥ 1; 1 = not looped.
        #[schemars(range(min = 1))]
        loops: u32,
        /// Count or fill.
        loop_type: LoopType,
        /// Cycle or offset.
        position_loop: PositionLoop,
        /// Composite role.
        role: Role,
        /// Deformable by a skeleton.
        deformable: bool,
        /// Shown in scene views, playback and export.
        #[schemars(extend("x-command" = "element.visibility.set"))]
        visible_in_scene: bool,
        /// Cell-based elements: shown in the Exposure Sheet and Drawing View.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-command" = "element.visibility.set", "x-kindSpecific" = true))]
        visible_in_xsheet: Option<bool>,
        /// Locked in scene views.
        #[schemars(extend("x-command" = "element.lock.set"))]
        locked: bool,
        /// Timeline display state of parents.
        collapsed: bool,
        /// Marker colour; `null` = category default.
        #[schemars(extend("x-command" = "element.colour.set"))]
        marker_colour: Option<Colour>,
        /// Background colour; `null` = category default.
        #[schemars(extend("x-command" = "element.colour.set"))]
        background_colour: Option<Colour>,
        /// Element note (≤ 10 000 characters).
        #[serde(default, skip_serializing_if = "str_is_empty")]
        #[schemars(length(max = 10_000), extend("x-omitDefault" = true, "x-command" = "element.note.set"))]
        note: String,
        /// Exposure Sheet column width, 20–1000 view pixels.
        #[schemars(range(min = 20, max = 1000), extend("x-unit" = "pixel"))]
        column_width: u32,
        /// Cell-based: the drawing bank (shared by clones).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-ref" = "drawingBank", "readOnly" = true, "x-kindSpecific" = true))]
        drawing_bank_id: Option<Id>,
        /// Cell-based: canonical exposure runs from frame 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        exposure: Option<Vec<ExposureRun>>,
        /// Cell-based: notes by frame.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        #[schemars(extend("x-omitDefault" = true))]
        cell_notes: BTreeMap<u32, String>,
        /// Image elements: import options (section 09; SI-008).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        import_options: Option<Map<String, Value>>,
        /// Sound: streamed playback.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        streamed: Option<bool>,
        /// Sound: the Exposure Sheet shows mouths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        show_mouth_shapes: Option<bool>,
        /// Sound: placements sorted by `startFrame`, non-overlapping.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        placements: Option<Vec<SoundPlacement>>,
        /// Sound: one mapping per mapped drawing element.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        lip_sync_mappings: Option<Vec<LipSyncMapping>>,
        /// Media: sequences, non-overlapping.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        sequences: Option<Vec<MediaSequence>>,
        /// Transformable elements: the static transform.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        transform: Option<Transform>,
        /// Pegs: export grouping.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        export_group: Option<ExportGroup>,
        /// Cameras: FOV settings.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        camera: Option<CameraBlock>,
        /// Skeletons: display and deformation settings.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        skeleton: Option<SkeletonBlock>,
        /// Bones: rest pose and influence.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        bone: Option<BoneBlock>,
        /// Effect parameter elements: the slot.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true, "readOnly" = true))]
        slot: Option<ParamSlot>,
        /// Effects: static parameter values (exact shape per `elementType`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        params: Option<Map<String, Value>>,
        /// Particle effects: blocks, consecutive.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        blocks: Option<Vec<ParticleBlock>>,
        /// Animated channels by channel id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        channels: Option<BTreeMap<String, Channel>>,
        /// Creation time.
        #[schemars(extend("readOnly" = true))]
        created_at: Timestamp,
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

impl_object!(Element, Element, name);

/// Inputs of [`Element::new`].
#[derive(Debug, Clone)]
pub struct ElementInit {
    /// The new element's ID.
    pub id: Id,
    /// Its type.
    pub element_type: ElementType,
    /// Its (already validated) name.
    pub name: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// Cell-based drawing/annotation/image elements: the bank.
    pub drawing_bank_id: Option<Id>,
    /// Non-cell elements: `(start, duration)` of the bar (default: frame 1, scene length at creation).
    pub bar: (u32, u32),
    /// Effect-parameter elements: the slot.
    pub slot: Option<ParamSlot>,
}

impl Element {
    /// A new element with the defaults of 10 §4.5–§4.6 for its type.
    #[must_use]
    pub fn new(init: ElementInit) -> Self {
        let t = init.element_type;
        let cell = t.is_cell_based();
        let has = |p: &str| t.specific_properties().contains(&p);
        let params = match t {
            ElementType::ColourTransform => to_map(&super::effect::ColourTransformParams::default()),
            ElementType::Clipping => to_map(&super::effect::ClippingParams::default()),
            ElementType::DropShadow => to_map(&super::effect::DropShadowParams::default()),
            ElementType::Blur => to_map(&super::effect::BlurParams::default()),
            ElementType::Shake => to_map(&super::effect::ShakeParams::default()),
            ElementType::ParticleNature | ElementType::ParticleImpact => Some(Map::new()),
            _ => None,
        };
        Self {
            id: init.id,
            ty: ElementTag,
            rev: 0,
            name: init.name,
            element_type: t,
            start: (!cell).then_some(init.bar.0),
            duration: (!cell).then_some(init.bar.1),
            loops: 1,
            loop_type: LoopType::Count,
            position_loop: PositionLoop::Cycle,
            role: Role::Normal,
            deformable: false,
            visible_in_scene: true,
            visible_in_xsheet: cell.then_some(true),
            locked: false,
            collapsed: false,
            marker_colour: None,
            background_colour: None,
            note: String::new(),
            column_width: 80,
            drawing_bank_id: if has("drawingBankId") { init.drawing_bank_id } else { None },
            exposure: has("exposure").then(Vec::new),
            cell_notes: BTreeMap::new(),
            import_options: has("importOptions").then(Map::new),
            streamed: has("streamed").then_some(false),
            show_mouth_shapes: has("showMouthShapes").then_some(false),
            placements: has("placements").then(Vec::new),
            lip_sync_mappings: has("lipSyncMappings").then(Vec::new),
            sequences: has("sequences").then(Vec::new),
            transform: PivotSet::of(t).map(Transform::default_for),
            export_group: has("exportGroup").then(ExportGroup::default),
            camera: has("camera").then(CameraBlock::default),
            skeleton: has("skeleton").then(SkeletonBlock::default),
            bone: has("bone").then_some(BoneBlock {
                rest: BoneRest { x: 0.0, y: 0.0, rotation: 0.0, length: 1.0 },
                influence: BoneInfluence {
                    influence_type: InfluenceType::Infinite,
                    length_radius: 0.75,
                    width_radius: 0.35,
                    region: None,
                },
            }),
            slot: if has("slot") { init.slot } else { None },
            params,
            blocks: has("blocks").then(Vec::new),
            channels: has("channels").then(BTreeMap::new),
            created_at: init.created_at,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}

fn to_map<T: Serialize>(v: &T) -> Option<Map<String, Value>> {
    match serde_json::to_value(v) {
        Ok(Value::Object(m)) => Some(m),
        _ => None,
    }
}
