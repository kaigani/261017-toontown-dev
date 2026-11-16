//! The menu as the person sees it (ruling D-38; C43B-R2S-03): one platform-aware model from which both the native menu
//! and the window tree's menu bar are made, so a label, an enabled state or a check mark cannot differ between them.
//! Pure data and logic, unit-tested; `menu.rs` builds the native items from the same labels and sets them from the
//! model at every refresh, then reports the model to the kernel.

use serde_json::{Map, Value, json};

use crate::bindings::{CommandItem, Item, Manifest, Platform, Predefined};

/// The live facts of one command item, from the kernel's availability (ruling O-10).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemFacts {
    /// The command can run now.
    pub enabled: bool,
    /// Its check mark, for an item that has one.
    pub checked: Option<bool>,
    /// A live label (undo and redo name their step), replacing the manifest's.
    pub label: Option<String>,
    /// The parameters a click runs (the reference completed for the current state).
    pub params: Value,
    /// Why it is disabled.
    pub reason: Option<String>,
}

/// One entry of Open Recent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recent {
    /// The project's path.
    pub path: String,
    /// Its name.
    pub name: String,
    /// Whether the project still exists.
    pub exists: bool,
}

/// What the model is made from: the facts of the command items in the menu's traversal order (the order `cmd<n>` ids
/// are given in), the keymap's global bindings, Open Recent, the workspaces and the application's name.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// By native id (`cmd<n>`).
    pub items: std::collections::BTreeMap<String, ItemFacts>,
    /// `keymap.list {context: global}`.
    pub keymap: Vec<Value>,
    /// `project.recent.list`.
    pub recent: Vec<Recent>,
    /// `workspace.list`: name and whether it is current.
    pub workspaces: Vec<(String, bool)>,
    /// The application's name (macOS names it in About, Hide and Quit).
    pub app_name: String,
}

/// An Open Recent entry's label: a project that no longer exists is marked.
#[must_use]
pub fn recent_label(name: &str, exists: bool) -> String {
    if exists { name.to_owned() } else { format!("{name} (missing)") }
}

/// A workspace entry's label: the current one is marked with a check.
#[must_use]
pub fn workspace_label(name: &str, current: bool) -> String {
    if current { format!("\u{2713} {name}") } else { name.to_owned() }
}

/// A predefined item's label as the platform shows it: macOS names the application in About and Hide; Windows and
/// Linux show About alone (their Help menu also has the declared "About Toontown…" item). The native item is built with
/// this text, so the two cannot differ, and the window tree never shows the manifest's token (C43B-R2-04).
#[must_use]
pub fn predefined_label(item: Predefined, app_name: &str, platform: Platform) -> String {
    match item {
        Predefined::About if platform == Platform::Macos => format!("About {app_name}"),
        Predefined::About => "About".to_owned(),
        Predefined::Services => "Services".to_owned(),
        Predefined::Hide => format!("Hide {app_name}"),
        Predefined::HideOthers => "Hide Others".to_owned(),
        Predefined::ShowAll => "Show All".to_owned(),
    }
}

/// The global binding of a command reference: its canonical keys (`Primary+Z`).
#[must_use]
pub fn binding_keys(keymap: &[Value], item: &CommandItem) -> Option<String> {
    let params = Value::Object(item.params_object());
    keymap
        .iter()
        .find(|b| {
            b.get("commandId").and_then(Value::as_str) == Some(item.command.as_str())
                && b.get("params").cloned().unwrap_or_else(|| json!({})) == params
        })
        .and_then(|b| b.get("keys").and_then(Value::as_str))
        .map(str::to_owned)
}

/// The menus for `platform`, each a node `{role: menu, id, label, enabled, children}`; a command item
/// `{role: menuitem, id, nativeId, label, enabled, reason?, checked?, shortcut, commandId, command: {commandId, params}}`;
/// a predefined item `{role: menuitem, id, label, enabled, predefined}`; a separator `{role: separator, id}`; Open
/// Recent's and the workspaces' entries as menu items with their commands.
#[must_use]
pub fn model(manifest: &Manifest, platform: Platform, f: &Facts) -> Vec<Value> {
    let mut next = 0usize;
    manifest
        .menus
        .iter()
        .filter(|m| m.on(platform))
        .map(|m| {
            let path = format!("menu:{}", m.label);
            json!({ "role": "menu", "id": format!("menu.{}", m.id), "label": m.label, "enabled": true,
                    "children": items(&m.items, platform, f, &path, &mut next) })
        })
        .collect()
}

