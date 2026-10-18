//! Native menu bar built from the UI-bindings manifest, and dispatch of menu clicks.
//!
//! Item ids are command ids. A click is dispatched by the binding's [`Handler`]: `host` commands run here,
//! `frontend` and `pending` commands are forwarded to the web front end as a [`MenuCommandEvent`]. From M1 the host
//! will route every command through the engine's executor instead (11 §2); the event shape stays.

use serde::Serialize;
use tauri::menu::{AboutMetadata, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::bindings::{CommandItem, Handler, Item, Manifest, Platform, Predefined};

/// Event name of [`MenuCommandEvent`].
pub const MENU_COMMAND_EVENT: &str = "menu-command";

/// Payload sent to the front end when a menu item it handles (or a pending one) is chosen.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuCommandEvent {
    /// Command id.
    pub command: String,
    /// Fixed parameters from the manifest.
    pub params: serde_json::Value,
    /// `frontend` or `pending`.
    pub handler: &'static str,
    /// Milestone of a pending command.
    pub milestone: Option<String>,
}

/// What a menu click resolves to.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// `app.quit` (no project can be open in M0, so the unsaved-changes rule of 01 §1.2 cannot trigger yet).
    Quit,
    /// `window.fullScreen.set` toggled from the menu.
    ToggleFullScreen,
    /// Forward to the front end.
    Forward(MenuCommandEvent),
    /// The id is not a bound command on this platform (OS-provided items never reach dispatch).
    Unbound(String),
}

/// Resolves a clicked item id to an action. Pure, so it is unit-tested without a Tauri runtime.
#[must_use]
pub fn resolve(manifest: &Manifest, platform: Platform, id: &str) -> MenuAction {
    let Some(item) = manifest.find(platform, id) else {
        return MenuAction::Unbound(id.to_owned());
    };
    match (item.handler, item.command.as_str()) {
        (Handler::Host, "app.quit") => MenuAction::Quit,
        (Handler::Host, "window.fullScreen.set") => MenuAction::ToggleFullScreen,
        (Handler::Host, other) => MenuAction::Unbound(other.to_owned()),
        (Handler::Frontend, _) => MenuAction::Forward(event(item, "frontend")),
        (Handler::Pending, _) => MenuAction::Forward(event(item, "pending")),
    }
}

fn event(item: &CommandItem, handler: &'static str) -> MenuCommandEvent {
    MenuCommandEvent {
        command: item.command.clone(),
        params: item.params.clone().unwrap_or_else(|| serde_json::json!({})),
        handler,
        milestone: item.milestone.clone(),
    }
}

/// Builds the native menu bar for the current platform. No project is open in M0, so enablement is
/// `enabledWithoutProject`.
///
/// # Errors
/// Propagates Tauri menu construction errors.
pub fn build<R: Runtime>(app: &AppHandle<R>, manifest: &Manifest) -> tauri::Result<Menu<R>> {
    let platform = Platform::current();
    let mut bar = MenuBuilder::new(app);
    for menu in manifest.menus.iter().filter(|m| m.on(platform)) {
        let mut sub = SubmenuBuilder::with_id(app, format!("menu.{}", menu.id), &menu.label);
        for item in menu.items.iter().filter(|i| i.on(platform)) {
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
                    let mut b =
                        MenuItemBuilder::with_id(c.command.as_str(), &c.label).enabled(c.enabled_without_project);
                    if let Some(acc) = c.accelerator.as_ref().and_then(|a| a.for_platform(platform)) {
                        b = b.accelerator(acc);
                    }
                    sub.item(&b.build(app)?)
                }
            };
        }
        bar = bar.item(&sub.build()?);
    }
    bar.build()
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

/// Handles a menu click.
pub fn on_event<R: Runtime>(app: &AppHandle<R>, manifest: &Manifest, id: &str) {
    match resolve(manifest, Platform::current(), id) {
        MenuAction::Quit => app.exit(0),
        MenuAction::ToggleFullScreen => {
            if let Some(window) = app.get_webview_window("main") {
                let next = !window.is_fullscreen().unwrap_or(false);
                if let Err(e) = window.set_fullscreen(next) {
                    log_error(&format!("window.fullScreen.set failed: {e}"));
                }
            }
        }
        MenuAction::Forward(event) => {
            if let Err(e) = app.emit_to("main", MENU_COMMAND_EVENT, event) {
                log_error(&format!("cannot forward menu command: {e}"));
            }
        }
        // Predefined items are handled by the OS and never arrive here with a command id; anything else is a bug.
        MenuAction::Unbound(id) => log_error(&format!("menu item `{id}` has no binding")),
    }
}

fn log_error(message: &str) {
    use std::io::Write;
    // M1 replaces this with the application log (11 §10 `internal` correlation ids).
    let _ = writeln!(std::io::stderr(), "toontown: {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest::embedded().unwrap()
    }

    #[test]
    fn host_commands() {
        for p in [Platform::Windows, Platform::Macos] {
            assert_eq!(resolve(&manifest(), p, "app.quit"), MenuAction::Quit);
            assert_eq!(resolve(&manifest(), p, "window.fullScreen.set"), MenuAction::ToggleFullScreen);
        }
    }

    #[test]
    fn frontend_command_carries_params() {
        let MenuAction::Forward(e) = resolve(&manifest(), Platform::Windows, "workspace.reset") else {
            panic!("expected forward");
        };
        assert_eq!(e.handler, "frontend");
        assert_eq!(e.params, serde_json::json!({ "name": "Default", "select": true }));
    }

    #[test]
    fn pending_command_carries_milestone() {
        let MenuAction::Forward(e) = resolve(&manifest(), Platform::Macos, "project.create") else {
            panic!("expected forward");
        };
        assert_eq!(e.handler, "pending");
        assert!(e.milestone.unwrap().contains("M1"));
    }

    #[test]
    fn unknown_and_platform_hidden_ids_are_unbound() {
        assert_eq!(resolve(&manifest(), Platform::Windows, "nope.nope"), MenuAction::Unbound("nope.nope".into()));
    }

    #[test]
    fn event_serializes_camel_case() {
        let e = event(manifest().find(Platform::Windows, "project.open").unwrap(), "pending");
        let v = serde_json::to_value(e).unwrap();
        assert_eq!(v["command"], "project.open");
        assert_eq!(v["handler"], "pending");
        assert!(v["milestone"].is_string());
    }
}
