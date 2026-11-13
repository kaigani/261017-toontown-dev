//! Floating panels as separate windows (01 §5.4; ruling O-2; reviews/M4-design-floating-windows.md).
//!
//! The kernel decides which panels float, whether they are shown and where; the host reconciles the OS windows with
//! that after every state change ([`plan`] is the pure part). A float's gestures go back as commands: moving or
//! resizing it ends in `window.panel.set {geometry}`, its close button in `window.panel.set {visible: false}`, gaining
//! focus in `window.panel.set {focus: true}`. Changes the host makes itself are not reported back as gestures.

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::host::Host;

/// Label prefix of the float windows (`float-<panelId>`).
pub const PREFIX: &str = "float-";

/// A float the kernel wants shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// Panel id.
    pub panel_id: String,
    /// Window title (the panel's title).
    pub title: String,
    /// Logical geometry `(x, y, width, height)`.
    pub geometry: (i32, i32, u32, u32),
}

/// What the host does to the OS windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open a window for a float.
    Open(Wanted),
    /// Close the window of a panel no longer floating or shown.
    Close(String),
    /// Move or resize a window to its kernel geometry.
    Place(String, (i32, i32, u32, u32)),
}

/// The window label of a panel.
#[must_use]
pub fn label(panel_id: &str) -> String {
    format!("{PREFIX}{panel_id}")
}

/// The reconciliation: from the floats the kernel wants and the float windows open now (label and logical geometry),
/// the windows to open, close and place. Pure; a geometry within 1 px is left alone (rounding of scaled sizes).
#[must_use]
pub fn plan(wanted: &[Wanted], open: &[(String, (i32, i32, u32, u32))]) -> Vec<Action> {
    let mut actions = Vec::new();
    let wanted_labels: BTreeSet<String> = wanted.iter().map(|w| label(&w.panel_id)).collect();
    for (l, _) in open {
        if !wanted_labels.contains(l) {
            actions.push(Action::Close(l.clone()));
        }
    }
    for w in wanted {
        let l = label(&w.panel_id);
        match open.iter().find(|(o, _)| *o == l) {
            None => actions.push(Action::Open(w.clone())),
            Some((_, g)) => {
                let near = |a: i64, b: i64| (a - b).abs() <= 1;
                let (x, y, wd, ht) = w.geometry;
                let same = near(i64::from(g.0), i64::from(x))
                    && near(i64::from(g.1), i64::from(y))
                    && near(i64::from(g.2), i64::from(wd))
                    && near(i64::from(g.3), i64::from(ht));
                if !same {
                    actions.push(Action::Place(l, w.geometry));
                }
            }
        }
    }
    actions
}

/// The floats the kernel wants shown, from `window.panel.list`.
#[must_use]
pub fn wanted_of(panels: &[Value]) -> Vec<Wanted> {
    panels
        .iter()
        .filter(|p| p["floating"] == json!(true) && p["visible"] == json!(true))
        .filter_map(|p| {
            let g = &p["geometry"];
            let num = |k: &str| g.get(k).and_then(Value::as_i64);
            Some(Wanted {
                panel_id: p["panelId"].as_str()?.to_owned(),
                title: p["title"].as_str().unwrap_or_default().to_owned(),
                geometry: (
                    i32::try_from(num("x").unwrap_or(80)).ok()?,
                    i32::try_from(num("y").unwrap_or(80)).ok()?,
                    u32::try_from(num("width").unwrap_or(480)).ok()?,
                    u32::try_from(num("height").unwrap_or(360)).ok()?,
                ),
            })
        })
        .collect()
}

/// Labels whose OS events are the host's own (opening, raising or placing a window), until they settle.
static APPLYING: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// The geometry the host last gave each float (or last reported for it): an OS move or resize that ends there is the
/// host's own placement echoing back, not a user gesture.
static PLACED: Mutex<Vec<(String, (i32, i32, u32, u32))>> = Mutex::new(Vec::new());

fn placed(label: &str, g: (i32, i32, u32, u32)) {
    let mut p = PLACED.lock().unwrap_or_else(PoisonError::into_inner);
    p.retain(|(l, _)| l != label);
    p.push((label.to_owned(), g));
}

