//! `app.*` (01 §12.1, §4): application state and the current context.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes, command};

use super::common::{Direction, NoParams, OnUnsaved, event_not_invocable, invalid};
use crate::app::{AppContext, AppState, Mode, ProjectRef, mode_of_view};
use crate::cx::Cx;
use crate::rules;
use crate::session::Session;

/// `AppState` of a session (also used to detect `app.state.changed`).
pub(crate) fn app_state(s: &Session) -> AppState {
    let project = s.project.as_ref().map(|p| {
        let info = ProjectRef {
            id: p.doc.project().id().as_str().to_owned(),
            name: p.doc.project().name().clone(),
            path: p.path.clone(),
            dirty: p.dirty(),
            global_library_dirty: false,
            read_only: p.read_only,
        };
        (&p.doc, info)
    });
    s.app.state(project, &s.ui)
}

#[command(
    id = "app.state.get",
    kind = query,
    label = "Get application state",
    scopes = [read],
    example(title = "Read the current project, scene, frame, element, selection and workspace", params = "{}"),
    agent_only = "the GUI shows this state directly; agents read it as their first call (11 §7.1 get_state)",
)]
pub(crate) fn app_state_get(cx: &mut Cx, _p: NoParams) -> Result<AppState, CommandError> {
    let mut st = app_state(cx.session());
    // 11 §7.1: the connected agent sessions (C1.4 agent hub).
    st.agent_sessions = cx
        .session()
        .agents
        .sessions
        .values()
        .map(|s| crate::app::AgentSessionState {
            session_id: s.session_id.clone(),
            name: s.client.clone(),
            acting: false,
        })
        .collect();
    if let crate::types::Origin::Agent { session, .. } = cx.origin().clone() {
        let hub = &cx.session().agents;
        if let Some(s) = hub.sessions.get(&session) {
            st.agent = Some(crate::app::CallerAgentState {
                session_id: s.session_id.clone(),
                client: s.client.clone(),
                policy: s.policy.as_str().to_owned(),
                scopes: s.scopes.iter().map(|x| crate::agents::scope_name(*x).to_owned()).collect(),
                paused: s.paused,
                limits: serde_json::to_value(&hub.limits).unwrap_or(serde_json::Value::Null),
            });
        }
    }
    Ok(st)
}

/// `app.context.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ContextSetParams {
    /// Scene to make current (id or path).
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame (1-based).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// Element to make current (must belong to the scene).
    #[serde(default)]
    element_id: Option<String>,
    /// Drawing to make current (must belong to the element's drawing bank).
    #[serde(default)]
    drawing_id: Option<String>,
}

#[command(
    id = "app.context.set",
    kind = ui,
    label = "Set current context",
    scopes = [ui],
    needs_project,
    example(title = "Go to frame 12 of the first scene", params = r#"{"sceneId": "scene:#1", "frame": 12}"#),
    example(title = "Make an element current by path", params = r#"{"elementId": "scene:#1/element:#1"}"#),
    agent_only = "the GUI sets these parts one by one (scene list, frame field, element selection)",
)]
pub(crate) fn app_context_set(cx: &mut Cx, p: ContextSetParams) -> Result<AppState, CommandError> {
    let mut next = cx.app().clone();
    if let Some(s) = &p.scene_id {
        let key = cx.resolve_as(s, ObjectType::Scene)?;
        let id = Id::parse(&key)?;
        if next.scene.as_ref() != Some(&id) {
            next.scene = Some(id);
            next.element = None;
            next.drawing = None;
        }
    }
    let doc = cx.document()?.clone();
    if let Some(e) = &p.element_id {
        let key = cx.resolve_as(e, ObjectType::Element)?;
        let id = Id::parse(&key)?;
        if doc.scene_of(&id) != next.scene.as_ref() {
            return Err(invalid("/elementId", format!("element `{key}` is not in the current scene")));
        }
        next.element = Some(id);
        next.drawing = None;
    }
    if let Some(f) = p.frame {
        next.frame = f;
        next.drawing = None;
    }
    if let Some(d) = &p.drawing_id {
        let key = cx.resolve_as(d, ObjectType::Drawing)?;
        let id = Id::parse(&key)?;
        let bank = next.element.as_ref().and_then(|e| doc.element(e)).and_then(|e| e.drawing_bank_id().clone());
        if bank.is_none() || doc.bank_of(&id) != bank.as_ref() {
            return Err(invalid("/drawingId", format!("drawing `{key}` is not in the current element's drawings")));
        }
        next.drawing = Some(id);
    }
    next.repair(&doc);
    *cx.app_mut() = next;
    cx.set_targets(json!({ "sceneId": cx.app().scene, "frame": cx.app().frame, "elementId": cx.app().element }));
    Ok(app_state(cx.session()))
}

