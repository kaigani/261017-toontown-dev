//! The kernel API of the desktop host (M4 C4.1; ADR-0021, `reviews/M4-design-busy-arbitration.md`): sensor reports,
//! key dispatch, user priority for agent calls, and the event count the UI-freshness wait uses.
//!
//! None of these are commands. A sensor report changes no document, setting or layout and is not audited; agents
//! cannot send one. Key dispatch only *resolves* a key to a command reference: the caller then invokes that command
//! through the ordinary entry points, so a key press and an agent's `key` tool run exactly the same command.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, Kind};

use crate::session::Event;
use crate::shell::{ModalReport, Screen, Size};
use crate::{ExecutorHandle, Origin};

/// The event bus: subscribers and the number of events broadcast so far.
#[derive(Debug, Default)]
pub(crate) struct Bus {
    pub subs: Mutex<Vec<Sender<Event>>>,
    pub seq: AtomicU64,
}

/// State shared between the document thread, the host and agent callers.
#[derive(Debug)]
pub(crate) struct ShellShared {
    /// The reported modal state (also kept in the session for `app.state.get`).
    modal: Mutex<Option<ModalReport>>,
    /// The reported UI tree with the engine event number it reflects, as one value (C41-R1-14): `None` before any
    /// report; a tree reported without a number is kept with `seq: None` and is never fresh.
    rendered: Mutex<Option<Rendered>>,
    /// `waitForUserMs` (01 §12.1 rule 8, 11 §6.4), in milliseconds.
    wait_for_user_ms: AtomicU64,
    /// A host window renders the UI (attached mode).
    attached: std::sync::atomic::AtomicBool,
    /// The panels the kernel shows as floating windows, published by the document thread after every request and
    /// before its reply (round 3, C41-R3-06): a tree reported after a command was answered is judged against the
    /// arrangement that command left.
    floats: Mutex<Vec<String>>,
}

impl ShellShared {
    /// Publishes the kernel's visible floats (the document thread, after each request).
    pub(crate) fn publish_floats(&self, s: &crate::session::Session) {
        let now = visible_floats(&s.ui.panels());
        let mut f = self.floats.lock().unwrap_or_else(PoisonError::into_inner);
        if *f != now {
            *f = now;
        }
    }
}

impl Default for ShellShared {
    fn default() -> Self {
        Self {
            modal: Mutex::new(None),
            rendered: Mutex::new(None),
            wait_for_user_ms: AtomicU64::new(10_000),
            attached: std::sync::atomic::AtomicBool::new(false),
            floats: Mutex::new(Vec::new()),
        }
    }
}

/// Broadcasts the agent-activity notice (`shell.agentActing`, host only) around an agent request, so the agent
/// indicator can show the acting state while the command runs (01 §12.1 rule 9).
pub(crate) fn acting(bus: &Bus, origin: &Origin, command: &str, on: bool) {
    if let Origin::Agent { session, name, .. } = origin {
        let e = Event {
            name: "shell.agentActing".to_owned(),
            payload: json!({ "sessionId": session, "client": name, "commandId": command, "acting": on }),
        };
        let mut subs = bus.subs.lock().unwrap_or_else(PoisonError::into_inner);
        subs.retain(|s| s.send(e.clone()).is_ok());
        bus.seq.fetch_add(1, Ordering::AcqRel);
    }
}

/// Reads a member that is present: `null` becomes `Some(None)`, a value `Some(Some(v))`; an absent member stays `None`
/// through `#[serde(default)]`.
fn present<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// An engine's recovery ownership token and its fence (C41-R1-15).
#[derive(Debug)]
pub(crate) struct RecoveryFence {
    pub engine: String,
    pub fence: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// The recovery ownership of an abandoned engine, named for the engine that replaces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryOwner {
    engine: String,
}

