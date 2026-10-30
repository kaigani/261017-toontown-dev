//! Object types of the document model (10 §2.1).

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Every addressable object type (10 §2.1). The string form is the lower-camelCase name used in paths, schemas,
/// `doc.object.find` filters, command results and change events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectType {
    /// The project (root).
    Project,
    /// A palette.
    Palette,
    /// A palette style.
    PaletteStyle,
    /// A swatch (identified by its colour ID).
    Swatch,
    /// A pen preset.
    Pen,
    /// A media asset.
    Asset,
    /// A drawing bank (one per clone group).
    DrawingBank,
    /// A drawing.
    Drawing,
    /// A node of a drawing (layer, group, text, bitmap).
    DrawingNode,
    /// A stroke of a layer.
    Stroke,
    /// A scene.
    Scene,
    /// A storyline transition (edit point).
    Transition,
    /// An element of a scene.
    Element,
    /// An animation channel of an element.
    Channel,
    /// A key of a 1-D channel.
    Key,
    /// A point of a motion-path channel (10 §4.12; SI-006).
    MotionPoint,
    /// A block of a particle effect.
    ParticleBlock,
    /// A placement of a sound element.
    SoundPlacement,
    /// A sequence of a media element.
    MediaSequence,
    /// A library (project-local in the document).
    Library,
    /// A library folder.
    LibraryFolder,
    /// A library item.
    LibraryItem,
}

impl ObjectType {
    /// All object types in a fixed order.
    pub const ALL: [ObjectType; 22] = [
        Self::Project,
        Self::Palette,
        Self::PaletteStyle,
        Self::Swatch,
        Self::Pen,
        Self::Asset,
        Self::DrawingBank,
        Self::Drawing,
        Self::DrawingNode,
        Self::Stroke,
        Self::Scene,
        Self::Transition,
        Self::Element,
        Self::Channel,
        Self::Key,
        Self::MotionPoint,
        Self::ParticleBlock,
        Self::SoundPlacement,
        Self::MediaSequence,
        Self::Library,
        Self::LibraryFolder,
        Self::LibraryItem,
    ];

