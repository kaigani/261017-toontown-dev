//! The application context the kernel tracks for commands and agents (01 §12.4 `AppState`, §4; 11 §5.1 special
//! references): current scene, frame, element and drawing, the working mode and the selection. It is session state,
//! never document state (10 §2.7 transient), and it follows the document: when a change removes the current scene or
//! element, or a selected object, the context is repaired after the transaction.

use schemars::JsonSchema;
use serde::Serialize;
use tt_model::document::{Document, ResolveContext, Target};
use tt_model::path::SpecialRef;
use tt_model::{Id, ObjectType};

use crate::ui::UiState;

/// The mode a view type implies (01 §4): a drawing view is drawing mode; camera, top and side views are staging.
#[must_use]
pub fn mode_of_view(panel: &str) -> Mode {
    if panel == "drawingView" { Mode::Drawing } else { Mode::Staging }
}

/// The working mode (01 §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Drawing mode.
    Drawing,
    /// Staging (animation) mode.
    Staging,
}

/// The selection of one context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    /// `drawing`, `staging`, …
    pub context: String,
    /// Selected object keys (plain or compound IDs), in selection order.
    pub ids: Vec<String>,
}

/// The current context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppContext {
    pub scene: Option<Id>,
    pub frame: u32,
    pub element: Option<Id>,
    /// A drawing made current explicitly (`app.context.set {drawingId}`); otherwise the current drawing is the one
    /// the current element exposes at the current frame.
    pub drawing: Option<Id>,
    /// The selection of the context used last (reported by `AppState`).
    pub selection: Selection,
    /// The selection of every context (01 §6.4: each context keeps its own; switching focus clears none).
    pub selections: std::collections::BTreeMap<String, Vec<String>>,
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            scene: None,
            frame: 1,
            element: None,
            drawing: None,
            selection: Selection { context: "staging".to_owned(), ids: Vec::new() },
            selections: std::collections::BTreeMap::new(),
        }
    }
}

/// `{id, name, frameCount}` of a scene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneRef {
    /// Scene id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Scene length (derived, 10 §4.3).
    pub frame_count: u64,
}

/// `{id, name, type}` of an element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ElementRef {
    /// Element id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Element type (01 §12.4 reports `elementType` as `type`).
    #[serde(rename = "type")]
    pub ty: String,
}

/// `{id, name}` of a drawing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct DrawingRef {
    /// Drawing id.
    pub id: String,
    /// Name.
    pub name: String,
}

/// The open project in `AppState`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    /// Project id.
    pub id: String,
    /// Name.
    pub name: String,
    /// File path (`null` for an unsaved project).
    pub path: Option<String>,
    /// Modified since the last save.
    pub dirty: bool,
    /// The global library has unsaved changes (08; always `false` until M9).
    pub global_library_dirty: bool,
    /// Opened read-only.
    pub read_only: bool,
}

/// The active view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveView {
    /// View id (the kernel has one view per view panel; ids equal panel ids until M4).
    pub view_id: String,
    /// Panel id.
    pub panel_id: String,
    /// `camera`, `drawing`, `top`, `side`.
    #[serde(rename = "type")]
    pub ty: String,
}

/// The selection in `AppState`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SelectionState {
    /// Selection context.
    pub context: String,
    /// Selected ids.
    pub ids: Vec<String>,
    /// Object types present in the selection.
    pub kinds: Vec<String>,
}

/// The workspace in `AppState`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WorkspaceState {
    /// Current workspace.
    pub name: String,
    /// The arrangement differs from the stored one.
    pub modified: bool,
}

/// Playback in `AppState` (M5 drives it; the kernel reports stopped).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PlaybackState {
    /// Playing.
    pub playing: bool,
}

/// An agent session in `AppState` (C1.4 fills the list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionState {
    /// Session id.
    pub session_id: String,
    /// Client display name.
    pub name: String,
    /// Currently running a command.
    pub acting: bool,
}

/// The active tool (M4 C4.2 selects tools; `null` until then).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTool {
    /// Tool id.
    pub tool_id: String,
}

/// A modal GUI state (never set by the kernel: 11 P5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Modal {
    /// `dialog`, `gesture` or `progress`.
    pub kind: String,
    /// Title.
    pub title: String,
}

/// `AppState` (01 §12.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    /// Open project or `null`.
    pub project: Option<ProjectRef>,
    /// Current scene.
    pub scene: Option<SceneRef>,
    /// Current frame (`null` without a scene).
    pub frame: Option<u32>,
    /// Current element.
    pub element: Option<ElementRef>,
    /// Current drawing.
    pub drawing: Option<DrawingRef>,
    /// Working mode (`null` without a project).
    pub mode: Option<Mode>,
    /// Active view.
    pub active_view: Option<ActiveView>,
    /// Focused panel.
    pub focused_panel: Option<String>,
    /// Active tool.
    pub active_tool: Option<ActiveTool>,
    /// Selection.
    pub selection: SelectionState,
    /// Workspace.
    pub workspace: WorkspaceState,
    /// Playback.
    pub playback: PlaybackState,
    /// Start screen shown (no project).
    pub start_screen_visible: bool,
    /// Modal state (always `null` in the kernel).
    pub modal: Option<Modal>,
    /// Connected agent sessions.
    pub agent_sessions: Vec<AgentSessionState>,
    /// Full-screen presentation (01 §5.6; explicit `enabled` state, readiness bullet 4).
    pub full_screen: bool,
    /// The calling agent session (11 §7.1 `get_state`: "session scopes and policy"); absent for the user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<CallerAgentState>,
}

