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

/// Where a window's canvas for a view is, in the window's view pixels, and its device pixel ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    /// `x, y, w, h` in the window's view pixels.
    pub rect: [f64; 4],
    /// Device pixels per view pixel.
    pub dpr: f64,
}

#[derive(Debug)]
struct Pending {
    origin: Origin,
    view: String,
    tx: mpsc::Sender<Envelope>,
}

/// The host's [`AttachedIo`].
#[derive(Debug)]
pub struct WindowIo {
    app: AppHandle,
    /// Window label → view id → presence.
    presence: Mutex<HashMap<String, HashMap<String, Presence>>>,
    pending: Mutex<HashMap<u64, Pending>>,
    next: AtomicU64,
}

fn refusal(code: &str, message: impl Into<String>, details: Value) -> CommandError {
    CommandError::with_details(code, message.into(), details)
}

impl WindowIo {
    /// Routes of the application `app`.
    pub fn new(app: AppHandle) -> Self {
        Self { app, presence: Mutex::new(HashMap::new()), pending: Mutex::new(HashMap::new()), next: AtomicU64::new(1) }
    }

    /// A window's canvas for `view` appeared, moved, or (`None`) went away.
    pub fn set_presence(&self, window: &str, view: &str, presence: Option<Presence>) {
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

    /// Everything a closed or reloaded window reported is forgotten.
    pub fn forget_window(&self, window: &str) {
        self.presence.lock().unwrap_or_else(PoisonError::into_inner).remove(window);
    }

    fn window_of(&self, view: &str) -> Option<(String, Presence)> {
        let p = self.presence.lock().unwrap_or_else(PoisonError::into_inner);
        // A float of the view wins over the main window (a floating panel is not in the main window).
        let mut found: Vec<(String, Presence)> =
            p.iter().filter_map(|(w, views)| views.get(view).map(|pr| (w.clone(), *pr))).collect();
        found.sort_by_key(|(w, _)| !w.starts_with(crate::floats::PREFIX));
        found.into_iter().next()
    }

    /// The window's commit of an agent gesture it played: runs it with the agent's origin, never routed again, and
    /// answers both the window and the waiting agent call.
    pub fn finish(&self, id: u64, request: Value) -> Envelope {
        let pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
        let Some(p) = pending else {
            return Envelope::error(
                refusal("ref.notFound", format!("no agent gesture {id} is waiting"), json!({ "agentRequest": id })),
                None,
            );
        };
        let host = self.app.state::<crate::host::Host>();
        let env = host.exec().pointer_in_window(p.origin, request);
        let _ = p.tx.send(env.clone());
        env
    }

    /// A user press during an agent gesture (P2): the agent's gesture is discarded and its call answers `ui.busy`
    /// with `reason: "userInput"` (SI-135); nothing is committed.
    pub fn abort(&self, id: u64) {
        let pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
        if let Some(p) = pending {
            let e = refusal(
                "ui.busy",
                "the user pressed in the view while the agent's gesture was being played; nothing was committed",
                json!({ "reason": "userInput", "viewId": p.view }),
            );
            let _ = p.tx.send(Envelope::error(e, None));
        }
    }
}

impl AttachedIo for WindowIo {
    fn route_pointer(&self, view: &str, origin: &Origin, request: &Value, budget: Duration) -> Option<Envelope> {
        let (window, _) = self.window_of(view)?;
        let id = self.next.fetch_add(1, Ordering::AcqRel);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, Pending { origin: origin.clone(), view: view.to_owned(), tx });
        let payload = json!({ "requestId": id, "viewId": view, "request": request });
        let sent = tt_engine::wire::to_wire_json(&payload)
            .map_err(|e| e.message)
            .and_then(|text| self.app.emit_to(window.as_str(), AGENT_INPUT_EVENT, text).map_err(|e| e.to_string()));
        if let Err(e) = sent {
            self.pending.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
            return Some(Envelope::error(
                refusal("internal", format!("cannot reach the window showing {view}: {e}"), json!({ "viewId": view })),
                None,
            ));
        }
        match rx.recv_timeout(budget) {
            Ok(env) => Some(env),
            Err(_) => {
                self.pending.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
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

    fn capture(
        &self,
        target: &str,
        region: Option<[f64; 4]>,
        max_side: u32,
        budget: Duration,
    ) -> Result<CapturedPng, CommandError> {
        // The target: the whole main window, a float's whole window, or a view's canvas rectangle.
        let float = format!("{}{target}", crate::floats::PREFIX);
        let (window, rect, dpr, view_id) = if target == "window" {
            ("main".to_owned(), None, None, String::new())
        } else if self.app.get_webview_window(&float).is_some() {
            (float, None, None, target.to_owned())
        } else if let Some((w, p)) = self.window_of(target) {
            (w, Some(p.rect), Some(p.dpr), target.to_owned())
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
        let png = crate::capture::snapshot(&webview, budget).map_err(|m| {
            refusal("render.failed", format!("the window could not be captured: {m}"), json!({ "target": target }))
        })?;
        let (w, h, px) = tt_render::decode_png(&png, 16_384)
            .ok_or_else(|| refusal("render.failed", "the window capture is not a PNG", json!({ "target": target })))?;
        let (w, h) = (u32::from(w), u32::from(h));
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