/// Engines abandoned in this process ([`ExecutorHandle::abandon`]); [`ExecutorHandle::release_abandoned_recovery`]
/// releases theirs, never a live engine's.
static ABANDONED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A sensor report (ADR-0021): every member present replaces that fact; absent members are left as they are.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShellReport {
    /// The modal state: `{kind, title}`, or `null` when it ended. Absent (`None`) and `null` (`Some(None)`) differ on
    /// the wire: plain serde would read `null` as absent and a closed dialog would never be reported (C41-R1-01).
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    pub modal: Option<Option<ModalReport>>,
    /// A text field has focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_focus: Option<bool>,
    /// The status bar's description segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The status bar's pointer segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    /// View canvas sizes by view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_sizes: Option<std::collections::BTreeMap<String, Size>>,
    /// The screens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screens: Option<Vec<Screen>>,
    /// The main window is up (attached mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached: Option<bool>,
    /// The rendered UI tree (11 §7.5 `inspect_ui`) and the last engine event number it reflects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_tree: Option<Value>,
    /// The engine event number the front end has rendered (with `ui_tree`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendered_seq: Option<u64>,
    /// A message the GUI showed (emitted as `ui.message.shown` with `source: "user"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_shown: Option<Value>,
    /// `true`: no window has a tree any more (the main window reloads, every window went away): the stored tree and
    /// its sequence are cleared, explicitly, so an absent `uiTree` is never read as "the old tree still stands"
    /// (round 3, C41-R3-04).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_tree_cleared: Option<bool>,
}

/// What a key sequence runs (01 §8.4).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "outcome")]
pub enum KeyResolution {
    /// No binding in the current mode or `global`.
    Unbound {
        /// Canonical sequence.
        keys: String,
    },
    /// Shortcuts are suspended (`modal`) or the key belongs to the focused text field (`textField`).
    Suspended {
        /// Canonical sequence.
        keys: String,
        /// Why.
        reason: String,
    },
    /// The bound command and its parameters (fixed parameters, plus a toggle's next state when the binding leaves
    /// it open). `dialog` is true when required parameters are still missing: the GUI collects them with a dialog
    /// (01 §0); an agent's call fails with `arg.missing`.
    Command {
        /// Canonical sequence.
        keys: String,
        /// Command id.
        command_id: String,
        /// Parameters.
        params: Value,
        /// The command can run now. Always true from [`ExecutorHandle::resolve_key`], which answers a disabled command's
        /// `command.disabled` instead (C41-R2-09); a menu or toolbar reference ([`ExecutorHandle::complete_reference`])
        /// can be disabled.
        enabled: bool,
        /// Required parameters are missing.
        dialog: bool,
    },
}

/// Focus-changing `ui` commands refused for agents while a user dialog is open (11 §6.4 `ui.modalOpen`).
const FOCUS_CHANGING: &[&str] = &[
    "window.panel.set",
    "window.panel.move",
    "view.active.set",
    "view.type.set",
    "app.mode.set",
    "workspace.select",
    "workspace.layout.set",
    "workspace.reset",
    "window.fullScreen.set",
    "window.minimized.set",
    "window.raiseAll.run",
    "window.geometry.set",
    "ui.preferences.open",
    "ui.help.open",
    "ui.startScreen.set",
];

/// The poll interval of a waiting agent caller.
const WAIT_POLL: Duration = Duration::from_millis(20);

/// The next state of a toggle the binding leaves open: the boolean parameter and its negated current value.
fn toggle_fill(
    s: &crate::session::Session,
    command: &str,
    params: &Map<String, Value>,
) -> Option<(&'static str, bool)> {
    let ui = &s.ui;
    Some(match command {
        "window.fullScreen.set" => ("enabled", !ui.full_screen()),
        "window.floatingPanels.set" => ("visible", !ui.floating_visible()),
        "window.sidePanelsExtended.set" => ("enabled", !ui.side_panels_extended()),
        "window.minimized.set" => ("minimized", !ui.shell().minimized),
        "ui.contextHelpMode.set" => ("enabled", !ui.shell().context_help),
        "ui.statusBar.set" => {
            ("visible", !s.env.pref("interface.statusBar.visible").and_then(|v| v.as_bool()).unwrap_or(true))
        }
        "ui.startScreen.set" => {
            let project = s.project.as_ref().map(|p| p.doc.project().id().as_str().to_owned());
            ("visible", !ui.shell().start_screen_visible(project.as_deref()))
        }
        "window.panel.set" => {
            // Shown and in front: hide; hidden or behind another tab: show and bring forward (01 §5.4).
            let id = params.get("panelId").and_then(Value::as_str)?;
            let entry = ui.panels().into_iter().find(|p| p.panel_id == id);
            ("visible", !entry.is_some_and(|p| p.visible && (p.current || p.floating)))
        }
        "window.toolbar.set" => {
            let id = params.get("toolbarId").and_then(Value::as_str)?;
            let l = ui.layout();
            let visible = if id == "main" {
                l.main_toolbar.visible
            } else {
                l.tool_palettes.iter().find(|p| p.id == id).is_some_and(|p| p.visible)
            };
            ("visible", !visible)
        }
        _ => return None,
    })
}

