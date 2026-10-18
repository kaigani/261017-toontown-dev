//! Palettes, palette styles and swatches (10 §4.13, 03 §1.3–§1.4, §12.3).

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::{PaletteStyleTag, PaletteTag, SwatchTag};
use crate::value::Colour;

model_struct! {
    /// A palette style: an alternative set of values for all swatches of its palette (03 §1.5).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "paletteStyle"))]
    pub struct PaletteStyle {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `paletteStyle`.
        #[serde(rename = "type")]
        ty: PaletteStyleTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Unique in the palette.
        #[schemars(extend("x-command" = "colour.style.rename"))]
        name: String,
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

impl_object!(PaletteStyle, PaletteStyle, name);

impl PaletteStyle {
    /// A style with empty `meta`/`ext`.
    #[must_use]
    pub fn new(id: Id, name: String) -> Self {
        Self { id, ty: PaletteStyleTag, rev: 0, name, meta: Meta::new(), ext: Ext::default() }
    }
}

/// Kind of a swatch (03 §1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SwatchKind {
    /// One RGBA colour per style.
    Solid,
    /// Linear gradient stops per style.
    LinearGradient,
    /// Radial gradient stops per style.
    RadialGradient,
    /// Bitmap texture per style.
    Texture,
}

/// A gradient stop (03 §12.3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GradientStop {
    /// Position along the gradient, 0–1.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub position: f64,
    /// Colour at the stop.
    pub rgba: Colour,
}

/// The value of a swatch in one style (03 §12.3): `{rgba}`, `{stops}` or `{image, tile}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SwatchValue {
    /// A solid colour.
    Solid(SolidValue),
    /// A gradient (linear or radial; the swatch `kind` says which).
    Gradient(GradientValue),
    /// A texture.
    Texture(TextureValue),
}

/// `{rgba}`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SolidValue {
    /// The colour.
    pub rgba: Colour,
}

/// `{stops}` with at least two stops sorted by position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GradientValue {
    /// The stops.
    #[schemars(length(min = 2))]
    pub stops: Vec<GradientStop>,
}

/// `{image, tile}`: a texture image entry (under `textures/`) and the tile flag; `width`/`height` informative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextureValue {
    /// Package entry path of the image, e.g. `textures/fabric-day.png`.
    #[schemars(length(min = 1, max = 1024))]
    pub image: String,
    /// Repeat the image (true) or stretch it over the zone (false).
    pub tile: bool,
    /// Informative pixel width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1), extend("x-omitDefault" = true))]
    pub width: Option<u32>,
    /// Informative pixel height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1), extend("x-omitDefault" = true))]
    pub height: Option<u32>,
}

model_struct! {
    /// A swatch (03 §1.4, §12.3). Its identifier field is `colourId` (10 §2.2); `id` is accepted as an alias on input.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "swatch"))]
    pub struct Swatch {
        /// Immutable colour ID, unique across the project.
        #[serde(alias = "id")]
        #[schemars(extend("readOnly" = true))]
        colour_id: Id,
        /// Always `swatch`.
        #[serde(rename = "type")]
        ty: SwatchTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Label; names may repeat.
        #[schemars(extend("x-command" = "colour.swatch.set"))]
        name: String,
        /// Solid, gradient or texture; shared by all styles.
        #[schemars(extend("x-command" = "colour.swatch.set"))]
        kind: SwatchKind,
        /// Whether a gradient/texture fill follows bone deformation (03 §7.5).
        deformable: bool,
        /// Exactly one value per style of the palette, keyed by style ID, in style order.
        #[schemars(extend("x-command" = "colour.swatch.set"))]
        values: IndexMap<Id, SwatchValue>,
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

impl crate::entities::ModelObject for Swatch {
    const TYPE: crate::types::ObjectType = crate::types::ObjectType::Swatch;
    fn object_id(&self) -> &Id {
        &self.colour_id
    }
    fn object_rev(&self) -> u64 {
        self.rev
    }
    fn object_name(&self) -> Option<&str> {
        Some(&self.name)
    }
}

impl Swatch {
    /// A solid swatch with the same colour in every given style.
    #[must_use]
    pub fn solid(colour_id: Id, name: String, styles: &[Id], rgba: Colour) -> Self {
        let values = styles.iter().map(|s| (s.clone(), SwatchValue::Solid(SolidValue { rgba }))).collect();
        Self {
            colour_id,
            ty: SwatchTag,
            rev: 0,
            name,
            kind: SwatchKind::Solid,
            deformable: false,
            values,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }

    /// Whether `value` has the shape required by `kind` (03 §12.3).
    #[must_use]
    pub fn value_matches_kind(kind: SwatchKind, value: &SwatchValue) -> bool {
        matches!(
            (kind, value),
            (SwatchKind::Solid, SwatchValue::Solid(_))
                | (SwatchKind::LinearGradient | SwatchKind::RadialGradient, SwatchValue::Gradient(_))
                | (SwatchKind::Texture, SwatchValue::Texture(_))
        )
    }
}

model_struct! {
    /// A palette (10 §4.13, 03 §1.3 and §12.3).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "palette"))]
    pub struct Palette {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `palette`.
        #[serde(rename = "type")]
        ty: PaletteTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Unique in the project.
        #[schemars(extend("x-command" = "colour.palette.rename"))]
        name: String,
        /// Styles in order; at least one.
        #[schemars(length(min = 1))]
        styles: Vec<PaletteStyle>,
        /// Swatches in user order; may be empty.
        swatches: Vec<Swatch>,
        /// Internal palette never shown to the user (03 §1.2; SI-005).
        #[schemars(extend("readOnly" = true))]
        hidden: bool,
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

impl_object!(Palette, Palette, name);

impl Palette {
    /// A palette from its parts.
    #[must_use]
    pub fn new(id: Id, name: String, styles: Vec<PaletteStyle>, swatches: Vec<Swatch>) -> Self {
        Self {
            id,
            ty: PaletteTag,
            rev: 0,
            name,
            styles,
            swatches,
            hidden: false,
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}
