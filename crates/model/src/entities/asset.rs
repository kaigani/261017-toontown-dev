//! Assets (10 §4.16, MED-01…MED-06).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::AssetTag;
use crate::value::Timestamp;

/// Media kind of an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    /// Audio.
    Sound,
    /// Bitmap image.
    Image,
    /// Movie.
    Movie,
    /// Vector file linked by media elements.
    Vector,
}

/// Where the bytes live (10 §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AssetStorage {
    /// Copied into the package under `media/`.
    Embedded,
    /// Referenced in place (file or library item).
    Linked,
}

/// Kind of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LinkKind {
    /// An external file.
    File,
    /// A library item (08 LIB-002).
    LibraryItem,
}

/// The link of a `linked` asset (10 §4.16). Fields that do not apply to the link kind are `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssetLink {
    /// File or library item.
    pub kind: LinkKind,
    /// Files: path relative to the folder containing the project.
    pub path: Option<String>,
    /// Files: absolute path at link time.
    pub absolute_path: Option<String>,
    /// Library items: library ID.
    pub library_id: Option<Id>,
    /// Library items: item ID.
    pub item_id: Option<Id>,
    /// Library items: template version seen at link time (08 LIB-007).
    pub version_id: Option<String>,
}

model_struct! {
    /// The project-wide registry entry of one media file (10 §4.16).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "asset"))]
    pub struct Asset {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `asset`.
        #[serde(rename = "type")]
        ty: AssetTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Informational name (file name by default).
        name: String,
        /// Media kind.
        kind: AssetKind,
        /// IANA media type, e.g. `audio/wav`.
        #[schemars(length(min = 1, max = 255))]
        media_type: String,
        /// Embedded or linked.
        storage: AssetStorage,
        /// Package entry path when embedded (`media/<assetId>.<ext>`); `null` when linked.
        entry: Option<String>,
        /// The link when linked; `null` when embedded.
        link: Option<AssetLink>,
        /// Size of the file bytes (64-bit).
        #[schemars(extend("x-unit" = "byte"))]
        size: u64,
        /// Lower-case hex SHA-256 of the file bytes.
        #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
        sha256: String,
        /// Import time.
        #[schemars(extend("readOnly" = true))]
        imported_at: Timestamp,
        /// Where the file was imported from (informational).
        source_path: Option<String>,
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

impl_object!(Asset, Asset, name);
