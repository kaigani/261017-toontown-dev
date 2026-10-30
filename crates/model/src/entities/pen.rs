//! Pens (10 §4.2, 02 §6.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::PenTag;

/// Tool family of a pen (02 §6.1 "Type"). Persisted as `kind` because `type` is the envelope field (SI-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PenKind {
    /// Used by the Brush (and Pencil/shape thickness).
    Brush,
    /// Used by the Eraser.
    Eraser,
}

/// Tip shape (02 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Tip {
    /// Round nib (default).
    #[default]
    Round,
    /// Oval nib.
    Oval,
    /// Square nib.
    Square,
    /// Flat nib.
    Flat,
}

model_struct! {
    /// A pen preset (10 §4.2). Unique name in the project.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "pen"))]
    pub struct Pen {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `pen`.
        #[serde(rename = "type")]
        ty: PenTag,
        /// Revision of the last change (10 REV-02).
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Unique in the project; default `Pen <n>`.
        #[schemars(extend("x-command" = "tool.pen.rename"))]
        name: String,
        /// Brush or eraser pen (02 §6.1; SI-004).
        kind: PenKind,
        /// Width at lowest pressure, 0–100 du; `minSize ≤ maxSize`.
        #[schemars(range(min = 0.0, max = 100.0), extend("x-unit" = "du"))]
        min_size: f64,
        /// Width at full pressure, 0–100 du.
        #[schemars(range(min = 0.0, max = 100.0), extend("x-unit" = "du"))]
        max_size: f64,
        /// Curve-fitting tolerance and pressure smoothing, 0–10 (10 §4.2: int).
        #[schemars(range(min = 0, max = 10))]
        smoothness: u8,
        /// Tip shape.
        tip: Tip,
        /// Orientation of non-round tips, 0–360 degrees (02 §6.1; SI-004).
        #[schemars(range(min = 0.0, max = 360.0), extend("x-unit" = "degree"))]
        tip_angle: f64,
        /// Colour bound to the pen; `null` = use the current colour.
        #[schemars(extend("x-ref" = "swatch"))]
        colour_id: Option<Id>,
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

impl_object!(Pen, Pen, name);

impl Pen {
    /// A pen with the given settings and empty `meta`/`ext`.
    #[must_use]
    pub fn new(id: Id, name: String, kind: PenKind, min_size: f64, max_size: f64, smoothness: u8) -> Self {
        Self {
            id,
            ty: PenTag,
            rev: 0,
            name,
            kind,
            min_size,
            max_size,
            smoothness,
            tip: Tip::Round,
            tip_angle: 0.0,
            colour_id: None,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }

    /// The default pen set of a new project (02 §6.1 / §14.3): brush pens "Pen 1"…"Pen 5" with maximum sizes 2, 4,
    /// 8, 16, 32 du and minimum 25 % of maximum; eraser pens "Eraser 1"…"Eraser 3" with 10, 20, 40 du; round;
    /// smoothness 3. `ids` supplies one ID per pen, in that order.
    #[must_use]
    pub fn default_set(mut ids: impl FnMut() -> Id) -> Vec<Pen> {
        let mut pens = Vec::new();
        for (i, max) in [2.0, 4.0, 8.0, 16.0, 32.0].into_iter().enumerate() {
            pens.push(Pen::new(ids(), format!("Pen {}", i + 1), PenKind::Brush, max * 0.25, max, 3));
        }
        for (i, size) in [10.0, 20.0, 40.0].into_iter().enumerate() {
            pens.push(Pen::new(ids(), format!("Eraser {}", i + 1), PenKind::Eraser, size, size, 3));
        }
        pens
    }
}
