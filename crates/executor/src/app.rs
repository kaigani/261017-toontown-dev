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

/// The working mode (01 §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Drawing mode.
    Drawing,
    /// Staging (animation) mode.
    Staging,
}

/// A selection list, shared: copying the context (rollback, savepoints, observation) costs nothing however many
/// cells are selected; equal lists compare by pointer first (C31-R1-14).
pub(crate) type Ids = std::sync::Arc<Vec<String>>;

/// The selection of one context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    /// `drawing`, `staging`, …
    pub context: String,
    /// Selected object keys (plain or compound IDs), in selection order.
    pub ids: Ids,
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
    pub selections: std::collections::BTreeMap<String, Ids>,
    /// The Contour Editor's anchor selection (02 §7.1.2; `draw.pointSelection.set`): compound stroke keys
    /// (`<drawingId>/<strokeId>`) with anchor indices, in selection order.
    pub points: Vec<(String, u32)>,
    /// The drawings `points` holds anchors of (every `<drawingId>` prefix of its keys), kept with it by
    /// [`AppContext::set_points`] and repair: a precheck asks whether a drawing has selected anchors with one lookup
    /// (ruling O-24).
    pub point_drawings: std::collections::BTreeSet<String>,
    /// Tool memory of the session that is not document content (the Line tool's last end per drawing: `x,y`).
    pub tool_memory: std::collections::BTreeMap<String, String>,
    /// The current brush pen and eraser pen (02 §6.1; project scope, 02 §12.3: saved in the project's `session.json`).
    pub pens: CurrentPens,
    /// The current colour and its palette (03 §2.4; `colour.current.set`, ruling D-11; saved in `session.json`).
    pub colour: Option<(Id, Id)>,
}

/// The current pen of each tool family (02 §6.1: Brush and Eraser each remember their own).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CurrentPens {
    /// Current brush pen.
    pub brush: Option<Id>,
    /// Current eraser pen.
    pub eraser: Option<Id>,
}

/// Every prefix of the keys that ends before a `/` (`<drawingId>` of `<drawingId>/<localId>`, whatever the IDs
/// contain).
pub(crate) fn prefixes<'a>(keys: impl Iterator<Item = &'a str>) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for k in keys {
        for (i, _) in k.match_indices('/') {
            out.insert(k[..i].to_owned());
        }
    }
    out
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            scene: None,
            frame: 1,
            element: None,
            drawing: None,
            selection: Selection { context: "staging".to_owned(), ids: Ids::default() },
            selections: std::collections::BTreeMap::new(),
            points: Vec::new(),
            point_drawings: std::collections::BTreeSet::new(),
            tool_memory: std::collections::BTreeMap::new(),
            pens: CurrentPens::default(),
            colour: None,
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

/// A modal GUI state (reported by the desktop host, never set by a command: 11 P5).
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
    /// Modal state (reported by the desktop host; `null` headless).
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
    cell_ref_parts(k).map(|(e, f)| (e.to_owned(), f))
}

/// [`parse_cell_ref`] without allocating: the element part borrowed from `k`.
pub(crate) fn cell_ref_parts(k: &str) -> Option<(&str, u32)> {
    let rest = k.strip_prefix("element:@")?;
    let (element, frame) = rest.split_once("/cell:#")?;
    Some((element, frame.parse().ok()?))
}

/// Whether a selection entry (an object key or a cell reference) still exists.
/// Whether a stroke (compound key) exists and has the anchor.
fn anchor_exists(doc: &Document, k: &str, anchor: u32) -> bool {
    let Some((d, s)) = k.split_once('/') else { return false };
    let Ok(id) = Id::parse(d) else { return false };
    doc.drawing(&id).is_some_and(|dr| {
        tt_vector::drawing::locate(dr, s)
            .and_then(|l| tt_vector::drawing::stroke_at(dr, &l.pointer))
            .is_some_and(|st| (anchor as usize) < st.points().len())
    })
}

/// Whether a zone of a drawing exists, by its derived ID: `(drawing ID, zone ID)`.
pub type ZoneCheck<'a> = &'a dyn Fn(&str, &str) -> bool;

fn selectable_exists(doc: &Document, k: &str, zones: Option<ZoneCheck<'_>>) -> bool {
    // Derived zone IDs are not document objects: they are checked against the current planar maps (C23-R2-03).
    if let Some((d, z)) = k.split_once('/')
        && z.starts_with("zn_")
    {
        return zones.is_some_and(|check| check(d, z));
    }
    match parse_cell_ref(k) {
        Some((element, _)) => doc.locate(&element).is_some(),
        None => doc.locate(k).is_some(),
    }
}

impl AppContext {
    /// A copy without the (possibly large) selection lists: the reported selection keeps its context only.
    #[must_use]
    pub(crate) fn clone_without_selection_ids(&self) -> Self {
        Self {
            scene: self.scene.clone(),
            frame: self.frame,
            element: self.element.clone(),
            drawing: self.drawing.clone(),
            selection: Selection { context: self.selection.context.clone(), ids: Ids::default() },
            selections: std::collections::BTreeMap::new(),
            points: self.points.clone(),
            point_drawings: self.point_drawings.clone(),
            tool_memory: self.tool_memory.clone(),
            pens: self.pens.clone(),
            colour: self.colour.clone(),
        }
    }