/// The calling agent session in `AppState` (C14-R1-16: one state shape for `get_state` and `app.state.get`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallerAgentState {
    /// Session id.
    pub session_id: String,
    /// Client name.
    pub client: String,
    /// Policy preset.
    pub policy: String,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Paused by the user.
    pub paused: bool,
    /// Session limits (11 §6.5).
    pub limits: serde_json::Value,
}

fn first_element(doc: &Document, scene: &Id) -> Option<Id> {
    doc.scene(scene).and_then(|s| doc.tree_order(s).into_iter().next())
}

/// A cell selection entry: `element:@<elementId>/cell:#<frame>` (a reference that resolves to the cell).
#[must_use]
pub fn cell_ref(element: &str, frame: u32) -> String {
    format!("element:@{element}/cell:#{frame}")
}

/// The element and frame of a cell selection entry.
#[must_use]
pub fn parse_cell_ref(k: &str) -> Option<(String, u32)> {
    let rest = k.strip_prefix("element:@")?;
    let (element, frame) = rest.split_once("/cell:#")?;
    Some((element.to_owned(), frame.parse().ok()?))
}

/// Whether a selection entry (an object key or a cell reference) still exists.
fn selectable_exists(doc: &Document, k: &str) -> bool {
    match parse_cell_ref(k) {
        Some((element, _)) => doc.locate(&element).is_some(),
        None => doc.locate(k).is_some(),
    }
}

impl AppContext {
    /// The selection of one context.
    #[must_use]
    pub fn selection_of(&self, context: &str) -> Vec<String> {
        self.selections.get(context).cloned().unwrap_or_default()
    }

    /// Sets the selection of a context and makes it the reported one.
    pub fn set_selection(&mut self, context: &str, ids: Vec<String>) {
        context.clone_into(&mut self.selection.context);
        self.selection.ids.clone_from(&ids);
        self.selections.insert(context.to_owned(), ids);
    }

    /// The context for a newly opened document: its first scene, frame 1, the scene's first element.
    pub fn for_document(doc: &Document) -> Self {
        let scene = doc.project().scenes().ids().first().cloned();
        let element = scene.as_ref().and_then(|s| first_element(doc, s));
        Self { scene, element, ..Self::default() }
    }

    /// Repairs the context after a document change: a removed scene falls back to the first scene, a removed or
    /// foreign element to the scene's first element, a drawing outside the element's bank is cleared, selected
    /// objects that no longer exist are dropped. Returns whether anything changed.
    pub fn repair(&mut self, doc: &Document) -> bool {
        let before = self.clone();
        if self.scene.as_ref().is_none_or(|s| doc.scene(s).is_none()) {
            self.scene = doc.project().scenes().ids().first().cloned();
            self.element = None;
        }
        if let Some(e) = &self.element
            && (doc.element(e).is_none() || doc.scene_of(e) != self.scene.as_ref())
        {
            self.element = None;
        }
        if self.element.is_none() {
            self.element = self.scene.as_ref().and_then(|s| first_element(doc, s));
        }
        if let Some(d) = &self.drawing
            && !self.drawing_belongs(doc, d)
        {
            self.drawing = None;
        }
        self.frame = self.frame.max(1);
        self.selection.ids.retain(|k| selectable_exists(doc, k));
        for ids in self.selections.values_mut() {
            ids.retain(|k| selectable_exists(doc, k));
        }
        *self != before
    }

    fn drawing_belongs(&self, doc: &Document, drawing: &Id) -> bool {
        let bank = self.element.as_ref().and_then(|e| doc.element(e)).and_then(|e| e.drawing_bank_id().clone());
        bank.is_some() && doc.bank_of(drawing) == bank.as_ref()
    }

    /// The drawing exposed by `element` at `frame` (cell-based elements).
    pub fn exposed_drawing(doc: &Document, element: &Id, frame: u32) -> Option<Id> {
        let e = doc.element(element)?;
        let runs = e.exposure().as_ref()?;
        let mut at = 1u64;
        for (drawing, count) in runs {
            let end = at + u64::from(*count);
            if u64::from(frame) < end {
                return drawing.clone();
            }
            at = end;
        }
        None
    }

    /// The current drawing: explicit, else the current element's exposure at the current frame.
    pub fn current_drawing(&self, doc: &Document) -> Option<Id> {
        if let Some(d) = &self.drawing
            && self.drawing_belongs(doc, d)
        {
            return Some(d.clone());
        }
        Self::exposed_drawing(doc, self.element.as_ref()?, self.frame)
    }

