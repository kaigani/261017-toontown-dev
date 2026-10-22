//! Kernel-level window, panel and workspace state (01 §5.2–§5.6, §12.4; M0 readiness bullet 4).
//!
//! The kernel owns the arrangement the GUI shows — the current workspace, its dock layout, floating panels, tool
//! palettes, main toolbar, the separate full-screen arrangement and the full-screen flag — so that agents can read
//! and set it through commands (`window.panel.list`, `workspace.get`, `window.fullScreen.set {enabled}`, …) and the
//! desktop shell (M4) renders what this state says. Layout changes are not undoable and do not modify the project
//! (01 §5.5). Persistence per user (01 §11) is part of the settings work in C1.3.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tt_registry::CommandError;

/// The panel catalogue (01 §5.2): id and title.
pub const PANELS: &[(&str, &str)] = &[
    ("cameraView", "Camera view"),
    ("drawingView", "Drawing view"),
    ("topView", "Top view"),
    ("sideView", "Side view"),
    ("timeline", "Timeline"),
    ("xsheet", "Exposure sheet"),
    ("functionEditor", "Function editor"),
    ("properties", "Properties"),
    ("colour", "Colour"),
    ("pen", "Pen"),
    ("text", "Text"),
    ("cells", "Cells"),
    ("library", "Library"),
    ("storyline", "Storyline"),
    ("sceneManager", "Scene manager"),
    ("effectPreview", "Effect preview"),
    ("motionRecording", "Motion recording"),
    ("lipSync", "Lip-sync"),
    ("history", "History"),
    ("soundEditor", "Sound element editor"),
    ("capture", "Capture window"),
];

/// The view panels that can be the central view (01 §5.1, §5.4).
pub const VIEW_PANELS: &[&str] = &["cameraView", "drawingView", "topView", "sideView"];

/// Tool palettes (01 §5.2).
pub const TOOL_PALETTES: &[&str] = &["drawingTools", "animationTools", "boneTools", "effectTools", "soundTools"];

/// A geometry in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width.
    #[schemars(range(min = 1, max = 32767))]
    pub width: u32,
    /// Height.
    #[schemars(range(min = 1, max = 32767))]
    pub height: u32,
}

/// A point in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Position {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
}

/// One tab group of a dock area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TabGroup {
    /// Panel ids, in tab order.
    pub tabs: Vec<String>,
    /// The tab in front.
    pub current: String,
    /// On the first group of an area: the area's width (left/right) or height (top/bottom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
}

/// The four dock areas, each an ordered stack of tab groups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Docks {
    /// Left area.
    #[serde(default)]
    pub left: Vec<TabGroup>,
    /// Right area.
    #[serde(default)]
    pub right: Vec<TabGroup>,
    /// Top area.
    #[serde(default)]
    pub top: Vec<TabGroup>,
    /// Bottom area.
    #[serde(default)]
    pub bottom: Vec<TabGroup>,
}

/// A floating panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Floating {
    /// Panel id.
    pub panel_id: String,
    /// Shown.
    pub visible: bool,
    /// Window geometry.
    pub geometry: Geometry,
}

/// A tool palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolPalette {
    /// Palette id (`drawingTools`, …).
    pub id: String,
    /// Shown.
    pub visible: bool,
    /// Floating (else docked with the main toolbar).
    pub floating: bool,
    /// Floating position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

/// The main toolbar configuration (01 §5.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MainToolbar {
    /// Shown.
    pub visible: bool,
    /// Items: command references, group ids, `separator`, `space`, `flexibleSpace`.
    pub items: Vec<String>,
}

/// Corner ownership (01 §5.4 *Extend side panels*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Corners {
    /// Left and right areas run full height.
    Sides,
    /// Top and bottom areas run full width.
    TopBottom,
}

/// An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Arrangement {
    /// The central view.
    pub central_panel: String,
    /// Corner ownership.
    pub corners: Corners,
    /// Dock areas.
    pub docks: Docks,
    /// Floating panels.
    #[serde(default)]
    pub floating: Vec<Floating>,
    /// Tool palettes.
    #[serde(default)]
    pub tool_palettes: Vec<ToolPalette>,
    /// Main toolbar.
    pub main_toolbar: MainToolbar,
}

/// A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate
/// full-screen arrangement (01 §5.6). Panels not mentioned are hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Layout {
    /// The central view.
    pub central_panel: String,
    /// Corner ownership.
    pub corners: Corners,
    /// Dock areas.
    pub docks: Docks,
    /// Floating panels.
    #[serde(default)]
    pub floating: Vec<Floating>,
    /// Tool palettes.
    #[serde(default)]
    pub tool_palettes: Vec<ToolPalette>,
    /// Main toolbar.
    pub main_toolbar: MainToolbar,
    /// Full-screen arrangement (same shape without `fullScreen`).
    pub full_screen: Arrangement,
}

/// The normal and the full-screen arrangement of a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Arrangements {
    pub normal: Arrangement,
    pub full_screen: Arrangement,
}

