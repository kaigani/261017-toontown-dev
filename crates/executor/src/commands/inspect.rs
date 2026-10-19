//! `inspect.*` (11 §15.2): visual and structural inspection. Declared with typed parameters; the frame, hit-test,
//! coordinate and contact-sheet commands come with the renderer (M3 C3.3), the GUI tree and screenshots with the
//! desktop shell (M4 C4.2).

#![allow(
    dead_code,
    reason = "declared commands: parameters are decoded and validated now and read by the handler of the owning checkpoint"
)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tt_registry::{CommandError, command};

use super::common::{Point, Rect};
use crate::cx::Cx;

/// Description detail.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Detail {
    /// Summary.
    #[default]
    Summary,
    /// Everything.
    Full,
}

/// `inspect.frame.describe` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FrameParams {
    /// Scene (default current).
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame (default current).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// Camera (default the scene's current camera).
    #[serde(default)]
    camera_id: Option<String>,
    /// Detail.
    #[serde(default)]
    detail: Detail,
}

#[command(
    id = "inspect.frame.describe",
    kind = query,
    label = "Describe frame",
    scopes = [read],
    needs_project,
    not_implemented = "M3 C3.3",
    example(title = "What the current frame shows", params = "{}"),
    agent_only = "agents verify frames structurally (11 §7.5 describe_frame)",
)]
pub(crate) fn inspect_frame_describe(_cx: &mut Cx, _p: FrameParams) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("inspect.frame.describe", "M3 C3.3"))
}

/// Coordinate space.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Space {
    /// View pixels.
    View,
    /// Drawing space.
    Drawing,
    /// Scene space.
    Scene,
    /// Camera space.
    Camera,
    /// Rendered image pixels.
    Image,
}

/// `inspect.hitTest` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct HitParams {
    /// Space of `point`.
    space: Space,
    /// The point.
    point: Point,
    /// Scene.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// View.
    #[serde(default)]
    view: Option<String>,
}

/// `{hits}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Hits {
    /// Hits, front to back.
    hits: Vec<Map<String, Value>>,
}

#[command(
    id = "inspect.hitTest",
    kind = query,
    label = "Hit test",
    scopes = [read],
    needs_project,
    not_implemented = "M3 C3.3",
    example(title = "What is at the camera centre", params = r#"{"space": "camera", "point": {"x": 0, "y": 0}}"#),
    agent_only = "agents locate objects by position (11 §7.5 hit_test)",
)]
pub(crate) fn inspect_hit_test(_cx: &mut Cx, _p: HitParams) -> Result<Hits, CommandError> {
    Err(CommandError::not_implemented("inspect.hitTest", "M3 C3.3"))
}

/// A point with an optional depth.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Point3 {
    /// X.
    x: f64,
    /// Y.
    y: f64,
    /// Z.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    z: Option<f64>,
}

/// `inspect.coordinates.convert` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ConvertParams {
    /// Source space.
    from: String,
    /// Target space.
    to: String,
    /// Points.
    #[schemars(length(min = 1, max = 100_000))]
    points: Vec<Point3>,
    /// Scene.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// Element.
    #[serde(default)]
    element_id: Option<String>,
    /// View.
    #[serde(default)]
    view: Option<String>,
}

/// `{points}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Points {
    /// Converted points.
    points: Vec<Point3>,
}

#[command(
    id = "inspect.coordinates.convert",
    kind = query,
    label = "Convert coordinates",
    scopes = [read],
    needs_project,
    not_implemented = "M3 C3.3",
    example(title = "Field units to camera space", params = r#"{"from": "field", "to": "camera", "points": [{"x": 1, "y": 1}]}"#),
    agent_only = "agents convert between coordinate spaces (11 §7.5 convert_coordinates)",
)]
pub(crate) fn inspect_coordinates_convert(_cx: &mut Cx, _p: ConvertParams) -> Result<Points, CommandError> {
    Err(CommandError::not_implemented("inspect.coordinates.convert", "M3 C3.3"))
}

/// `inspect.contactSheet.render` parameters.
/// The frames of a contact sheet (spec A `inspect.contactSheet.render`): a list, or a stepped range.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of M3 C3.3")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum FrameSelection {
    /// The frames listed.
    List(#[schemars(length(min = 1, max = 1000), inner(range(min = 1, max = 100_000)))] Vec<u32>),
    /// `from` to `to` every `step` frames.
    Range {
        /// First frame.
        #[schemars(range(min = 1, max = 100_000))]
        from: u32,
        /// Last frame.
        #[schemars(range(min = 1, max = 100_000))]
        to: u32,
        /// Step (default 1).
        #[serde(default)]
        #[schemars(range(min = 1, max = 100_000))]
        step: Option<u32>,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ContactSheetParams {
    /// Scene.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frames: a list, or `{from, to, step}`.
    frames: FrameSelection,
    /// Columns.
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    columns: Option<u32>,
    /// Cell width.
    #[serde(default)]
    #[schemars(range(min = 16, max = 2048))]
    cell_width: Option<u32>,
    /// Frame labels.
    #[serde(default)]
    labels: Option<bool>,
}

#[command(
    id = "inspect.contactSheet.render",
    kind = query,
    label = "Contact sheet",
    scopes = [read],
    needs_project,
    not_implemented = "M3 C3.3",
    example(title = "Every fourth frame of 1–24", params = r#"{"frames": {"from": 1, "to": 24, "step": 4}}"#),
    agent_only = "agents review motion in one image (11 §7.5 render_contact_sheet)",
)]
pub(crate) fn inspect_contact_sheet_render(
    _cx: &mut Cx,
    _p: ContactSheetParams,
) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("inspect.contactSheet.render", "M3 C3.3"))
}

/// `inspect.ui.tree` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UiTreeParams {
    /// Window, panel or dialog.
    #[serde(default)]
    target: Option<String>,
    /// Depth.
    #[serde(default)]
    #[schemars(range(min = 1, max = 32))]
    depth: Option<u32>,
}

#[command(
    id = "inspect.ui.tree",
    kind = query,
    label = "UI tree",
    scopes = [read],
    not_implemented = "M4 C4.2",
    example(title = "The window's accessibility tree, 3 levels", params = r#"{"depth": 3}"#),
    agent_only = "agents inspect the GUI (11 §7.8 inspect_ui)",
)]
pub(crate) fn inspect_ui_tree(_cx: &mut Cx, _p: UiTreeParams) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("inspect.ui.tree", "M4 C4.2"))
}

/// `inspect.screenshot` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScreenshotParams {
    /// Window, view, panel or dialog.
    #[serde(default)]
    target: Option<String>,
    /// Region.
    #[serde(default)]
    region: Option<Rect>,
}

#[command(
    id = "inspect.screenshot",
    kind = query,
    label = "Screenshot",
    scopes = [read],
    not_implemented = "M4 C4.2",
    example(title = "The main window", params = "{}"),
    agent_only = "agents look at the GUI (11 §7.8 screenshot)",
)]
pub(crate) fn inspect_screenshot(_cx: &mut Cx, _p: ScreenshotParams) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("inspect.screenshot", "M4 C4.2"))
}
