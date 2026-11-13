//! The native menu bar (M4 C4.1): built from the UI-bindings manifest (v2), kept live from executor state —
//! enablement (registered, implemented, project rule of 01 §1.3, undo/redo availability), check marks (panels, tool
//! palettes, status bar, main toolbar, full screen, side panels), undo/redo labels naming the step and its agent
//! (01 §6.1), accelerators from the keymap in native notation, Open Recent and the workspace list.
//!
//! A click is completed by the kernel (a toggle the item leaves open gets its next state) and forwarded to the front
//! end as a [`MenuCommandEvent`]; the front end runs it through the one command entry point, with a dialog first when
//! the user must give parameters. On macOS the menu owns its key equivalents; elsewhere the shortcut is shown in the
//! label and the webview's key dispatch runs it, so a key never runs twice.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use serde_json::{Map, Value, json};
use tauri::menu::{
    AboutMetadata, CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItem, MenuItemBuilder, Submenu,
    SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, Wry};

use crate::bindings::{CommandItem, Item, Manifest, Platform, Predefined};
use crate::host::Host;

/// Event name of [`MenuCommandEvent`].
pub const MENU_COMMAND_EVENT: &str = "menu-command";

/// Payload sent to the front end when a menu item is chosen (engine wire type, JSON text; ADR-0012).
pub use tt_engine::wire::{MenuCommandEvent, MenuHandler};

/// A built command item and its native handle.
enum Handle {
    Plain(MenuItem<Wry>),
    Check(CheckMenuItem<Wry>),
}

struct Entry {
    item: CommandItem,
    handle: Handle,
}

/// The live menu: command entries by native id, the dynamic submenus and their current targets.
#[derive(Default)]
pub struct MenuState {
    entries: BTreeMap<String, Entry>,
    recent: Option<Submenu<Wry>>,
    recent_paths: Vec<String>,
    workspaces: Option<Submenu<Wry>>,
    workspace_names: Vec<String>,
}

/// Managed state wrapper.
#[derive(Default)]
pub struct MenuSlot(pub Mutex<MenuState>);

fn log(message: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "toontown: {message}");
}

fn about_metadata<R: Runtime>(app: &AppHandle<R>) -> AboutMetadata<'static> {
    let info = app.state::<tt_engine::Engine>().info();
    AboutMetadata {
        name: Some(info.name.to_owned()),
        version: Some(info.version.to_owned()),
        comments: Some(format!(
            "Working name. Format {}, registry {}. {}-{}.",
            info.format_version, info.registry_version, info.platform, info.arch
        )),
        ..AboutMetadata::default()
    }
}

fn build_items<'a>(
    app: &'a AppHandle,
    items: &[Item],
    platform: Platform,
    mut sub: SubmenuBuilder<'a, Wry, AppHandle>,
    state: &mut MenuState,
    next: &mut usize,
    pending_dynamic: &mut Vec<(&'static str, String)>,
) -> tauri::Result<SubmenuBuilder<'a, Wry, AppHandle>> {
    for item in items.iter().filter(|i| i.on(platform)) {
        sub = match item {
            Item::Separator { .. } => sub.separator(),
            Item::Predefined { item, .. } => match item {
                Predefined::About => sub.about(Some(about_metadata(app))),
                Predefined::Services => sub.services(),
                Predefined::Hide => sub.hide(),
                Predefined::HideOthers => sub.hide_others(),
                Predefined::ShowAll => sub.show_all(),
            },
            Item::Command(c) => {
                let id = format!("cmd{next}");
                *next += 1;
                let handle = if c.check.is_some() {
                    Handle::Check(CheckMenuItemBuilder::with_id(&id, &c.label).enabled(false).build(app)?)
                } else {
                    Handle::Plain(MenuItemBuilder::with_id(&id, &c.label).enabled(false).build(app)?)
                };
                sub = match &handle {
                    Handle::Plain(m) => sub.item(m),
                    Handle::Check(m) => sub.item(m),
                };
                state.entries.insert(id, Entry { item: c.clone(), handle });
                sub
            }
            Item::Submenu { id, label, items, .. } => {
                let inner = SubmenuBuilder::with_id(app, format!("menu.{id}"), label);
                let inner = build_items(app, items, platform, inner, state, next, pending_dynamic)?;
                let built = inner.build()?;
                if items.iter().any(|i| matches!(i, Item::Recent)) {
                    state.recent = Some(built.clone());
                }
                if items.iter().any(|i| matches!(i, Item::Workspaces)) {
                    state.workspaces = Some(built.clone());
                }
                sub.item(&built)
            }
            Item::Recent => {
                pending_dynamic.push(("recent", String::new()));
                sub
            }
            Item::Workspaces => {
                pending_dynamic.push(("workspaces", String::new()));
                sub
            }
        };
    }
    Ok(sub)
}