    /// The type name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Palette => "palette",
            Self::PaletteStyle => "paletteStyle",
            Self::Swatch => "swatch",
            Self::Pen => "pen",
            Self::Asset => "asset",
            Self::DrawingBank => "drawingBank",
            Self::Drawing => "drawing",
            Self::DrawingNode => "drawingNode",
            Self::Stroke => "stroke",
            Self::Scene => "scene",
            Self::Transition => "transition",
            Self::Element => "element",
            Self::Channel => "channel",
            Self::Key => "key",
            Self::MotionPoint => "motionPoint",
            Self::ParticleBlock => "particleBlock",
            Self::SoundPlacement => "soundPlacement",
            Self::MediaSequence => "mediaSequence",
            Self::Library => "library",
            Self::LibraryFolder => "libraryFolder",
            Self::LibraryItem => "libraryItem",
        }
    }

    /// Parses a type name or one of its path aliases (10 §2.1: `style`, `layer`, `keyframe`, `placement`, `folder`,
    /// `template`, `item`).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        if let Some(t) = Self::ALL.iter().copied().find(|t| t.as_str() == name) {
            return Some(t);
        }
        Some(match name {
            "style" => Self::PaletteStyle,
            "layer" => Self::DrawingNode,
            "keyframe" => Self::Key,
            "placement" => Self::SoundPlacement,
            "folder" => Self::LibraryFolder,
            "template" | "item" => Self::LibraryItem,
            _ => return None,
        })
    }

    /// Path aliases of this type (10 §2.1).
    #[must_use]
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::PaletteStyle => &["style"],
            Self::DrawingNode => &["layer"],
            Self::Key => &["keyframe"],
            Self::SoundPlacement => &["placement"],
            Self::LibraryFolder => &["folder"],
            Self::LibraryItem => &["template", "item"],
            _ => &[],
        }
    }

    /// The prefix of generated IDs (10 ID-02, design decision D1; ADR-0010).
    #[must_use]
    pub const fn id_prefix(self) -> &'static str {
        match self {
            Self::Project => "pr",
            Self::Palette => "pal",
            Self::PaletteStyle => "sty",
            Self::Swatch => "col",
            Self::Pen => "pen",
            Self::Asset => "as",
            Self::DrawingBank => "db",
            Self::Drawing => "dr",
            Self::DrawingNode => "nd",
            Self::Stroke => "st",
            Self::Scene => "sc",
            Self::Transition => "tr",
            Self::Element => "el",
            Self::Channel => "ch",
            Self::Key => "ky",
            Self::MotionPoint => "mp",
            Self::ParticleBlock => "blk",
            Self::SoundPlacement => "pl",
            Self::MediaSequence => "ms",
            Self::Library => "lib",
            Self::LibraryFolder => "lf",
            Self::LibraryItem => "li",
        }
    }

    /// Possible parent types (10 §2.1). `drawing`'s parent is its bank, addressed through any element that uses it.
    #[must_use]
    pub const fn parents(self) -> &'static [ObjectType] {
        match self {
            Self::Project => &[],
            Self::Palette | Self::Pen | Self::Asset | Self::DrawingBank | Self::Scene | Self::Library => {
                &[Self::Project]
            }
            Self::PaletteStyle | Self::Swatch => &[Self::Palette],
            Self::Drawing => &[Self::DrawingBank],
            Self::DrawingNode => &[Self::Drawing, Self::DrawingNode],
            Self::Stroke => &[Self::DrawingNode],
            Self::Transition => &[Self::Project, Self::Scene],
            Self::Element => &[Self::Scene],
            Self::Channel | Self::ParticleBlock | Self::SoundPlacement | Self::MediaSequence => &[Self::Element],
            Self::Key | Self::MotionPoint => &[Self::Channel],
            Self::LibraryFolder | Self::LibraryItem => &[Self::Library, Self::LibraryFolder],
        }
    }

    /// Whether paths may select objects of this type by quoted name (10 §2.1).
    #[must_use]
    pub const fn name_addressable(self) -> bool {
        matches!(
            self,
            Self::Palette
                | Self::PaletteStyle
                | Self::Swatch
                | Self::Pen
                | Self::Asset
                | Self::Drawing
                | Self::DrawingNode
                | Self::Scene
                | Self::Element
                | Self::Channel
                | Self::Key
                | Self::Library
                | Self::LibraryFolder
                | Self::LibraryItem
        )
    }

    /// Whether objects of this type carry a `name` in their envelope (10 §2.2 "present for named types").
    #[must_use]
    pub const fn has_name(self) -> bool {
        matches!(
            self,
            Self::Project
                | Self::Palette
                | Self::PaletteStyle
                | Self::Swatch
                | Self::Pen
                | Self::Asset
                | Self::Drawing
                | Self::DrawingNode
                | Self::Scene
                | Self::Element
                | Self::Channel
                | Self::Library
                | Self::LibraryFolder
                | Self::LibraryItem
        )
    }

    /// Whether the package splits objects of this type into their own entry (10 §6.3 LAY-01).
    #[must_use]
    pub const fn is_entry(self) -> bool {
        matches!(
            self,
            Self::Project
                | Self::Palette
                | Self::Scene
                | Self::Element
                | Self::DrawingBank
                | Self::Drawing
                | Self::Library
        )
    }

    /// Whether objects of this type may carry container-local IDs (10 ID-03). Their project-wide form is
    /// `<containerId>/<localId>`.
    #[must_use]
    pub const fn local_id_container(self) -> Option<ObjectType> {
        match self {
            Self::DrawingNode | Self::Stroke => Some(Self::Drawing),
            Self::Key | Self::MotionPoint => Some(Self::Channel),
            _ => None,
        }
    }

    /// The name of the identifier field: `colourId` for swatches, `id` otherwise (10 §2.2).
    #[must_use]
    pub const fn id_field(self) -> &'static str {
        match self {
            Self::Swatch => "colourId",
            _ => "id",
        }
    }

    /// One-line description (used by `doc.schema.list`).
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Project => "The project: settings, palettes, pens, assets, drawing banks, scenes, local library.",
            Self::Palette => "An ordered set of swatches with one value per palette style (03).",
            Self::PaletteStyle => "An alternative set of values for all swatches of a palette (03).",
            Self::Swatch => "A colour, gradient or texture identified by its colour ID (03).",
            Self::Pen => "A brush or eraser size/shape preset (02).",
            Self::Asset => "A media file used by the project, embedded or linked (07, 09).",
            Self::DrawingBank => "The drawings of one clone group (04).",
            Self::Drawing => "A vector or bitmap drawing (02, 04).",
            Self::DrawingNode => "A layer, group, text or bitmap node of a drawing (02).",
            Self::Stroke => "A brush, pencil or invisible stroke of a layer (02).",
            Self::Scene => "A scene: elements, Timeline tree and Exposure Sheet order (04, 08).",
            Self::Transition => "A storyline transition at an edit point (08).",
            Self::Element => "An element of a scene (04).",
            Self::Channel => "An animation channel of an element (05).",
            Self::Key => "A key of a 1-D channel (05).",
            Self::MotionPoint => "A keyframe or control point of a motion path (05).",
            Self::ParticleBlock => "A simulation block of a particle effect (06).",
            Self::SoundPlacement => "A placement of a sound asset in a sound element (07).",
            Self::MediaSequence => "A run of frames of a media element showing one linked source (07).",
            Self::Library => "A library of templates and media items (08).",
            Self::LibraryFolder => "A folder of a library (08).",
            Self::LibraryItem => "A template, image, movie or sound item of a library (08).",
        }
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ObjectType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ObjectType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::ALL
            .iter()
            .copied()
            .find(|t| t.as_str() == s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown object type `{s}`")))
    }
}