fn items(list: &[Item], platform: Platform, f: &Facts, path: &str, next: &mut usize) -> Vec<Value> {
    let mut out = Vec::new();
    for (n, item) in list.iter().filter(|i| i.on(platform)).enumerate() {
        match item {
            Item::Separator { .. } => out.push(json!({ "role": "separator", "id": format!("{path}/-{n}") })),
            Item::Predefined { item, .. } => {
                let label = predefined_label(*item, &f.app_name, platform);
                out.push(json!({ "role": "menuitem", "id": format!("{path}/{label}"), "label": label, "enabled": true,
                                 "predefined": format!("{item:?}") }));
            }
            Item::Submenu { label, items: sub, .. } => {
                let id = format!("{path}/{label}");
                let children = items(sub, platform, f, &id, next);
                out.push(json!({ "role": "menu", "id": id, "label": label, "enabled": true, "children": children }));
            }
            Item::Command(c) => {
                let native = format!("cmd{next}");
                *next += 1;
                let facts = f.items.get(&native).cloned().unwrap_or_default();
                let label = facts.label.clone().unwrap_or_else(|| c.label.clone());
                let mut node = Map::new();
                node.insert("role".into(), json!("menuitem"));
                node.insert("id".into(), json!(format!("{path}/{}", c.label)));
                node.insert("nativeId".into(), json!(native));
                node.insert("label".into(), json!(label));
                node.insert("enabled".into(), json!(facts.enabled));
                if let Some(r) = &facts.reason {
                    node.insert("reason".into(), json!(r));
                }
                if c.check.is_some() {
                    node.insert("checked".into(), json!(facts.checked.unwrap_or(false)));
                }
                node.insert("shortcut".into(), json!(binding_keys(&f.keymap, c)));
                node.insert("commandId".into(), json!(c.command));
                let params =
                    if facts.params.is_object() { facts.params.clone() } else { Value::Object(c.params_object()) };
                node.insert("command".into(), json!({ "commandId": c.command, "params": params }));
                out.push(Value::Object(node));
            }
            Item::Recent => {
                for (k, r) in f.recent.iter().enumerate() {
                    out.push(json!({ "role": "menuitem", "id": format!("dyn:recent:{k}"), "label": recent_label(&r.name, r.exists),
                                     "enabled": true, "commandId": "project.open",
                                     "command": { "commandId": "project.open", "params": { "path": r.path } } }));
                }
            }
            Item::Workspaces => {
                for (k, (name, current)) in f.workspaces.iter().enumerate() {
                    out.push(json!({ "role": "menuitem", "id": format!("dyn:workspace:{k}"), "label": workspace_label(name, *current),
                                     "enabled": true, "checked": current, "commandId": "workspace.select",
                                     "command": { "commandId": "workspace.select", "params": { "name": name } } }));
                }
            }
        }
    }
    out
}

/// A node by its native id (`cmd<n>`), searched through the model.
#[must_use]
pub fn by_native_id<'a>(menus: &'a [Value], id: &str) -> Option<&'a Value> {
    fn walk<'a>(n: &'a Value, id: &str) -> Option<&'a Value> {
        if n.get("nativeId").and_then(Value::as_str) == Some(id) {
            return Some(n);
        }
        n.get("children").and_then(Value::as_array).and_then(|c| c.iter().find_map(|x| walk(x, id)))
    }
    menus.iter().find_map(|m| walk(m, id))
}

