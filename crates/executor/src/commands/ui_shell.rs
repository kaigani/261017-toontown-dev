//! `ui.*` (01 §2, §5.8, §9, §12.2; M4 C4.1): tips and "don't show again", the status bar, themes, the start screen,
//! context help, help texts, the preferences window and the `ui.message.shown` event.
//!
//! Requests that only the desktop shell can carry out (open the preferences window, open the help viewer) change no
//! kernel state beyond what they name; after acceptance they are published as `shell.*` events that the host applies
//! and the MCP event pump does not forward (ADR-0021). In headless mode they are accepted and change nothing visible.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use crate::cx::{Cx, Effect, SettingsChange};

/// The suppressible messages (01 §9.4): id and what each one conveys.
pub(crate) const TIPS: &[(&str, &str)] = &[
    ("timing.elementsHiddenInDrawingMode", "New elements (pegs, cameras, effects) are not visible in drawing mode."),
    ("drawing.strokeHidden", "A stroke was added that stays invisible until stroke display is turned on."),
    (
        "anim.motionPointConverted",
        "Motion points were found between the last two keyframes; the last one was converted to a keyframe.",
    ),
    ("storyline.sceneExtended", "A scene was extended with blank frames to accommodate transitions."),
    ("import.layersHint", "Layered vector files keep their layers when imported with the import command."),
];

fn unknown_message(id: &str) -> CommandError {
    CommandError::with_details("ui.unknownMessage", format!("no message `{id}`"), json!({ "messageId": id }))
        .hint("ui.tips.list returns the suppressible messages")
}

/// One suppressible message.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tip {
    /// Message id.
    message_id: String,
    /// What it conveys.
    description: String,
    /// Suppressed ("don't show again" was checked).
    suppressed: bool,
}

#[command(
    id = "ui.tips.list",
    kind = query,
    label = "List tips",
    scopes = [read],
    example(title = "Every suppressible message", params = "{}"),
    gui = "Preferences › Tips (M4 C4.1)",
)]
pub(crate) fn ui_tips_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<Tip>, CommandError> {
    let suppressed = cx.suppressed();
    Ok(TIPS
        .iter()
        .map(|(id, d)| Tip {
            message_id: (*id).to_owned(),
            description: (*d).to_owned(),
            suppressed: suppressed.contains(*id),
        })
        .collect())
}

/// `ui.tips.set` parameters and result.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TipSet {
    /// Message id.
    #[schemars(length(min = 1, max = 256))]
    message_id: String,
    /// Suppress (true) or show again (false).
    suppressed: bool,
}

#[command(
    id = "ui.tips.set",
    kind = ui,
    label = "Suppress or re-enable a tip",
    scopes = [ui],
    example(title = "Don't show the drawing-mode tip again", params = r#"{"messageId": "timing.elementsHiddenInDrawingMode", "suppressed": true}"#),
    gui = "Message boxes: Don't show this again; Preferences › Tips (M4 C4.1)",
)]
pub(crate) fn ui_tips_set(cx: &mut Cx, p: TipSet) -> Result<TipSet, CommandError> {
    if !TIPS.iter().any(|(id, _)| *id == p.message_id) {
        return Err(unknown_message(&p.message_id));
    }
    let mut s = cx.suppressed();
    let changed = if p.suppressed { s.insert(p.message_id.clone()) } else { s.remove(&p.message_id) };
    if changed {
        cx.defer(Effect::Settings(SettingsChange::Suppressed(s)));
    }
    Ok(p)
}

/// `ui.tips.reset` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TipsReset {
    /// Messages shown again.
    reenabled: usize,
}

#[command(
    id = "ui.tips.reset",
    kind = ui,
    label = "Show all tips again",
    scopes = [ui],
    example(title = "Show every tip again", params = "{}"),
    gui = "Preferences › Tips: Show all tips again (M4 C4.1)",
)]
pub(crate) fn ui_tips_reset(cx: &mut Cx, _p: NoParams) -> Result<TipsReset, CommandError> {
    let s = cx.suppressed();
    let reenabled = s.len();
    if reenabled > 0 {
        cx.defer(Effect::Settings(SettingsChange::Suppressed(BTreeSet::new())));
    }
    Ok(TipsReset { reenabled })
}

/// `{visible}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Visible {
    /// Shown.
    visible: bool,
}

#[command(
    id = "ui.statusBar.set",
    kind = toggle,
    label = "Status Bar",
    scopes = [ui],
    example(title = "Hide the status bar", params = r#"{"visible": false}"#),
    gui = "View menu › Status Bar (M4 C4.1)",
)]
pub(crate) fn ui_status_bar_set(cx: &mut Cx, p: Visible) -> Result<Visible, CommandError> {
    // The same as the preference `interface.statusBar.visible` (01 §12.2), so it persists like one.
    let mut m = Map::new();
    m.insert("interface.statusBar.visible".into(), json!(p.visible));
    if cx.pref("interface.statusBar.visible") != Some(json!(p.visible)) {
        super::prefs::stage(cx, m)?;
    }
    Ok(p)
}

