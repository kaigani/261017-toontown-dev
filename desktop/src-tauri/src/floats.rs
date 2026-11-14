//! Floating panels as separate windows (01 §5.4; ruling O-2; reviews/M4-design-floating-windows.md).
//!
//! The kernel decides which panels float, whether they are shown and where; the host reconciles the OS windows with
//! that after every state change ([`plan`] is the pure part). A float's gestures go back as commands: moving or
//! resizing it ends in `window.panel.set {geometry}`, its close button in `window.panel.set {visible: false}`, gaining
//! focus in `window.panel.set {focus: true}`. Changes the host makes itself are not reported back as gestures
//! ([`EchoGuard`]); a gesture's geometry is sent once it pauses ([`Debounce`]). One reconciliation runs at a time,
//! from the current kernel state ([`Coalescer`]; C41-R2-10).

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::host::Host;

/// Label prefix of the float windows (`float-<panelId>`).
pub const PREFIX: &str = "float-";

/// A logical geometry `(x, y, width, height)`.
pub type Rect = (i32, i32, u32, u32);

/// A float the kernel wants shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// Panel id.
    pub panel_id: String,
    /// Window title (the panel's title).
    pub title: String,
    /// Logical geometry `(x, y, width, height)`.
    pub geometry: Rect,
}

/// What the host does to the OS windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open a window for a float.
    Open(Wanted),
    /// Close the window of a panel no longer floating or shown.
    Close(String),
    /// Move or resize a window to its kernel geometry.
    Place(String, Rect),
}

/// The window label of a panel.
#[must_use]
pub fn label(panel_id: &str) -> String {
    format!("{PREFIX}{panel_id}")
}