/// Builds the native menu bar for the current platform (every item starts disabled; [`refresh`] sets live state).
///
/// # Errors
/// Propagates Tauri menu construction errors.
pub fn build(app: &AppHandle, manifest: &Manifest) -> tauri::Result<Menu<Wry>> {
    let platform = Platform::current();
    let mut bar = MenuBuilder::new(app);
    let mut state = MenuState::default();
    let mut next = 0usize;
    let mut dynamic = Vec::new();
    for menu in manifest.menus.iter().filter(|m| m.on(platform)) {
        let sub = SubmenuBuilder::with_id(app, format!("menu.{}", menu.id), &menu.label);
        let sub = build_items(app, &menu.items, platform, sub, &mut state, &mut next, &mut dynamic)?;
        bar = bar.item(&sub.build()?);
    }
    if let Some(slot) = app.try_state::<MenuSlot>() {
        *slot.0.lock().unwrap_or_else(PoisonError::into_inner) = state;
    }
    bar.build()
}

/// Whether an engine event can change what the menu shows.
#[must_use]
pub fn refreshes(event: &str) -> bool {
    matches!(
        event,
        "app.state.changed"
            | "edit.history.changed"
            | "doc.undone"
            | "doc.redone"
            | "project.opened"
            | "project.closed"
            | "project.saved"
            | "project.dirty.changed"
            | "workspace.changed"
            | "prefs.changed"
            | "keymap.changed"
    )
}

/// The live facts the menu shows, read from the executor as the user: the availability of every command item (the one
/// source every GUI surface reads; ruling O-10), the keymap for accelerators, the recent list and the workspaces.
struct Facts {
    availability: BTreeMap<String, tt_executor::host::Availability>,
    keymap: Vec<Value>,
    recent: Vec<Value>,
    workspaces: Vec<Value>,
}

fn facts(app: &AppHandle, host: &Host) -> Facts {
    let q = |cmd: &str, params: Value| host.run(cmd, params, false, None).result.unwrap_or(Value::Null);
    let arr = |v: Value| v.as_array().cloned().unwrap_or_default();
    let items: Vec<(String, String, serde_json::Map<String, Value>)> = app
        .try_state::<MenuSlot>()
        .map(|slot| {
            let st = slot.0.lock().unwrap_or_else(PoisonError::into_inner);
            st.entries.iter().map(|(id, e)| (id.clone(), e.item.command.clone(), e.item.params_object())).collect()
        })
        .unwrap_or_default();
    let refs = items.iter().map(|(_, c, p)| (c.clone(), p.clone())).collect();
    let list = host.exec().availability(refs).unwrap_or_default();
    let availability = items.into_iter().map(|(id, _, _)| id).zip(list).collect();
    Facts {
        availability,
        keymap: arr(q("keymap.list", json!({ "context": "global" }))),
        recent: arr(q("project.recent.list", json!({}))),
        workspaces: arr(q("workspace.list", json!({}))),
    }
}

/// The accelerator of a command reference from the keymap, in the menu's notation (`CmdOrCtrl+Shift+Z`).
fn accelerator(keymap: &[Value], item: &CommandItem) -> Option<String> {
    let params = Value::Object(item.params_object());
    let b = keymap.iter().find(|b| {
        b.get("commandId").and_then(Value::as_str) == Some(item.command.as_str())
            && b.get("params").cloned().unwrap_or_else(|| json!({})) == params
    })?;
    let keys = b.get("keys").and_then(Value::as_str)?;
    let mut out: Vec<String> = Vec::new();
    for part in keys.split('+') {
        out.push(match part {
            "Primary" => "CmdOrCtrl".to_owned(),
            "Ctrl" => "Ctrl".to_owned(),
            "Alt" => "Alt".to_owned(),
            "Shift" => "Shift".to_owned(),
            "," => "Comma".to_owned(),
            "." => "Period".to_owned(),
            "/" => "Slash".to_owned(),
            "\\" => "Backslash".to_owned(),
            ";" => "Semicolon".to_owned(),
            "'" => "Quote".to_owned(),
            "[" => "BracketLeft".to_owned(),
            "]" => "BracketRight".to_owned(),
            "-" => "Minus".to_owned(),
            "=" => "Equal".to_owned(),
            "`" => "Backquote".to_owned(),
            "Left" => "ArrowLeft".to_owned(),
            "Right" => "ArrowRight".to_owned(),
            "Up" => "ArrowUp".to_owned(),
            "Down" => "ArrowDown".to_owned(),
            other => other.to_owned(),
        });
    }
    Some(out.join("+"))
}