/// `{elementId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ElementIdResult {
    /// The current element (`null` when the scene has none).
    element_id: Option<String>,
}

/// `app.currentElement.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentElementSetParams {
    /// The element (id or path); its scene becomes current.
    element_id: String,
}

#[command(
    id = "app.currentElement.set",
    kind = ui,
    label = "Make element current",
    scopes = [ui],
    needs_project,
    example(title = "Make the first element of the first scene current", params = r#"{"elementId": "scene:#1/element:#1"}"#),
    gui = "Timeline and Exposure Sheet: click an element; camera view: select an element (M4)",
)]
pub(crate) fn app_current_element_set(
    cx: &mut Cx,
    p: CurrentElementSetParams,
) -> Result<ElementIdResult, CommandError> {
    let key = cx.resolve_as(&p.element_id, ObjectType::Element)?;
    let id = Id::parse(&key)?;
    let scene = cx.document()?.scene_of(&id).cloned();
    let app = cx.app_mut();
    if app.scene != scene {
        app.scene = scene;
    }
    app.element = Some(id);
    app.drawing = None;
    Ok(ElementIdResult { element_id: Some(key) })
}

/// Element order for `app.currentElement.move`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ElementOrder {
    /// Exposure-sheet order when the Exposure Sheet has focus, else Timeline order.
    #[default]
    Auto,
    /// Timeline order (depth-first, parents first).
    Timeline,
    /// Exposure Sheet column order (cell-based elements).
    Xsheet,
}

/// `app.currentElement.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentElementMoveParams {
    /// Previous or next.
    direction: Direction,
    /// Which order to follow.
    #[serde(default)]
    order: ElementOrder,
}

#[command(
    id = "app.currentElement.move",
    kind = ui,
    label = "Previous/next element",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Next element in Timeline order", params = r#"{"direction": "next", "order": "timeline"}"#),
    gui = "Keyboard shortcuts in the Timeline and Exposure Sheet (M4)",
)]
pub(crate) fn app_current_element_move(
    cx: &mut Cx,
    p: CurrentElementMoveParams,
) -> Result<ElementIdResult, CommandError> {
    let doc = cx.document()?.clone();
    let scene = cx.app().scene.clone().and_then(|s| doc.scene(&s).cloned());
    let Some(scene) = scene else { return Ok(ElementIdResult { element_id: None }) };
    let xsheet = match p.order {
        ElementOrder::Xsheet => true,
        ElementOrder::Timeline => false,
        ElementOrder::Auto => matches!(cx.ui().focused(), Some("xsheet" | "cells")),
    };
    let order: Vec<Id> = if xsheet { scene.column_order().clone() } else { doc.tree_order(&scene) };
    let cur = cx.app().element.clone();
    let pos = cur.as_ref().and_then(|c| order.iter().position(|x| x == c));
    let next = match (pos, p.direction) {
        (None, _) => order.first().cloned(),
        (Some(i), Direction::Next) => order.get(i + 1).cloned().or(cur),
        (Some(i), Direction::Previous) => i.checked_sub(1).and_then(|j| order.get(j).cloned()).or(cur),
    };
    let app = cx.app_mut();
    if app.element != next {
        app.drawing = None;
    }
    app.element.clone_from(&next);
    Ok(ElementIdResult { element_id: next.map(|e| e.as_str().to_owned()) })
}

/// `{frame}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrameResult {
    /// The current frame.
    frame: u32,
}

/// `app.frame.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrameSetParams {
    /// Frame, 1 to the scene length.
    #[schemars(range(min = 1, max = 100_000))]
    frame: u32,
}

fn scene_length(cx: &Cx) -> Result<u32, CommandError> {
    let doc = cx.document()?;
    let len = cx.app().scene.as_ref().and_then(|s| doc.scene(s)).map_or(1, |s| doc.scene_length(s));
    Ok(u32::try_from(len).unwrap_or(u32::MAX).max(1))
}