/// `ui.statusBar.get` result: the segment texts of 01 §9.1.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusBar {
    /// The status bar is shown.
    visible: bool,
    /// Long description of the item under the pointer (empty when none).
    description: String,
    /// Current scene and frame.
    context: String,
    /// Pointer coordinates in the current view and the selection summary.
    pointer: String,
    /// Achieved frame rate and played range during playback (empty otherwise).
    playback: String,
    /// The agent-session indicator (empty when no agent is connected).
    agent_indicator: String,
}

/// The agent-indicator text (01 §12.1 rule 9): connected clients and the acting state.
pub(crate) fn agent_indicator(cx: &Cx) -> String {
    let sessions: Vec<String> = cx
        .session()
        .agents
        .sessions
        .values()
        .map(|s| {
            let mut t = s.client.clone();
            if s.paused {
                t.push_str(" (paused)");
            }
            t
        })
        .collect();
    if sessions.is_empty() { String::new() } else { format!("Agent: {}", sessions.join(", ")) }
}

fn status_context(cx: &Cx) -> String {
    let Ok(doc) = cx.document() else { return String::new() };
    let app = cx.app();
    match app.scene.as_ref().and_then(|s| doc.scene(s)) {
        Some(s) => format!("{} \u{00B7} frame {}", s.name(), app.frame),
        None => String::new(),
    }
}

fn selection_summary(cx: &Cx) -> String {
    let n = cx.app().selection.ids.len();
    match n {
        0 => String::new(),
        1 => "1 selected".to_owned(),
        n => format!("{n} selected"),
    }
}

#[command(
    id = "ui.statusBar.get",
    kind = query,
    label = "Read the status bar",
    scopes = [read],
    example(title = "The status bar's segments", params = "{}"),
    agent_only = "the GUI shows the status bar itself",
)]
pub(crate) fn ui_status_bar_get(cx: &mut Cx, _p: NoParams) -> Result<StatusBar, CommandError> {
    let sensors = cx.ui().shell().sensors.clone();
    let mut pointer = sensors.pointer.clone();
    let sel = selection_summary(cx);
    if !sel.is_empty() {
        if !pointer.is_empty() {
            pointer.push_str(" \u{00B7} ");
        }
        pointer.push_str(&sel);
    }
    Ok(StatusBar {
        visible: cx.pref("interface.statusBar.visible").and_then(|v| v.as_bool()).unwrap_or(true),
        description: sensors.description,
        context: status_context(cx),
        pointer,
        playback: String::new(),
        agent_indicator: agent_indicator(cx),
    })
}

/// One theme.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Theme {
    /// Theme id (a value of `interface.theme`).
    theme_id: String,
    /// Display name.
    name: String,
    /// Shipped with the application.
    built_in: bool,
    /// A dark theme.
    dark: bool,
    /// The colour tokens of a user theme (`--tt-<token>`); built-in themes carry their colours in the front end
    /// (additive member, SI-115).
    #[serde(skip_serializing_if = "Option::is_none")]
    colours: Option<Map<String, Value>>,
}

#[command(
    id = "ui.theme.list",
    kind = query,
    label = "List themes",
    scopes = [read],
    example(title = "Available themes", params = "{}"),
    gui = "Preferences › Interface: theme list with preview (M4 C4.1)",
)]
pub(crate) fn ui_theme_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<Theme>, CommandError> {
    let mut out: Vec<Theme> = crate::themes::built_in()
        .into_iter()
        .map(|t| Theme { theme_id: t.theme_id, name: t.name, built_in: true, dark: t.dark, colours: None })
        .collect();
    out.extend(cx.session().env.settings.themes.iter().filter(|t| t.valid).map(|t| Theme {
        theme_id: t.theme_id.clone(),
        name: t.name.clone(),
        built_in: false,
        dark: t.dark,
        colours: t.colours.clone(),
    }));
    Ok(out)
}

#[command(
    id = "ui.startScreen.set",
    kind = toggle,
    label = "Start Screen",
    scopes = [ui],
    example(title = "Close the start screen", params = r#"{"visible": false}"#),
    gui = "Help menu › Start Screen; Escape on the start screen (M4 C4.1)",
)]
pub(crate) fn ui_start_screen_set(cx: &mut Cx, p: Visible) -> Result<Visible, CommandError> {
    let project = cx.session().project.as_ref().map(|p| p.doc.project().id().as_str().to_owned());
    cx.ui_mut().shell_mut().start_screen = Some((p.visible, project));
    Ok(p)
}

/// `{enabled}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Enabled {
    /// On or off.
    enabled: bool,
}

#[command(
    id = "ui.contextHelpMode.set",
    kind = toggle,
    label = "Context Help",
    scopes = [ui],
    example(title = "Enter context-help mode", params = r#"{"enabled": true}"#),
    gui = "Help menu › Context Help; Shift+F1 (M4 C4.1)",
)]
pub(crate) fn ui_context_help_mode_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    cx.ui_mut().shell_mut().context_help = p.enabled;
    Ok(p)
}

