//! Drawing banks, drawings and vector content (10 §4.8–§4.11, 02 §3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::palette::Swatch;
use super::pen::Tip;
use super::{ChildList, Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::{DrawingBankTag, DrawingNodeTag, DrawingTag, StrokeTag};
use crate::value::{Blob, Matrix2d, Point2, Timestamp};

/// Mouth shape letters (07 §10; 10 §4.8 `mouthShape`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub enum MouthShape {
    /// A.
    A,
    /// B.
    B,
    /// C.
    C,
    /// D.
    D,
    /// E.
    E,
    /// F.
    F,
    /// G.
    G,
    /// X (rest).
    X,
}

/// Kind of a stroke (02 §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StrokeKind {
    /// Filled outline.
    Brush,
    /// Centre line drawn with a line style.
    Pencil,
    /// Never rendered; separates and closes zones.
    Invisible,
}

/// Line type of a pencil line style (02 §3.2; other types MAY be added).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LineType {
    /// Solid line.
    Solid,
}

/// Line style of a pencil stroke.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LineStyle {
    /// Thickness in drawing units.
    #[schemars(range(min = 0.0), extend("x-unit" = "du"))]
    pub thickness: f64,
    /// Line type.
    pub line_type: LineType,
}

/// The pen recorded on a brush spine (02 §3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SpinePen {
    /// The pen used, if it still exists.
    #[schemars(extend("x-ref" = "pen"))]
    pub pen_id: Option<Id>,
    /// Tip shape.
    pub tip: Tip,
    /// Minimum size (du).
    #[schemars(range(min = 0.0, max = 100.0), extend("x-unit" = "du"))]
    pub min_size: f64,
    /// Maximum size (du).
    #[schemars(range(min = 0.0, max = 100.0), extend("x-unit" = "du"))]
    pub max_size: f64,
}

/// One anchor of a piecewise cubic Bézier: `[x, y, inX, inY, outX, outY]`, handles as offsets from the anchor.
pub type BezierPoint = [f64; 6];

/// A pressure sample `[t, p]`, both in 0–1.
pub type PressureSample = [f64; 2];

/// The spine of an unedited brush stroke (02 §3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Spine {
    /// The fitted centre path.
    #[schemars(extend("x-unit" = "du"))]
    pub points: Vec<BezierPoint>,
    /// Pressure map: `(t, p)` samples ordered by `t`.
    pub pressure: Vec<PressureSample>,
    /// Pen used.
    pub pen: SpinePen,
}

/// The fill of the zone on one side of a stroke (10 §4.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ZoneFill {
    /// Colour ID; `null` = unpainted.
    #[schemars(extend("x-ref" = "swatch"))]
    pub colour_id: Option<Id>,
    /// Gradient/texture placement; `null` for none.
    pub matrix: Option<Matrix2d>,
}

/// Fills on the left and right side of a stroke, relative to point order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sides {
    /// Left side.
    pub left: Option<ZoneFill>,
    /// Right side.
    pub right: Option<ZoneFill>,
}

model_struct! {
    /// A stroke of a layer (10 §4.9, 02 §3.2). `lineStyle` is non-null for pencil strokes only; `spine` only for
    /// unedited brush strokes.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "stroke"))]
    pub struct Stroke {
        /// ID, unique within the drawing (ID-03).
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `stroke`.
        #[serde(rename = "type")]
        ty: StrokeTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Brush, pencil or invisible.
        kind: StrokeKind,
        /// Whether the path is closed (brush outlines are).
        closed: bool,
        /// Anchors with handle offsets; at least 2 (1 for a closed dab).
        #[schemars(length(min = 1), extend("x-unit" = "du"))]
        points: Vec<BezierPoint>,
        /// Brush: fill colour; pencil: line colour; `null` for invisible strokes.
        #[schemars(extend("x-ref" = "swatch"))]
        colour_id: Option<Id>,
        /// Placement of a gradient/texture colour space for brush strokes.
        fill_matrix: Option<Matrix2d>,
        /// Pencil strokes only.
        line_style: Option<LineStyle>,
        /// Brush strokes while unedited.
        spine: Option<Spine>,
        /// Zone fills on each side.
        sides: Sides,
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

impl_object!(Stroke, Stroke);

/// Kind of a drawing node (02 §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    /// Planar stroke graph.
    Layer,
    /// Child nodes.
    Group,
    /// Rich text object.
    Text,
    /// Embedded raster image.
    Bitmap,
}

/// Type hint of a layer (02 §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LayerHint {
    /// Contour artwork.
    Brush,
    /// Centre-line artwork.
    Pencil,
}

/// Width mode of a text node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum WidthMode {
    /// No wrapping.
    Auto,
    /// Word wrap at `width`.
    Fixed,
}

/// Vertical script position of a text run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Script {
    /// Normal.
    Normal,
    /// Subscript.
    Subscript,
    /// Superscript.
    Superscript,
}

/// Paragraph alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Alignment {
    /// Left.
    Left,
    /// Centre.
    Centre,
    /// Right.
    Right,
    /// Justify.
    Justify,
}

/// Character formatting of a run of a text node (10 §4.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextRun {
    /// First code point.
    pub start: u32,
    /// Number of code points.
    pub length: u32,
    /// Font family name (kept even when the font is missing).
    pub font: String,
    /// Size in drawing units.
    #[schemars(range(min = 0.0), extend("x-unit" = "du"))]
    pub size: f64,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Script position.
    pub script: Script,
    /// Manual kerning.
    pub kerning: f64,
    /// Automatic kerning.
    pub auto_kerning: bool,
    /// Fill colour.
    #[schemars(extend("x-ref" = "swatch"))]
    pub colour_id: Option<Id>,
}