#[command(
    id = "app.frame.set",
    kind = ui,
    label = "Go to frame",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Go to frame 1", params = r#"{"frame": 1}"#),
    gui = "Frame field of the playback toolbar; Timeline and Exposure Sheet frame headers (M4)",
)]
pub(crate) fn app_frame_set(cx: &mut Cx, p: FrameSetParams) -> Result<FrameResult, CommandError> {
    let len = scene_length(cx)?;
    if p.frame > len {
        return Err(CommandError::with_details(
            codes::ARG_INVALID,
            format!("frame {} is beyond the scene length {len}", p.frame),
            json!({ "pointer": "/frame", "allowed": { "minimum": 1, "maximum": len } }),
        ));
    }
    let app = cx.app_mut();
    app.frame = p.frame;
    app.drawing = None;
    Ok(FrameResult { frame: p.frame })
}

/// Step unit for `app.frame.move`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FrameUnit {
    /// One frame.
    #[default]
    Frame,
    /// To the next/previous frame where the current element exposes another drawing.
    Drawing,
}

fn one() -> u32 {
    1
}

/// `app.frame.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrameMoveParams {
    /// Previous or next.
    direction: Direction,
    /// Frames or drawings.
    #[serde(default)]
    unit: FrameUnit,
    /// How many steps.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 10_000))]
    count: u32,
}

#[command(
    id = "app.frame.move",
    kind = ui,
    label = "Previous/next frame",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Next frame", params = r#"{"direction": "next"}"#),
    example(title = "Back to the previous drawing change", params = r#"{"direction": "previous", "unit": "drawing"}"#),
    gui = "Playback toolbar and keyboard shortcuts (M4)",
)]
pub(crate) fn app_frame_move(cx: &mut Cx, p: FrameMoveParams) -> Result<FrameResult, CommandError> {
    let len = scene_length(cx)?;
    let mut frame = cx.app().frame.clamp(1, len);
    match p.unit {
        FrameUnit::Frame => {
            frame = match p.direction {
                Direction::Next => frame.saturating_add(p.count).min(len),
                Direction::Previous => frame.saturating_sub(p.count).max(1),
            };
        }
        FrameUnit::Drawing => {
            let doc = cx.document()?.clone();
            if let Some(el) = cx.app().element.clone() {
                for _ in 0..p.count {
                    let cur = AppContext::exposed_drawing(&doc, &el, frame);
                    let mut f = frame;
                    loop {
                        f = match p.direction {
                            Direction::Next if f < len => f + 1,
                            Direction::Previous if f > 1 => f - 1,
                            _ => break,
                        };
                        if AppContext::exposed_drawing(&doc, &el, f) != cur {
                            break;
                        }
                    }
                    frame = f;
                }
            }
        }
    }
    let app = cx.app_mut();
    app.frame = frame;
    app.drawing = None;
    Ok(FrameResult { frame })
}

/// `app.mode.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ModeSetParams {
    /// Drawing or staging.
    mode: ModeParam,
    /// The view to switch (a view panel id); default the central view.
    #[serde(default)]
    view_id: Option<String>,
}

/// Working mode parameter.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ModeParam {
    /// Drawing mode.
    Drawing,
    /// Staging mode.
    Staging,
}

/// `{mode, viewId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModeResult {
    /// The mode.
    mode: Mode,
    /// The view.
    view_id: String,
}

#[command(
    id = "app.mode.set",
    kind = ui,
    label = "Set working mode",
    scopes = [ui],
    needs_project,
    example(title = "Switch to drawing mode", params = r#"{"mode": "drawing"}"#),
    gui = "Mode switch of the view toolbar (M4)",
)]
pub(crate) fn app_mode_set(cx: &mut Cx, p: ModeSetParams) -> Result<ModeResult, CommandError> {
    // 01 §4: the mode follows the active view; setting it activates a view of the requested type (C12-R1-15).
    let mode = match p.mode {
        ModeParam::Drawing => Mode::Drawing,
        ModeParam::Staging => Mode::Staging,
    };
    let allowed: &[&str] = match mode {
        Mode::Drawing => &["drawingView"],
        Mode::Staging => &["cameraView", "topView", "sideView"],
    };
    let current = cx.ui().central_panel().to_owned();
    let view = match p.view_id {
        Some(v) if allowed.contains(&v.as_str()) => v,
        Some(v) => {
            return Err(CommandError::with_details(
                codes::ARG_INVALID,
                format!(
                    "`{v}` is not a view of the {} mode",
                    if mode == Mode::Drawing { "drawing" } else { "staging" }
                ),
                json!({ "pointer": "/viewId", "allowed": { "enum": allowed } }),
            ));
        }
        None if allowed.contains(&current.as_str()) => current.clone(),
        None => allowed[0].to_owned(),
    };
    if view != current {
        cx.ui_mut().move_panel(&view, "central", None, "tab", None)?;
    }
    Ok(ModeResult { mode: mode_of_view(cx.ui().central_panel()), view_id: view })
}