/// `ui.help.get` parameters (exactly one target).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct HelpParams {
    /// A command.
    #[serde(default)]
    #[schemars(length(max = 256))]
    command_id: Option<String>,
    /// A panel (01 §5.2) or tool palette.
    #[serde(default)]
    #[schemars(length(max = 256))]
    panel_id: Option<String>,
    /// A preference.
    #[serde(default)]
    #[schemars(length(max = 256))]
    preference_key: Option<String>,
    /// A message.
    #[serde(default)]
    #[schemars(length(max = 256))]
    message_id: Option<String>,
}

/// `{title, text}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Help {
    /// Title.
    title: String,
    /// Help content (Markdown).
    text: String,
}

#[command(
    id = "ui.help.get",
    kind = query,
    label = "Get help",
    scopes = [read],
    example(title = "Help for Undo", params = r#"{"commandId": "edit.undo"}"#),
    example(title = "Help for a preference", params = r#"{"preferenceKey": "files.autosave.intervalMinutes"}"#),
    gui = "Context-help pop-ups and tooltips (M4 C4.1)",
)]
pub(crate) fn ui_help_get(cx: &mut Cx, p: HelpParams) -> Result<Help, CommandError> {
    let given = [p.command_id.is_some(), p.panel_id.is_some(), p.preference_key.is_some(), p.message_id.is_some()]
        .iter()
        .filter(|g| **g)
        .count();
    if given != 1 {
        return Err(CommandError::with_details(
            if given == 0 { codes::ARG_MISSING } else { codes::ARG_INVALID },
            "give exactly one of commandId, panelId, preferenceKey, messageId",
            json!({ "pointer": "/commandId" }),
        ));
    }
    if let Some(id) = &p.command_id {
        let bindings = super::keymap::current(cx);
        return crate::help::command(id, &bindings).map(|(title, text)| Help { title, text });
    }
    if let Some(id) = &p.panel_id {
        return crate::help::panel(id).map(|(title, text)| Help { title, text });
    }
    if let Some(key) = &p.preference_key {
        let current = cx.pref(key);
        return crate::help::preference(key, current.as_ref()).map(|(title, text)| Help { title, text });
    }
    let id = p.message_id.unwrap_or_default();
    crate::help::message(&id).map(|(title, text)| Help { title, text }).ok_or_else(|| unknown_message(&id))
}

/// `ui.help.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct HelpOpenParams {
    /// A documentation topic (a `get_docs` topic, a command id, a panel id or a preference key).
    #[serde(default)]
    #[schemars(length(max = 256))]
    topic: Option<String>,
}

/// `{}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Empty {}

#[command(
    id = "ui.help.open",
    kind = ui,
    label = "Documentation",
    scopes = [ui],
    example(title = "Open the documentation", params = "{}"),
    example(title = "Open the agent guide", params = r#"{"topic": "agent-guide"}"#),
    gui = "Help menu › Documentation; F1 / Primary+? (M4 C4.1)",
)]
pub(crate) fn ui_help_open(cx: &mut Cx, p: HelpOpenParams) -> Result<Empty, CommandError> {
    cx.emit("shell.help", json!({ "topic": p.topic }));
    Ok(Empty {})
}

/// `ui.preferences.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrefsOpenParams {
    /// The category to show (a Preferences category id such as `interface`, or `keyboard`, `tips`).
    #[serde(default)]
    #[schemars(length(max = 64))]
    category: Option<String>,
}

#[command(
    id = "ui.preferences.open",
    kind = ui,
    label = "Preferences…",
    scopes = [ui],
    example(title = "Open the preferences", params = "{}"),
    example(title = "Open the shortcut editor", params = r#"{"category": "keyboard"}"#),
    gui = "Application menu › Settings… (macOS) / Edit › Preferences… (M4 C4.1)",
)]
pub(crate) fn ui_preferences_open(cx: &mut Cx, p: PrefsOpenParams) -> Result<Empty, CommandError> {
    if let Some(c) = &p.category {
        let known = crate::prefs::catalogue().categories.iter().any(|x| &x.id == c)
            || ["keyboard", "tips"].contains(&c.as_str());
        if !known {
            return Err(CommandError::with_details(
                codes::ARG_INVALID,
                format!("no preferences category `{c}`"),
                json!({ "pointer": "/category" }),
            ));
        }
    }
    cx.emit("shell.preferences", json!({ "category": p.category }));
    Ok(Empty {})
}

/// `ui.message.shown` payload (01 §12.2).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MessageShown {
    /// Message id (`area.reason`).
    message_id: String,
    /// `info`, `warning`, `error` or `question`.
    severity: String,
    /// Text.
    text: String,
    /// Structured details.
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
    /// `user` or `agent`.
    source: String,
    /// The GUI suppressed it ("don't show again").
    suppressed: bool,
}

#[command(
    id = "ui.message.shown",
    kind = event,
    label = "Message shown",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn ui_message_shown(_cx: &mut Cx, _p: NoParams) -> Result<MessageShown, CommandError> {
    Err(super::common::event_not_invocable("ui.message.shown"))
}