/// The reconciliation: from the floats the kernel wants and the float windows open now (label and logical geometry),
/// the windows to open, close and place. Pure; a geometry within 1 px is left alone (rounding of scaled sizes).
#[must_use]
pub fn plan(wanted: &[Wanted], open: &[(String, Rect)]) -> Vec<Action> {
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

/// How long a window's OS events count as the host's own after the host opened, raised or placed it.
pub const SETTLE: Duration = Duration::from_millis(400);

/// The guard against the host's own window changes echoing back as gestures (design note §1, §4): a pure value with
/// the clock supplied, so its timing is tested without sleeping.
#[derive(Debug, Default)]
pub struct EchoGuard {
    /// Labels whose OS events are the host's own until the instant given.
    settling: Vec<(String, Instant)>,
    /// The geometry the host last gave each float (or last reported for it).
    placed: Vec<(String, Rect)>,
}

impl EchoGuard {
    /// The host is about to open, raise or place `label` at `now`: its OS events count as the host's own for
    /// [`SETTLE`].
    pub fn applying(&mut self, label: &str, now: Instant) {
        self.settling.retain(|(l, _)| l != label);
        self.settling.push((label.to_owned(), now + SETTLE));
    }

    /// The host gave `label` the geometry `g` (or the user's gesture left it there).
    pub fn placed(&mut self, label: &str, g: Rect) {
        self.placed.retain(|(l, _)| l != label);
        self.placed.push((label.to_owned(), g));
    }

    /// Whether `label`'s OS events at `now` are still the host's own.
    #[must_use]
    pub fn settling(&self, label: &str, now: Instant) -> bool {
        self.settling.iter().any(|(l, until)| l == label && now < *until)
    }

    /// Whether an OS move or resize of `label` to `g` at `now` is the host's own change echoing back: within the
    /// settle time, or ending (within 1 px) where the host last placed it (late events of a new window).
    #[must_use]
    pub fn is_echo(&self, label: &str, g: Rect, now: Instant) -> bool {
        self.settling(label, now) || self.placed.iter().any(|(l, p)| l == label && near(*p, g))
    }
}

fn near(a: Rect, b: Rect) -> bool {
    let n = |x: i64, y: i64| (x - y).abs() <= 1;
    n(i64::from(a.0), i64::from(b.0))
        && n(i64::from(a.1), i64::from(b.1))
        && n(i64::from(a.2), i64::from(b.2))
        && n(i64::from(a.3), i64::from(b.3))
}

static GUARD: Mutex<EchoGuard> = Mutex::new(EchoGuard { settling: Vec::new(), placed: Vec::new() });

fn guard() -> std::sync::MutexGuard<'static, EchoGuard> {
    GUARD.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How long a float's geometry must stay unchanged before it is sent (a trailing debounce: a long drag sends one
/// `window.panel.set`, when the gesture pauses; design note §1).
pub const QUIET: Duration = Duration::from_millis(250);

/// The trailing debounce of float geometry gestures: a pure value with the clock supplied.
#[derive(Debug, Default)]
pub struct Debounce {
    /// Per panel: the latest geometry and when it arrived.
    pending: Vec<(String, Value, Instant)>,
}

impl Debounce {
    /// A geometry event of `panel` at `now`; `true` when no send is pending for it yet (the caller starts a timer).
    pub fn event(&mut self, panel: &str, geometry: Value, now: Instant) -> bool {
        let first = !self.pending.iter().any(|(p, _, _)| p == panel);
        self.pending.retain(|(p, _, _)| p != panel);
        self.pending.push((panel.to_owned(), geometry, now));
        first
    }

    /// At `now`: the geometry to send for `panel` when it has been quiet for [`QUIET`] (taken), else when to look
    /// again.
    pub fn due(&mut self, panel: &str, now: Instant) -> Result<Option<Value>, Instant> {
        let Some(i) = self.pending.iter().position(|(p, _, _)| p == panel) else { return Ok(None) };
        let at = self.pending[i].2 + QUIET;
        if now < at {
            return Err(at);
        }
        Ok(Some(self.pending.remove(i).1))
    }
}

static DEBOUNCE: Mutex<Debounce> = Mutex::new(Debounce { pending: Vec::new() });

/// One reconciliation at a time, always from the current kernel state (C41-R2-10): requests are numbered; a pass
/// reads the kernel for the newest request; a pass whose snapshot a newer request has superseded is not applied (the
/// next pass reads again). A pure value, tested with interleaved requests.
#[derive(Debug, Default)]
pub struct Coalescer {
    /// The newest request.
    latest: u64,
    /// The request the running pass read the kernel for.
    taken: u64,
    /// A pass is running (its worker loops until no request is left).
    running: bool,
}

impl Coalescer {
    /// A new request; `true` when no pass is running (the caller starts the worker).
    pub fn request(&mut self) -> bool {
        self.latest += 1;
        !std::mem::replace(&mut self.running, true)
    }

    /// The next pass: the newest request to read the kernel for, or `None` (the worker ends).
    pub fn next_pass(&mut self) -> Option<u64> {
        if self.taken < self.latest {
            self.taken = self.latest;
            Some(self.taken)
        } else {
            self.running = false;
            None
        }
    }

    /// Whether a snapshot read for request `n` is still current (no newer request since).
    #[must_use]
    pub fn current(&self, n: u64) -> bool {
        n == self.latest
    }
}

static PASSES: Mutex<Coalescer> = Mutex::new(Coalescer { latest: 0, taken: 0, running: false });

fn passes() -> std::sync::MutexGuard<'static, Coalescer> {
    PASSES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The main window is minimised (the kernel's state, from the last `shell.window` the host applied): the floats are
/// hidden with it (design note §5).
static MAIN_MINIMIZED: AtomicBool = AtomicBool::new(false);
/// The next pass raises every float it shows (`window.raiseAll.run`).
static RAISE: AtomicBool = AtomicBool::new(false);

/// What the next reconciliation should know about the main window: minimised or not, and whether to raise the floats.
pub fn main_window_state(app: &AppHandle, minimized: bool, raise: bool) {
    MAIN_MINIMIZED.store(minimized, Ordering::Release);
    if raise {
        RAISE.store(true, Ordering::Release);
    }
    reconcile(app);
}

/// Asks for a reconciliation of the float windows with the kernel. One pass runs at a time on its own thread; requests
/// made meanwhile are coalesced into one more pass that reads the kernel again (C41-R2-10).
pub fn reconcile(app: &AppHandle) {
    if !passes().request() {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("tt-float-reconcile".into()).spawn(move || {
        while let Some(n) = passes().next_pass() {
            let host = app.state::<Host>();
            let panels = host
                .run("window.panel.list", json!({}), false, None)
                .result
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            let wanted = wanted_of(&panels);
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let a = app.clone();
            let queued = app.run_on_main_thread(move || {
                // A snapshot a newer request superseded is never applied: the next pass reads the kernel again.
                if passes().current(n) {
                    apply(&a, &wanted);
                }
                let _ = tx.send(());
            });
            if queued.is_err() {
                break;
            }
            // The next pass starts after this one was applied (one at a time); a main thread that does not run the
            // closure (the application is ending) does not hold the worker forever.
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }
    });
    if spawned.is_err() {
        // No worker: let the next request start one.
        passes().running = false;
    }
}

/// The logical geometry and visibility of a window.
fn facts(w: &tauri::WebviewWindow) -> Option<(Rect, bool)> {
    let scale = w.scale_factor().unwrap_or(1.0);
    let pos = w.outer_position().ok()?;
    let size = w.inner_size().ok()?;
    let lg = |v: f64| (v / scale).round();
    let g = (
        lg(f64::from(pos.x)) as i32,
        lg(f64::from(pos.y)) as i32,
        lg(f64::from(size.width)) as u32,
        lg(f64::from(size.height)) as u32,
    );
    Some((g, w.is_visible().unwrap_or(true)))
}

fn apply(app: &AppHandle, wanted: &[Wanted]) {
    let windows: Vec<(String, tauri::WebviewWindow)> =
        app.webview_windows().into_iter().filter(|(l, _)| l.starts_with(PREFIX)).collect();
    let open: Vec<(String, Rect)> = windows.iter().filter_map(|(l, w)| facts(w).map(|(g, _)| (l.clone(), g))).collect();
    let shown = !MAIN_MINIMIZED.load(Ordering::Acquire);
    let now = Instant::now();
    for action in plan(wanted, &open) {
        match action {
            Action::Open(w) => {
                let l = label(&w.panel_id);
                {
                    let mut g = guard();
                    g.applying(&l, now);
                    g.placed(&l, w.geometry);
                }
                if let Some(host) = app.try_state::<Host>() {
                    host.expect_window(&l);
                }
                let (x, y, wd, ht) = w.geometry;
                let url = WebviewUrl::App(format!("index.html?panel={}", w.panel_id).into());
                let built = WebviewWindowBuilder::new(app, &l, url)
                    .title(if w.title.is_empty() { w.panel_id.clone() } else { w.title.clone() })
                    .inner_size(f64::from(wd), f64::from(ht))
                    .position(f64::from(x), f64::from(y))
                    .min_inner_size(160.0, 120.0)
                    .visible(shown)
                    .build();
                match built {
                    // A float has no menu bar of its own (design note §2); on macOS the menu is the application's.
                    Ok(win) => {
                        if !cfg!(target_os = "macos") {
                            let _ = win.remove_menu();
                        }
                    }
                    Err(e) => {
                        if let Some(host) = app.try_state::<Host>() {
                            host.forget_window_reports(&l);
                        }
                        crate::shell::notice(
                            app,
                            "window.floatFailed",
                            "warning",
                            &format!("The floating panel could not open: {e}"),
                        );
                    }
                }
            }
            Action::Close(l) => {
                if let Some(w) = app.get_webview_window(&l) {
                    let _ = w.destroy();
                }
            }
            Action::Place(l, (x, y, wd, ht)) => {
                if let Some(w) = app.get_webview_window(&l) {
                    // Moving then resizing passes through an in-between geometry: the settle time covers it.
                    {
                        let mut g = guard();
                        g.applying(&l, now);
                        g.placed(&l, (x, y, wd, ht));
                    }
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
    // Every float the kernel wants is shown exactly while the main window is not minimised; the global flag is already
    // in `wanted` (a hidden float is not wanted, so its window was closed above). Raise all raises them.
    let raise = RAISE.swap(false, Ordering::AcqRel);
    for (l, w) in &windows {
        if !wanted.iter().any(|x| label(&x.panel_id) == *l) {
            continue;
        }
        let visible = facts(w).is_some_and(|(_, v)| v);
        if visible != shown || (raise && shown) {
            guard().applying(l, now);
            if shown {
                let _ = w.show();
                if raise {
                    let _ = w.set_focus();
                }
            } else {
                let _ = w.hide();
            }
        }
    }
    if raise && let Some(main) = app.get_webview_window("main") {
        let _ = main.set_focus();
    }
}

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
            if guard().settling(&l, Instant::now()) {
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
            let now = Instant::now();
            {
                let mut guard = guard();
                if guard.is_echo(&l, g, now) {
                    return;
                }
                guard.placed(&l, g);
            }
            let geometry = json!({ "x": g.0, "y": g.1, "width": g.2, "height": g.3 });
            let first = DEBOUNCE.lock().unwrap_or_else(PoisonError::into_inner).event(&panel_id, geometry, now);
            if first {
                let _ = std::thread::Builder::new().name("tt-float-geometry".into()).spawn(move || {
                    loop {
                        let due =
                            DEBOUNCE.lock().unwrap_or_else(PoisonError::into_inner).due(&panel_id, Instant::now());
                        match due {
                            Err(at) => std::thread::sleep(at.saturating_duration_since(Instant::now())),
                            Ok(None) => return,
                            Ok(Some(g)) => {
                                let host = app.state::<Host>();
                                let _ = host.run(
                                    "window.panel.set",
                                    json!({ "panelId": panel_id, "geometry": g, "focus": false }),
                                    false,
                                    None,
                                );
                                return;
                            }
                        }
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

    fn w(id: &str, g: Rect) -> Wanted {
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

    #[test]
    fn the_echo_guard_settles_on_a_supplied_clock() {
        let t0 = Instant::now();
        let mut g = EchoGuard::default();
        g.applying("float-history", t0);
        g.placed("float-history", (100, 100, 400, 300));
        // Inside the settle time every event is the host's own; after it, only the placed geometry (within 1 px).
        assert!(g.is_echo("float-history", (5, 5, 50, 50), t0 + SETTLE - Duration::from_millis(1)));
        assert!(!g.is_echo("float-history", (5, 5, 50, 50), t0 + SETTLE));
        assert!(g.is_echo("float-history", (101, 99, 401, 300), t0 + SETTLE * 10));
        assert!(!g.is_echo("float-history", (102, 100, 400, 300), t0 + SETTLE * 10));
        // Another window is not covered; a new placement replaces the old one.
        assert!(!g.is_echo("float-colour", (100, 100, 400, 300), t0));
        g.placed("float-history", (300, 300, 400, 300));
        assert!(!g.is_echo("float-history", (100, 100, 400, 300), t0 + SETTLE));
        assert!(!g.settling("float-history", t0 + SETTLE));
        g.applying("float-history", t0 + SETTLE);
        assert!(g.settling("float-history", t0 + SETTLE + Duration::from_millis(399)));
    }

    #[test]
    fn a_long_drag_sends_one_geometry_when_it_pauses() {
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        let mut d = Debounce::default();
        assert!(d.event("history", json!(1), t0), "the first event starts the timer");
        assert!(!d.event("history", json!(2), t0 + ms(200)));
        assert!(!d.event("history", json!(3), t0 + ms(400)));
        // The drag goes on: nothing is due until it has been quiet for QUIET after the last event.
        assert_eq!(d.due("history", t0 + ms(500)), Err(t0 + ms(400) + QUIET));
        assert_eq!(d.due("history", t0 + ms(400) + QUIET), Ok(Some(json!(3))));
        assert_eq!(d.due("history", t0 + ms(2000)), Ok(None), "sent once");
        assert!(d.event("history", json!(4), t0 + ms(3000)), "a new gesture starts a new timer");
    }

    #[test]
    fn reconciliation_passes_never_apply_a_superseded_snapshot() {
        let mut c = Coalescer::default();
        // Hide requested: the worker starts and reads the kernel for request 1.
        assert!(c.request());
        assert_eq!(c.next_pass(), Some(1));
        // Show requested while pass 1 runs: no second worker; snapshot 1 is superseded and not applied.
        assert!(!c.request());
        assert!(!c.current(1));
        assert_eq!(c.next_pass(), Some(2));
        assert!(c.current(2));
        // Two more requests during pass 2 coalesce into one pass that reads the kernel for the newest.
        assert!(!c.request());
        assert!(!c.request());
        assert!(!c.current(2));
        assert_eq!(c.next_pass(), Some(4));
        assert!(c.current(4));
        assert_eq!(c.next_pass(), None, "nothing left: the worker ends");
        assert!(c.request(), "a later request starts a worker again");
        assert_eq!(c.next_pass(), Some(5));
    }

    #[test]
    fn a_globally_hidden_float_is_not_wanted_and_its_window_closes() {
        let shown = json!({ "panelId": "history", "title": "History", "visible": true, "floating": true,
                            "geometry": { "x": 10, "y": 20, "width": 300, "height": 200 } });
        let open = vec![("float-history".to_owned(), (10, 20, 300, 200))];
        assert!(plan(&wanted_of(&[shown]), &open).is_empty());
        // `window.floatingPanels.set {visible: false}`: the kernel lists the float as not visible.
        let hidden = json!({ "panelId": "history", "title": "History", "visible": false, "floating": true,
                             "geometry": { "x": 10, "y": 20, "width": 300, "height": 200 } });
        assert_eq!(plan(&wanted_of(&[hidden]), &open), vec![Action::Close("float-history".into())]);
    }
}