fn resolve_on(core: &mut crate::run::Core, keys: &str) -> Result<KeyResolution, CommandError> {
    let platform = crate::keymap::Platform::current();
    let seq = crate::keymap::parse(keys, platform)?;
    let canonical = seq.canonical();
    let registry = core.registry();
    core.with_lent_session(|cx| resolve_in(registry, cx, canonical, &seq, platform))
        .unwrap_or_else(|| Err(CommandError::internal("the session is unavailable")))
}

fn resolve_in(
    registry: &tt_registry::Registry,
    cx: &crate::cx::Cx,
    canonical: String,
    seq: &crate::keymap::Sequence,
    platform: crate::keymap::Platform,
) -> Result<KeyResolution, CommandError> {
    let s = cx.session();
    if s.ui.shell().sensors.modal.as_ref().is_some_and(|m| m.kind == "dialog") {
        return Ok(KeyResolution::Suspended { keys: canonical, reason: "modal".into() });
    }
    if s.ui.shell().sensors.text_focus && !seq.primary && !seq.alt && !seq.ctrl {
        return Ok(KeyResolution::Suspended { keys: canonical, reason: "textField".into() });
    }
    let bindings = crate::keymap::effective(&s.env.settings.keymap, platform);
    let mode = match s.ui.mode() {
        crate::app::Mode::Drawing => crate::keymap::Context::Drawing,
        crate::app::Mode::Staging => crate::keymap::Context::Staging,
    };
    let mode = s.project.as_ref().map(|_| mode);
    let Some(b) = crate::keymap::resolve(&bindings, &canonical, mode).cloned() else {
        return Ok(KeyResolution::Unbound { keys: canonical });
    };
    // The key dispatch reads the same availability as every other surface (ruling O-10). A key bound to a command
    // that cannot run now answers that command's own refusal (11 §7.6: the envelope of the command the key invoked;
    // §10 `command.disabled` with the reason; C41-R2-09): the GUI consumes it with no message box (01 §8.4).
    let a = availability_in(registry, cx, &b.command_id, b.params.clone().unwrap_or_default());
    if !a.enabled {
        let reason = a.reason.unwrap_or_default();
        return Err(CommandError::with_details(
            tt_registry::codes::COMMAND_DISABLED,
            format!("`{}` is not enabled: {reason}", b.command_id),
            json!({ "commandId": b.command_id, "reason": reason, "keys": canonical }),
        ));
    }
    Ok(KeyResolution::Command {
        keys: canonical,
        command_id: b.command_id,
        params: a.params,
        enabled: a.enabled,
        dialog: a.dialog,
    })
}

/// Whether a command reference can run now, what it shows and how it runs, for every GUI surface (ruling O-10): the
/// native menu, toolbars, the tool rail, context menus, dialog buttons and the key dispatch all read this one answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    /// Command id.
    pub command_id: String,
    /// The parameters as the GUI runs them (a toggle the reference leaves open gets its next state).
    pub params: Value,
    /// The command can run now: registered, not an event, implemented, a project when it needs one, and its
    /// enablement rule satisfied (the same checks the transaction pipeline makes, 11 §8).
    pub enabled: bool,
    /// Why not, when disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The check mark of a toggle: its current state (a panel: shown).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// A label that depends on the state ("Undo Move Peg — agent: Assistant", 01 §6.1); `None`: the static label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Required parameters are still missing: the GUI collects them with a dialog (01 §0).
    pub dialog: bool,
}

/// Why a command reference cannot run now (ruling O-23; reviews/M4-design-availability.md): the pipeline's static
/// checks in their order (11 §8: registered, not an event, a project when it needs one, the declared enablement rule,
/// implemented), then, for a complete reference (`precheck`), the command's declared precheck, the same function the
/// pipeline runs before the handler. Nothing else of the pipeline runs; a panicking precheck is contained.
fn unavailable(
    registry: &tt_registry::Registry,
    cx: &crate::cx::Cx,
    command_id: &str,
    params: Option<&Value>,
) -> Option<String> {
    let Some(entry) = registry.get(command_id) else { return Some(format!("`{command_id}` is not registered")) };
    if entry.spec.kind == Kind::Event {
        return Some("an event cannot be invoked".into());
    }
    if let Some(c) = entry.command {
        if c.decl.needs_project && cx.session().project.is_none() {
            return Some("no project is open".into());
        }
        if let Some(check) = c.decl.enablement.check
            && let Err(d) = check(cx)
        {
            return Some(d.reason);
        }
    }
    if let tt_registry::Status::NotImplemented { owner } = entry.status() {
        return Some(format!("arrives with {owner}"));
    }
    let (Some(pre), Some(params)) = (entry.command.and_then(|c| c.decl.precheck), params) else { return None };
    let any: &dyn std::any::Any = cx;
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pre(any, params))) {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.message),
        Err(_) => Some(format!("internal: the precheck of `{command_id}` failed unexpectedly")),
    }
}

