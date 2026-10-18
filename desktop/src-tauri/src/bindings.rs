//! The UI-bindings manifest (`desktop/src/ui-bindings/menu.json`): every main-menu item and the command id it is
//! bound to (11 §2 "GUI binding"). This module is pure data and logic, independent of the Tauri runtime, so it is
//! unit-tested and, from M1, diffed against the command registry by the parity test (11 §14.1).

use serde::Deserialize;

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

/// Who executes a bound command in this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Handler {
    /// Executed natively by the Tauri host.
    Host,
    /// Forwarded to and executed by the web front end.
    Frontend,
    /// Bound but not implemented in this build; invoking it reports the milestone.
    Pending,
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

/// A keyboard accelerator: one string for every platform, or one per platform.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Accelerator {
    /// Same accelerator everywhere (`CmdOrCtrl+S`).
    All(String),
    /// Per platform.
    PerPlatform {
        /// On Windows.
        windows: Option<String>,
        /// On macOS.
        macos: Option<String>,
        /// On Linux.
        linux: Option<String>,
    },
}

impl Accelerator {
    /// The accelerator for `platform`, if any.
    #[must_use]
    pub fn for_platform(&self, platform: Platform) -> Option<&str> {
        match self {
            Self::All(s) => Some(s),
            Self::PerPlatform { windows, macos, linux } => match platform {
                Platform::Windows => windows.as_deref(),
                Platform::Macos => macos.as_deref(),
                Platform::Linux => linux.as_deref(),
            },
        }
    }
}

/// A bound command item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandItem {
    /// Registered command id (`namespace.object.verb`).
    pub command: String,
    /// Menu label.
    pub label: String,
    /// Keyboard accelerator.
    #[serde(default)]
    pub accelerator: Option<Accelerator>,
    /// Fixed parameters passed with the command.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// Enabled when no project is open (01 §1.3).
    pub enabled_without_project: bool,
    /// Who executes it in this build.
    pub handler: Handler,
    /// For `pending` items: the milestone that implements it.
    #[serde(default)]
    pub milestone: Option<String>,
    /// Platforms that show the item (default: all).
    #[serde(default)]
    pub platforms: Option<Vec<Platform>>,
}

/// One menu item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
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
}

impl Item {
    fn platforms(&self) -> Option<&[Platform]> {
        match self {
            Self::Command(c) => c.platforms.as_deref(),
            Self::Separator { platforms } | Self::Predefined { platforms, .. } => platforms.as_deref(),
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

    /// Command items visible on `platform`, in menu order.
    pub fn commands(&self, platform: Platform) -> impl Iterator<Item = &CommandItem> {
        self.menus.iter().filter(move |m| m.on(platform)).flat_map(move |m| {
            m.items.iter().filter(move |i| i.on(platform)).filter_map(|i| match i {
                Item::Command(c) => Some(c),
                _ => None,
            })
        })
    }

    /// The binding for `command` on `platform` (command ids are unique per platform).
    #[must_use]
    pub fn find(&self, platform: Platform, command: &str) -> Option<&CommandItem> {
        self.commands(platform).find(|c| c.command == command)
    }

    fn validate(&self) -> Result<(), BindingsError> {
        if self.version != 1 {
            return Err(BindingsError::Invalid(format!("unsupported version {}", self.version)));
        }
        for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
            let mut seen = std::collections::BTreeSet::new();
            for c in self.commands(platform) {
                if !is_command_id(&c.command) {
                    return Err(BindingsError::Invalid(format!("`{}` is not a command id", c.command)));
                }
                if !seen.insert(c.command.as_str()) {
                    return Err(BindingsError::Invalid(format!("`{}` bound twice on {platform:?}", c.command)));
                }
                if c.handler == Handler::Pending && c.milestone.is_none() {
                    return Err(BindingsError::Invalid(format!("pending `{}` has no milestone", c.command)));
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
        assert!(m.commands(Platform::Windows).count() > 10);
    }

    #[test]
    fn task_menus_exist_on_both_product_platforms() {
        let m = Manifest::embedded().unwrap();
        for platform in [Platform::Windows, Platform::Macos] {
            let ids: Vec<_> = m.menus.iter().filter(|x| x.on(platform)).map(|x| x.id.as_str()).collect();
            for required in ["file", "edit", "view", "help"] {
                assert!(ids.contains(&required), "{required} missing on {platform:?}: {ids:?}");
            }
        }
        // macOS: the application menu comes first and holds Quit (01 §5.1).
        let mac_first = m.menus.iter().find(|x| x.on(Platform::Macos)).unwrap();
        assert_eq!(mac_first.id, "app");
        assert!(mac_first.items.iter().any(|i| matches!(i, Item::Command(c) if c.command == "app.quit")));
    }

    #[test]
    fn quit_is_bound_on_every_platform() {
        let m = Manifest::embedded().unwrap();
        for p in [Platform::Windows, Platform::Macos, Platform::Linux] {
            assert_eq!(m.find(p, "app.quit").map(|c| c.handler), Some(Handler::Host));
        }
    }

    #[test]
    fn enablement_without_project_follows_01_1_3() {
        // 01 §1.3: without a project, edit and project-scoped commands are disabled.
        let m = Manifest::embedded().unwrap();
        for c in m.commands(Platform::Windows) {
            let must_be_disabled = c.command.starts_with("edit.")
                || c.command.starts_with("view.")
                || matches!(c.command.as_str(), "project.save" | "project.saveAs" | "project.close");
            if must_be_disabled {
                assert!(!c.enabled_without_project, "{} must be disabled without a project", c.command);
            }
        }
    }

    #[test]
    fn per_platform_accelerators() {
        let m = Manifest::embedded().unwrap();
        let fs = m.find(Platform::Macos, "window.fullScreen.set").unwrap();
        let acc = fs.accelerator.as_ref().unwrap();
        assert_eq!(acc.for_platform(Platform::Macos), Some("Ctrl+Cmd+F"));
        assert_eq!(acc.for_platform(Platform::Windows), Some("F11"));
    }

    #[test]
    fn rejects_duplicates_bad_ids_and_missing_milestones() {
        let dup = r#"{"version":1,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"a.b","label":"x","enabledWithoutProject":true,"handler":"host"},
            {"kind":"command","command":"a.b","label":"y","enabledWithoutProject":true,"handler":"host"}]}]}"#;
        assert!(matches!(Manifest::parse(dup), Err(BindingsError::Invalid(_))));
        let bad_id = r#"{"version":1,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"Quit","label":"x","enabledWithoutProject":true,"handler":"host"}]}]}"#;
        assert!(matches!(Manifest::parse(bad_id), Err(BindingsError::Invalid(_))));
        let no_ms = r#"{"version":1,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"a.b","label":"x","enabledWithoutProject":true,"handler":"pending"}]}]}"#;
        assert!(matches!(Manifest::parse(no_ms), Err(BindingsError::Invalid(_))));
        let unknown_field = r#"{"version":1,"menus":[{"id":"f","label":"F","items":[
            {"kind":"command","command":"a.b","label":"x","enabledWithoutProject":true,"handler":"host","x":1}]}]}"#;
        assert!(matches!(Manifest::parse(unknown_field), Err(BindingsError::Malformed(_))));
    }

    #[test]
    fn platform_specific_duplicates_are_allowed() {
        // app.quit appears in the macOS app menu and in File on Windows/Linux, never twice on one platform.
        let m = Manifest::embedded().unwrap();
        let count = |p| m.commands(p).filter(|c| c.command == "app.quit").count();
        assert_eq!(count(Platform::Macos), 1);
        assert_eq!(count(Platform::Windows), 1);
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