/// `app.info.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InfoResult {
    /// Application name.
    name: String,
    /// Version.
    version: String,
    /// Platform.
    platform: String,
    /// UI locale.
    locale: String,
    /// Available UI languages.
    languages: Vec<String>,
    /// Project format version.
    format_version: String,
}

#[command(
    id = "app.info.get",
    kind = query,
    label = "Application information",
    scopes = [read],
    example(title = "Read name, version, platform, locale and languages", params = "{}"),
    gui = "About window (M4)",
)]
pub(crate) fn app_info_get(_cx: &mut Cx, _p: NoParams) -> Result<InfoResult, CommandError> {
    // The UI language: `LC_ALL`/`LC_MESSAGES`/`LANG` (POSIX form `en_GB.UTF-8` → `en-GB`), else `en`. Only English
    // texts exist before localisation (M10), so `languages` is `["en"]` (11 §13: an `en` copy is always available).
    let raw = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .unwrap_or_else(|| "en".to_owned());
    let locale = raw.split(['.', '@']).next().unwrap_or("en").replace('_', "-");
    Ok(InfoResult {
        name: "Toontown".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        locale: if locale.is_empty() { "en".to_owned() } else { locale },
        languages: vec!["en".to_owned()],
        format_version: tt_model::FORMAT_VERSION.to_owned(),
    })
}

/// `app.quit` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct QuitParams {
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// `{quitting}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct QuitResult {
    /// The application quits.
    quitting: bool,
}

/// `project.unsavedChanges` (01 §11.2 rule 3).
pub(crate) fn unsaved(cx: &Cx) -> CommandError {
    let (id, name, path) = cx.session().project.as_ref().map_or((None, None, None), |p| {
        (Some(p.doc.project().id().as_str().to_owned()), Some(p.doc.project().name().clone()), p.path.clone())
    });
    CommandError::with_details(
        "project.unsavedChanges",
        "the project has unsaved changes",
        json!({ "projectId": id, "name": name, "path": path, "projectDirty": true, "globalLibraryDirty": false }),
    )
    .hint("pass onUnsaved: \"save\" or \"discard\"")
}

#[command(
    id = "app.quit",
    kind = action,
    label = "Quit",
    scopes = [app],
    confirm = ["discardUnsaved"],
    example(title = "Quit, discarding unsaved changes", params = r#"{"onUnsaved": "discard"}"#),
)]
pub(crate) fn app_quit(cx: &mut Cx, p: QuitParams) -> Result<QuitResult, CommandError> {
    let dirty = cx.session().project.as_ref().is_some_and(crate::session::OpenProject::dirty);
    if dirty && p.on_unsaved == OnUnsaved::Fail {
        return Err(unsaved(cx));
    }
    crate::persistence::resolve_unsaved(cx, p.on_unsaved)?;
    // 01 §1.2: on quit the current workspace is saved if it was modified and `general.workspace.autoSave` is on (the
    // settings files are written after the command); the project closes and its recovery data goes (AUTO-04).
    if !cx.dry_run() && cx.session().env.pref_bool("general.workspace.autoSave") && cx.ui().workspace_state().1 {
        let _ = cx.ui_mut().save(None);
    }
    if cx.has_project() {
        cx.defer(crate::cx::Effect::Close);
    }
    Ok(QuitResult { quitting: true })
}

/// `app.state.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StateChanged {
    /// Top-level `AppState` fields that changed.
    changed: Vec<String>,
    /// The new state.
    state: AppState,
}

#[command(
    id = "app.state.changed",
    kind = event,
    label = "Application state changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn app_state_changed(_cx: &mut Cx, _p: NoParams) -> Result<StateChanged, CommandError> {
    Err(event_not_invocable("app.state.changed"))
}
