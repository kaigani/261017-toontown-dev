//! The UI-bindings manifest (`desktop/src/ui-bindings/menu.json`, version 2; M4 C4.1): every main-menu item and the
//! command reference it is bound to (11 §2 "GUI binding"). Pure data and logic, independent of the Tauri runtime, so
//! it is unit-tested and diffed against the command registry by the parity test (11 §14.1).

use serde::Deserialize;
use serde_json::Value;

/// The manifest source, embedded at compile time.
pub const MENU_MANIFEST_JSON: &str = include_str!("../../src/ui-bindings/menu.json");

/// Errors loading the manifest.
#[derive(Debug, thiserror::Error)]
pub enum BindingsError {
    /// The JSON did not match the manifest schema.
    #[error("menu manifest is malformed: {0}")]
    Malformed(#[from] serde_json::Error),
    /// A structural rule was violated.
    #[error("menu manifest is invalid: {0}")]
    Invalid(String),
}

/// Operating systems that can carry menu items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    /// Windows.
    Windows,
    /// macOS.
    Macos,
    /// Linux (not a product target; only so the workspace builds on Linux CI).
    Linux,
}

impl Platform {
    /// The platform this binary was compiled for.
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Native items supplied by the OS menu implementation (not commands; exempt from the parity test).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Predefined {
    /// About panel.
    About,
    /// macOS Services submenu.
    Services,
    /// macOS Hide.
    Hide,
    /// macOS Hide Others.
    HideOthers,
    /// macOS Show All.
    ShowAll,
}

/// A bound command item: a command reference (id plus fixed parameters) and how it is shown.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandItem {
    /// Registered command id (`namespace.object.verb`).
    pub command: String,
    /// Menu label.
    pub label: String,
    /// Fixed parameters.
    #[serde(default)]
    pub params: Option<Value>,
    /// Enabled when no project is open (01 §1.3).
    pub enabled_without_project: bool,
    /// The state shown as a check mark (`statusBar`, `mainToolbar`, `fullScreen`, `sidePanelsExtended`,
    /// `panel:<id>`, `palette:<id>`).
    #[serde(default)]
    pub check: Option<String>,
    /// The dialog that collects the parameters the user must give (documentation; the front end opens it).
    #[serde(default)]
    pub dialog: Option<String>,
    /// Platforms that show the item (default: all).
    #[serde(default)]
    pub platforms: Option<Vec<Platform>>,
}

impl CommandItem {
    /// The fixed parameters as an object (empty when none).
    #[must_use]
    pub fn params_object(&self) -> serde_json::Map<String, Value> {
        match &self.params {
            Some(Value::Object(m)) => m.clone(),
            _ => serde_json::Map::new(),
        }
    }
}

/// One menu item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
#[allow(
    clippy::large_enum_variant,
    reason = "the manifest is parsed once at start-up; serde_json's order-preserving map makes params larger"
)]
pub enum Item {
    /// A command binding.
    Command(CommandItem),
    /// A separator.
    Separator {
        /// Platforms that show the item (default: all).
        #[serde(default)]
        platforms: Option<Vec<Platform>>,
    },
    /// An OS-provided item.
    Predefined {
        /// Which one.
        item: Predefined,
        /// Platforms that show the item (default: all).
        #[serde(default)]
        platforms: Option<Vec<Platform>>,
    },
    /// A submenu.
    Submenu {
        /// Stable id.
        id: String,
        /// Title.
        label: String,
        /// Items.
        items: Vec<Item>,
        /// Platforms that show the item (default: all).
        #[serde(default)]
        platforms: Option<Vec<Platform>>,
    },
    /// The recent-projects entries (`project.open {path}` each), filled at run time.
    Recent,
    /// The workspaces (`workspace.select {name}` each), filled at run time.
    Workspaces,
}

impl Item {
    fn platforms(&self) -> Option<&[Platform]> {
        match self {
            Self::Command(c) => c.platforms.as_deref(),
            Self::Separator { platforms } | Self::Predefined { platforms, .. } | Self::Submenu { platforms, .. } => {
                platforms.as_deref()
            }
            Self::Recent | Self::Workspaces => None,
        }
    }