    /// `AppState` (01 §12.4).
    pub fn state(&self, project: Option<(&Document, ProjectRef)>, ui: &UiState) -> AppState {
        let (workspace, modified) = ui.workspace_state();
        let focused = ui.focused().map(str::to_owned);
        let mut st = AppState {
            project: None,
            scene: None,
            frame: None,
            element: None,
            drawing: None,
            mode: None,
            active_view: None,
            focused_panel: focused,
            active_tool: None,
            selection: SelectionState { context: self.selection.context.clone(), ids: Vec::new(), kinds: Vec::new() },
            workspace: WorkspaceState { name: workspace, modified },
            playback: PlaybackState { playing: false },
            start_screen_visible: project.is_none(),
            modal: None,
            agent_sessions: Vec::new(),
            agent: None,
            full_screen: ui.full_screen(),
        };
        let Some((doc, info)) = project else { return st };
        st.project = Some(info);
        // 01 §4: the mode is derived from the type of the active view (the central view of the arrangement in use).
        st.mode = Some(mode_of_view(ui.central_panel()));
        let central = ui.central_panel().to_owned();
        st.active_view = Some(ActiveView {
            view_id: central.clone(),
            ty: central.trim_end_matches("View").to_owned(),
            panel_id: central,
        });
        if let Some(s) = self.scene.as_ref().and_then(|s| doc.scene(s)) {
            st.scene = Some(SceneRef {
                id: s.id().as_str().to_owned(),
                name: s.name().clone(),
                frame_count: doc.scene_length(s),
            });
            st.frame = Some(self.frame);
        }
        if let Some(e) = self.element.as_ref().and_then(|e| doc.element(e)) {
            st.element = Some(ElementRef {
                id: e.id().as_str().to_owned(),
                name: e.name().clone(),
                ty: e.element_type().as_str().to_owned(),
            });
        }
        if let Some(d) = self.current_drawing(doc).and_then(|d| doc.drawing(&d)) {
            st.drawing = Some(DrawingRef { id: d.id().as_str().to_owned(), name: d.name().clone() });
        }
        st.selection.ids.clone_from(&self.selection.ids);
        let mut kinds: Vec<String> = Vec::new();
        for k in &self.selection.ids {
            if let Some(l) = doc.locate(k) {
                let t = l.ty.as_str().to_owned();
                if !kinds.contains(&t) {
                    kinds.push(t);
                }
            }
        }
        st.selection.kinds = kinds;
        st
    }
}

/// The special-reference context of one invocation (11 §5.1; ADR-0013 §6).
///
/// - `@current` as the first path segment is the current element when there is one, else the current scene (so
///   `@current/drawing:"3"` names a drawing of the current element and `@current/layer:#1` is not valid). A bare
///   `@current` given to a parameter that expects a type resolves to the current object of that type
///   ([`SpecialContext::current_of`]).
/// - `@selection` is the selected object when exactly one is selected; list parameters expand it to every selected
///   object.
/// - `@activeView` is the object the central view shows: the current drawing for the drawing view, else the current
///   scene.
/// - `@activeCamera` is the current scene's current camera.
pub(crate) struct SpecialContext<'a> {
    pub app: &'a AppContext,
    pub ui: &'a UiState,
    pub doc: &'a Document,
}

fn object(ty: ObjectType, id: &Id) -> Target {
    Target::Object { ty, key: id.as_str().to_owned() }
}

impl SpecialContext<'_> {
    /// The current object of a type: scene, element, drawing, camera (an element), project.
    pub fn current_of(&self, ty: ObjectType) -> Option<Target> {
        match ty {
            ObjectType::Scene => self.app.scene.as_ref().map(|s| object(ObjectType::Scene, s)),
            ObjectType::Element => self.app.element.as_ref().map(|e| object(ObjectType::Element, e)),
            ObjectType::Drawing => self.app.current_drawing(self.doc).map(|d| object(ObjectType::Drawing, &d)),
            ObjectType::Project => Some(object(ObjectType::Project, self.doc.project().id())),
            _ => None,
        }
    }
}

impl ResolveContext for SpecialContext<'_> {
    fn special(&self, r: SpecialRef) -> Option<Target> {
        match r {
            SpecialRef::Current => self.current_of(ObjectType::Element).or_else(|| self.current_of(ObjectType::Scene)),
            SpecialRef::Selection => match self.app.selection.ids.as_slice() {
                [only] => self.doc.locate(only).map(|l| Target::Object { ty: l.ty, key: l.key.clone() }),
                _ => None,
            },
            SpecialRef::ActiveView => {
                if self.ui.central_panel() == "drawingView" {
                    self.current_of(ObjectType::Drawing)
                } else {
                    self.current_of(ObjectType::Scene)
                }
            }
            SpecialRef::ActiveCamera => {
                let scene = self.app.scene.as_ref().and_then(|s| self.doc.scene(s))?;
                scene.current_camera_id().as_ref().map(|c| object(ObjectType::Element, c))
            }
        }
    }
}
