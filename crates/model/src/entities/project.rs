//! The project (10 §4.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::asset::Asset;
use super::drawing::DrawingBank;
use super::library::Library;
use super::palette::Palette;
use super::pen::Pen;
use super::scene::Scene;
use super::transition::Transition;
use super::{ChildList, ChildRef, Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::ProjectTag;
use crate::value::Timestamp;

model_struct! {
    /// The project (10 §4.1). Stored in `project.json` with stubs of palettes, drawing banks, scenes and the local
    /// library.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "project"))]
    pub struct Project {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `project`.
        #[serde(rename = "type")]
        ty: ProjectTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Project name; default `Untitled`; also the default package file stem.
        name: String,
        /// Creation time.
        #[schemars(extend("readOnly" = true))]
        created_at: Timestamp,
        /// Set by each save.
        #[schemars(extend("readOnly" = true))]
        modified_at: Timestamp,
        /// Frames per second, 1–120; NTSC rates are exact as written.
        #[schemars(range(min = 1.0, max = 120.0), extend("x-command" = "project.properties.set"))]
        frame_rate: f64,
        /// Camera (output) width, 16–8192 pixels.
        #[schemars(range(min = 16, max = 8192), extend("x-unit" = "pixel", "x-command" = "project.properties.set"))]
        camera_width: u32,
        /// Camera (output) height, 16–8192 pixels.
        #[schemars(range(min = 16, max = 8192), extend("x-unit" = "pixel", "x-command" = "project.properties.set"))]
        camera_height: u32,
        /// Whether template swatch values overwrite project values with the same colour IDs (03 §1.2; SI-005).
        auto_colour_update: bool,
        /// Edit point before the first scene.
        start_transition: Transition,
        /// Palettes in display order; at least one.
        #[schemars(length(min = 1))]
        palettes: ChildList<Palette>,
        /// Pens; at least one.
        #[schemars(length(min = 1))]
        pens: Vec<Pen>,
        /// Media assets.
        assets: Vec<Asset>,
        /// Drawing banks (unordered; written in ID order).
        drawing_banks: ChildList<DrawingBank>,
        /// Scenes in storyline order; at least one.
        #[schemars(length(min = 1))]
        scenes: ChildList<Scene>,
        /// The project-local library.
        library: ChildRef<Library>,
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

impl_object!(Project, Project, name);

/// Inputs of [`Project::new`].
#[derive(Debug, Clone)]
pub struct ProjectInit {
    /// Project ID.
    pub id: Id,
    /// Validated name.
    pub name: String,
    /// Creation (and initial modification) time.
    pub created_at: Timestamp,
    /// Frame rate.
    pub frame_rate: f64,
    /// Camera size in pixels.
    pub camera_size: (u32, u32),
    /// Start transition.
    pub start_transition: Transition,
    /// Palette IDs.
    pub palettes: Vec<Id>,
    /// Pens.
    pub pens: Vec<Pen>,
    /// Drawing bank IDs.
    pub drawing_banks: Vec<Id>,
    /// Scene IDs.
    pub scenes: Vec<Id>,
    /// Local library ID.
    pub library: Id,
}

impl Project {
    /// A project from its parts.
    #[must_use]
    pub fn new(init: ProjectInit) -> Self {
        Self {
            id: init.id,
            ty: ProjectTag,
            rev: 0,
            name: init.name,
            created_at: init.created_at,
            modified_at: init.created_at,
            frame_rate: init.frame_rate,
            camera_width: init.camera_size.0,
            camera_height: init.camera_size.1,
            auto_colour_update: false,
            start_transition: init.start_transition,
            palettes: ChildList::new(init.palettes),
            pens: init.pens,
            assets: Vec::new(),
            drawing_banks: ChildList::new(init.drawing_banks),
            scenes: ChildList::new(init.scenes),
            library: ChildRef::new(init.library),
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}
