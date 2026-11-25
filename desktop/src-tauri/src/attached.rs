//! Attached mode's routes into the windows (M4 C4.2b; design note 2 §6, design note 5 §2, ruling D-6), installed on
//! the executor as its [`AttachedIo`]:
//! - an agent's `pointer` call for a view a window shows goes to that window's input queue (`agent-input`), is played
//!   through the window's own collector and overlay, and comes back through `gesture_commit` (P1); a user press during
//!   it aborts it through `gesture_abort` (P2: `ui.busy`, `reason: "userInput"`, SI-135);
//! - `inspect.screenshot` captures the window's own pixels through the platform's webview snapshot ([`crate::capture`]),
//!   cropped to the target's rectangle.
//!
//! Which window shows which view is what the windows' canvases report (`canvas_presence`): a view without a mounted
//! canvas keeps the headless path.
//!
//! A routed call waits in [`Routed`] (owned by the host, so it outlives an engine restart) with the engine generation
//! it started on and the window it went to. It is answered at once, nothing committed, when its engine is replaced
//! (D-7b: the SI-120 refusal, `command.timeout` with `reason: "engineRestarted"`), when its window is destroyed or
//! reloaded, or when that window's canvas for the view goes away (D-7a: `ui.viewNotFound`, `reason: "windowClosed"`,
//! SI-141).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
use tt_executor::{AttachedIo, CapturedPng, CommandError, Envelope, Origin};

/// The host event carrying an agent's gesture to a window.
pub const AGENT_INPUT_EVENT: &str = "agent-input";

/// The host event carrying an agent's Shift+F10 to the main window (M5 rulings E-48, E-52): the window opens the named
/// control's context menu, or its focused control's, and answers what it did through `context_menu_answer`.
pub const AGENT_CONTEXT_MENU_EVENT: &str = "agent-context-menu";

/// The view name a routed Shift+F10 waits under in [`Routed`] (it acts on the window, not on a view's canvas).
const CONTEXT_MENU: &str = "contextMenu";

/// Where a window's canvas for a view is, in the window's view pixels, and its device pixel ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    /// `x, y, w, h` in the window's view pixels.
    pub rect: [f64; 4],
    /// Device pixels per view pixel.
    pub dpr: f64,
}

/// One routed agent gesture waiting for its window.
#[derive(Debug)]
pub struct Pending {
    /// The agent's origin, for the commit.
    pub origin: Origin,
    /// The view it acts on.
    pub view: String,
    /// The window it was sent to.
    pub window: String,
    /// The engine generation its call started on.
    pub generation: u64,
    tx: mpsc::Sender<Envelope>,
}

impl Pending {
    /// Answers the waiting agent call.
    pub fn answer(&self, env: Envelope) {
        let _ = self.tx.send(env);
    }
}

/// The routed calls waiting for their windows, by request id (ids are never reused, across restarts too).
#[derive(Debug)]
pub struct Routed {
    pending: Mutex<HashMap<u64, Pending>>,
    next: AtomicU64,
}

impl Default for Routed {
    fn default() -> Self {
        Self { pending: Mutex::new(HashMap::new()), next: AtomicU64::new(1) }
    }
}

/// The SI-120 refusal of a routed call whose engine was replaced (ruling O-14, D-7b).
fn restarted(started: u64, now: u64) -> Envelope {
    Envelope::error(
        refusal(
            "command.timeout",
            "the engine was restarted while the window played this gesture; nothing of it was committed",
            json!({ "reason": "engineRestarted", "generation": started, "currentGeneration": now }),
        ),
        None,
    )
}

/// The refusal of a routed call whose window or canvas went away (D-7a; SI-141).
fn window_gone(view: &str) -> Envelope {
    Envelope::error(
        refusal(
            "ui.viewNotFound",
            format!("the window showing {view} closed before the gesture was committed; nothing was committed"),
            json!({ "reason": "windowClosed", "viewId": view }),
        ),
        None,
    )
}