/// The model as the kernel's window tree carries it: the native ids removed.
#[must_use]
pub fn for_tree(menus: &[Value]) -> Value {
    fn strip(n: &Value) -> Value {
        match n {
            Value::Object(m) => Value::Object(
                m.iter().filter(|(k, _)| k.as_str() != "nativeId").map(|(k, v)| (k.clone(), strip(v))).collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(strip).collect()),
            other => other.clone(),
        }
    }
    Value::Array(menus.iter().map(strip).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(menus: &'a [Value], pred: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
        fn walk<'a>(n: &'a Value, pred: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
            if pred(n) {
                return Some(n);
            }
            n.get("children").and_then(Value::as_array).and_then(|c| c.iter().find_map(|x| walk(x, pred)))
        }
        menus.iter().find_map(|m| walk(m, pred))
    }

    fn facts() -> Facts {
        Facts {
            recent: vec![
                Recent { path: "/p/Walk.animpkg".into(), name: "Walk".into(), exists: true },
                Recent { path: "/p/Gone.animpkg".into(), name: "Gone".into(), exists: false },
            ],
            workspaces: vec![("Default".into(), false), ("Drawing".into(), true)],
            app_name: "Toontown".into(),
            ..Facts::default()
        }
    }

    #[test]
    fn a_missing_recent_project_is_marked_as_the_native_menu_marks_it() {
        let m = Manifest::embedded().unwrap();
        let menus = model(&m, Platform::Windows, &facts());
        let gone = find(&menus, &|n| n["id"] == "dyn:recent:1").expect("the second recent entry");
        assert_eq!(gone["label"], "Gone (missing)");
        assert_eq!(gone["label"], recent_label("Gone", false));
        let walk = find(&menus, &|n| n["id"] == "dyn:recent:0").unwrap();
        assert_eq!(walk["label"], "Walk");
        assert_eq!(walk["command"], json!({ "commandId": "project.open", "params": { "path": "/p/Walk.animpkg" } }));
    }

    #[test]
    fn the_current_workspace_is_marked_and_checked() {
        let m = Manifest::embedded().unwrap();
        let menus = model(&m, Platform::Windows, &facts());
        let current = find(&menus, &|n| n["id"] == "dyn:workspace:1").expect("the current workspace");
        assert_eq!(current["label"], "\u{2713} Drawing");
        assert_eq!(current["checked"], true);
        let other = find(&menus, &|n| n["id"] == "dyn:workspace:0").unwrap();
        assert_eq!(other["label"], "Default");
        assert_eq!(other["checked"], false);
    }

    #[test]
    fn the_macos_predefined_items_carry_their_native_labels() {
        let m = Manifest::embedded().unwrap();
        let menus = model(&m, Platform::Macos, &facts());
        let labels: Vec<String> = ["About", "Services", "Hide", "HideOthers", "ShowAll"]
            .iter()
            .map(|p| {
                find(&menus, &|n| n["predefined"] == *p).unwrap_or_else(|| panic!("no {p} item"))["label"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(labels, ["About Toontown", "Services", "Hide Toontown", "Hide Others", "Show All"]);
    }

    /// C43B-R2-04: Windows shows one predefined item, Help's About, with the label the native menu displays (About),
    /// never the manifest's token; the declared About Toontown… item is a separate command item.
    #[test]
    fn the_windows_predefined_about_carries_its_native_label() {
        fn walk(n: &Value, out: &mut Vec<Value>) {
            if n.get("predefined").is_some() {
                out.push(n.clone());
            }
            for c in n.get("children").and_then(Value::as_array).into_iter().flatten() {
                walk(c, out);
            }
        }
        let m = Manifest::embedded().unwrap();
        let menus = model(&m, Platform::Windows, &facts());
        let mut predefined = Vec::new();
        for menu in &menus {
            walk(menu, &mut predefined);
        }
        assert_eq!(predefined.len(), 1, "{predefined:?}");
        assert_eq!(predefined[0]["label"], "About");
        assert_eq!(predefined[0]["predefined"], "About");
        assert_eq!(predefined[0]["label"], predefined_label(Predefined::About, "Toontown", Platform::Windows));
        let declared = find(&menus, &|n| n["commandId"] == "app.info.get").expect("the declared About item");
        assert_eq!(declared["label"], "About Toontown\u{2026}");
    }

    #[test]
    fn command_items_take_their_state_from_the_facts_in_traversal_order() {
        let m = Manifest::embedded().unwrap();
        let mut f = facts();
        // The first command item of the Windows menus, whatever it is, and an undo label.
        let first = m.commands(Platform::Windows)[0].clone();
        f.items.insert("cmd0".into(), ItemFacts { enabled: true, params: json!({ "x": 1 }), ..ItemFacts::default() });
        let menus = model(&m, Platform::Windows, &f);
        let node = by_native_id(&menus, "cmd0").unwrap();
        assert_eq!(node["commandId"], first.command.as_str());
        assert_eq!(node["enabled"], true);
        assert_eq!(node["command"]["params"], json!({ "x": 1 }));
        // An item without facts is disabled, with its fixed parameters.
        let second = by_native_id(&menus, "cmd1").unwrap();
        assert_eq!(second["enabled"], false);
        // The tree's copy has no native ids.
        assert!(!for_tree(&menus).to_string().contains("nativeId"));
    }
}