/// Whether `g` is (within 1 px) the geometry the host last placed for `label`.
fn is_placed(label: &str, g: (i32, i32, u32, u32)) -> bool {
    let near = |a: i64, b: i64| (a - b).abs() <= 1;
    PLACED.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|(l, p)| {
        l == label
            && near(i64::from(p.0), i64::from(g.0))
            && near(i64::from(p.1), i64::from(g.1))
            && near(i64::from(p.2), i64::from(g.2))
            && near(i64::from(p.3), i64::from(g.3))
    })
}

fn applying(label: &str) -> bool {
    APPLYING.lock().unwrap_or_else(PoisonError::into_inner).contains(label)
}

fn mark_applying(app: &AppHandle, label: &str) {
    APPLYING.lock().unwrap_or_else(PoisonError::into_inner).insert(label.to_owned());
    let (l, _a) = (label.to_owned(), app.clone());
    let _ = std::thread::Builder::new().name("tt-float-settle".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        APPLYING.lock().unwrap_or_else(PoisonError::into_inner).remove(&l);
    });
}

/// Reconciles the float windows with the kernel (on the main thread).
pub fn reconcile(app: &AppHandle) {
    let host = app.state::<Host>();
    let panels = host
        .run("window.panel.list", json!({}), false, None)
        .result
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let wanted = wanted_of(&panels);
    let a = app.clone();
    let _ = app.run_on_main_thread(move || apply(&a, &wanted));
}

fn apply(app: &AppHandle, wanted: &[Wanted]) {
    let open: Vec<(String, (i32, i32, u32, u32))> = app
        .webview_windows()
        .into_iter()
        .filter(|(l, _)| l.starts_with(PREFIX))
        .filter_map(|(l, w)| {
            let scale = w.scale_factor().unwrap_or(1.0);
            let pos = w.outer_position().ok()?;
            let size = w.inner_size().ok()?;
            let lg = |v: f64| (v / scale).round();
            Some((
                l,
                (
                    lg(f64::from(pos.x)) as i32,
                    lg(f64::from(pos.y)) as i32,
                    lg(f64::from(size.width)) as u32,
                    lg(f64::from(size.height)) as u32,
                ),
            ))
        })
        .collect();
    for action in plan(wanted, &open) {
        match action {
            Action::Open(w) => {
                let l = label(&w.panel_id);
                mark_applying(app, &l);
                placed(&l, w.geometry);
                let (x, y, wd, ht) = w.geometry;
                let url = WebviewUrl::App(format!("index.html?panel={}", w.panel_id).into());
                let built = WebviewWindowBuilder::new(app, &l, url)
                    .title(if w.title.is_empty() { w.panel_id.clone() } else { w.title.clone() })
                    .inner_size(f64::from(wd), f64::from(ht))
                    .position(f64::from(x), f64::from(y))
                    .min_inner_size(160.0, 120.0)
                    .build();
                match built {
                    // A float has no menu bar of its own (design note §2); on macOS the menu is the application's.
                    Ok(win) => {
                        if !cfg!(target_os = "macos") {
                            let _ = win.remove_menu();
                        }
                    }
                    Err(e) => crate::shell::notice(
                        app,
                        "window.floatFailed",
                        "warning",
                        &format!("The floating panel could not open: {e}"),
                    ),
                }
            }
            Action::Close(l) => {
                if let Some(w) = app.get_webview_window(&l) {
                    let _ = w.destroy();
                }
            }
            Action::Place(l, (x, y, wd, ht)) => {
                if let Some(w) = app.get_webview_window(&l) {
                    // Moving then resizing passes through an in-between geometry: the settle window covers it.
                    mark_applying(app, &l);
                    placed(&l, (x, y, wd, ht));
                    let scale = w.scale_factor().unwrap_or(1.0);
                    let _ = w.set_position(PhysicalPosition::new(
                        (f64::from(x) * scale).round() as i32,
                        (f64::from(y) * scale).round() as i32,
                    ));
                    let _ = w.set_size(PhysicalSize::new(
                        (f64::from(wd) * scale).round() as u32,
                        (f64::from(ht) * scale).round() as u32,
                    ));
                }
            }
        }
    }
}

/// Raises or hides every float with the main window (raise all, minimise, restore).
pub fn show_all(app: &AppHandle, shown: bool) {
    for (l, w) in app.webview_windows() {
        if l.starts_with(PREFIX) {
            if shown {
                let _ = w.show();
            } else {
                let _ = w.hide();
            }
        }
    }
}

/// The latest geometry of a float, sent once the gesture pauses.
static PENDING: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());