impl Routed {
    /// Registers a call for `view` sent to `window` on engine generation `generation`: its id and the receiver its
    /// answer arrives on.
    pub fn register(
        &self,
        origin: &Origin,
        view: &str,
        window: &str,
        generation: u64,
    ) -> (u64, mpsc::Receiver<Envelope>) {
        let id = self.next.fetch_add(1, Ordering::AcqRel);
        let (tx, rx) = mpsc::channel();
        let p = Pending { origin: origin.clone(), view: view.to_owned(), window: window.to_owned(), generation, tx };
        self.pending.lock().unwrap_or_else(PoisonError::into_inner).insert(id, p);
        (id, rx)
    }

    /// Removes and returns call `id`.
    pub fn take(&self, id: u64) -> Option<Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner).remove(&id)
    }

    /// Calls waiting now.
    pub fn len(&self) -> usize {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Whether no call waits.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn drain(&self, mut gone: impl FnMut(&Pending) -> bool) -> Vec<Pending> {
        let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        let ids: Vec<u64> = map.iter().filter(|(_, p)| gone(p)).map(|(id, _)| *id).collect();
        ids.into_iter().filter_map(|id| map.remove(&id)).collect()
    }

    /// The engine was replaced and generation `now` runs: every call of an earlier generation answers the SI-120
    /// refusal at once (D-7b), nothing committed.
    pub fn engine_restarted(&self, now: u64) {
        for p in self.drain(|p| p.generation < now) {
            p.answer(restarted(p.generation, now));
        }
    }

    /// `window` was destroyed or reloaded (`view: None`), or its canvas for `view` went away: its calls answer
    /// `ui.viewNotFound` at once (D-7a), nothing committed.
    pub fn window_gone(&self, window: &str, view: Option<&str>) {
        for p in self.drain(|p| p.window == window && view.is_none_or(|v| p.view == v)) {
            p.answer(window_gone(&p.view));
        }
    }
}

/// The host's [`AttachedIo`].
#[derive(Debug)]
pub struct WindowIo {
    app: AppHandle,
    /// Window label → view id → presence.
    presence: Mutex<HashMap<String, HashMap<String, Presence>>>,
}

fn refusal(code: &str, message: impl Into<String>, details: Value) -> CommandError {
    CommandError::with_details(code, message.into(), details)
}

impl WindowIo {
    /// Routes of the application `app`.
    pub fn new(app: AppHandle) -> Self {
        Self { app, presence: Mutex::new(HashMap::new()) }
    }

