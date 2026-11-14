//! Shell state the kernel owns (M4 C4.1; ADR-0021): the view windows and their transforms (01 §5.3, §6.5), the main
//! window's geometry (01 §5.4, §11), the start screen and context-help mode (01 §2, §9.3), and the *sensor facts* the
//! desktop host reports (modal state, text focus, pointer and hover texts, view sizes, screens).
//!
//! The kernel never calls a window system. Commands change this state; native-window changes are published after
//! acceptance as `shell.*` events, which only the host applies (the MCP event pump does not forward them). In
//! headless mode the same state is a virtual window and virtual views (11 §3), so every command behaves the same.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_registry::CommandError;

/// The zoom table of 01 §6.5 (factors).
pub const ZOOM_TABLE: &[f64] =
    &[0.0625, 0.125, 0.25, 0.333, 0.5, 0.667, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 32.0, 64.0];
/// Zoom limits (01 §6.5: 6.25 %-6400 %).
pub const ZOOM_MIN: f64 = 0.0625;
/// See [`ZOOM_MIN`].
pub const ZOOM_MAX: f64 = 64.0;
/// One rotation step (01 §6.5, §13.14).
pub const ROTATION_STEP: f64 = 30.0;
/// The view-window size before the host reports one (and in headless mode), in logical pixels.
pub const VIRTUAL_VIEW_SIZE: (f64, f64) = (960.0, 540.0);
/// The headless Drawing view's size (SI-069: 1920 x 1080 px, the drawing origin at its centre).
pub const VIRTUAL_DRAWING_VIEW_SIZE: (f64, f64) = (1920.0, 1080.0);

/// A view window's type (01 §4, §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ViewType {
    /// Drawing view.
    Drawing,
    /// Camera view.
    Camera,
    /// Top view.
    Top,
    /// Side view.
    Side,
}

impl ViewType {
    /// The type's name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ViewType::Drawing => "drawing",
            ViewType::Camera => "camera",
            ViewType::Top => "top",
            ViewType::Side => "side",
        }
    }

    /// The type a view panel shows by default (`drawingView` → drawing, …).
    #[must_use]
    pub fn of_panel(panel: &str) -> Option<Self> {
        match panel {
            "drawingView" => Some(ViewType::Drawing),
            "cameraView" => Some(ViewType::Camera),
            "topView" => Some(ViewType::Top),
            "sideView" => Some(ViewType::Side),
            _ => None,
        }
    }

    /// The working mode this type implies (01 §4).
    #[must_use]
    pub fn mode(self) -> crate::app::Mode {
        if self == ViewType::Drawing { crate::app::Mode::Drawing } else { crate::app::Mode::Staging }
    }
}

/// A point in document or view coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    /// x.
    pub x: f64,
    /// y.
    pub y: f64,
}

/// A view transform (01 §6.5): zoom factor, rotation of the view in degrees (clockwise positive), and the document
/// point at the centre of the view (drawing space for a Drawing view, scene space for staging views).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewTransform {
    /// Zoom factor (1 = 100 %).
    pub zoom: f64,
    /// View rotation in degrees, in (−180, 180].
    pub rotation_degrees: f64,
    /// Document point at the view centre.
    pub centre: Point,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self { zoom: 1.0, rotation_degrees: 0.0, centre: Point { x: 0.0, y: 0.0 } }
    }
}

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Size {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// One view window (`viewId` is its panel id; SI-112).
#[derive(Debug, Clone, PartialEq)]
pub struct ViewWindow {
    /// View id = panel id.
    pub view_id: String,
    /// Current type.
    pub ty: ViewType,
    /// The type before the last switch (01 §4 *Switch active view*).
    pub previous: Option<ViewType>,
    /// Transform per type (switching type and back restores it; 01 §6.5).
    pub transforms: BTreeMap<ViewType, ViewTransform>,
    /// Canvas size reported by the host (or the virtual default).
    pub size: Size,
    /// The last pointer position over the canvas, in view pixels (sensor; `view.pan.set {atPointer}`).
    pub pointer: Option<Point>,
}

impl ViewWindow {
    fn new(panel: &str, ty: ViewType) -> Self {
        Self {
            view_id: panel.to_owned(),
            ty,
            previous: None,
            transforms: BTreeMap::new(),
            size: if panel == "drawingView" {
                Size { width: VIRTUAL_DRAWING_VIEW_SIZE.0, height: VIRTUAL_DRAWING_VIEW_SIZE.1 }
            } else {
                Size { width: VIRTUAL_VIEW_SIZE.0, height: VIRTUAL_VIEW_SIZE.1 }
            },
            pointer: None,
        }
    }

    /// The transform of the current type.
    #[must_use]
    pub fn transform(&self) -> ViewTransform {
        self.transforms.get(&self.ty).copied().unwrap_or_default()
    }

    /// Sets the transform of the current type.
    pub fn set_transform(&mut self, t: ViewTransform) {
        self.transforms.insert(self.ty, t);
    }
}

/// A screen (01 §12.2 `window.geometry.get`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    /// Left, logical pixels.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Scale factor (device pixels per logical pixel).
    pub scale: f64,
}