/// Window events of a float window: gestures become commands (the design note's §1 and §4).
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let l = window.label().to_owned();
    let Some(panel_id) = l.strip_prefix(PREFIX).map(str::to_owned) else { return };
    let app = window.app_handle().clone();
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            // 01 §5.4: closing a floating window hides the panel; the reconciliation then closes the window.
            api.prevent_close();
            let _ = std::thread::Builder::new().name("tt-float-close".into()).spawn(move || {
                let host = app.state::<Host>();
                let _ = host.run("window.panel.set", json!({ "panelId": panel_id, "visible": false }), false, None);
            });
        }
        WindowEvent::Destroyed => {
            if let Some(host) = app.try_state::<Host>() {
                host.forget_window_reports(&l);
            }
        }
        WindowEvent::Focused(true) => {
            if applying(&l) {
                return;
            }
            let _ = std::thread::Builder::new().name("tt-float-focus".into()).spawn(move || {
                let host = app.state::<Host>();
                let _ = host.run("window.panel.set", json!({ "panelId": panel_id, "focus": true }), false, None);
            });
        }
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            let scale = window.scale_factor().unwrap_or(1.0);
            let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else { return };
            if window.is_minimized().unwrap_or(false) {
                return;
            }
            let lg = |v: f64| (v / scale).round();
            let g = (
                lg(f64::from(pos.x)) as i32,
                lg(f64::from(pos.y)) as i32,
                (lg(f64::from(size.width)) as u32).max(1),
                (lg(f64::from(size.height)) as u32).max(1),
            );
            // The host's own placement echoing back is not a gesture: right after it (the settle window) or when the
            // window ends exactly where the host put it (late events of a new window).
            if applying(&l) || is_placed(&l, g) {
                return;
            }
            placed(&l, g);
            let geometry = json!({ "x": g.0, "y": g.1, "width": g.2, "height": g.3 });
            let first = {
                let mut p = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
                let first = !p.iter().any(|(k, _)| *k == panel_id);
                p.retain(|(k, _)| *k != panel_id);
                p.push((panel_id.clone(), geometry));
                first
            };
            if first {
                let _ = std::thread::Builder::new().name("tt-float-geometry".into()).spawn(move || {
                    std::thread::sleep(Duration::from_millis(250));
                    let latest = {
                        let mut p = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
                        let found = p.iter().position(|(k, _)| *k == panel_id).map(|i| p.remove(i).1);
                        found
                    };
                    if let Some(g) = latest {
                        let host = app.state::<Host>();
                        let _ = host.run(
                            "window.panel.set",
                            json!({ "panelId": panel_id, "geometry": g, "focus": false }),
                            false,
                            None,
                        );
                    }
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(id: &str, g: (i32, i32, u32, u32)) -> Wanted {
        Wanted { panel_id: id.into(), title: id.into(), geometry: g }
    }

    #[test]
    fn the_plan_opens_closes_and_places() {
        let wanted = vec![w("history", (100, 100, 400, 300)), w("colour", (50, 60, 320, 480))];
        let open =
            vec![("float-history".to_owned(), (100, 101, 400, 300)), ("float-library".to_owned(), (0, 0, 200, 200))];
        let actions = plan(&wanted, &open);
        assert_eq!(
            actions,
            vec![Action::Close("float-library".into()), Action::Open(w("colour", (50, 60, 320, 480)))],
            "history is within 1 px: left alone"
        );
        let moved = plan(&[w("history", (300, 100, 400, 300))], &[("float-history".into(), (100, 100, 400, 300))]);
        assert_eq!(moved, vec![Action::Place("float-history".into(), (300, 100, 400, 300))]);
        assert!(plan(&[], &[]).is_empty());
    }

    #[test]
    fn only_shown_floats_are_wanted() {
        let panels = vec![
            json!({ "panelId": "history", "title": "History", "visible": true, "floating": true,
                    "geometry": { "x": 10, "y": 20, "width": 300, "height": 200 } }),
            json!({ "panelId": "colour", "title": "Colour", "visible": false, "floating": true,
                    "geometry": { "x": 10, "y": 20, "width": 300, "height": 200 } }),
            json!({ "panelId": "timeline", "title": "Timeline", "visible": true, "floating": false }),
        ];
        assert_eq!(
            wanted_of(&panels),
            vec![Wanted { panel_id: "history".into(), title: "History".into(), geometry: (10, 20, 300, 200) }]
        );
    }
}