    /// The selection of one context.
    #[must_use]
    pub fn selection_of(&self, context: &str) -> Vec<String> {
        self.selections.get(context).map(|v| v.as_ref().clone()).unwrap_or_default()
    }

    /// The selection of a context, borrowed (no copy of a large list).
    pub(crate) fn selection_list(&self, context: &str) -> &[String] {
        self.selections.get(context).map_or(&[], |v| v.as_slice())
    }

    /// The shared selection list of a context (its identity changes whenever its content does: lists are replaced,
    /// and repair's in-place edit reallocates a list that is also referenced elsewhere).
    pub(crate) fn selection_shared(&self, context: &str) -> Option<&Ids> {
        self.selections.get(context)
    }

    /// Replaces the anchor selection and its drawing index.
    pub(crate) fn set_points(&mut self, points: Vec<(String, u32)>) {
        self.points = points;
        self.point_drawings = prefixes(self.points.iter().map(|(k, _)| k.as_str()));
    }

    /// Whether the anchor selection holds anchors of `drawing` (one lookup).
    pub(crate) fn has_points_in(&self, drawing: &str) -> bool {
        self.point_drawings.contains(drawing)
    }

    /// Sets the selection of a context and makes it the reported one.
    pub fn set_selection(&mut self, context: &str, ids: Vec<String>) {
        context.clone_into(&mut self.selection.context);
        let ids = Ids::new(ids);
        self.selection.ids = ids.clone();
        self.selections.insert(context.to_owned(), ids);
    }

    /// Sets the selection of a context without making it the reported one (a mirrored context).
    pub fn mirror_selection(&mut self, context: &str, from: &str) {
        if let Some(ids) = self.selections.get(from).cloned() {
            self.selections.insert(context.to_owned(), ids);
        }
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
        self.repair_with(doc, None)
    }

    /// [`AppContext::repair`], keeping selected zones that still exist in the current planar maps (`zones`; without
    /// it, zone references are dropped).
    pub fn repair_with(&mut self, doc: &Document, zones: Option<ZoneCheck<'_>>) -> bool {
        self.repair_checked(doc, zones, true)
    }

    /// [`AppContext::repair_with`]; `selections: false` skips the selection lists (the caller knows neither they nor
    /// the document changed since they were last checked).
    pub fn repair_checked(&mut self, doc: &Document, zones: Option<ZoneCheck<'_>>, selections: bool) -> bool {
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
        // Cell references of one element share its existence check, parsed without allocating (a 100 000-cell
        // selection is one lookup per element per call, not one per cell; C31-R1-14).
        let mut elements: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
        let mut last: Option<(String, bool)> = None;
        let mut exists = |k: &str| match cell_ref_parts(k) {
            Some((element, _)) => match &last {
                Some((e, ok)) if e == element => *ok,
                _ => {
                    let ok = match elements.get(element) {
                        Some(ok) => *ok,
                        None => {
                            let ok = doc.locate(element).is_some();
                            elements.insert(element.to_owned(), ok);
                            ok
                        }
                    };
                    last = Some((element.to_owned(), ok));
                    ok
                }
            },
            None => selectable_exists(doc, k, zones),
        };
        // A list is copied only when one of its entries is gone (shared lists stay shared), and a list shared by
        // several contexts (the reported selection, its context, a mirror) is checked once.
        let mut whole: Vec<Ids> = Vec::new();
        let mut check = |ids: &mut Ids| {
            if whole.iter().any(|w| std::sync::Arc::ptr_eq(w, ids)) {
                return;
            }
            if !ids.iter().all(|k| exists(k)) {
                std::sync::Arc::make_mut(ids).retain(|k| exists(k));
            }
            whole.push(ids.clone());
        };
        if selections {
            check(&mut self.selection.ids);
            for ids in self.selections.values_mut() {
                check(ids);
            }
        }
        self.points.retain(|(k, anchor)| anchor_exists(doc, k, *anchor));
        if self.points.len() != before.points.len() {
            self.point_drawings = prefixes(self.points.iter().map(|(k, _)| k.as_str()));
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
            start_screen_visible: ui.shell().start_screen_visible(project.as_ref().map(|(_, info)| info.id.as_str())),
            // The modal state the desktop host reports (M4 C4.1; never set by a command, 11 P5).
            modal: ui.shell().sensors.modal.as_ref().map(|m| Modal { kind: m.kind.clone(), title: m.title.clone() }),
            agent_sessions: Vec::new(),
            agent: None,
            full_screen: ui.full_screen(),
        };
        let Some((doc, info)) = project else { return st };
        st.project = Some(info);
        // 01 §4: the mode is derived from the type of the active view window (M4 C4.1: the one focused last while
        // shown, else the central view of the arrangement in use).
        st.mode = Some(ui.mode());
        let active = ui.active_view().to_owned();
        st.active_view = Some(ActiveView {
            view_id: active.clone(),
            ty: ui.active_view_type().as_str().to_owned(),
            panel_id: active,
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
        st.selection.ids = self.selection.ids.as_ref().clone();
        let mut kinds: Vec<String> = Vec::new();
        for k in self.selection.ids.iter() {
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