    fn host(&self) -> tauri::State<'_, crate::host::Host> {
        self.app.state::<crate::host::Host>()
    }

    /// A window's canvas for `view` appeared, moved, or (`None`) went away; a call routed to it then answers at once.
    pub fn set_presence(&self, window: &str, view: &str, presence: Option<Presence>) {
        {
            let mut p = self.presence.lock().unwrap_or_else(PoisonError::into_inner);
            let views = p.entry(window.to_owned()).or_default();
            match presence {
                Some(v) => {
                    views.insert(view.to_owned(), v);
                }
                None => {
                    views.remove(view);
                }
            }
        }
        if presence.is_none() {
            self.host().routed().window_gone(window, Some(view));
        }
    }

    /// Everything a closed or reloaded window reported is forgotten (its routed calls are answered by
    /// [`crate::host::Host::forget_window_reports`]).
    pub fn forget_window(&self, window: &str) {
        self.presence.lock().unwrap_or_else(PoisonError::into_inner).remove(window);
    }

    /// Whether window `label` is open and shown now (a closed float may be hidden before it is destroyed).
    fn shown(&self, label: &str) -> bool {
        self.app.get_webview_window(label).is_some_and(|w| w.is_visible().unwrap_or(false))
    }

    /// The window showing `view` at the time of the call (C42B-R1-05): among the windows whose canvas reported the view,
    /// those open and shown now; a float of the view wins over the main window (a floating panel is not in the main
    /// window).
    fn window_of(&self, view: &str) -> Option<(String, Presence)> {
        let mut found: Vec<(String, Presence)> = {
            let p = self.presence.lock().unwrap_or_else(PoisonError::into_inner);
            p.iter().filter_map(|(w, views)| views.get(view).map(|pr| (w.clone(), *pr))).collect()
        };
        found.retain(|(w, _)| self.shown(w));
        found.sort_by_key(|(w, _)| !w.starts_with(crate::floats::PREFIX));
        found.into_iter().next()
    }

    /// The window's commit, made on engine generation `generation`, of an agent gesture it played: runs it with the
    /// agent's origin, never routed again, and answers both the window and the waiting agent call. A commit of another
    /// generation than its call's or the running engine's is refused with the SI-120 refusal (D-7b), nothing run.
    pub fn finish(&self, id: u64, generation: u64, request: Value) -> Envelope {
        let host = self.host();
        let Some(p) = host.routed().take(id) else {
            return Envelope::error(
                refusal("ref.notFound", format!("no agent gesture {id} is waiting"), json!({ "agentRequest": id })),
                None,
            );
        };
        let now = host.generation();
        if p.generation != now || generation != now {
            let env = restarted(p.generation.min(generation), now);
            p.answer(env.clone());
            return env;
        }
        let env = host.exec().pointer_in_window(p.origin.clone(), request);
        p.answer(env.clone());
        env
    }

    /// The window's answer to a routed Shift+F10 (M5 ruling E-52): `opened` (a menu opened: `menuId`), `noMenu` (the
    /// control exists and offers none: `handled: false`), `notFound` (no control has that id: `view.notFound`).
    pub fn context_menu_answer(&self, id: u64, outcome: &str, target: Option<String>, menu_id: Option<String>) {
        let Some(p) = self.host().routed().take(id) else { return };
        let env = match outcome {
            "opened" => Envelope::ok(json!({ "handled": true, "contextMenu": target, "menuId": menu_id }), None),
            "noMenu" => Envelope::ok(json!({ "handled": false, "reason": "noContextMenu", "control": target }), None),
            // The window drew nothing within the bound (a minimised window): no menu opened, and the window retired the
            // one the event asked for, so it does not appear later (M5 ruling E-66).
            "notRendered" => {
                Envelope::ok(json!({ "handled": false, "reason": "notRendering", "control": target }), None)
            }
            _ => Envelope::error(
                refusal(
                    "view.notFound",
                    match &target {
                        Some(t) => format!("the window has no control `{t}`"),
                        None => "the window has no focused control".to_owned(),
                    },
                    json!({ "view": target, "reason": "noControl", "pointer": "/view" }),
                ),
                None,
            ),
        };
        p.answer(env);
    }

    /// A user press during an agent gesture (P2): the agent's gesture is discarded and its call answers `ui.busy`
    /// with `reason: "userInput"` (SI-135); nothing is committed.
    pub fn abort(&self, id: u64) {
        if let Some(p) = self.host().routed().take(id) {
            let e = refusal(
                "ui.busy",
                "the user pressed in the view while the agent's gesture was being played; nothing was committed",
                json!({ "reason": "userInput", "viewId": p.view }),
            );
            p.answer(Envelope::error(e, None));
        }
    }
}

impl AttachedIo for WindowIo {
    fn route_pointer(&self, view: &str, origin: &Origin, request: &Value, budget: Duration) -> Option<Envelope> {
        let (window, _) = self.window_of(view)?;
        let host = self.host();
        if let Some(refused) = host.activate_gesture_tool(origin, view, request) {
            return Some(refused);
        }
        let generation = host.generation();
        let (id, rx) = host.routed().register(origin, view, &window, generation);
        // The window plays it only while it follows the generation it was sent on (D-7b).
        let payload = json!({ "requestId": id, "viewId": view, "generation": generation, "request": request });
        let sent = tt_engine::wire::to_wire_json(&payload)
            .map_err(|e| e.message)
            .and_then(|text| self.app.emit_to(window.as_str(), AGENT_INPUT_EVENT, text).map_err(|e| e.to_string()));
        if let Err(e) = sent {
            host.routed().take(id);
            return Some(Envelope::error(
                refusal("internal", format!("cannot reach the window showing {view}: {e}"), json!({ "viewId": view })),
                None,
            ));
        }
        match rx.recv_timeout(budget) {
            Ok(env) => Some(env),
            Err(_) => {
                host.routed().take(id);
                Some(Envelope::error(
                    refusal(
                        "command.timeout",
                        format!("the window showing {view} did not play the gesture within {} ms", budget.as_millis()),
                        json!({ "commandId": "pointer", "viewId": view }),
                    ),
                    None,
                ))
            }
        }
    }