impl From<Layout> for Arrangements {
    fn from(l: Layout) -> Self {
        Self {
            normal: Arrangement {
                central_panel: l.central_panel,
                corners: l.corners,
                docks: l.docks,
                floating: l.floating,
                tool_palettes: l.tool_palettes,
                main_toolbar: l.main_toolbar,
            },
            full_screen: l.full_screen,
        }
    }
}

impl From<&Arrangements> for Layout {
    fn from(a: &Arrangements) -> Self {
        let n = a.normal.clone();
        Self {
            central_panel: n.central_panel,
            corners: n.corners,
            docks: n.docks,
            floating: n.floating,
            tool_palettes: n.tool_palettes,
            main_toolbar: n.main_toolbar,
            full_screen: a.full_screen.clone(),
        }
    }
}

/// A workspace (01 §5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Workspace {
    pub name: String,
    pub built_in: bool,
    /// The bundled workspace this one is (its name may have changed; C12-R1-16).
    pub bundled: Option<String>,
    pub layout: Arrangements,
}

/// A panel as listed by `window.panel.list` (01 A).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PanelEntry {
    /// Panel id.
    pub panel_id: String,
    /// Title.
    pub title: String,
    /// Shown.
    pub visible: bool,
    /// Floating.
    pub floating: bool,
    /// Dock area (`left` … `bottom`, `central`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// Index of its tab group within the area.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_group: Option<usize>,
    /// In front of its tab group.
    pub current: bool,
    /// Floating geometry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Geometry>,
    /// Has focus.
    pub focused: bool,
}

/// A workspace as listed by `workspace.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    /// Name.
    pub name: String,
    /// Built in (cannot be deleted).
    pub built_in: bool,
    /// The current arrangement differs from the stored one (current workspace only).
    pub modified: bool,
    /// Position in the list (1-based).
    pub position: usize,
    /// The current workspace.
    pub current: bool,
}

fn group(tabs: &[&str], size: Option<u32>) -> TabGroup {
    TabGroup {
        tabs: tabs.iter().map(|t| (*t).to_owned()).collect(),
        current: tabs.first().map(|t| (*t).to_owned()).unwrap_or_default(),
        size,
    }
}

fn palettes(visible: &[&str]) -> Vec<ToolPalette> {
    TOOL_PALETTES
        .iter()
        .enumerate()
        .map(|(i, id)| ToolPalette {
            id: (*id).to_owned(),
            visible: visible.contains(id),
            floating: false,
            position: Some(Position { x: 10, y: 120 + 40 * i32::try_from(i).unwrap_or(0) }),
        })
        .collect()
}

/// Main-toolbar items that are not a single command reference (01 §5.7: group pop-ups, drop-downs, compound
/// buttons) and the spacers. Every other item is a registered command id (ADR-0013 §8).
pub const TOOLBAR_GROUPS: &[&str] = &[
    "separator",
    "space",
    "flexibleSpace",
    "gridType",
    "gridOverlay",
    "onionSkinPrevious",
    "onionSkinNext",
    "firstFrame",
    "previousFrame",
    "playStop",
    "nextFrame",
    "lastFrame",
    "cameraList",
    "sceneList",
    "workspaceList",
];

