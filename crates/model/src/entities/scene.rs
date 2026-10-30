//! Scenes (10 §4.3, 08 §13.1).

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::element::Element;
use super::transition::Transition;
use super::{ChildList, Ext, Meta, ext_is_empty, meta_is_empty};
use crate::id::Id;
use crate::types::SceneTag;
use crate::value::Colour;

/// A node of the scene tree: `{id, children}` (10 §4.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TreeNode {
    /// The element.
    #[schemars(extend("x-ref" = "element"))]
    pub id: Id,
    /// Children, top to bottom as displayed in the Timeline.
    pub children: Vec<TreeNode>,
}

model_struct! {
    /// A scene (10 §4.3). Stored in `scenes/<sceneId>/scene.json` with element stubs.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    #[schemars(extend("x-objectType" = "scene"))]
    pub struct Scene {
        /// Immutable ID.
        #[schemars(extend("readOnly" = true))]
        id: Id,
        /// Always `scene`.
        #[serde(rename = "type")]
        ty: SceneTag,
        /// Revision of the last change.
        #[schemars(extend("readOnly" = true))]
        rev: u64,
        /// Unique in the project; default `Scene-<n>`.
        #[schemars(extend("x-command" = "storyline.scene.rename"))]
        name: String,
        /// Hidden scenes are skipped by storyline playback and export.
        #[schemars(extend("x-command" = "storyline.scene.visible.set"))]
        visible: bool,
        /// Background colour (default `#FFFFFFFF`).
        #[schemars(extend("x-command" = "storyline.scene.set"))]
        background_colour: Colour,
        /// The current camera; must be a camera of this scene when it has one.
        #[schemars(extend("x-ref" = "element", "x-command" = "anim.camera.current.set"))]
        current_camera_id: Option<Id>,
        /// Active style per palette: exactly one entry per palette, in palette order.
        #[schemars(extend("x-command" = "colour.style.select"))]
        palette_styles: IndexMap<Id, Id>,
        /// Edit point after this scene.
        out_transition: Transition,
        /// Timeline order and hierarchy; every element exactly once.
        tree: Vec<TreeNode>,
        /// Exposure Sheet order of the cell-based elements, back to front.
        column_order: Vec<Id>,
        /// The elements (unordered; written in ID order).
        elements: ChildList<Element>,
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

impl_object!(Scene, Scene, name);

/// Inputs of [`Scene::new`].
#[derive(Debug, Clone)]
pub struct SceneInit {
    /// Scene ID.
    pub id: Id,
    /// Validated name.
    pub name: String,
    /// The out-transition (type `none` for new scenes).
    pub out_transition: Transition,
    /// Active style per palette.
    pub palette_styles: IndexMap<Id, Id>,
    /// Elements, the tree, the column order and the current camera.
    pub elements: Vec<Id>,
    /// Timeline tree.
    pub tree: Vec<TreeNode>,
    /// Exposure Sheet order.
    pub column_order: Vec<Id>,
    /// Current camera.
    pub current_camera_id: Option<Id>,
}

impl Scene {
    /// A scene from its parts with default settings (visible, white background).
    #[must_use]
    pub fn new(init: SceneInit) -> Self {
        Self {
            id: init.id,
            ty: SceneTag,
            rev: 0,
            name: init.name,
            visible: true,
            background_colour: Colour::WHITE,
            current_camera_id: init.current_camera_id,
            palette_styles: init.palette_styles,
            out_transition: init.out_transition,
            tree: init.tree,
            column_order: init.column_order,
            elements: ChildList::new(init.elements),
            meta: Meta::new(),
            ext: Ext::default(),
        }
    }
}