/// The native notation shown in labels where the menu does not own the key (Windows, Linux).
fn native_label(keys: &str) -> String {
    keys.replace("CmdOrCtrl", "Ctrl")
}

/// Re-reads the facts and updates every item on the main thread.
pub fn refresh(app: &AppHandle) {
    let host = app.state::<Host>();
    let f = facts(app, &host);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || apply(&app2, &f));
}

fn apply(app: &AppHandle, f: &Facts) {
    let Some(slot) = app.try_state::<MenuSlot>() else { return };
    let mut st = slot.0.lock().unwrap_or_else(PoisonError::into_inner);
    let mac = Platform::current() == Platform::Macos;
    for (id, entry) in &st.entries {
        let a = f.availability.get(id);
        let on = a.is_some_and(|a| a.enabled);
        let acc = accelerator(&f.keymap, &entry.item);
        let mut label = a.and_then(|a| a.label.clone()).unwrap_or_else(|| entry.item.label.clone());
        if !mac && let Some(a) = &acc {
            label = format!("{label}\t{}", native_label(a));
        }
        match &entry.handle {
            Handle::Plain(m) => {
                let _ = m.set_enabled(on);
                let _ = m.set_text(&label);
                if mac {
                    let _ = m.set_accelerator(acc.as_deref());
                }
            }
            Handle::Check(m) => {
                let _ = m.set_enabled(on);
                let _ = m.set_text(&label);
                if entry.item.check.is_some() {
                    let _ = m.set_checked(a.and_then(|a| a.checked).unwrap_or(false));
                }
                if mac {
                    let _ = m.set_accelerator(acc.as_deref());
                }
            }
        }
    }
    // Open Recent and the workspace list (01 §3.3, §5.5).
    let recent_paths: Vec<(String, String, bool)> = f
        .recent
        .iter()
        .filter_map(|r| {
            Some((
                r.get("path")?.as_str()?.to_owned(),
                r.get("name")?.as_str()?.to_owned(),
                r.get("exists").and_then(Value::as_bool).unwrap_or(true),
            ))
        })
        .collect();
    if let Some(sub) = st.recent.clone() {
        rebuild_dynamic(
            app,
            &sub,
            "recent",
            &recent_paths
                .iter()
                .map(|(p, n, e)| (p.clone(), if *e { n.clone() } else { format!("{n} (missing)") }))
                .collect::<Vec<_>>(),
        );
    }
    st.recent_paths = recent_paths.into_iter().map(|(p, _, _)| p).collect();
    let names: Vec<(String, String)> = f
        .workspaces
        .iter()
        .filter_map(|w| {
            let n = w.get("name")?.as_str()?.to_owned();
            let current = w.get("current").and_then(Value::as_bool).unwrap_or(false);
            Some((n.clone(), if current { format!("\u{2713} {n}") } else { n }))
        })
        .collect();
    if let Some(sub) = st.workspaces.clone() {
        rebuild_dynamic(app, &sub, "workspace", &names);
    }
    st.workspace_names = names.into_iter().map(|(n, _)| n).collect();
}

/// Replaces the leading dynamic items of a submenu (`dyn:<kind>:<n>`) with new ones.
fn rebuild_dynamic(app: &AppHandle, sub: &Submenu<Wry>, kind: &str, entries: &[(String, String)]) {
    if let Ok(items) = sub.items() {
        for it in items {
            if it.id().as_ref().starts_with(&format!("dyn:{kind}:")) {
                let _ = sub.remove(&it);
            }
        }
    }
    for (i, (_, label)) in entries.iter().enumerate() {
        if let Ok(m) = MenuItemBuilder::with_id(format!("dyn:{kind}:{i}"), label).build(app) {
            let _ = sub.insert(&m, i);
        }
    }
}

/// What a click resolves to (before the kernel completes it).
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// A command reference.
    Command(String, Map<String, Value>),
    /// The id is not bound.
    Unbound(String),
}