/// The main window's geometry.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowGeometry {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Maximised.
    pub maximized: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self { x: 80, y: 60, width: 1280, height: 800, maximized: false }
    }
}

/// The modal state the host reports (01 §12.4 `modal`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModalReport {
    /// `dialog`, `gesture` or `progress`.
    pub kind: String,
    /// Title of the dialog or gesture.
    pub title: String,
}

/// Sensor facts (ADR-0021): reported by the host, never set by a command.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sensors {
    /// A modal dialog, gesture or progress window is open.
    pub modal: Option<ModalReport>,
    /// A text field has focus (unmodified keys go to the field; 01 §8.4).
    pub text_focus: bool,
    /// Status-bar description segment: the hovered item's long description (01 §9.1).
    pub description: String,
    /// Status-bar pointer segment text (pointer coordinates in the current unit).
    pub pointer: String,
    /// The host's main window is up (attached mode): native-window commands are applied.
    pub attached: bool,
    /// The UI tree the front end rendered last (11 §7.5 `inspect_ui`).
    pub ui_tree: Option<serde_json::Value>,
    /// The engine event number that tree reflects (`None`: not vouched for).
    pub ui_tree_seq: Option<u64>,
}

/// Everything in this module, part of [`crate::ui::UiState`] so that rollback and savepoints carry it.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellState {
    /// The four view windows.
    pub views: Vec<ViewWindow>,
    /// The view window the user focused last (active while visible; else the central view).
    pub active_view: Option<String>,
    /// Main window geometry.
    pub window: WindowGeometry,
    /// Minimised.
    pub minimized: bool,
    /// Screens (the host reports them; one virtual screen otherwise).
    pub screens: Vec<Screen>,
    /// An explicit start-screen state and the project it was set under (`None`: the rule of 01 §2).
    pub start_screen: Option<(bool, Option<String>)>,
    /// `general.startup.behaviour` is `startScreen` (01 §2: shown at launch and after Close), kept in step with the
    /// preference.
    pub start_screen_by_default: bool,
    /// Context-help mode (01 §9.3).
    pub context_help: bool,
    /// Sensor facts.
    pub sensors: Sensors,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            views: ["cameraView", "drawingView", "topView", "sideView"]
                .iter()
                .filter_map(|p| ViewType::of_panel(p).map(|t| ViewWindow::new(p, t)))
                .collect(),
            active_view: None,
            window: WindowGeometry::default(),
            minimized: false,
            screens: vec![Screen { x: 0, y: 0, width: 1920, height: 1080, scale: 1.0 }],
            start_screen: None,
            start_screen_by_default: true,
            context_help: false,
            sensors: Sensors::default(),
        }
    }
}

/// `view.notFound` (01 §12.1 rule 7).
pub fn view_not_found(id: &str) -> CommandError {
    CommandError::with_details("view.notFound", format!("no view window `{id}`"), json!({ "viewId": id }))
        .hint("view.list returns the view windows (cameraView, drawingView, topView, sideView)")
}

impl ShellState {
    /// Whether the start screen is shown for the project state `project` (01 §2): an explicit `ui.startScreen.set`
    /// made under the same project state wins; otherwise it is shown without a project when the start-up behaviour
    /// is the start screen.
    #[must_use]
    pub fn start_screen_visible(&self, project: Option<&str>) -> bool {
        match &self.start_screen {
            Some((v, at)) if at.as_deref() == project => *v,
            _ => project.is_none() && self.start_screen_by_default,
        }
    }

    /// A view window by id.
    pub fn view(&self, id: &str) -> Result<&ViewWindow, CommandError> {
        self.views.iter().find(|v| v.view_id == id).ok_or_else(|| view_not_found(id))
    }

    /// A view window by id, mutable.
    pub fn view_mut(&mut self, id: &str) -> Result<&mut ViewWindow, CommandError> {
        self.views.iter_mut().find(|v| v.view_id == id).ok_or_else(|| view_not_found(id))
    }

    /// Clamps the window to the reported screens so it stays reachable (01 §5.4 visibility guarantee): at least
    /// 100 × 40 logical pixels of its title area lie on some screen, and it is no larger than the largest screen.
    #[must_use]
    pub fn clamp(&self, g: WindowGeometry) -> WindowGeometry {
        let Some(largest) = self.screens.iter().max_by_key(|s| u64::from(s.width) * u64::from(s.height)) else {
            return g;
        };
        let mut g = g;
        g.width = g.width.clamp(1, largest.width.max(1));
        g.height = g.height.clamp(1, largest.height.max(1));
        let visible = |s: &Screen| {
            let (sx, sy) = (i64::from(s.x), i64::from(s.y));
            let (sw, sh) = (i64::from(s.width), i64::from(s.height));
            let (x, y, w) = (i64::from(g.x), i64::from(g.y), i64::from(g.width));
            x + w.min(100) > sx && x < sx + sw - 100.min(sw) + 1 && y >= sy && y < sy + sh - 40.min(sh) + 1
        };
        if !self.screens.iter().any(visible) {
            g.x = largest.x + i32::try_from((largest.width - g.width) / 2).unwrap_or(0);
            g.y = largest.y + i32::try_from((largest.height - g.height) / 2).unwrap_or(0);
        }
        g
    }
}