/// Paragraph formatting of a text node (10 §4.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Paragraph {
    /// First code point.
    pub start: u32,
    /// Number of code points.
    pub length: u32,
    /// Alignment.
    pub alignment: Alignment,
    /// Left margin (du).
    #[schemars(extend("x-unit" = "du"))]
    pub margin_left: f64,
    /// Right margin (du).
    #[schemars(extend("x-unit" = "du"))]
    pub margin_right: f64,
    /// First-line indent (du).
    #[schemars(extend("x-unit" = "du"))]
    pub indent: f64,
    /// Line spacing factor.
    pub line_spacing: f64,
}

model_struct! {
    /// A node of a drawing (10 §4.9): a layer (`hint`, `strokes`), a group (`children`), a text object (`transform`,
    /// `widthMode`, `width`, `text`, `runs`, `paragraphs`) or a bitmap (`image`, `width`, `height`, `transform`).
    /// Properties of other kinds are absent.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "drawingNode"))]
    pub struct DrawingNode {
        /// ID, unique within the drawing (ID-03).
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `drawingNode`.
        #[serde(rename = "type")]
        ty: DrawingNodeTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Optional name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-omitDefault" = true))]
        name: Option<String>,
        /// Layer, group, text or bitmap; read-only.
        #[schemars(extend("readOnly" = true))]
        kind: NodeKind,
        /// Layer: type hint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        hint: Option<LayerHint>,
        /// Layer: strokes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        strokes: Option<Vec<Stroke>>,
        /// Group: child nodes, back to front.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        children: Option<Vec<DrawingNode>>,
        /// Text and bitmap: placement in drawing space.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        transform: Option<Matrix2d>,
        /// Text: auto or fixed width.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        width_mode: Option<WidthMode>,
        /// Text: wrap width in du (fixed mode); bitmap: pixel width 1–16384.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        width: Option<f64>,
        /// Bitmap: pixel height 1–16384.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        height: Option<f64>,
        /// Text: the characters (≤ 100 000).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(length(max = 100_000), extend("x-kindSpecific" = true))]
        text: Option<String>,
        /// Text: character runs covering the text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        runs: Option<Vec<TextRun>>,
        /// Text: paragraphs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        paragraphs: Option<Vec<Paragraph>>,
        /// Bitmap: PNG pixels (8-bit RGBA, straight alpha).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        image: Option<Blob>,
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

impl_object!(DrawingNode, DrawingNode, optional_name);

impl DrawingNode {
    /// An empty layer.
    #[must_use]
    pub fn layer(id: Id, hint: LayerHint) -> Self {
        Self {
            id,
            ty: DrawingNodeTag,
            rev: 0,
            name: None,
            kind: NodeKind::Layer,
            hint: Some(hint),
            strokes: Some(Vec::new()),
            children: None,
            transform: None,
            width_mode: None,
            width: None,
            height: None,
            text: None,
            runs: None,
            paragraphs: None,
            image: None,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}

/// A style reference inside a colour-recovery entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecoveryStyle {
    /// Style ID.
    pub style_id: Id,
    /// Style name.
    pub style_name: String,
}

/// A recovery copy of one swatch used by a drawing (10 §4.11, 03 §11.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ColourRecoveryEntry {
    /// The colour ID.
    pub colour_id: Id,
    /// Owning palette ID.
    pub palette_id: Id,
    /// Owning palette name.
    pub palette_name: String,
    /// The palette's styles.
    pub styles: Vec<RecoveryStyle>,
    /// The full swatch (all style values).
    pub swatch: Swatch,
}

model_struct! {
    /// A drawing (10 §4.8). Stored in its own package entry `banks/<bankId>/<drawingId>.json`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "drawing"))]
    pub struct Drawing {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `drawing`.
        #[serde(rename = "type")]
        ty: DrawingTag,
        /// Revision of the last change (the drawing revision of 02 §3.12).
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Unique in the bank; default next unused integer.
        #[schemars(extend("x-command" = "element.drawing.rename"))]
        name: String,
        /// Locked drawings reject content edits with `drawing.locked`.
        #[schemars(extend("x-command" = "element.drawingLock.set"))]
        locked: bool,
        /// Drawing pivot (du).
        #[schemars(extend("x-unit" = "du"))]
        pivot: Point2,
        /// Mouth shape for automatic lip-sync mapping.
        mouth_shape: Option<MouthShape>,
        /// Nodes, back to front.
        nodes: Vec<DrawingNode>,
        /// One entry per colour ID the drawing uses (written on save).
        colour_recovery: Vec<ColourRecoveryEntry>,
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

impl_object!(Drawing, Drawing, name);

impl Drawing {
    /// An empty drawing.
    #[must_use]
    pub fn empty(id: Id, name: String, created_at: Timestamp) -> Self {
        Self {
            id,
            ty: DrawingTag,
            rev: 0,
            name,
            locked: false,
            pivot: Point2::default(),
            mouth_shape: None,
            nodes: Vec::new(),
            colour_recovery: Vec::new(),
            created_at,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}

model_struct! {
    /// The drawings of one clone group (10 §4.8). Stored in `banks/<bankId>/bank.json` with drawing stubs.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "drawingBank"))]
    pub struct DrawingBank {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `drawingBank`.
        #[serde(rename = "type")]
        ty: DrawingBankTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Drawings (unordered; written in natural name order).
        drawings: ChildList<Drawing>,
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

impl_object!(DrawingBank, DrawingBank);

impl DrawingBank {
    /// A bank holding the given drawings.
    #[must_use]
    pub fn new(id: Id, drawings: Vec<Id>) -> Self {
        Self {
            id,
            ty: DrawingBankTag,
            rev: 0,
            drawings: ChildList::new(drawings),
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}