/// References one availability request evaluates at most (ruling O-23).
const AVAILABILITY_CHUNK: usize = 32;
/// The time after which an availability request ends its chunk (ruling O-24): it holds the document thread at most
/// this plus one precheck.
const AVAILABILITY_SLICE: Duration = Duration::from_millis(5);

/// The check mark of a toggle reference: the toggled state now.
fn checked_of(s: &crate::session::Session, command_id: &str, params: &Map<String, Value>) -> Option<bool> {
    if command_id == "window.panel.set" {
        let id = params.get("panelId").and_then(Value::as_str)?;
        return Some(s.ui.panels().into_iter().any(|p| p.panel_id == id && p.visible));
    }
    toggle_fill(s, command_id, params).map(|(_, next)| !next)
}

/// "Undo <label>" / "Redo <label>", with the agent's name for an agent's step (01 §6.1).
fn history_label(s: &crate::session::Session, undo: bool) -> Option<String> {
    let h = &s.project.as_ref()?.history;
    let position = h.position();
    let step = if undo { h.steps().get(position.checked_sub(1)?)? } else { h.steps().get(position)? };
    let mut label = format!("{} {}", if undo { "Undo" } else { "Redo" }, step.label);
    if let crate::types::Origin::Agent { name, .. } = &step.origin {
        label.push_str(&format!(" \u{2014} agent: {name}"));
    }
    Some(label)
}

/// The availability of one reference in `cx` (a dry-run context of the session).
pub(crate) fn availability_in(
    registry: &tt_registry::Registry,
    cx: &crate::cx::Cx,
    command_id: &str,
    params: Map<String, Value>,
) -> Availability {
    let s = cx.session();
    let checked = registry
        .get(command_id)
        .filter(|e| e.spec.kind == Kind::Toggle)
        .and_then(|_| checked_of(s, command_id, &params));
    let (params, _, dialog) = complete(registry, s, command_id, params);
    // The static checks first; a complete reference that passes them is then asked of its handler (C41-R3-03). A
    // reference with required parameters missing is completed by a dialog (01 §0): its refusals depend on what the
    // user enters, so only the static checks apply to it.
    let reason = unavailable(registry, cx, command_id, if dialog { None } else { Some(&params) });
    let label = match command_id {
        "edit.undo" => history_label(s, true),
        "edit.redo" => history_label(s, false),
        _ => None,
    };
    Availability {
        command_id: command_id.to_owned(),
        params,
        enabled: reason.is_none(),
        reason,
        checked,
        label,
        dialog,
    }
}

/// Completes a command reference as the GUI invokes it: a toggle the reference leaves open gets its next state;
/// `enabled` says whether the command can run now; `dialog` whether required parameters are still missing.
fn complete(
    registry: &tt_registry::Registry,
    s: &crate::session::Session,
    command_id: &str,
    mut params: Map<String, Value>,
) -> (Value, bool, bool) {
    let entry = registry.get(command_id);
    if entry.is_some_and(|e| e.spec.kind == Kind::Toggle)
        && let Some((name, next)) = toggle_fill(s, command_id, &params)
        && !params.contains_key(name)
    {
        params.insert(name.to_owned(), json!(next));
    }
    let schema = entry.map(tt_registry::Entry::params_schema).unwrap_or(Value::Null);
    let dialog = schema
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|r| r.iter().filter_map(Value::as_str).any(|k| !params.contains_key(k)));
    let enabled = entry.is_some_and(|e| {
        e.spec.kind != Kind::Event
            && matches!(e.status(), tt_registry::Status::Implemented)
            && e.command.is_none_or(|c| !c.decl.needs_project || s.project.is_some())
    });
    (Value::Object(params), enabled, dialog)
}

fn ui_busy(m: &ModalReport, waited: Duration) -> CommandError {
    CommandError::with_details(
        "ui.busy",
        format!("the user is busy ({} \u{201C}{}\u{201D}); the agent's change waited and gave up", m.kind, m.title),
        json!({ "blockedBy": { "kind": m.kind, "title": m.title }, "waitedMs": u64::try_from(waited.as_millis()).unwrap_or(u64::MAX) }),
    )
    .hint("retry when the user is done; app.state.get reports `modal`")
}