/// Default main toolbar set, in order (01 §5.7).
fn default_toolbar() -> MainToolbar {
    MainToolbar {
        visible: true,
        items: [
            "drawAid.grid.set",
            "gridType",
            "gridOverlay",
            "separator",
            "drawAid.onionSkin.set",
            "onionSkinPrevious",
            "onionSkinNext",
            "drawAid.autoLightTable.set",
            "separator",
            "firstFrame",
            "playStop",
            "lastFrame",
            "playback.loop.set",
            "space",
            "lipsync.panel.open",
            "flexibleSpace",
            "cameraList",
            "sceneList",
            "workspaceList",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect(),
    }
}

fn arrangement(central: &str, corners: Corners, docks: Docks, palettes_visible: &[&str]) -> Arrangement {
    Arrangement {
        central_panel: central.to_owned(),
        corners,
        docks,
        floating: Vec::new(),
        tool_palettes: palettes(palettes_visible),
        main_toolbar: default_toolbar(),
    }
}

/// The full-screen arrangement of a built-in workspace: the same central view, docks hidden, palettes kept.
fn full_screen_of(a: &Arrangement) -> Arrangement {
    Arrangement { docks: Docks::default(), floating: Vec::new(), ..a.clone() }
}

/// The bundled arrangements of the built-in workspaces (01 §5.5 table, recommended defaults).
pub(crate) fn built_in_workspaces() -> Vec<Workspace> {
    let all4 = ["drawingTools", "animationTools", "boneTools", "effectTools"];
    let default = arrangement(
        "cameraView",
        Corners::Sides,
        Docks {
            right: vec![group(&["colour", "properties", "library"], Some(330)), group(&["pen", "text", "cells"], None)],
            bottom: vec![group(&["timeline", "xsheet", "functionEditor", "storyline"], Some(190))],
            ..Docks::default()
        },
        &all4,
    );
    let staging = arrangement(
        "cameraView",
        Corners::TopBottom,
        Docks {
            right: vec![
                group(&["properties", "cells"], Some(330)),
                group(&["sideView"], None),
                group(&["topView"], None),
            ],
            bottom: vec![group(&["timeline", "functionEditor"], Some(190)), group(&["library"], None)],
            ..Docks::default()
        },
        &all4,
    );
    let drawing = arrangement(
        "drawingView",
        Corners::Sides,
        Docks {
            right: vec![group(&["xsheet"], Some(330)), group(&["colour", "pen", "text"], None)],
            ..Docks::default()
        },
        &["drawingTools", "animationTools"],
    );
    let focused = arrangement(
        "drawingView",
        Corners::Sides,
        Docks { right: vec![group(&["colour"], Some(300)), group(&["pen"], None)], ..Docks::default() },
        &["drawingTools"],
    );
    [("Default", default), ("Staging", staging), ("Drawing", drawing), ("Focused drawing", focused)]
        .into_iter()
        .map(|(name, a)| Workspace {
            name: name.to_owned(),
            built_in: true,
            bundled: Some(name.to_owned()),
            layout: Arrangements { full_screen: full_screen_of(&a), normal: a },
        })
        .collect()
}

fn ui_error(code: &str, message: impl Into<String>, details: serde_json::Value) -> CommandError {
    CommandError::with_details(code, message, details)
}

fn panel_not_found(id: &str) -> CommandError {
    ui_error("window.panelNotFound", format!("no panel `{id}`"), serde_json::json!({ "panelId": id }))
}

/// Where a panel was before it was hidden or floated (01 §5.4: showing it again restores it there).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    area: String,
    group: usize,
}

/// The window, panel and workspace state.
#[derive(Debug, Clone)]
pub struct UiState {
    workspaces: Vec<Workspace>,
    current: String,
    /// The live arrangement (normal and full-screen) of the current workspace.
    live: Arrangements,
    full_screen: bool,
    floating_hidden: bool,
    focused: Option<String>,
    remembered: BTreeMap<String, Place>,
}

impl Default for UiState {
    fn default() -> Self {
        let workspaces = built_in_workspaces();
        let live = workspaces.first().map(|w| w.layout.clone()).unwrap_or_else(|| Arrangements {
            normal: arrangement("cameraView", Corners::Sides, Docks::default(), &[]),
            full_screen: arrangement("cameraView", Corners::Sides, Docks::default(), &[]),
        });
        Self {
            current: workspaces.first().map(|w| w.name.clone()).unwrap_or_default(),
            workspaces,
            live,
            full_screen: false,
            floating_hidden: false,
            focused: Some("cameraView".to_owned()),
            remembered: BTreeMap::new(),
        }
    }
}

const AREAS: [&str; 4] = ["left", "right", "top", "bottom"];

fn area_mut<'a>(docks: &'a mut Docks, area: &str) -> Option<&'a mut Vec<TabGroup>> {
    match area {
        "left" => Some(&mut docks.left),
        "right" => Some(&mut docks.right),
        "top" => Some(&mut docks.top),
        "bottom" => Some(&mut docks.bottom),
        _ => None,
    }
}

fn area_ref<'a>(docks: &'a Docks, area: &str) -> &'a [TabGroup] {
    match area {
        "left" => &docks.left,
        "right" => &docks.right,
        "top" => &docks.top,
        _ => &docks.bottom,
    }
}

impl UiState {
    /// The arrangement in use (normal or full-screen).
    fn active(&self) -> &Arrangement {
        if self.full_screen { &self.live.full_screen } else { &self.live.normal }
    }

    fn active_mut(&mut self) -> &mut Arrangement {
        if self.full_screen { &mut self.live.full_screen } else { &mut self.live.normal }
    }