    /// Whether the item is shown on `platform`.
    #[must_use]
    pub fn on(&self, platform: Platform) -> bool {
        self.platforms().is_none_or(|p| p.contains(&platform))
    }
}

/// A top-level menu.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Menu {
    /// Stable id.
    pub id: String,
    /// Title.
    pub label: String,
    /// Platforms that show the menu (default: all).
    #[serde(default)]
    pub platforms: Option<Vec<Platform>>,
    /// Items in order.
    pub items: Vec<Item>,
}

impl Menu {
    /// Whether the menu is shown on `platform`.
    #[must_use]
    pub fn on(&self, platform: Platform) -> bool {
        self.platforms.as_deref().is_none_or(|p| p.contains(&platform))
    }
}

/// The whole manifest.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    /// JSON comment (ignored).
    #[serde(rename = "$comment", default)]
    pub comment: Option<String>,
    /// Manifest format version.
    pub version: u32,
    /// Menus in order (on macOS the first visible one becomes the application menu).
    pub menus: Vec<Menu>,
}

fn walk<'a>(items: &'a [Item], platform: Platform, out: &mut Vec<&'a CommandItem>) {
    for i in items.iter().filter(|i| i.on(platform)) {
        match i {
            Item::Command(c) => out.push(c),
            Item::Submenu { items, .. } => walk(items, platform, out),
            _ => {}
        }
    }
}

impl Manifest {
    /// Parses and validates the embedded manifest.
    ///
    /// # Errors
    /// [`BindingsError`] when the manifest is malformed or violates a structural rule.
    pub fn embedded() -> Result<Self, BindingsError> {
        Self::parse(MENU_MANIFEST_JSON)
    }

    /// Parses and validates a manifest.
    ///
    /// # Errors
    /// [`BindingsError`] when the manifest is malformed or violates a structural rule.
    pub fn parse(json: &str) -> Result<Self, BindingsError> {
        let manifest: Self = serde_json::from_str(json)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Command items visible on `platform`, in menu order (submenus flattened).
    #[must_use]
    pub fn commands(&self, platform: Platform) -> Vec<&CommandItem> {
        let mut out = Vec::new();
        for m in self.menus.iter().filter(|m| m.on(platform)) {
            walk(&m.items, platform, &mut out);
        }
        out
    }

    /// The first binding of `command` on `platform`.
    #[must_use]
    pub fn find(&self, platform: Platform, command: &str) -> Option<&CommandItem> {
        self.commands(platform).into_iter().find(|c| c.command == command)
    }

    fn validate(&self) -> Result<(), BindingsError> {
        if self.version != 2 {
            return Err(BindingsError::Invalid(format!("unsupported version {}", self.version)));
        }
        for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
            let mut seen = std::collections::BTreeSet::new();
            for c in self.commands(platform) {
                if !is_command_id(&c.command) {
                    return Err(BindingsError::Invalid(format!("`{}` is not a command id", c.command)));
                }
                if c.params.as_ref().is_some_and(|p| !p.is_object()) {
                    return Err(BindingsError::Invalid(format!("`{}` has params that are not an object", c.command)));
                }
                // One command reference (id + parameters) per platform.
                let key = (c.command.clone(), serde_json::to_string(&c.params_object()).unwrap_or_default());
                if !seen.insert(key) {
                    return Err(BindingsError::Invalid(format!("`{}` bound twice on {platform:?}", c.command)));
                }
            }
        }
        Ok(())
    }
}