/// The UI tree the front end rendered and the engine event number it vouches for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rendered {
    /// The last engine event the tree reflects (`None`: not vouched for, never fresh).
    pub seq: Option<u64>,
}

/// The panels the kernel shows as floating windows now (their window labels are `float-<panelId>`).
pub(crate) fn visible_floats(panels: &[crate::ui::PanelEntry]) -> Vec<String> {
    panels.iter().filter(|p| p.floating && p.visible).map(|p| p.panel_id.clone()).collect()
}

/// The sequence a combined tree vouches for (round 3, C41-R3-06; reviews/M4-design-floating-windows.md §3a): `seq`
/// when every float the kernel shows has its window node (`float-<panelId>`) among the tree's children, else `None`
/// (unvouched). Pure.
#[must_use]
pub fn vouched(seq: Option<u64>, tree: &Value, kernel_floats: &[String]) -> Option<u64> {
    let ids: Vec<&str> = tree
        .get("children")
        .and_then(Value::as_array)
        .map(|c| c.iter().filter_map(|n| n.get("id").and_then(Value::as_str)).collect())
        .unwrap_or_default();
    let all = kernel_floats.iter().all(|p| ids.contains(&format!("float-{p}").as_str()));
    seq.filter(|_| all)
}

/// Whether a reported tree replaces the stored one (C41-R1-14): a sequence from the future (beyond the last broadcast
/// event) is refused; an older one than the stored tree's does not replace it; a tree without a sequence is kept but
/// never fresh.
fn accept_tree(stored: Option<&Rendered>, seq: Option<u64>, broadcast: u64) -> bool {
    match seq {
        Some(n) if n > broadcast => false,
        Some(n) => stored.and_then(|r| r.seq).is_none_or(|old| n >= old),
        None => true,
    }
}

impl ExecutorHandle {
    /// Applies a sensor report from the desktop host (ADR-0021). Never blocks: the modal flag is shared at once, the
    /// rest is posted to the document thread and applied between requests.
    pub fn report_shell(&self, report: ShellReport) {
        if let Some(m) = &report.modal {
            *self.shell.modal.lock().unwrap_or_else(PoisonError::into_inner) = m.clone();
        }
        // The tree and its sequence are judged together, once, here; a refused tree is not posted.
        let mut report = report;
        let cleared = report.ui_tree_cleared == Some(true);
        if cleared {
            *self.shell.rendered.lock().unwrap_or_else(PoisonError::into_inner) = None;
            report.ui_tree = None;
        }
        if report.ui_tree.is_some() {
            let mut stored = self.shell.rendered.lock().unwrap_or_else(PoisonError::into_inner);
            if accept_tree(stored.as_ref(), report.rendered_seq, self.events_broadcast()) {
                // Vouched only when every float the kernel shows has its node (C41-R3-06).
                let floats = self.shell.floats.lock().unwrap_or_else(PoisonError::into_inner).clone();
                let tree = report.ui_tree.as_ref().unwrap_or(&Value::Null);
                *stored = Some(Rendered { seq: vouched(report.rendered_seq, tree, &floats) });
            } else {
                report.ui_tree = None;
            }
        }
        // A sequence without a tree vouches for nothing.
        let tree_seq = report.rendered_seq.take();
        if let Some(a) = report.attached {
            self.shell.attached.store(a, Ordering::Release);
        }
        let shared = self.shell.clone();
        let _ = self.tx.send(crate::Request::Run(Box::new(move |core| {
            let Some(s) = core.session_mut() else { return };
            // `app.state.changed` follows a reported change of an `AppState` field (`modal`; 01 §12.3).
            let before = crate::run::observe(s);
            let sh = s.ui.shell_mut();
            if let Some(m) = report.modal {
                sh.sensors.modal = m;
            }
            if let Some(t) = report.text_focus {
                sh.sensors.text_focus = t;
            }
            if let Some(d) = report.description {
                sh.sensors.description = d;
            }
            if let Some(p) = report.pointer {
                sh.sensors.pointer = p;
            }
            if let Some(a) = report.attached {
                sh.sensors.attached = a;
            }
            if let Some(sizes) = report.view_sizes {
                for (id, size) in sizes {
                    if let Ok(v) = sh.view_mut(&id)
                        && size.width.is_finite()
                        && size.height.is_finite()
                        && size.width >= 1.0
                        && size.height >= 1.0
                    {
                        v.size = size;
                    }
                }
            }
            if let Some(screens) = report.screens.filter(|s| !s.is_empty()) {
                sh.screens = screens;
                let clamped = sh.clamp(sh.window);
                sh.window = clamped;
            }
            // The same order rule on the document thread, where reports posted from several threads may arrive out of
            // order: a newer tree is never replaced by an older one.
            if cleared {
                sh.sensors.ui_tree = None;
                sh.sensors.ui_tree_seq = None;
            }
            if let Some(tree) = report.ui_tree
                && accept_tree(Some(&Rendered { seq: sh.sensors.ui_tree_seq }), tree_seq, u64::MAX)
            {
                // A float the kernel shows that has no window node in the tree makes it unvouched, whatever the
                // windows that did report vouch for (round 3, C41-R3-06): the expected windows come from the
                // kernel's arrangement, not from how far the host has got in opening windows.
                let floats = visible_floats(&s.ui.panels());
                let seq = vouched(tree_seq, &tree, &floats);
                if seq != tree_seq {
                    let mut stored = shared.rendered.lock().unwrap_or_else(PoisonError::into_inner);
                    if stored.as_ref().is_some_and(|r| r.seq == tree_seq) {
                        *stored = Some(Rendered { seq: None });
                    }
                }
                let sh = s.ui.shell_mut();
                sh.sensors.ui_tree = Some(tree);
                sh.sensors.ui_tree_seq = seq;
            }
            if let Some(msg) = report.message_shown {
                let mut m = msg.as_object().cloned().unwrap_or_default();
                m.insert("source".into(), json!("user"));
                m.entry("suppressed").or_insert(json!(false));
                s.emit("ui.message.shown", Value::Object(m));
            }
            crate::run::Core::settle(s, &before);
        })));
    }