    fn route_context_menu(&self, target: Option<&str>, origin: &Origin, budget: Duration) -> Option<Envelope> {
        let window = "main";
        self.app.get_webview_window(window)?;
        let host = self.host();
        let generation = host.generation();
        let (id, rx) = host.routed().register(origin, CONTEXT_MENU, window, generation);
        // The budget goes with the request: the window retires a turn whose caller has timed out (M5 ruling E-63).
        let budget_ms = u64::try_from(budget.as_millis()).unwrap_or(u64::MAX);
        let payload = json!({ "requestId": id, "target": target, "generation": generation, "budgetMs": budget_ms });
        let sent = tt_engine::wire::to_wire_json(&payload)
            .map_err(|e| e.message)
            .and_then(|text| self.app.emit_to(window, AGENT_CONTEXT_MENU_EVENT, text).map_err(|e| e.to_string()));
        if let Err(e) = sent {
            host.routed().take(id);
            return Some(Envelope::error(
                refusal("internal", format!("cannot reach the main window: {e}"), json!({ "view": target })),
                None,
            ));
        }
        match rx.recv_timeout(budget) {
            Ok(env) => Some(env),
            Err(_) => {
                host.routed().take(id);
                Some(Envelope::error(
                    refusal(
                        "command.timeout",
                        format!("the window did not answer the context-menu key within {} ms", budget.as_millis()),
                        json!({ "commandId": "key", "view": target }),
                    ),
                    None,
                ))
            }
        }
    }