    fn stored(&self) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.name == self.current)
    }

    /// The current workspace's name and whether the live arrangement differs from the stored one.
    #[must_use]
    pub fn workspace_state(&self) -> (String, bool) {
        (self.current.clone(), self.stored().is_none_or(|w| w.layout != self.live))
    }

    /// Whether full-screen presentation is on.
    #[must_use]
    pub fn full_screen(&self) -> bool {
        self.full_screen
    }

    /// The focused panel.
    #[must_use]
    pub fn focused(&self) -> Option<&str> {
        self.focused.as_deref()
    }

    /// The current layout (live normal + full-screen arrangements).
    #[must_use]
    pub fn layout(&self) -> Layout {
        Layout::from(&self.live)
    }

    /// The central view (the active view) of the arrangement in use.
    #[must_use]
    pub fn central_panel(&self) -> &str {
        // The arrangement in use: the full-screen one while full screen is on (01 §5.6; C12-R1-15).
        &self.active().central_panel
    }

    /// Where a panel is in an arrangement.
    fn find(a: &Arrangement, panel: &str) -> Option<(String, Option<usize>, bool)> {
        if a.central_panel == panel {
            return Some(("central".to_owned(), None, true));
        }
        for area in AREAS {
            for (gi, g) in area_ref(&a.docks, area).iter().enumerate() {
                if g.tabs.iter().any(|t| t == panel) {
                    return Some((area.to_owned(), Some(gi), g.current == panel));
                }
            }
        }
        None
    }

    /// `window.panel.list`.
    #[must_use]
    pub fn panels(&self) -> Vec<PanelEntry> {
        let a = self.active();
        PANELS
            .iter()
            .map(|(id, title)| {
                let docked = Self::find(a, id);
                let floating = a.floating.iter().find(|f| f.panel_id == *id);
                let visible = docked.is_some() || floating.is_some_and(|f| f.visible && !self.floating_hidden);
                PanelEntry {
                    panel_id: (*id).to_owned(),
                    title: (*title).to_owned(),
                    visible,
                    floating: floating.is_some(),
                    area: docked.as_ref().map(|d| d.0.clone()),
                    tab_group: docked.as_ref().and_then(|d| d.1),
                    current: docked.as_ref().is_some_and(|d| d.2) || floating.is_some(),
                    geometry: floating.map(|f| f.geometry),
                    focused: self.focused.as_deref() == Some(*id),
                }
            })
            .collect()
    }

    fn check_panel(id: &str) -> Result<(), CommandError> {
        if PANELS.iter().any(|(p, _)| *p == id) { Ok(()) } else { Err(panel_not_found(id)) }
    }

    /// Removes a docked panel from its tab group, remembering the place; drops emptied groups.
    fn undock(&mut self, panel: &str) -> Option<Place> {
        let a = self.active_mut();
        for area in AREAS {
            let groups = area_mut(&mut a.docks, area)?;
            for gi in 0..groups.len() {
                let Some(g) = groups.get_mut(gi) else { continue };
                if let Some(pos) = g.tabs.iter().position(|t| t == panel) {
                    g.tabs.remove(pos);
                    if g.current == panel {
                        g.current = g.tabs.first().cloned().unwrap_or_default();
                    }
                    if g.tabs.is_empty() {
                        let size = g.size;
                        groups.remove(gi);
                        if let (Some(s), Some(first)) = (size, groups.first_mut()) {
                            first.size.get_or_insert(s);
                        }
                    }
                    let place = Place { area: area.to_owned(), group: gi };
                    return Some(place);
                }
            }
        }
        None
    }

    fn dock_at(&mut self, panel: &str, place: Option<Place>) {
        let place = place.unwrap_or(Place { area: "right".to_owned(), group: usize::MAX });
        let a = self.active_mut();
        let Some(groups) = area_mut(&mut a.docks, &place.area) else { return };
        match groups.get_mut(place.group) {
            Some(g) => {
                g.tabs.push(panel.to_owned());
                panel.clone_into(&mut g.current);
            }
            None => groups.push(TabGroup { tabs: vec![panel.to_owned()], current: panel.to_owned(), size: None }),
        }
    }

    /// `window.panel.set`: show/hide, float/dock, focus; `"active"` addresses the focused panel.
    ///
    /// # Errors
    /// `window.panelNotFound`; `arg.invalid` for hiding or floating the central view.
    pub fn set_panel(
        &mut self,
        panel: &str,
        visible: Option<bool>,
        floating: Option<bool>,
        focus: bool,
        geometry: Option<Geometry>,
    ) -> Result<PanelEntry, CommandError> {
        let panel = if panel == "active" { self.focused.clone().unwrap_or_default() } else { panel.to_owned() };
        Self::check_panel(&panel)?;
        let central = self.active().central_panel == panel;
        if central && (visible == Some(false) || floating == Some(true)) {
            return Err(ui_error(
                "arg.invalid",
                "the central view cannot be hidden or floated; move another view to the centre first",
                serde_json::json!({ "panelId": panel, "pointer": "/panelId" }),
            ));
        }
        let is_floating = self.active().floating.iter().any(|f| f.panel_id == panel);
        let is_docked = !central && Self::find(self.active(), &panel).is_some();
        let want_visible = visible.unwrap_or(true);
        let want_floating = floating.unwrap_or(is_floating);
        if !central {
            if !want_visible {
                if is_docked && let Some(place) = self.undock(&panel) {
                    self.remembered.insert(panel.clone(), place);
                }
                if let Some(f) = self.active_mut().floating.iter_mut().find(|f| f.panel_id == panel) {
                    f.visible = false;
                }
                if self.focused.as_deref() == Some(panel.as_str()) {
                    self.focused = None;
                }
            } else if want_floating {
                if is_docked && let Some(place) = self.undock(&panel) {
                    self.remembered.insert(panel.clone(), place);
                }
                let a = self.active_mut();
                match a.floating.iter_mut().find(|f| f.panel_id == panel) {
                    Some(f) => {
                        f.visible = true;
                        if let Some(g) = geometry {
                            f.geometry = g;
                        }
                    }
                    None => a.floating.push(Floating {
                        panel_id: panel.clone(),
                        visible: true,
                        geometry: geometry.unwrap_or(Geometry { x: 80, y: 80, width: 480, height: 360 }),
                    }),
                }
            } else {
                // Docked and visible.
                if is_floating {
                    self.active_mut().floating.retain(|f| f.panel_id != panel);
                }
                if is_docked {
                    // Bring its tab forward (01 §5.4).
                    let a = self.active_mut();
                    for area in AREAS {
                        if let Some(groups) = area_mut(&mut a.docks, area) {
                            for g in groups.iter_mut().filter(|g| g.tabs.contains(&panel)) {
                                panel.clone_into(&mut g.current);
                            }
                        }
                    }
                } else {
                    let place = self.remembered.remove(&panel);
                    self.dock_at(&panel, place);
                }
            }
        }
        if focus && want_visible {
            self.focused = Some(panel.clone());
        }
        self.panels().into_iter().find(|p| p.panel_id == panel).ok_or_else(|| panel_not_found(&panel))
    }

    /// `window.panel.move`: dock a panel into an area, next to or tabbed with another panel.
    ///
    /// # Errors
    /// `window.panelNotFound`; `arg.invalid` for a non-view central panel, a panel moved relative to itself, or a
    /// `relativeTo` panel that is not docked in `area`.
    pub fn move_panel(
        &mut self,
        panel: &str,
        area: &str,
        relative_to: Option<&str>,
        position: &str,
        size: Option<u32>,
    ) -> Result<Layout, CommandError> {
        Self::check_panel(panel)?;
        if let Some(r) = relative_to {
            Self::check_panel(r)?;
            if r == panel {
                return Err(ui_error(
                    "arg.invalid",
                    "a panel cannot be placed relative to itself",
                    serde_json::json!({ "pointer": "/relativeTo" }),
                ));
            }
        }
        if area == "central" {
            if !VIEW_PANELS.contains(&panel) {
                return Err(ui_error(
                    "arg.invalid",
                    "only a view (camera, drawing, top, side) can be the central view",
                    serde_json::json!({ "pointer": "/panelId", "allowed": VIEW_PANELS }),
                ));
            }
            let _ = self.undock(panel);
            self.active_mut().floating.retain(|f| f.panel_id != panel);
            let old = std::mem::replace(&mut self.active_mut().central_panel, panel.to_owned());
            if old != panel {
                self.dock_at(&old, None);
            }
            return Ok(self.layout());
        }
        if self.active().central_panel == panel {
            return Err(ui_error(
                "arg.invalid",
                "move another view to the centre before docking the central view",
                serde_json::json!({ "pointer": "/panelId" }),
            ));
        }
        let _ = self.undock(panel);
        self.active_mut().floating.retain(|f| f.panel_id != panel);
        let a = self.active_mut();
        let Some(groups) = area_mut(&mut a.docks, area) else {
            return Err(ui_error(
                "arg.invalid",
                format!("unknown area `{area}`"),
                serde_json::json!({ "pointer": "/area" }),
            ));
        };
        let target = match relative_to {
            Some(r) => match groups.iter().position(|g| g.tabs.iter().any(|t| t == r)) {
                Some(i) => Some(i),
                None => {
                    return Err(ui_error(
                        "arg.invalid",
                        format!("`{r}` is not docked in the {area} area"),
                        serde_json::json!({ "pointer": "/relativeTo" }),
                    ));
                }
            },
            None => None,
        };
        let new_group = || TabGroup { tabs: vec![panel.to_owned()], current: panel.to_owned(), size: None };
        match (target, position) {
            (Some(i), "tab") => {
                if let Some(g) = groups.get_mut(i) {
                    g.tabs.push(panel.to_owned());
                    panel.clone_into(&mut g.current);
                }
            }
            (Some(i), "before" | "above") => groups.insert(i, new_group()),
            (Some(i), _) => groups.insert(i + 1, new_group()),
            (None, "tab") if !groups.is_empty() => {
                if let Some(g) = groups.last_mut() {
                    g.tabs.push(panel.to_owned());
                    panel.clone_into(&mut g.current);
                }
            }
            (None, "before" | "above") => groups.insert(0, new_group()),
            (None, _) => groups.push(new_group()),
        }
        if let (Some(s), Some(first)) = (size, groups.first_mut()) {
            first.size = Some(s);
        }
        self.remembered.remove(panel);
        Ok(self.layout())
    }

    /// The arrangement in use (normal or full-screen).
    pub(crate) fn active_arrangement(&self) -> &Arrangement {
        self.active()
    }

    /// `window.toolbar.set`.
    ///
    /// # Errors
    /// `arg.invalid` for floating the main toolbar.
    pub(crate) fn set_toolbar(
        &mut self,
        id: &str,
        visible: bool,
        floating: Option<bool>,
        position: Option<Position>,
    ) -> Result<(), CommandError> {
        let a = self.active_mut();
        if id == "main" {
            if floating == Some(true) || position.is_some() {
                return Err(ui_error(
                    "arg.invalid",
                    "the main toolbar cannot float",
                    serde_json::json!({ "pointer": "/floating" }),
                ));
            }
            a.main_toolbar.visible = visible;
            return Ok(());
        }
        let p = a.tool_palettes.iter_mut().find(|p| p.id == id).ok_or_else(|| {
            ui_error("arg.invalid", format!("unknown toolbar `{id}`"), serde_json::json!({ "pointer": "/toolbarId" }))
        })?;
        p.visible = visible;
        if let Some(f) = floating {
            p.floating = f;
        }
        if position.is_some() {
            p.position = position;
        }
        Ok(())
    }

    /// `window.toolbar.items.set` (items validated by the command).
    pub(crate) fn set_toolbar_items(&mut self, items: Vec<String>) -> Vec<String> {
        self.active_mut().main_toolbar.items.clone_from(&items);
        items
    }

    /// `window.toolbar.items.reset`.
    pub(crate) fn reset_toolbar(&mut self) -> MainToolbar {
        let t = default_toolbar();
        self.active_mut().main_toolbar.items.clone_from(&t.items);
        self.active().main_toolbar.clone()
    }

    /// `window.fullScreen.set {enabled}` (01 §5.6): switches between the normal and the full-screen arrangement.
    pub fn set_full_screen(&mut self, enabled: bool) {
        self.full_screen = enabled;
    }

    /// `window.floatingPanels.set {visible}` (01 §5.4).
    pub fn set_floating_visible(&mut self, visible: bool) {
        self.floating_hidden = !visible;
    }

    /// Whether floating panels and palettes are shown.
    #[must_use]
    pub fn floating_visible(&self) -> bool {
        !self.floating_hidden
    }

    /// `window.sidePanelsExtended.set {enabled}` (01 §5.4).
    pub fn set_side_panels_extended(&mut self, enabled: bool) {
        self.active_mut().corners = if enabled { Corners::Sides } else { Corners::TopBottom };
    }

    /// Whether side panels own the corners.
    #[must_use]
    pub fn side_panels_extended(&self) -> bool {
        self.active().corners == Corners::Sides
    }

    /// `workspace.list`.
    #[must_use]
    pub fn workspaces(&self) -> Vec<WorkspaceEntry> {
        let (_, modified) = self.workspace_state();
        self.workspaces
            .iter()
            .enumerate()
            .map(|(i, w)| WorkspaceEntry {
                name: w.name.clone(),
                built_in: w.built_in,
                modified: w.name == self.current && modified,
                position: i + 1,
                current: w.name == self.current,
            })
            .collect()
    }

    fn not_found(name: &str) -> CommandError {
        ui_error("workspace.notFound", format!("no workspace `{name}`"), serde_json::json!({ "name": name }))
    }

    /// `workspace.get`: the current arrangement, or a stored workspace's layout.
    ///
    /// # Errors
    /// `workspace.notFound`.
    pub fn workspace_layout(&self, name: Option<&str>) -> Result<Layout, CommandError> {
        match name {
            None => Ok(self.layout()),
            Some(n) => self
                .workspaces
                .iter()
                .find(|w| w.name == n)
                .map(|w| Layout::from(&w.layout))
                .ok_or_else(|| Self::not_found(n)),
        }
    }

    /// `workspace.select`; saves the current arrangement first when asked (01 §5.5).
    ///
    /// # Errors
    /// `workspace.notFound`.
    pub fn select(&mut self, name: &str, save_current: bool) -> Result<(), CommandError> {
        // Validate the target first, then save the current arrangement, then read the target (which may be the
        // workspace just saved; C12-R1-16).
        self.workspace_layout(Some(name))?;
        if save_current {
            self.save(None)?;
        }
        let layout = self.workspace_layout(Some(name))?;
        self.current = name.to_owned();
        self.live = layout.into();
        Ok(())
    }

    /// `workspace.save`: stores the current arrangement into a workspace (default: the current one).
    ///
    /// # Errors
    /// `workspace.notFound`.
    pub fn save(&mut self, name: Option<&str>) -> Result<String, CommandError> {
        let name = name.unwrap_or(&self.current).to_owned();
        let live = self.live.clone();
        let w = self.workspaces.iter_mut().find(|w| w.name == name).ok_or_else(|| Self::not_found(&name))?;
        w.layout = live;
        Ok(name)
    }

    fn check_new_name(&self, name: &str) -> Result<String, CommandError> {
        let n = name.trim();
        if n.is_empty() || n.chars().count() > 128 || n.chars().any(char::is_control) {
            return Err(ui_error(
                "workspace.nameInvalid",
                "workspace names are 1–128 characters without control characters",
                serde_json::json!({ "name": name }),
            ));
        }
        if self.workspaces.iter().any(|w| w.name == n) {
            return Err(ui_error(
                "workspace.exists",
                format!("a workspace named `{n}` exists"),
                serde_json::json!({ "name": n }),
            ));
        }
        Ok(n.to_owned())
    }

    /// `workspace.create`.
    ///
    /// # Errors
    /// `workspace.nameInvalid`, `workspace.exists`, `arg.invalid` for a position out of range or an invalid layout.
    pub fn create(
        &mut self,
        name: &str,
        position: Option<usize>,
        layout: Option<Layout>,
    ) -> Result<(String, usize), CommandError> {
        let n = self.check_new_name(name)?;
        let layout: Arrangements = match layout {
            Some(l) => {
                let a = Arrangements::from(l);
                validate_layout(&a)?;
                a
            }
            None => self.live.clone(),
        };
        let at = position.unwrap_or(self.workspaces.len() + 1);
        if at == 0 || at > self.workspaces.len() + 1 {
            return Err(ui_error(
                "arg.invalid",
                format!("position must be 1–{}", self.workspaces.len() + 1),
                serde_json::json!({ "pointer": "/position" }),
            ));
        }
        self.workspaces.insert(at - 1, Workspace { name: n.clone(), built_in: false, bundled: None, layout });
        Ok((n, at))
    }

    /// `workspace.rename`.
    ///
    /// # Errors
    /// `workspace.notFound`, `workspace.nameInvalid`, `workspace.exists`.
    pub fn rename(&mut self, name: &str, new_name: &str) -> Result<String, CommandError> {
        if !self.workspaces.iter().any(|w| w.name == name) {
            return Err(Self::not_found(name));
        }
        if name == new_name.trim() {
            return Ok(name.to_owned());
        }
        let n = self.check_new_name(new_name)?;
        for w in &mut self.workspaces {
            if w.name == name {
                n.clone_into(&mut w.name);
            }
        }
        if self.current == name {
            n.clone_into(&mut self.current);
        }
        Ok(n)
    }

    /// `workspace.delete`: user workspaces only, never the last one; deleting the current one selects the first.
    ///
    /// # Errors
    /// `workspace.notFound`, `workspace.builtIn`, `workspace.last`.
    pub fn delete(&mut self, name: &str) -> Result<String, CommandError> {
        let w = self.workspaces.iter().find(|w| w.name == name).ok_or_else(|| Self::not_found(name))?;
        if w.built_in {
            return Err(ui_error(
                "workspace.builtIn",
                "built-in workspaces cannot be deleted",
                serde_json::json!({ "name": name }),
            ));
        }
        if self.workspaces.len() == 1 {
            return Err(ui_error(
                "workspace.last",
                "the last workspace cannot be deleted",
                serde_json::json!({ "name": name }),
            ));
        }
        self.workspaces.retain(|w| w.name != name);
        if self.current == name
            && let Some(first) = self.workspaces.first()
        {
            self.current = first.name.clone();
            self.live = first.layout.clone();
        }
        Ok(name.to_owned())
    }

    /// `workspace.reset`: restores built-in workspaces to their bundled arrangement; `select` applies it.
    ///
    /// # Errors
    /// `workspace.notFound`; `workspace.builtIn` is not an error here, but a user workspace has no bundled
    /// arrangement (`arg.invalid`).
    pub fn reset(&mut self, name: Option<&str>, all: bool, select: bool) -> Result<Vec<String>, CommandError> {
        // Built-in workspaces are identified by their bundled identity, so a renamed one is still restorable (01 §5.5).
        let bundled = built_in_workspaces();
        let targets: Vec<String> = if all {
            self.workspaces.iter().filter(|w| w.bundled.is_some()).map(|w| w.name.clone()).collect()
        } else {
            let n = name.unwrap_or(&self.current).to_owned();
            match self.workspaces.iter().find(|w| w.name == n) {
                Some(w) if w.bundled.is_some() => vec![n],
                Some(_) => {
                    return Err(ui_error(
                        "arg.invalid",
                        format!("`{n}` is a user workspace and has no bundled arrangement"),
                        serde_json::json!({ "pointer": "/name" }),
                    ));
                }
                None => return Err(Self::not_found(&n)),
            }
        };
        for w in self.workspaces.iter_mut().filter(|w| targets.contains(&w.name)) {
            if let Some(b) = w.bundled.as_ref().and_then(|id| bundled.iter().find(|b| b.bundled.as_ref() == Some(id))) {
                w.layout = b.layout.clone();
            }
        }
        if select && let Some(first) = targets.first() {
            self.select(first, false)?;
        }
        Ok(targets)
    }

    /// `workspace.layout.set`: applies a layout to the window without saving it.
    ///
    /// # Errors
    /// `arg.invalid` for an invalid layout.
    pub fn apply_layout(&mut self, layout: Layout) -> Result<Layout, CommandError> {
        let a = Arrangements::from(layout);
        validate_layout(&a)?;
        self.live = a;
        Ok(self.layout())
    }

    /// The workspaces as persisted per user (01 §11: user workspaces, modified built-in workspaces, the workspace to
    /// restore at launch).
    #[must_use]
    pub(crate) fn persisted(&self) -> serde_json::Value {
        let list: Vec<serde_json::Value> = self
            .workspaces
            .iter()
            .map(|w| {
                serde_json::json!({
                    "name": w.name,
                    "builtIn": w.built_in,
                    "bundled": w.bundled,
                    "layout": Layout::from(&w.layout),
                })
            })
            .collect();
        serde_json::json!({ "workspaces": list, "current": self.current })
    }

    /// Restores persisted workspaces (invalid entries are skipped; bundled workspaces missing from the file are added
    /// back). Returns whether anything was restored.
    pub(crate) fn restore(&mut self, v: &serde_json::Value) -> bool {
        let Some(list) = v.get("workspaces").and_then(serde_json::Value::as_array) else { return false };
        let mut restored: Vec<Workspace> = Vec::new();
        for w in list {
            let (Some(name), Some(layout)) = (
                w.get("name").and_then(serde_json::Value::as_str),
                w.get("layout").and_then(|l| serde_json::from_value::<Layout>(l.clone()).ok()),
            ) else {
                continue;
            };
            let a = Arrangements::from(layout);
            if validate_layout(&a).is_err() || name.trim().is_empty() || restored.iter().any(|x| x.name == name) {
                continue;
            }
            restored.push(Workspace {
                name: name.to_owned(),
                built_in: w.get("builtIn").and_then(serde_json::Value::as_bool).unwrap_or(false),
                bundled: w.get("bundled").and_then(serde_json::Value::as_str).map(str::to_owned),
                layout: a,
            });
        }
        if restored.is_empty() {
            return false;
        }
        for b in built_in_workspaces() {
            let bundled = b.bundled.clone().unwrap_or_else(|| b.name.clone());
            let present = restored
                .iter()
                .any(|w| w.bundled.as_deref() == Some(bundled.as_str()) || (w.built_in && w.name == b.name));
            if !present && !restored.iter().any(|w| w.name == b.name) {
                restored.push(b);
            }
        }
        let current = v
            .get("current")
            .and_then(serde_json::Value::as_str)
            .filter(|c| restored.iter().any(|w| w.name == *c))
            .map(str::to_owned)
            .or_else(|| restored.first().map(|w| w.name.clone()))
            .unwrap_or_default();
        if let Some(w) = restored.iter().find(|w| w.name == current) {
            self.live = w.layout.clone();
        }
        self.workspaces = restored;
        self.current = current;
        true
    }
}