    /// Resolves a key sequence to the command it runs now (01 §8.4); the caller invokes it.
    ///
    /// # Errors
    /// `keymap.invalidSequence` and `keymap.reservedKey` for a malformed sequence; `command.disabled` (details
    /// `commandId`, `reason`, `keys`) for a key bound to a command that cannot run now; `command.timeout` when the
    /// document thread does not answer within the budget.
    pub fn resolve_key(&self, keys: &str) -> Result<KeyResolution, CommandError> {
        let keys = keys.to_owned();
        crate::Poster(self.tx.clone()).run(self.config.budget, move |core| resolve_on(core, &keys))?
    }

    /// Completes a command reference the way the GUI invokes it (menus, toolbars): a toggle the reference leaves open
    /// gets its next state, and the answer says whether the command is enabled and whether a dialog must collect
    /// missing parameters. The same completion the key dispatch applies.
    ///
    /// # Errors
    /// `command.timeout` when the document thread does not answer within the budget.
    pub fn complete_reference(
        &self,
        command_id: &str,
        params: Map<String, Value>,
    ) -> Result<KeyResolution, CommandError> {
        let id = command_id.to_owned();
        crate::Poster(self.tx.clone()).run(self.config.budget, move |core| {
            let registry = core.registry();
            let a = core
                .with_lent_session(|cx| availability_in(registry, cx, &id, params))
                .ok_or_else(|| CommandError::internal("the session is unavailable"))?;
            Ok(KeyResolution::Command {
                keys: String::new(),
                command_id: id,
                params: a.params,
                enabled: a.enabled,
                dialog: a.dialog,
            })
        })?
    }

    /// The availability of command references for the GUI (ruling O-10): enabled, check mark, dynamic label and the
    /// parameters as run, for each reference, from one dry-run context of the current state. The native menu, the
    /// toolbars, the tool rail and the key dispatch all read this.
    ///
    /// # Errors
    /// `command.timeout` when the document thread does not answer within the budget.
    pub fn availability(&self, refs: Vec<(String, Map<String, Value>)>) -> Result<Vec<Availability>, CommandError> {
        self.availability_timed(refs).map(|(out, _)| out)
    }

    /// [`Self::availability`] and how long each of its requests held the document thread (tests of the budgets of
    /// rulings O-23 and O-24).
    #[doc(hidden)]
    pub fn availability_holds(&self, refs: Vec<(String, Map<String, Value>)>) -> Result<Vec<Duration>, CommandError> {
        self.availability_timed(refs).map(|(_, holds)| holds)
    }