/// Resolves a clicked id. Pure over the menu state, so it is unit-tested without a window system.
fn resolve(st: &MenuState, id: &str) -> MenuAction {
    if let Some(i) = id.strip_prefix("dyn:recent:").and_then(|n| n.parse::<usize>().ok()) {
        return match st.recent_paths.get(i) {
            Some(p) => MenuAction::Command(
                "project.open".into(),
                json!({ "path": p }).as_object().cloned().unwrap_or_default(),
            ),
            None => MenuAction::Unbound(id.to_owned()),
        };
    }
    if let Some(i) = id.strip_prefix("dyn:workspace:").and_then(|n| n.parse::<usize>().ok()) {
        return match st.workspace_names.get(i) {
            Some(n) => MenuAction::Command(
                "workspace.select".into(),
                json!({ "name": n }).as_object().cloned().unwrap_or_default(),
            ),
            None => MenuAction::Unbound(id.to_owned()),
        };
    }
    match st.entries.get(id) {
        Some(e) => MenuAction::Command(e.item.command.clone(), e.item.params_object()),
        None => MenuAction::Unbound(id.to_owned()),
    }
}

/// Handles a menu click: the kernel completes the reference, the front end runs it.
pub fn on_event(app: &AppHandle, id: &str) {
    let action = {
        let Some(slot) = app.try_state::<MenuSlot>() else { return };
        let st = slot.0.lock().unwrap_or_else(PoisonError::into_inner);
        resolve(&st, id)
    };
    let MenuAction::Command(command, params) = action else {
        return log(&format!("menu item `{id}` has no binding"));
    };
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let host = app.state::<Host>();
        let params = match host.exec().complete_reference(&command, params.clone()) {
            Ok(tt_executor::host::KeyResolution::Command { params: Value::Object(p), .. }) => p,
            _ => params,
        };
        let event = MenuCommandEvent { command, params, handler: MenuHandler::Frontend, milestone: None };
        match tt_engine::wire::to_wire_json(&event) {
            Ok(text) => {
                if let Err(e) = app.emit_to("main", MENU_COMMAND_EVENT, text) {
                    log(&format!("cannot forward menu command: {e}"));
                }
            }
            Err(e) => log(&format!("cannot serialise menu command: {}", e.message)),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerators_follow_the_keymap_notation() {
        let keymap = vec![
            json!({ "commandId": "edit.redo", "keys": "Primary+Shift+Z", "context": "global", "isDefault": true }),
            json!({ "commandId": "view.zoom.set", "params": { "steps": 1 }, "keys": "Primary+=", "context": "global", "isDefault": true }),
        ];
        let item = |command: &str, params: Option<Value>| CommandItem {
            command: command.into(),
            label: "x".into(),
            params,
            enabled_without_project: true,
            check: None,
            dialog: None,
            platforms: None,
        };
        assert_eq!(accelerator(&keymap, &item("edit.redo", None)).as_deref(), Some("CmdOrCtrl+Shift+Z"));
        assert_eq!(
            accelerator(&keymap, &item("view.zoom.set", Some(json!({ "steps": 1 })))).as_deref(),
            Some("CmdOrCtrl+Equal")
        );
        assert_eq!(accelerator(&keymap, &item("view.zoom.set", Some(json!({ "steps": -1 })))), None);
        assert_eq!(native_label("CmdOrCtrl+Shift+Z"), "Ctrl+Shift+Z");
    }

    #[test]
    fn dynamic_and_unknown_ids() {
        let st = MenuState {
            recent_paths: vec!["/a.animpkg".into()],
            workspace_names: vec!["Default".into()],
            ..MenuState::default()
        };
        assert_eq!(
            resolve(&st, "dyn:recent:0"),
            MenuAction::Command(
                "project.open".into(),
                json!({ "path": "/a.animpkg" }).as_object().cloned().unwrap_or_default()
            )
        );
        assert_eq!(
            resolve(&st, "dyn:workspace:0"),
            MenuAction::Command(
                "workspace.select".into(),
                json!({ "name": "Default" }).as_object().cloned().unwrap_or_default()
            )
        );
        assert_eq!(resolve(&st, "dyn:recent:5"), MenuAction::Unbound("dyn:recent:5".into()));
        assert_eq!(resolve(&st, "nope"), MenuAction::Unbound("nope".into()));
    }
}
