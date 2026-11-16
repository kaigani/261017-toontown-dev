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
use crate::menu_model::{self, ItemFacts, Recent, predefined_label};

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
    /// The manifest the menu was built from (the model is made from it at every refresh, C43B-R2S-03).
    manifest: Option<Manifest>,
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

/// The application's name as the menus show it (macOS names it in About and Hide).
fn app_name<R: Runtime>(app: &AppHandle<R>) -> String {
    app.state::<tt_engine::Engine>().info().name.to_owned()
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
            // Built with the model's text (C43B-R2S-03): the native label is the one the window tree carries.
            Item::Predefined { item, .. } => {
                let text = predefined_label(*item, &app_name(app));
                match item {
                    Predefined::About => sub.about_with_text(text, Some(about_metadata(app))),
                    Predefined::Services => sub.services_with_text(text),
                    Predefined::Hide => sub.hide_with_text(text),
                    Predefined::HideOthers => sub.hide_others_with_text(text),
                    Predefined::ShowAll => sub.show_all_with_text(text),
                }
            }
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
    state.manifest = Some(manifest.clone());
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
            // What a command item's availability depends on (the store re-reads its availability on the same events):
            // the menu, and the window tree that carries it (C43B-R2S-03), follow them.
            | "doc.changed"
            | "drawAid.changed"
            | "tool.options.changed"
            | "tool.active.changed"
            | "draw.selection.changed"
            | "edit.selection.changed"
            | "shell.view"
    )
}

/// The live facts the menu shows, read from the executor as the user: the availability of every command item (the one
/// source every GUI surface reads; ruling O-10), the keymap for accelerators, the recent list and the workspaces; and
/// the model made from them (C43B-R2S-03), which sets the native items and is the window tree's menu bar.
struct Facts {
    keymap: Vec<Value>,
    menus: Vec<Value>,
}

fn facts(app: &AppHandle, host: &Host) -> Facts {
    let q = |cmd: &str, params: Value| host.run(cmd, params, false, None).result.unwrap_or(Value::Null);
    let arr = |v: Value| v.as_array().cloned().unwrap_or_default();
    let (items, manifest): (Vec<(String, String, serde_json::Map<String, Value>)>, Option<Manifest>) = app
        .try_state::<MenuSlot>()
        .map(|slot| {
            let st = slot.0.lock().unwrap_or_else(PoisonError::into_inner);
            (
                st.entries.iter().map(|(id, e)| (id.clone(), e.item.command.clone(), e.item.params_object())).collect(),
                st.manifest.clone(),
            )
        })
        .unwrap_or_default();
    let refs = items.iter().map(|(_, c, p)| (c.clone(), p.clone())).collect();
    let list = host.exec().availability(refs).unwrap_or_default();
    let mut model_facts = menu_model::Facts {
        keymap: arr(q("keymap.list", json!({ "context": "global" }))),
        app_name: app_name(app),
        ..menu_model::Facts::default()
    };
    for ((id, _, _), a) in items.into_iter().zip(list) {
        model_facts.items.insert(
            id,
            ItemFacts { enabled: a.enabled, checked: a.checked, label: a.label, params: a.params, reason: a.reason },
        );
    }
    model_facts.recent = arr(q("project.recent.list", json!({})))
        .iter()
        .filter_map(|r| {
            Some(Recent {
                path: r.get("path")?.as_str()?.to_owned(),
                name: r.get("name")?.as_str()?.to_owned(),
                exists: r.get("exists").and_then(Value::as_bool).unwrap_or(true),
            })
        })
        .collect();
    model_facts.workspaces = arr(q("workspace.list", json!({})))
        .iter()
        .filter_map(|w| {
            Some((w.get("name")?.as_str()?.to_owned(), w.get("current").and_then(Value::as_bool).unwrap_or(false)))
        })
        .collect();
    let menus = manifest.map(|m| menu_model::model(&m, Platform::current(), &model_facts)).unwrap_or_default();
    Facts { keymap: model_facts.keymap, menus }
}