    fn availability_timed(
        &self,
        refs: Vec<(String, Map<String, Value>)>,
    ) -> Result<(Vec<Availability>, Vec<Duration>), CommandError> {
        // In chunks, each its own request on the document thread, so a user command waits at most one chunk (ruling
        // O-23; reviews/M4-design-availability.md §4). A chunk ends after `AVAILABILITY_CHUNK` references or once it
        // has held the thread `AVAILABILITY_SLICE` (the first pass after a large selection or clipboard changes reads
        // it once: ruling O-24), so a request holds the thread at most the slice plus one precheck.
        let mut out = Vec::with_capacity(refs.len());
        let mut holds = Vec::new();
        let mut rest: std::collections::VecDeque<(String, Map<String, Value>)> = refs.into();
        while !rest.is_empty() {
            let chunk = std::mem::take(&mut rest);
            let (part, left, held) = crate::Poster(self.tx.clone()).run(self.config.budget, move |core| {
                let registry = core.registry();
                let started = Instant::now();
                let mut chunk = chunk;
                let part = core
                    .with_lent_session(|cx| {
                        let mut part = Vec::new();
                        while let Some((id, params)) = chunk.pop_front() {
                            part.push(availability_in(registry, cx, &id, params));
                            if part.len() >= AVAILABILITY_CHUNK || started.elapsed() >= AVAILABILITY_SLICE {
                                break;
                            }
                        }
                        (part, chunk)
                    })
                    .ok_or_else(|| CommandError::internal("the session is unavailable"))?;
                Ok::<_, CommandError>((part.0, part.1, started.elapsed()))
            })??;
            out.extend(part);
            holds.push(held);
            rest = left;
        }
        Ok((out, holds))
    }

    /// The time one availability chunk held the document thread, measured on it (tests of ruling O-23's budget).
    #[doc(hidden)]
    pub fn availability_chunk_hold(&self, refs: Vec<(String, Map<String, Value>)>) -> Result<Duration, CommandError> {
        crate::Poster(self.tx.clone()).run(self.config.budget, move |core| {
            let registry = core.registry();
            let started = Instant::now();
            let _ = core.with_lent_session(|cx| {
                refs.into_iter().map(|(id, params)| availability_in(registry, cx, &id, params)).collect::<Vec<_>>()
            });
            started.elapsed()
        })
    }

    /// Releases the recovery data an earlier executor of this process left when it stopped answering, so this
    /// executor offers it as recoverable work (reviews/M4-design-engine-restart.md). Returns the number of entries.
    ///
    /// # Errors
    /// `command.timeout` when the document thread does not answer within the budget.
    pub fn release_abandoned_recovery(&self) -> Result<usize, CommandError> {
        let abandoned: Vec<String> = ABANDONED.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let mut released = 0;
        for engine in abandoned.iter().filter(|e| **e != self.recovery.engine) {
            released += self.release_recovery_of(&RecoveryOwner { engine: engine.clone() })?;
        }
        Ok(released)
    }

    /// Abandons this engine (it stopped answering; reviews/M4-design-engine-restart.md): fences it at once, without its
    /// document thread, so it writes no autosave, recovery data or heartbeat if it wakes up, and names its recovery
    /// ownership for the engine that replaces it ([`ExecutorHandle::release_recovery_of`]).
    #[must_use]
    pub fn abandon(&self) -> RecoveryOwner {
        self.recovery.fence.store(true, Ordering::Release);
        let mut list = ABANDONED.lock().unwrap_or_else(PoisonError::into_inner);
        if !list.contains(&self.recovery.engine) {
            list.push(self.recovery.engine.clone());
        }
        RecoveryOwner { engine: self.recovery.engine.clone() }
    }

    /// Releases the recovery data of the named abandoned engine only (C41-R1-15): it becomes recoverable work this
    /// engine offers. Other engines' data, live or not, is untouched. Returns the number of entries released.
    ///
    /// # Errors
    /// `command.timeout`.
    pub fn release_recovery_of(&self, owner: &RecoveryOwner) -> Result<usize, CommandError> {
        let engine = owner.engine.clone();
        crate::Poster(self.tx.clone()).run(self.config.budget, move |core| {
            let Some(s) = core.session() else { return 0 };
            let keep = s.project.as_ref().map(|p| p.store.recovery_id.clone());
            tt_persist::recovery::release_engine(&s.env.dirs.recovery, &engine, keep.as_deref())
        })
    }

    /// Sets `waitForUserMs` (01 §12.1 rule 8): how long a conflicting agent change waits for the user's modal
    /// interaction to end before `ui.busy`. 0 fails at once.
    pub fn set_wait_for_user(&self, wait: Duration) {
        self.shell.wait_for_user_ms.store(u64::try_from(wait.as_millis()).unwrap_or(u64::MAX), Ordering::Release);
    }

    /// The number of engine events broadcast so far.
    #[must_use]
    pub fn events_broadcast(&self) -> u64 {
        self.subscribers.seq.load(Ordering::Acquire)
    }

