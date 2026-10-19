//! `window.*` (01 §5.2–§5.7, §12.1) at the kernel level: panel visibility, docking, focus, floating panels,
//! corners, the full-screen arrangement and the main toolbar are queryable and settable state the desktop shell
//! renders (readiness bullet 4). Native window geometry belongs to the shell (M4 C4.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tt_registry::{CommandError, command};

use super::common::NoParams;
use crate::cx::Cx;
use crate::ui::{Layout, MainToolbar, PanelEntry, TOOL_PALETTES, TOOLBAR_GROUPS};

/// `{enabled}` parameters and result.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Enabled {
    /// The state to set (explicit, never a toggle; readiness bullet 4).
    enabled: bool,
}

#[command(
    id = "window.fullScreen.set",
    kind = toggle,
    label = "Full Screen",
    scopes = [ui],
    example(title = "Enter full screen", params = r#"{"enabled": true}"#),
    example(title = "Leave full screen", params = r#"{"enabled": false}"#),
)]
pub(crate) fn window_full_screen_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    cx.ui_mut().set_full_screen(p.enabled);
    Ok(Enabled { enabled: cx.ui().full_screen() })
}

#[command(
    id = "window.panel.list",
    kind = query,
    label = "List panels",
    scopes = [read],
    example(title = "Every panel with its visibility, place and focus", params = "{}"),
    agent_only = "the GUI shows the panels themselves",
)]
pub(crate) fn window_panel_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<PanelEntry>, CommandError> {
    Ok(cx.ui().panels())
}

/// `window.panel.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PanelSetParams {
    /// Panel id (01 §5.2) or `active` for the focused panel.
    panel_id: String,
    /// Show or hide.
    #[serde(default)]
    visible: Option<bool>,
    /// Float or dock.
    #[serde(default)]
    floating: Option<bool>,
    /// Give it focus.
    #[serde(default = "yes")]
    focus: bool,
    /// Floating geometry.
    #[serde(default)]
    geometry: Option<crate::ui::Geometry>,
}

fn yes() -> bool {
    true
}

#[command(
    id = "window.panel.set",
    kind = toggle,
    label = "Show/hide panel",
    scopes = [ui],
    example(title = "Show and focus the History panel", params = r#"{"panelId": "history", "visible": true}"#),
    example(title = "Float the Colour panel", params = r#"{"panelId": "colour", "floating": true, "geometry": {"x": 100, "y": 100, "width": 320, "height": 480}}"#),
    gui = "Windows menu and panel tab menus (M4)",
)]
pub(crate) fn window_panel_set(cx: &mut Cx, p: PanelSetParams) -> Result<PanelEntry, CommandError> {
    cx.ui_mut().set_panel(&p.panel_id, p.visible, p.floating, p.focus, p.geometry)
}

/// Dock area.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Area {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Top.
    Top,
    /// Bottom.
    Bottom,
    /// The central view.
    Central,
}

/// Placement relative to `relativeTo`.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Placement {
    /// As a tab.
    #[default]
    Tab,
    /// Before.
    Before,
    /// After.
    After,
    /// Above.
    Above,
    /// Below.
    Below,
}

/// `window.panel.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PanelMoveParams {
    /// Panel id.
    panel_id: String,
    /// Target area.
    area: Area,
    /// A panel docked in the area.
    #[serde(default)]
    relative_to: Option<String>,
    /// Placement.
    #[serde(default)]
    position: Placement,
    /// Area width or height.
    #[serde(default)]
    #[schemars(range(min = 1, max = 32767))]
    size: Option<u32>,
}

#[command(
    id = "window.panel.move",
    kind = ui,
    label = "Move panel",
    scopes = [ui],
    example(title = "Dock the History panel as a tab next to Properties", params = r#"{"panelId": "history", "area": "right", "relativeTo": "properties", "position": "tab"}"#),
    gui = "Dragging panel tabs (M4)",
)]
pub(crate) fn window_panel_move(cx: &mut Cx, p: PanelMoveParams) -> Result<Layout, CommandError> {
    let area = match p.area {
        Area::Left => "left",
        Area::Right => "right",
        Area::Top => "top",
        Area::Bottom => "bottom",
        Area::Central => "central",
    };
    let position = match p.position {
        Placement::Tab => "tab",
        Placement::Before => "before",
        Placement::After => "after",
        Placement::Above => "above",
        Placement::Below => "below",
    };
    cx.ui_mut().move_panel(&p.panel_id, area, p.relative_to.as_deref(), position, p.size)
}

/// `{visible}` parameters and result.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Visible {
    /// Show (true) or hide (false).
    visible: bool,
}

#[command(
    id = "window.floatingPanels.set",
    kind = toggle,
    label = "Show/hide floating panels",
    scopes = [ui],
    example(title = "Hide floating panels and palettes", params = r#"{"visible": false}"#),
    gui = "Windows menu (M4)",
)]
pub(crate) fn window_floating_panels_set(cx: &mut Cx, p: Visible) -> Result<Visible, CommandError> {
    cx.ui_mut().set_floating_visible(p.visible);
    Ok(Visible { visible: cx.ui().floating_visible() })
}