/// A layout names known panels and palettes, each at most once per arrangement, a view as the central panel, and
/// non-empty tab groups whose `current` is one of their tabs.
fn validate_layout(l: &Arrangements) -> Result<(), CommandError> {
    for (which, a) in [("", &l.normal), ("/fullScreen", &l.full_screen)] {
        let err =
            |pointer: String, msg: String| ui_error("arg.invalid", msg, serde_json::json!({ "pointer": pointer }));
        if !VIEW_PANELS.contains(&a.central_panel.as_str()) {
            return Err(err(
                format!("/layout{which}/centralPanel"),
                format!("`{}` cannot be the central view", a.central_panel),
            ));
        }
        let mut seen = vec![a.central_panel.clone()];
        for area in AREAS {
            for (gi, g) in area_ref(&a.docks, area).iter().enumerate() {
                let at = format!("/layout{which}/docks/{area}/{gi}");
                if g.tabs.is_empty() || !g.tabs.contains(&g.current) {
                    return Err(err(at, "a tab group has tabs and its current tab is one of them".to_owned()));
                }
                for t in &g.tabs {
                    if !PANELS.iter().any(|(p, _)| p == t) {
                        return Err(err(at.clone(), format!("unknown panel `{t}`")));
                    }
                    if seen.contains(t) {
                        return Err(err(at.clone(), format!("panel `{t}` appears twice")));
                    }
                    seen.push(t.clone());
                }
            }
        }
        for (i, f) in a.floating.iter().enumerate() {
            if !PANELS.iter().any(|(p, _)| *p == f.panel_id) || seen.contains(&f.panel_id) {
                return Err(err(
                    format!("/layout{which}/floating/{i}"),
                    format!("panel `{}` is unknown or placed twice", f.panel_id),
                ));
            }
            seen.push(f.panel_id.clone());
        }
        for (i, p) in a.tool_palettes.iter().enumerate() {
            if !TOOL_PALETTES.contains(&p.id.as_str()) {
                return Err(err(
                    format!("/layout{which}/toolPalettes/{i}"),
                    format!("unknown tool palette `{}`", p.id),
                ));
            }
        }
    }
    Ok(())
}
