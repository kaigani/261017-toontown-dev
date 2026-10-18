//! Libraries, folders and items (10 §4.17, 08 §1–§6).

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::{LibraryFolderTag, LibraryItemTag, LibraryTag, ObjectType};
use crate::value::{Blob, Timestamp};

/// Kind of a library (10 §4.17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LibraryKind {
    /// The project-local library.
    Local,
    /// The user's global library.
    Global,
    /// An opened library folder.
    Opened,
    /// The starter library.
    Starter,
}

/// Kind of a library item (10 §4.17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// A template (`.animtpl`).
    Template,
    /// An image file.
    Image,
    /// A movie file.
    Movie,
    /// A sound file.
    Sound,
}

/// A node of a template's element-tree summary (08 §6.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TemplateSummaryNode {
    /// Element type.
    pub element_type: String,
    /// Element name.
    pub name: String,
    /// Frame count.
    pub frame_count: u32,
    /// Children.
    pub children: Vec<TemplateSummaryNode>,
}

model_struct! {
    /// A library item (10 §4.17). Template metadata (08 §6.3) is present for templates only.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "libraryItem"))]
    pub struct LibraryItem {
        /// Immutable, globally unique ID (08 LIB-002).
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `libraryItem`.
        #[serde(rename = "type")]
        ty: LibraryItemTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Display name, unique among siblings (case-insensitive).
        #[schemars(extend("x-command" = "library.item.rename"))]
        name: String,
        /// Template, image, movie or sound.
        #[schemars(extend("readOnly" = true))]
        item_kind: ItemKind,
        /// Stored file name relative to the library's `files/` folder.
        #[schemars(extend("readOnly" = true))]
        file: String,
        /// Thumbnail, if any.
        thumbnail: Option<Blob>,
        /// Templates: version ID, changed whenever the content changes (08 LIB-007).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true, "readOnly" = true))]
        version_id: Option<String>,
        /// Templates: artist (08 LIB-010).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        artist: Option<String>,
        /// Templates: copyright.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        copyright: Option<String>,
        /// Templates: creation time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true, "readOnly" = true))]
        created_at: Option<Timestamp>,
        /// Templates: modification time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true, "readOnly" = true))]
        modified_at: Option<Timestamp>,
        /// Templates: size in bytes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true, "readOnly" = true))]
        size_bytes: Option<u64>,
        /// Templates: template format version.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true, "readOnly" = true))]
        format_version: Option<String>,
        /// Templates: source path or link (optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true))]
        source: Option<String>,
        /// Templates: element-tree summary.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-kindSpecific" = true, "readOnly" = true))]
        summary: Option<Vec<TemplateSummaryNode>>,
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

impl_object!(LibraryItem, LibraryItem, name);

model_struct! {
    /// A library folder (10 §4.17).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "libraryFolder"))]
    pub struct LibraryFolder {
        /// Immutable, globally unique ID (08 LIB-002).
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `libraryFolder`.
        #[serde(rename = "type")]
        ty: LibraryFolderTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Unique among siblings (case-insensitive).
        #[schemars(extend("x-command" = "library.folder.rename"))]
        name: String,
        /// Sub-folders and items.
        children: Vec<LibraryNode>,
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

impl_object!(LibraryFolder, LibraryFolder, name);

/// A child of a library or folder: a folder or an item, told apart by `type`.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // Items are the common case; boxing them would not save memory.
pub enum LibraryNode {
    /// A folder.
    Folder(LibraryFolder),
    /// An item.
    Item(LibraryItem),
}

impl Serialize for LibraryNode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Folder(f) => f.serialize(s),
            Self::Item(i) => i.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for LibraryNode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        match v.get("type").and_then(Value::as_str) {
            Some(t) if t == ObjectType::LibraryFolder.as_str() => {
                serde_json::from_value(v).map(Self::Folder).map_err(serde::de::Error::custom)
            }
            Some(t) if t == ObjectType::LibraryItem.as_str() => {
                serde_json::from_value(v).map(Self::Item).map_err(serde::de::Error::custom)
            }
            _ => Err(serde::de::Error::custom("a library child must have type `libraryFolder` or `libraryItem`")),
        }
    }
}

impl JsonSchema for LibraryNode {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "LibraryNode".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let folder = generator.subschema_for::<LibraryFolder>();
        let item = generator.subschema_for::<LibraryItem>();
        schemars::json_schema!({ "oneOf": [folder, item] })
    }
}

model_struct! {
    /// A library (10 §4.17). In the document this is the project-local library (`kind: "local"`), stored under
    /// `library/library.json` in the package layout of 10 §6.11.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "library"))]
    pub struct Library {
        /// Immutable, globally unique ID (08 LIB-002).
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `library`.
        #[serde(rename = "type")]
        ty: LibraryTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Library name.
        name: String,
        /// Local, global, opened or starter.
        #[schemars(extend("readOnly" = true))]
        kind: LibraryKind,
        /// Library format version (`MAJOR.MINOR`).
        #[schemars(regex(pattern = "^[0-9]+\\.[0-9]+$"), extend("readOnly" = true))]
        format_version: String,
        /// Top-level folders and items.
        children: Vec<LibraryNode>,
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

impl_object!(Library, Library, name);

impl Library {
    /// An empty project-local library.
    #[must_use]
    pub fn local(id: Id, name: String) -> Self {
        Self {
            id,
            ty: LibraryTag,
            rev: 0,
            name,
            kind: LibraryKind::Local,
            format_version: crate::FORMAT_VERSION.to_owned(),
            children: Vec::new(),
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}
