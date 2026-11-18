//! Open Recent's full path (01 §3.3 line 190: "Each menu entry shows the project name; the full path is shown in the
//! status bar or tooltip"; ruling D-48 item 8; `reviews/M4-design-panels.md` §13).
//!
//! Tauri's menu API gives a native item neither a tool tip nor a highlight event, so the host adds a little platform
//! code. **Windows:** the main window is subclassed and watches `WM_MENUSELECT`; a highlighted entry of Open Recent
//! sends its path to the status bar's left line (`menu-hint {text}`), and any other highlighted item or the menu closing
//! sends `{text: null}`. Tauri does not expose the menu bar's own handle of the submenu (only its popup copy's), so Open
//! Recent is the menu whose leading item labels are exactly the current entries' labels (entry `i` is inserted at
//! position `i`). **macOS:** each entry's `NSMenuItem` gets its path as its tool tip after every rebuild.
//!
//! A highlight changes nothing in the document or the application: no command is sent.

use std::sync::{Mutex, OnceLock, PoisonError};

use serde_json::json;
use tauri::{AppHandle, Emitter};

/// Event name of a menu hint for the main window's status bar (`{text: string | null}`).
pub const MENU_HINT_EVENT: &str = "menu-hint";

#[derive(Default)]
struct Entries {
    /// Open Recent's labels in menu order (as `menu_model::recent_label` makes them).
    labels: Vec<String>,
    /// Their paths, in the same order.
    paths: Vec<String>,
    /// The hint last sent (a repeated highlight sends nothing).
    last: Option<Option<String>>,
}

fn entries() -> &'static Mutex<Entries> {
    static E: OnceLock<Mutex<Entries>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(Entries::default()))
}

fn app() -> &'static OnceLock<AppHandle> {
    static A: OnceLock<AppHandle> = OnceLock::new();
    &A
}

/// A label as compared: the mnemonic markers (`&`) and an accelerator after a tab removed.
#[must_use]
pub fn normalised(label: &str) -> String {
    label.split('\t').next().unwrap_or_default().replace('&', "")
}

/// The hint for the highlighted item at `position` of a menu whose item labels are `menu_labels`: the path of Open
/// Recent's entry there, when that menu is Open Recent (its leading labels are exactly the entries' labels).
#[must_use]
pub fn hint_for(menu_labels: &[String], position: usize, labels: &[String], paths: &[String]) -> Option<String> {
    let n = labels.len();
    if n == 0 || n != paths.len() || menu_labels.len() < n || position >= n {
        return None;
    }
    let same = menu_labels[..n].iter().zip(labels).all(|(a, b)| normalised(a) == normalised(b));
    same.then(|| paths[position].clone())
}

/// Records Open Recent's entries after a rebuild (labels and paths in menu order); on macOS sets their tool tips.
pub fn set_entries(app: &AppHandle, labels: Vec<String>, paths: Vec<String>) {
    let _ = self::app().set(app.clone());
    {
        let mut e = entries().lock().unwrap_or_else(PoisonError::into_inner);
        e.labels.clone_from(&labels);
        e.paths.clone_from(&paths);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app.run_on_main_thread(move || macos::set_tool_tips(&labels, &paths));
    }
}

/// Sends the hint to the main window when it changed.
fn send(hint: Option<String>) {
    {
        let mut e = entries().lock().unwrap_or_else(PoisonError::into_inner);
        if e.last.as_ref() == Some(&hint) {
            return;
        }
        e.last = Some(hint.clone());
    }
    if let Some(app) = app().get()
        && let Ok(text) = tt_engine::wire::to_wire_json(&json!({ "text": hint }))
    {
        let _ = app.emit_to("main", MENU_HINT_EVENT, text);
    }
}

/// Installs the platform part on the main window (Windows: the `WM_MENUSELECT` watch).
pub fn install(app: &AppHandle) {
    let _ = self::app().set(app.clone());
    #[cfg(windows)]
    windows_hint::install(app);
}

#[cfg(windows)]
#[allow(unsafe_code, reason = "the menu's highlight is a Win32 message read through foreign calls (D-48 item 8)")]
mod windows_hint {
    use std::ffi::c_void;