/// `namespace.object[.verb]`: two or more lower-camelCase segments.
#[must_use]
pub fn is_command_id(id: &str) -> bool {
    let segments: Vec<_> = id.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|s| {
            let mut chars = s.chars();
            chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_alphanumeric())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_manifest_is_valid() {
        let m = Manifest::embedded().unwrap();
        assert!(m.commands(Platform::Windows).len() > 60);
    }

    #[test]
    fn task_menus_exist_on_both_product_platforms() {
        let m = Manifest::embedded().unwrap();
        for platform in [Platform::Windows, Platform::Macos] {
            let ids: Vec<_> = m.menus.iter().filter(|x| x.on(platform)).map(|x| x.id.as_str()).collect();
            for required in ["file", "edit", "view", "window", "help"] {
                assert!(ids.contains(&required), "{required} missing on {platform:?}: {ids:?}");
            }
        }
        // macOS: the application menu comes first and holds Settings and Quit (01 §5.1).
        let mac_first = m.menus.iter().find(|x| x.on(Platform::Macos)).unwrap();
        assert_eq!(mac_first.id, "app");
        let mac_cmds: Vec<&str> = {
            let mut out = Vec::new();
            walk(&mac_first.items, Platform::Macos, &mut out);
            out.iter().map(|c| c.command.as_str()).collect()
        };
        assert!(mac_cmds.contains(&"app.quit") && mac_cmds.contains(&"ui.preferences.open"));
    }

    #[test]
    fn every_panel_and_palette_has_a_checkable_item() {
        let m = Manifest::embedded().unwrap();
        let checks: Vec<String> =
            m.commands(Platform::Windows).iter().filter_map(|c| c.check.clone()).collect();
        for (p, _) in tt_executor::PANEL_CATALOGUE {
            assert!(checks.contains(&format!("panel:{p}")), "{p}");
        }
        assert!(checks.iter().filter(|c| c.starts_with("palette:")).count() == 5);
    }

    #[test]
    fn enablement_without_project_follows_01_1_3() {
        let m = Manifest::embedded().unwrap();
        for c in m.commands(Platform::Windows) {
            let must_be_disabled = c.command.starts_with("edit.")
                || c.command.starts_with("view.")
                || matches!(c.command.as_str(), "project.save" | "project.saveAs" | "project.close" | "project.properties.set");
            if must_be_disabled {
                assert!(!c.enabled_without_project, "{} must be disabled without a project", c.command);
            }
        }
    }

    #[test]
    fn rejects_duplicates_bad_ids_and_unknown_fields() {
        let dup = r#"{"version":2,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"a.b","label":"x","enabledWithoutProject":true},
            {"kind":"command","command":"a.b","label":"y","enabledWithoutProject":true}]}]}"#;
        assert!(matches!(Manifest::parse(dup), Err(BindingsError::Invalid(_))));
        let same_id_other_params = r#"{"version":2,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"a.b","label":"x","params":{"n":1},"enabledWithoutProject":true},
            {"kind":"command","command":"a.b","label":"y","params":{"n":2},"enabledWithoutProject":true}]}]}"#;
        assert!(Manifest::parse(same_id_other_params).is_ok());
        let bad_id = r#"{"version":2,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"Quit","label":"x","enabledWithoutProject":true}]}]}"#;
        assert!(matches!(Manifest::parse(bad_id), Err(BindingsError::Invalid(_))));
        let unknown_field = r#"{"version":2,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"a.b","label":"x","enabledWithoutProject":true,"x":1}]}]}"#;
        assert!(matches!(Manifest::parse(unknown_field), Err(BindingsError::Malformed(_))));
        let old = r#"{"version":1,"menus":[]}"#;
        assert!(matches!(Manifest::parse(old), Err(BindingsError::Invalid(_))));
    }

    #[test]
    fn quit_appears_once_per_platform() {
        let m = Manifest::embedded().unwrap();
        for p in [Platform::Macos, Platform::Windows] {
            assert_eq!(m.commands(p).iter().filter(|c| c.command == "app.quit").count(), 1);
        }
    }

    #[test]
    fn command_id_syntax() {
        assert!(is_command_id("app.quit"));
        assert!(is_command_id("window.fullScreen.set"));
        assert!(!is_command_id("quit"));
        assert!(!is_command_id("App.quit"));
        assert!(!is_command_id("app..quit"));
        assert!(!is_command_id("app.qu-it"));
    }
}