    fn capture(
        &self,
        target: &str,
        region: Option<[f64; 4]>,
        max_side: u32,
        budget: Duration,
    ) -> Result<CapturedPng, CommandError> {
        // The target, resolved from the windows open now (C42B-R1-05): the whole main window; a view's canvas rectangle in
        // the window showing it (a float or the main window); a shown float's whole window for a panel without a canvas.
        let float = format!("{}{target}", crate::floats::PREFIX);
        let (window, rect, dpr, view_id) = if target == "window" {
            ("main".to_owned(), None, None, String::new())
        } else if let Some((w, p)) = self.window_of(target) {
            (w, Some(p.rect), Some(p.dpr), target.to_owned())
        } else if self.shown(&float) {
            (float, None, None, target.to_owned())
        } else {
            return Err(refusal(
                "view.notFound",
                format!("no window shows `{target}`"),
                json!({ "target": target, "targets": ["window", "drawingView"] }),
            ));
        };
        let webview = self.app.get_webview_window(&window).ok_or_else(|| {
            refusal("view.notFound", format!("window `{window}` is not open"), json!({ "target": target }))
        })?;
        let scale = dpr.unwrap_or_else(|| webview.scale_factor().unwrap_or(1.0));
        let whole = webview.inner_size().map(|s| [0.0, 0.0, f64::from(s.width) / scale, f64::from(s.height) / scale]);
        let base = rect.or_else(|| whole.ok()).unwrap_or([0.0, 0.0, 1.0, 1.0]);
        // The rectangle to keep, in the window's view pixels: the region within the target.
        let keep = match region {
            Some([x, y, w, h]) => [base[0] + x, base[1] + y, w, h],
            None => base,
        };
        let (pw, ph) = ((keep[2] * scale).round().max(1.0), (keep[3] * scale).round().max(1.0));
        // S2: the caps are checked before capturing.
        if pw.max(ph) > f64::from(max_side) {
            return Err(refusal(
                "render.inlineTooLarge",
                format!("a {pw} x {ph} screenshot is over the inline limit of {max_side} pixels"),
                json!({ "width": pw, "height": ph, "maxInlinePixels": max_side }),
            ));
        }
        // D-62 item 2, D-65 item 2, D-69 item 1: a capture never contains the agent token. The guard is granted only
        // when every delivery of the token has been retired and the captured window is not between a navigation start
        // and its new document's paint: the windows are asked to hide it, and the capture waits (bounded) for the
        // acknowledgements; nothing is delivered until the capture ends.
        let host = self.host();
        let app = self.app.clone();
        let bound = budget.min(Duration::from_secs(2));
        let token_refusal = |why: &str| {
            refusal(
                "render.failed",
                format!("the agent token is shown in Preferences and {why}: nothing was captured"),
                json!({ "target": target, "reason": "tokenShown" }),
            )
        };
        let _hidden = host
            .reveals
            .begin_capture(&window, bound, move || {
                let _ = app.emit(crate::token_reveal::TOKEN_HIDE_EVENT, ());
            })
            .ok_or_else(|| token_refusal("was not hidden in time"))?;
        // D-69 item 1, belt and braces: while the token field exists, a capture of the main window has the field's
        // rectangle painted over by the host, from the page's answers just before and just after the snapshot.
        let field = window == crate::token_reveal::MAIN && host.reveals.field_present();
        let ask_rect = || {
            let app = self.app.clone();
            host.reveals.ask_rect(bound, move |request| {
                let _ = app.emit_to(
                    crate::token_reveal::MAIN,
                    crate::token_reveal::TOKEN_RECT_EVENT,
                    json!({ "request": request }),
                );
            })
        };
        let before = if field {
            Some(ask_rect().ok_or_else(|| token_refusal("its place was not reported in time"))?)
        } else {
            None
        };
        let png = crate::capture::snapshot(&webview, budget).map_err(|m| {
            refusal("render.failed", format!("the window could not be captured: {m}"), json!({ "target": target }))
        })?;
        let after = if field {
            Some(ask_rect().ok_or_else(|| token_refusal("its place was not reported in time"))?)
        } else {
            None
        };
        let (w, h, mut px) = tt_render::decode_png(&png, 16_384)
            .ok_or_else(|| refusal("render.failed", "the window capture is not a PNG", json!({ "target": target })))?;
        let (w, h) = (u32::from(w), u32::from(h));
        if field {
            match crate::token_reveal::overpaint_rect(before.flatten(), after.flatten()) {
                crate::token_reveal::Overpaint::Nothing => {}
                crate::token_reveal::Overpaint::Paint(r) => crate::token_reveal::overpaint(&mut px, w, h, r, scale),
                crate::token_reveal::Overpaint::Refuse => return Err(token_refusal("moved during the capture")),
            }
        }
        // Crop in device pixels, clamped to the capture.
        let x0 = ((keep[0] * scale).round().max(0.0) as u32).min(w.saturating_sub(1));
        let y0 = ((keep[1] * scale).round().max(0.0) as u32).min(h.saturating_sub(1));
        let cw = (pw as u32).min(w - x0).max(1);
        let ch = (ph as u32).min(h - y0).max(1);
        let mut out = Vec::with_capacity((cw * ch * 4) as usize);
        for row in y0..y0 + ch {
            let start = ((row * w + x0) * 4) as usize;
            out.extend_from_slice(&px[start..start + (cw * 4) as usize]);
        }
        // The capture is opaque, so its straight and premultiplied forms are equal.
        let png = tt_render::encode_png(&out, cw, ch)
            .map_err(|e| refusal("render.failed", e.to_string(), json!({ "target": target })))?;
        Ok(CapturedPng { png, width: cw, height: ch, view_id })
    }
}