/// The accelerator of a command reference from the keymap, in the menu's notation (`CmdOrCtrl+Shift+Z`).
fn accelerator(keymap: &[Value], item: &CommandItem) -> Option<String> {
    // The same binding the model's `shortcut` names (C43B-R2S-03).
    let keys = menu_model::binding_keys(keymap, item)?;
    let keys = keys.as_str();
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

/// Re-reads the facts and updates every item on the main thread. The menus are reported to the executor each time
/// (ruling D-38: the window tree's menu bar is built from them and the live availability), so an executor that a
/// restart replaced has them too.
pub fn refresh(app: &AppHandle) {
    let host = app.state::<Host>();
    let f = facts(app, &host);
    // The model is the window tree's menu bar (ruling D-38, C43B-R2S-03): what the native menu shows after `apply`.
    if let Ok(report) = serde_json::from_value(json!({ "menuBar": menu_model::for_tree(&f.menus) })) {
        host.exec().report_shell(report);
    }
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || apply(&app2, &f));
}

fn apply(app: &AppHandle, f: &Facts) {
    let Some(slot) = app.try_state::<MenuSlot>() else { return };
    let mut st = slot.0.lock().unwrap_or_else(PoisonError::into_inner);
    let mac = Platform::current() == Platform::Macos;
    for (id, entry) in &st.entries {
        // The model's state (C43B-R2S-03): the label, enabled state and check mark the window tree carries.
        let node = menu_model::by_native_id(&f.menus, id);
        let on = node.and_then(|n| n.get("enabled")).and_then(Value::as_bool).unwrap_or(false);
        let acc = accelerator(&f.keymap, &entry.item);
        let mut label =
            node.and_then(|n| n.get("label")).and_then(Value::as_str).map_or_else(|| entry.item.label.clone(), str::to_owned);
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
                    let _ = m.set_checked(node.and_then(|n| n.get("checked")).and_then(Value::as_bool).unwrap_or(false));
                }
                if mac {
                    let _ = m.set_accelerator(acc.as_deref());
                }
            }
        }
    }
    // Open Recent and the workspace list (01 §3.3, §5.5): the model's entries and labels (C43B-R2S-03).
    let dynamic = |prefix: &str| -> Vec<(String, String, Value)> {
        fn walk(n: &Value, prefix: &str, out: &mut Vec<(String, String, Value)>) {
            if n.get("id").and_then(Value::as_str).is_some_and(|id| id.starts_with(prefix)) {
                out.push((
                    n["id"].as_str().unwrap_or_default().to_owned(),
                    n["label"].as_str().unwrap_or_default().to_owned(),
                    n["command"]["params"].clone(),
                ));
            }
            for c in n.get("children").and_then(Value::as_array).into_iter().flatten() {
                walk(c, prefix, out);
            }
        }
        let mut out = Vec::new();
        for m in &f.menus {
            walk(m, prefix, &mut out);
        }
        out
    };
    let recent = dynamic("dyn:recent:");
    if let Some(sub) = st.recent.clone() {
        let entries: Vec<(String, String)> =
            recent.iter().map(|(_, label, p)| (p["path"].as_str().unwrap_or_default().to_owned(), label.clone())).collect();
        rebuild_dynamic(app, &sub, "recent", &entries);
    }
    st.recent_paths = recent.iter().map(|(_, _, p)| p["path"].as_str().unwrap_or_default().to_owned()).collect();
    let workspaces = dynamic("dyn:workspace:");
    if let Some(sub) = st.workspaces.clone() {
        let entries: Vec<(String, String)> =
            workspaces.iter().map(|(_, label, p)| (p["name"].as_str().unwrap_or_default().to_owned(), label.clone())).collect();
        rebuild_dynamic(app, &sub, "workspace", &entries);
    }
    st.workspace_names = workspaces.iter().map(|(_, _, p)| p["name"].as_str().unwrap_or_default().to_owned()).collect();
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