#[command(
    id = "window.sidePanelsExtended.set",
    kind = toggle,
    label = "Extend side panels",
    scopes = [ui],
    example(title = "Let the side areas run full height", params = r#"{"enabled": true}"#),
    gui = "Windows menu (M4)",
)]
pub(crate) fn window_side_panels_extended_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    cx.ui_mut().set_side_panels_extended(p.enabled);
    Ok(Enabled { enabled: cx.ui().side_panels_extended() })
}

/// A toolbar as listed (01 §5.7).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolbarEntry {
    /// `main` or a tool palette id.
    toolbar_id: String,
    /// Shown.
    visible: bool,
    /// Floating.
    floating: bool,
    /// Floating position.
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<crate::ui::Position>,
    /// Items (main toolbar).
    items: Vec<String>,
    /// Items that may be placed (main toolbar).
    allowed_items: Vec<String>,
}

/// Allowed main-toolbar items: the group ids and spacers, and every registered command that is not an event.
fn allowed_items() -> Vec<String> {
    let mut v: Vec<String> = TOOLBAR_GROUPS.iter().map(|s| (*s).to_owned()).collect();
    if let Ok(r) = crate::registry() {
        v.extend(r.entries().filter(|e| e.spec.kind != tt_registry::Kind::Event).map(|e| e.id().to_owned()));
    }
    v
}

fn toolbars(cx: &Cx) -> Vec<ToolbarEntry> {
    let a = cx.ui().active_arrangement();
    let mut out = vec![ToolbarEntry {
        toolbar_id: "main".into(),
        visible: a.main_toolbar.visible,
        floating: false,
        position: None,
        items: a.main_toolbar.items.clone(),
        allowed_items: allowed_items(),
    }];
    for p in &a.tool_palettes {
        out.push(ToolbarEntry {
            toolbar_id: p.id.clone(),
            visible: p.visible,
            floating: p.floating,
            position: p.position,
            items: Vec::new(),
            allowed_items: Vec::new(),
        });
    }
    out
}

#[command(
    id = "window.toolbar.list",
    kind = query,
    label = "List toolbars",
    scopes = [read],
    example(title = "The main toolbar and the tool palettes", params = "{}"),
    agent_only = "the GUI shows the toolbars themselves",
)]
pub(crate) fn window_toolbar_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<ToolbarEntry>, CommandError> {
    Ok(toolbars(cx))
}

/// Toolbar ids.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ToolbarId {
    /// The main toolbar.
    Main,
    /// Drawing tools palette.
    DrawingTools,
    /// Animation tools palette.
    AnimationTools,
    /// Bone tools palette.
    BoneTools,
    /// Effect tools palette.
    EffectTools,
    /// Sound tools palette.
    SoundTools,
}

impl ToolbarId {
    fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::DrawingTools => TOOL_PALETTES[0],
            Self::AnimationTools => TOOL_PALETTES[1],
            Self::BoneTools => TOOL_PALETTES[2],
            Self::EffectTools => TOOL_PALETTES[3],
            Self::SoundTools => TOOL_PALETTES[4],
        }
    }
}

/// `window.toolbar.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ToolbarSetParams {
    /// The toolbar.
    toolbar_id: ToolbarId,
    /// Shown.
    visible: bool,
    /// Floating (tool palettes).
    #[serde(default)]
    floating: Option<bool>,
    /// Floating position.
    #[serde(default)]
    position: Option<crate::ui::Position>,
}

#[command(
    id = "window.toolbar.set",
    kind = toggle,
    label = "Show/hide toolbar",
    scopes = [ui],
    example(title = "Hide the bone tools", params = r#"{"toolbarId": "boneTools", "visible": false}"#),
    gui = "Windows › Toolbars menu (M4)",
)]
pub(crate) fn window_toolbar_set(cx: &mut Cx, p: ToolbarSetParams) -> Result<ToolbarEntry, CommandError> {
    let id = p.toolbar_id.as_str();
    cx.ui_mut().set_toolbar(id, p.visible, p.floating, p.position)?;
    toolbars(cx).into_iter().find(|t| t.toolbar_id == id).ok_or_else(|| CommandError::internal("toolbar"))
}

/// `window.toolbar.items.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ToolbarItemsParams {
    /// The main toolbar.
    #[serde(default)]
    toolbar_id: MainOnly,
    /// Items: command references, group ids, `separator`, `space`, `flexibleSpace`.
    #[schemars(length(max = 200))]
    items: Vec<String>,
}

/// Only the main toolbar is configurable.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MainOnly {
    /// The main toolbar.
    #[default]
    Main,
}

/// `{items}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ItemsResult {
    /// The toolbar's items.
    items: Vec<String>,
}