impl JsonSchema for ObjectType {
    fn schema_name() -> Cow<'static, str> {
        "ObjectType".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let names: Vec<&str> = Self::ALL.iter().map(|t| t.as_str()).collect();
        json_schema!({ "type": "string", "enum": names })
    }
}

/// Declares a zero-sized marker that serializes as one constant type name: the envelope `type` field of one object
/// type. Deserializing any other string fails, so a palette JSON cannot be read as a scene.
macro_rules! type_tags {
    ($($tag:ident => $variant:ident),+ $(,)?) => {$(
        #[doc = concat!("The envelope `type` of a `", stringify!($variant), "` object (always the constant type name).")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $tag;

        impl $tag {
            /// The object type this tag stands for.
            pub const TYPE: ObjectType = ObjectType::$variant;
        }

        impl Serialize for $tag {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(ObjectType::$variant.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $tag {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                if s == ObjectType::$variant.as_str() {
                    Ok($tag)
                } else {
                    Err(serde::de::Error::custom(format!(
                        "expected type `{}`, found `{s}`",
                        ObjectType::$variant.as_str()
                    )))
                }
            }
        }

        impl JsonSchema for $tag {
            fn inline_schema() -> bool {
                true
            }
            fn schema_name() -> Cow<'static, str> {
                concat!(stringify!($variant), "Type").into()
            }
            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({ "type": "string", "const": ObjectType::$variant.as_str(), "readOnly": true })
            }
        }
    )+};
}

type_tags! {
    ProjectTag => Project,
    PaletteTag => Palette,
    PaletteStyleTag => PaletteStyle,
    SwatchTag => Swatch,
    PenTag => Pen,
    AssetTag => Asset,
    DrawingBankTag => DrawingBank,
    DrawingTag => Drawing,
    DrawingNodeTag => DrawingNode,
    StrokeTag => Stroke,
    SceneTag => Scene,
    TransitionTag => Transition,
    ElementTag => Element,
    ChannelTag => Channel,
    KeyTag => Key,
    MotionPointTag => MotionPoint,
    ParticleBlockTag => ParticleBlock,
    SoundPlacementTag => SoundPlacement,
    MediaSequenceTag => MediaSequence,
    LibraryTag => Library,
    LibraryFolderTag => LibraryFolder,
    LibraryItemTag => LibraryItem,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_aliases_resolve() {
        for t in ObjectType::ALL {
            assert_eq!(ObjectType::parse(t.as_str()), Some(t));
            for a in t.aliases() {
                assert_eq!(ObjectType::parse(a), Some(t));
            }
        }
        assert_eq!(ObjectType::parse("nope"), None);
    }

    #[test]
    fn prefixes_are_unique_and_valid() {
        let mut seen = std::collections::BTreeSet::new();
        for t in ObjectType::ALL {
            assert!(seen.insert(t.id_prefix()), "{t}");
            assert!(t.id_prefix().chars().all(|c| c.is_ascii_lowercase()));
        }
    }

    #[test]
    fn tags_reject_other_types() {
        assert!(serde_json::from_str::<SceneTag>("\"scene\"").is_ok());
        assert!(serde_json::from_str::<SceneTag>("\"element\"").is_err());
    }
}