    use tauri::{AppHandle, Manager};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMenuItemCount, GetMenuItemID, GetMenuStringW, HMENU, MF_BYPOSITION, MF_POPUP, WM_MENUSELECT,
    };

    /// The subclass's id (any constant unique to this subclass).
    const SUBCLASS_ID: usize = 0x5454_4D48;

    pub(super) fn install(app: &AppHandle) {
        let Some(window) = app.get_webview_window("main") else { return };
        let Ok(raw) = window.hwnd() else { return };
        let hwnd = HWND(raw.0.cast::<c_void>());
        // SAFETY: `hwnd` is the main window's live handle, on the thread that owns it (setup runs on the main thread);
        // the procedure is a plain function and the subclass lives as long as the window.
        unsafe {
            let _ = SetWindowSubclass(hwnd, Some(proc), SUBCLASS_ID, 0);
        }
    }

    unsafe extern "system" fn proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if msg == WM_MENUSELECT {
            // SAFETY: the message's own menu handle, read while the menu is shown.
            unsafe { selected(wparam, lparam) };
        }
        // SAFETY: the default processing of the same message.
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    /// `WM_MENUSELECT`: low word the item's id (or index for a submenu), high word the flags, `lparam` the menu.
    unsafe fn selected(wparam: WPARAM, lparam: LPARAM) {
        let item = (wparam.0 & 0xFFFF) as u32;
        let flags = ((wparam.0 >> 16) & 0xFFFF) as u32;
        let hmenu = HMENU(lparam.0 as *mut c_void);
        if flags == 0xFFFF && hmenu.0.is_null() {
            super::send(None); // the menu closed
            return;
        }
        if hmenu.0.is_null() || flags & MF_POPUP.0 != 0 {
            super::send(None); // a submenu's own item (Open Recent itself) is highlighted
            return;
        }
        // SAFETY: `hmenu` is the menu the system reports as shown.
        let count = unsafe { GetMenuItemCount(Some(hmenu)) }.max(0);
        let mut labels = Vec::with_capacity(count as usize);
        let mut position = None;
        for i in 0..count {
            // SAFETY: `i` is a position inside `hmenu`.
            if unsafe { GetMenuItemID(hmenu, i) } == item && position.is_none() {
                position = Some(i as usize);
            }
            let mut buf = [0u16; 512];
            // SAFETY: the buffer outlives the call and its length is passed.
            let n = unsafe { GetMenuStringW(hmenu, i as u32, Some(&mut buf), MF_BYPOSITION) }.max(0) as usize;
            labels.push(String::from_utf16_lossy(&buf[..n.min(buf.len())]));
        }
        let hint = position.and_then(|p| {
            let e = super::entries().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            super::hint_for(&labels, p, &e.labels, &e.paths)
        });
        super::send(hint);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, Message};
    use objc2_app_kit::{NSApplication, NSMenu};
    use objc2_foundation::NSString;

    /// The menu under `menu` (itself included) whose leading item titles are exactly `labels`.
    fn find(menu: &NSMenu, labels: &[String]) -> Option<Retained<NSMenu>> {
        let n = menu.numberOfItems();
        let titles: Vec<String> = (0..n).filter_map(|i| menu.itemAtIndex(i)).map(|it| it.title().to_string()).collect();
        let lead = titles.len() >= labels.len()
            && titles.iter().zip(labels).all(|(a, b)| super::normalised(a) == super::normalised(b));
        if lead {
            return Some(menu.retain());
        }
        (0..n).filter_map(|i| menu.itemAtIndex(i)).filter_map(|it| it.submenu()).find_map(|s| find(&s, labels))
    }

    pub(super) fn set_tool_tips(labels: &[String], paths: &[String]) {
        if labels.is_empty() || labels.len() != paths.len() {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else { return };
        let Some(main) = NSApplication::sharedApplication(mtm).mainMenu() else { return };
        let Some(menu) = find(&main, labels) else { return };
        for (i, path) in paths.iter().enumerate() {
            if let Some(item) = menu.itemAtIndex(i as isize) {
                item.setToolTip(Some(&NSString::from_str(path)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{hint_for, normalised};

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn an_open_recent_entry_names_its_path() {
        let labels = v(&["Walk", "Gone (missing)"]);
        let paths = v(&["C:\\p\\Walk.animpkg", "C:\\p\\Gone.animpkg"]);
        let menu = v(&["Walk", "Gone (missing)", "", "Clear"]);
        assert_eq!(hint_for(&menu, 0, &labels, &paths).as_deref(), Some("C:\\p\\Walk.animpkg"));
        assert_eq!(hint_for(&menu, 1, &labels, &paths).as_deref(), Some("C:\\p\\Gone.animpkg"));
        // Clear, and a separator, are not entries.
        assert_eq!(hint_for(&menu, 3, &labels, &paths), None);
        assert_eq!(hint_for(&menu, 2, &labels, &paths), None);
    }

    #[test]
    fn another_menu_gives_no_hint() {
        let labels = v(&["Walk"]);
        let paths = v(&["/p/Walk.animpkg"]);
        assert_eq!(hint_for(&v(&["New Project\tCtrl+N", "Open…\tCtrl+O"]), 0, &labels, &paths), None);
        // No recent project: nothing is an entry.
        assert_eq!(hint_for(&v(&["Clear"]), 0, &[], &[]), None);
    }

    #[test]
    fn mnemonics_and_accelerators_do_not_count() {
        assert_eq!(normalised("&Walk\tCtrl+1"), "Walk");
        let labels = v(&["Rock & Roll"]);
        let paths = v(&["/p/r.animpkg"]);
        assert_eq!(hint_for(&v(&["Rock && Roll"]), 0, &labels, &paths).as_deref(), Some("/p/r.animpkg"));
    }
}