#[command(
    id = "window.toolbar.items.set",
    kind = ui,
    label = "Customise toolbar",
    scopes = [ui],
    example(title = "Grid and playback only", params = r#"{"items": ["drawAid.grid.set", "separator", "playStop", "playback.loop.set"]}"#),
    gui = "Toolbar customisation sheet (M4)",
)]
pub(crate) fn window_toolbar_items_set(cx: &mut Cx, p: ToolbarItemsParams) -> Result<ItemsResult, CommandError> {
    let _ = p.toolbar_id;
    let registry = crate::registry().map_err(|e| CommandError::internal(e.0))?;
    for (i, it) in p.items.iter().enumerate() {
        let ok = TOOLBAR_GROUPS.contains(&it.as_str())
            || registry.get(it).is_some_and(|e| e.spec.kind != tt_registry::Kind::Event);
        if !ok {
            return Err(CommandError::with_details(
                tt_registry::codes::ARG_INVALID,
                format!("`{it}` is neither a registered command nor a toolbar group"),
                serde_json::json!({ "pointer": format!("/items/{i}") }),
            )
            .hint("window.toolbar.list lists allowedItems"));
        }
    }
    let items = cx.ui_mut().set_toolbar_items(p.items);
    Ok(ItemsResult { items })
}

/// `window.toolbar.items.reset` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ToolbarResetParams {
    /// The main toolbar.
    #[serde(default)]
    toolbar_id: MainOnly,
}

#[command(
    id = "window.toolbar.items.reset",
    kind = ui,
    label = "Reset toolbar",
    scopes = [ui],
    example(title = "Restore the default main toolbar", params = "{}"),
    gui = "Toolbar customisation sheet (M4)",
)]
pub(crate) fn window_toolbar_items_reset(cx: &mut Cx, p: ToolbarResetParams) -> Result<ItemsResult, CommandError> {
    let _ = p.toolbar_id;
    let MainToolbar { items, .. } = cx.ui_mut().reset_toolbar();
    Ok(ItemsResult { items })
}

/// Window geometry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowGeometry {
    /// Left.
    x: i32,
    /// Top.
    y: i32,
    /// Width.
    width: u32,
    /// Height.
    height: u32,
    /// Maximised.
    maximized: bool,
    /// Full screen.
    full_screen: bool,
    /// Screens.
    screens: Vec<Screen>,
}

/// A screen.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Screen {
    /// Left.
    x: i32,
    /// Top.
    y: i32,
    /// Width.
    width: u32,
    /// Height.
    height: u32,
    /// Scale factor.
    scale: f64,
}

#[command(
    id = "window.geometry.get",
    kind = query,
    label = "Window geometry",
    scopes = [read],
    not_implemented = "M4 C4.1",
    example(title = "Position, size and screens of the main window", params = "{}"),
    agent_only = "native window state read by agents",
)]
pub(crate) fn window_geometry_get(_cx: &mut Cx, _p: NoParams) -> Result<WindowGeometry, CommandError> {
    Err(CommandError::not_implemented("window.geometry.get", "M4 C4.1"))
}

/// `window.geometry.set` parameters.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GeometrySetParams {
    /// Left.
    #[serde(default)]
    x: Option<i32>,
    /// Top.
    #[serde(default)]
    y: Option<i32>,
    /// Width.
    #[serde(default)]
    #[schemars(range(min = 1, max = 32767))]
    width: Option<u32>,
    /// Height.
    #[serde(default)]
    #[schemars(range(min = 1, max = 32767))]
    height: Option<u32>,
    /// Maximised.
    #[serde(default)]
    maximized: Option<bool>,
}

#[command(
    id = "window.geometry.set",
    kind = ui,
    label = "Set window geometry",
    scopes = [ui],
    not_implemented = "M4 C4.1",
    example(title = "Resize the main window", params = r#"{"width": 1600, "height": 1000}"#),
    agent_only = "native window state set by agents; users drag the window",
)]
pub(crate) fn window_geometry_set(_cx: &mut Cx, _p: GeometrySetParams) -> Result<WindowGeometry, CommandError> {
    Err(CommandError::not_implemented("window.geometry.set", "M4 C4.1"))
}

/// `{minimized}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Minimized {
    /// Minimised.
    minimized: bool,
}

#[command(
    id = "window.minimized.set",
    kind = toggle,
    label = "Minimise",
    scopes = [ui],
    not_implemented = "M4 C4.1",
    example(title = "Minimise the main window", params = r#"{"minimized": true}"#),
    gui = "Window menu (M4)",
)]
pub(crate) fn window_minimized_set(_cx: &mut Cx, _p: Minimized) -> Result<Minimized, CommandError> {
    Err(CommandError::not_implemented("window.minimized.set", "M4 C4.1"))
}

#[command(
    id = "window.raiseAll.run",
    kind = ui,
    label = "Bring All to Front",
    scopes = [ui],
    not_implemented = "M4 C4.1",
    example(title = "Raise every application window", params = "{}"),
    gui = "Window menu (M4)",
)]
pub(crate) fn window_raise_all_run(_cx: &mut Cx, _p: NoParams) -> Result<super::common::Empty, CommandError> {
    Err(CommandError::not_implemented("window.raiseAll.run", "M4 C4.1"))
}