/// The next zoom table value up (`dir > 0`) or down from `zoom`, clamped to the table ends (01 §6.5).
#[must_use]
pub fn zoom_step(zoom: f64, steps: i64) -> f64 {
    let mut z = zoom;
    let up = steps > 0;
    for _ in 0..steps.unsigned_abs().min(64) {
        z = if up {
            ZOOM_TABLE.iter().copied().find(|t| *t > z * (1.0 + 1e-9)).unwrap_or(ZOOM_MAX)
        } else {
            ZOOM_TABLE.iter().rev().copied().find(|t| *t < z * (1.0 - 1e-9)).unwrap_or(ZOOM_MIN)
        };
    }
    z.clamp(ZOOM_MIN, ZOOM_MAX)
}

/// Normalises an angle in degrees into (−180, 180].
#[must_use]
pub fn normalise_degrees(d: f64) -> f64 {
    let r = d.rem_euclid(360.0);
    if r > 180.0 { r - 360.0 } else { r }
}

/// View pixels per document unit at zoom 1 for a view type and a camera width in pixels. The Drawing view shows one
/// drawing unit per view pixel at 100 % (SI-063, SI-069). The staging views show one camera pixel per view pixel on the
/// table plane at 100 %: the camera frame is 12 fields wide (ADR-0020 decision 4).
#[must_use]
pub fn pixels_per_unit(ty: ViewType, camera_width: f64) -> f64 {
    let w = if camera_width.is_finite() && camera_width > 0.0 { camera_width } else { 1920.0 };
    match ty {
        ViewType::Drawing => 1.0,
        _ => w / 12.0,
    }
}

/// Document point → view pixel (y up in documents, y down in views; clockwise view rotation).
#[must_use]
pub fn doc_to_view(t: &ViewTransform, size: Size, ppu: f64, p: Point) -> Point {
    let s = t.zoom * ppu;
    let (dx, dy) = ((p.x - t.centre.x) * s, -(p.y - t.centre.y) * s);
    let (sin, cos) = t.rotation_degrees.to_radians().sin_cos();
    Point { x: size.width / 2.0 + dx * cos - dy * sin, y: size.height / 2.0 + dx * sin + dy * cos }
}

/// View pixel → document point (the inverse of [`doc_to_view`]).
#[must_use]
pub fn view_to_doc(t: &ViewTransform, size: Size, ppu: f64, v: Point) -> Point {
    let s = t.zoom * ppu;
    let (vx, vy) = (v.x - size.width / 2.0, v.y - size.height / 2.0);
    let (sin, cos) = t.rotation_degrees.to_radians().sin_cos();
    let (dx, dy) = (vx * cos + vy * sin, -vx * sin + vy * cos);
    Point { x: t.centre.x + dx / s, y: t.centre.y - dy / s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_follow_the_table_and_clamp() {
        assert!((zoom_step(1.0, 1) - 1.5).abs() < 1e-12);
        assert!((zoom_step(1.0, -1) - 0.667).abs() < 1e-12);
        assert!((zoom_step(1.2, 1) - 1.5).abs() < 1e-12, "an intermediate value steps to the next table value");
        assert!((zoom_step(1.2, -1) - 1.0).abs() < 1e-12);
        assert!((zoom_step(64.0, 3) - 64.0).abs() < 1e-12);
        assert!((zoom_step(0.0625, -1) - 0.0625).abs() < 1e-12);
    }

    #[test]
    fn view_and_document_round_trip() {
        let t = ViewTransform { zoom: 2.5, rotation_degrees: 30.0, centre: Point { x: 10.0, y: -4.0 } };
        let size = Size { width: 800.0, height: 600.0 };
        for (x, y) in [(0.0, 0.0), (12.5, 3.0), (-100.0, 40.0)] {
            let v = doc_to_view(&t, size, 1.6, Point { x, y });
            let d = view_to_doc(&t, size, 1.6, v);
            assert!((d.x - x).abs() < 1e-9 && (d.y - y).abs() < 1e-9);
        }
        let c = doc_to_view(&t, size, 1.6, t.centre);
        assert!((c.x - 400.0).abs() < 1e-9 && (c.y - 300.0).abs() < 1e-9, "the centre is at the view centre");
        assert!((normalise_degrees(390.0) - 30.0).abs() < 1e-12);
        assert!((normalise_degrees(-180.0) - 180.0).abs() < 1e-12);
    }
}