    /// Whether a host window renders the UI (attached mode, reported by the host).
    #[must_use]
    pub fn attached(&self) -> bool {
        self.shell.attached.load(Ordering::Acquire)
    }

    /// Waits on the caller's thread until the front end has rendered every event broadcast before the call, at most
    /// `bound`; `true` when it has (headless: always, nothing renders). (`inspect_ui` freshness.)
    #[must_use]
    pub fn await_rendered(&self, bound: Duration) -> bool {
        if !self.attached() {
            return true;
        }
        let target = self.events_broadcast();
        let start = Instant::now();
        loop {
            let seq = self.shell.rendered.lock().unwrap_or_else(PoisonError::into_inner).as_ref().and_then(|r| r.seq);
            if seq.is_some_and(|n| n >= target) {
                return true;
            }
            if start.elapsed() >= bound {
                return false;
            }
            std::thread::sleep(WAIT_POLL.min(bound.saturating_sub(start.elapsed())));
        }
    }

    /// User priority for an agent call (01 §12.1 rule 8, 11 §6.4), on the caller's thread before anything is
    /// queued: a focus-changing `ui` command under a user dialog is refused (`ui.modalOpen`); an action, a job or a
    /// batch waits while a dialog or gesture is open, up to `waitForUserMs` and the call's budget, then `ui.busy`. A
    /// cancellation during the wait answers `command.timeout` with `reason: "cancelled"`. User calls and queries are
    /// never held.
    pub(crate) fn user_priority(
        &self,
        origin: &Origin,
        command: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommandError> {
        if !matches!(origin, Origin::Agent { .. }) {
            return Ok(());
        }
        let kind =
            if command == "batch" { Some(Kind::Action) } else { self.registry.get(command).map(|e| e.spec.kind) };
        let modal = || self.shell.modal.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let Some(m) = modal() else { return Ok(()) };
        match kind {
            Some(Kind::Ui | Kind::Toggle) if m.kind == "dialog" && FOCUS_CHANGING.contains(&command) => {
                Err(CommandError::with_details(
                    "ui.modalOpen",
                    format!("the user has a dialog open (\u{201C}{}\u{201D}); `{command}` would change focus", m.title),
                    json!({ "blockedBy": { "kind": m.kind, "title": m.title } }),
                ))
            }
            Some(Kind::Action | Kind::Job) if m.kind == "dialog" || m.kind == "gesture" => {
                let wait =
                    Duration::from_millis(self.shell.wait_for_user_ms.load(Ordering::Acquire)).min(self.config.budget);
                let start = Instant::now();
                loop {
                    match modal() {
                        None => return Ok(()),
                        Some(now) if now.kind != "dialog" && now.kind != "gesture" => return Ok(()),
                        Some(now) => {
                            if cancelled() {
                                return Err(crate::request_cancelled_error(command));
                            }
                            if start.elapsed() >= wait {
                                return Err(ui_busy(&now, start.elapsed()));
                            }
                        }
                    }
                    std::thread::sleep(
                        WAIT_POLL.min(wait.saturating_sub(start.elapsed())).max(Duration::from_millis(1)),
                    );
                }
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quorum invariant's kernel clause (reviews/M4-design-floating-windows.md §3a; C41-R3-06): a tree vouches for
    /// its sequence only when every float the kernel shows has its window node; no sequence vouches for nothing.
    #[test]
    fn a_tree_without_a_kernel_float_vouches_for_nothing() {
        let floats = vec!["history".to_owned(), "colour".to_owned()];
        let both = json!({ "id": "main", "children": [{ "id": "float-history" }, { "id": "float-colour" }] });
        let one = json!({ "id": "main", "children": [{ "id": "float-history" }] });
        assert_eq!(vouched(Some(7), &both, &floats), Some(7));
        assert_eq!(vouched(Some(7), &one, &floats), None, "a float the kernel shows is missing");
        assert_eq!(vouched(Some(7), &json!({ "id": "main" }), &[]), Some(7), "no float: the main window vouches");
        assert_eq!(vouched(None, &both, &floats), None, "a tree without a number vouches for nothing");
        // An older tree is refused by the order rule; a newer one replaces it; one from the future is refused.
        let stored = Rendered { seq: Some(5) };
        assert!(!accept_tree(Some(&stored), Some(4), 9));
        assert!(accept_tree(Some(&stored), Some(6), 9));
        assert!(!accept_tree(Some(&stored), Some(10), 9));
    }
}
