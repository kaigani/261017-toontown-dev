//! The agent token's deliveries to the windows, and the window capture's guard (ruling D-62 item 2, C44A-R1-41; D-65
//! item 2; D-69 item 1; design note 2 §2): no surface an agent can read ever contains the token.
//!
//! The host is the authority, not the page. Every answer of `agent_token` is a numbered delivery, counted as possibly
//! on screen until it is retired, and it is retired only by:
//! - an acknowledgement (`token_hidden`) the page's window-level controller sends after the replacement has been painted
//!   (the mask, or the page without the field), never by an unmounting component; or
//! - the page's bootstrap report that a new document has painted (`page_ready`) after a navigation the host has itself
//!   seen start and finish (its own page-load events), once per navigation (ruling D-72 item 1): the old document's
//!   deliveries end with its pixels; a report at any other time is ignored; or
//! - the window's destruction.
//!
//! A navigation start retires nothing: it marks the window loading, not capturable, until the new document says it
//! has painted. A capture (`inspect.screenshot`, whoever asked for it) first takes a [`CaptureGuard`]: granted only when
//! no delivery is outstanding and the captured window is not loading, after asking the windows to hide what they show
//! (`token-hide`) and waiting, bounded, for their acknowledgements; while a guard is held `agent_token` delivers
//! nothing. Belt and braces (D-69 item 1): while the token field exists, a capture of the main window has the field's
//! rectangle overpainted by the host, from the page's answers just before and just after the snapshot
//! ([`overpaint_rect`]); answers that differ, or no answer within the bound, refuse the capture.
//!
//! Only the main window takes part ([`caller_allowed`]): Preferences lives there, and a float's call is refused.

use std::collections::{HashMap, HashSet};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The host event that asks every window to hide a shown token now.
pub const TOKEN_HIDE_EVENT: &str = "token-hide";
/// The host event that asks the main window where the token field is (`{request}`; answered by `token_rect`).
pub const TOKEN_RECT_EVENT: &str = "token-rect";
/// The only window whose page may receive the token or report on it.
pub const MAIN: &str = "main";
/// The margin (CSS pixels) added around the field's rectangle when it is overpainted, and the most the field may move
/// between the two reads of its rectangle.
pub const OVERPAINT_MARGIN: f64 = 4.0;
/// The colour the host paints over the token field in a capture (opaque RGBA).
pub const OVERPAINT_RGBA: [u8; 4] = [0x80, 0x80, 0x80, 0xFF];

/// D-69 item 1: `agent_token`, `token_hidden`, `token_field` and `token_rect` are callable from the main window only.
#[must_use]
pub fn caller_allowed(window: &str) -> bool {
    window == MAIN
}

/// A rectangle `[x, y, width, height]` in the window's CSS pixels.
pub type Rect = [f64; 4];

/// Where a window's navigation stands, from the host's own page-load events (D-72 item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Navigation {
    /// Started, not finished: a `page_ready` now is too early.
    Started,
    /// Finished: the new document's `page_ready` is awaited (accepted once).
    Finished,
}

#[derive(Debug, Default)]
struct State {
    /// Deliveries not yet retired, with the window each went to.
    outstanding: HashMap<u64, String>,
    /// The last delivery's number.
    next: u64,
    /// Captures in progress.
    capturing: usize,
    /// Windows between a navigation start and their new document's accepted `page_ready`: not capturable.
    loading: HashSet<String>,
    /// D-72 item 1: each window's navigation as the host's own page-load events report it; `page_ready` is accepted
    /// only in `Finished`, once.
    navigations: HashMap<String, Navigation>,
    /// Whether the main window's page shows the token field now (`token_field`).
    field: bool,
    /// The last rectangle request's number, and the answers received (`None`: the page has no field).
    next_rect: u64,
    rects: HashMap<u64, Option<Rect>>,
}

/// The token's deliveries, and the captures in progress.
#[derive(Debug, Default)]
pub struct TokenReveals {
    state: Mutex<State>,
    changed: Condvar,
}

/// Held while a capture runs: no delivery is made until it is dropped.
#[derive(Debug)]
pub struct CaptureGuard<'a> {
    reveals: &'a TokenReveals,
}

impl Drop for CaptureGuard<'_> {
    fn drop(&mut self) {
        let mut s = self.reveals.lock();
        s.capturing = s.capturing.saturating_sub(1);
    }
}

impl TokenReveals {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn notify(&self) {
        self.changed.notify_all();
    }

    /// A delivery of the token to window `window`: its number, or `None` while a capture runs or for any window but the
    /// main one (nothing is delivered).
    pub fn deliver(&self, window: &str) -> Option<u64> {
        if !caller_allowed(window) {
            return None;
        }
        let mut s = self.lock();
        if s.capturing > 0 {
            return None;
        }
        s.next += 1;
        let id = s.next;
        s.outstanding.insert(id, window.to_owned());
        Some(id)
    }

    /// Window `window` acknowledges delivery `id` hidden: sent by its window-level controller after the replacement was
    /// painted, or at once for a delivery never painted. A delivery of another window, or an unknown one, is left as it
    /// is.
    pub fn hidden(&self, window: &str, id: u64) {
        let mut s = self.lock();
        if s.outstanding.get(&id).is_some_and(|w| w == window) {
            s.outstanding.remove(&id);
        }
        drop(s);
        self.notify();
    }

    /// A navigation started in `window` (the host's own page-load event; a reload, or the first load): its old
    /// document's pixels stay on screen until the new one paints, so nothing is retired; the window is not capturable
    /// until its new document's accepted [`Self::page_ready`].
    pub fn navigation_started(&self, window: &str) {
        let mut s = self.lock();
        s.loading.insert(window.to_owned());
        s.navigations.insert(window.to_owned(), Navigation::Started);
        drop(s);
        self.notify();
    }

    /// The navigation of `window` finished loading (the host's own page-load event): its new document's `page_ready`
    /// may now be accepted, once.
    pub fn navigation_finished(&self, window: &str) {
        let mut s = self.lock();
        if s.navigations.get(window) == Some(&Navigation::Started) {
            s.navigations.insert(window.to_owned(), Navigation::Finished);
        }
    }

    /// `window`'s new document reports it has painted (its bootstrap's report after the first frame). Accepted only for
    /// a navigation the host saw start and finish, once (D-72 item 1): then the old document's deliveries end with its
    /// pixels and the window is capturable again. A report with no navigation, before the load finished or a second
    /// time is ignored and changes nothing. Whether it was accepted.
    pub fn page_ready(&self, window: &str) -> bool {
        let mut s = self.lock();
        if s.navigations.get(window) != Some(&Navigation::Finished) {
            return false;
        }
        s.navigations.remove(window);
        s.loading.remove(window);
        s.outstanding.retain(|_, w| w != window);
        if window == MAIN {
            s.field = false;
        }
        drop(s);
        self.notify();
        true
    }

    /// A destroyed window shows nothing: its deliveries are retired.
    pub fn forget(&self, window: &str) {
        let mut s = self.lock();
        s.outstanding.retain(|_, w| w != window);
        s.loading.remove(window);
        s.navigations.remove(window);
        if window == MAIN {
            s.field = false;
        }
        drop(s);
        self.notify();
    }

    /// Whether any delivery is outstanding now.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.lock().outstanding.is_empty()
    }

    /// The main window's page shows the token field (`true`, from its mount) or no longer does (`false`, after the
    /// page without it has painted).
    pub fn field_shown(&self, window: &str, shown: bool) {
        if caller_allowed(window) {
            self.lock().field = shown;
        }
    }

    /// Whether the token field exists now (a capture of the main window then overpaints it).
    #[must_use]
    pub fn field_present(&self) -> bool {
        self.lock().field
    }

    /// Asks the main window where the token field is (`ask` emits the request with its number) and waits up to `bound`
    /// for the answer: `Some(Some(rect))`, `Some(None)` when the page has no field, `None` when no answer came in time.
    pub fn ask_rect(&self, bound: Duration, ask: impl FnOnce(u64)) -> Option<Option<Rect>> {
        let id = {
            let mut s = self.lock();
            s.next_rect += 1;
            s.next_rect
        };
        ask(id);
        let deadline = Instant::now() + bound;
        let mut s = self.lock();
        loop {
            if let Some(r) = s.rects.remove(&id) {
                return Some(r);
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            s = self.changed.wait_timeout(s, deadline - now).unwrap_or_else(PoisonError::into_inner).0;
        }
    }

    /// The main window's answer to rectangle request `request`.
    pub fn answer_rect(&self, window: &str, request: u64, rect: Option<Rect>) {
        if !caller_allowed(window) {
            return;
        }
        let mut s = self.lock();
        if request <= s.next_rect {
            s.rects.insert(request, rect);
        }
        drop(s);
        self.notify();
    }

    /// Starts a capture of `window`: from now no delivery is made. When a delivery is outstanding, `ask_hide` asks the
    /// windows to hide the token, and the call waits up to `bound` for every outstanding delivery to be retired and for
    /// `window` to be out of a navigation. `None` (the capture is refused, and ended) when either still holds after
    /// `bound`.
    pub fn begin_capture(&self, window: &str, bound: Duration, ask_hide: impl FnOnce()) -> Option<CaptureGuard<'_>> {
        let mut s = self.lock();
        s.capturing += 1;
        let guard = CaptureGuard { reveals: self };
        let blocked = |s: &State| !s.outstanding.is_empty() || s.loading.contains(window);
        if !blocked(&s) {
            return Some(guard);
        }
        let ask = !s.outstanding.is_empty();
        drop(s);
        if ask {
            ask_hide();
        }
        let deadline = Instant::now() + bound;
        let mut s = self.lock();
        while blocked(&s) {
            let now = Instant::now();
            if now >= deadline {
                drop(s);
                drop(guard);
                return None;
            }
            s = self.changed.wait_timeout(s, deadline - now).unwrap_or_else(PoisonError::into_inner).0;
        }
        drop(s);
        Some(guard)
    }
}

/// What a capture does about the token field (D-69 item 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overpaint {
    /// No field either time: nothing to paint.
    Nothing,
    /// Paint this rectangle (CSS pixels).
    Paint(Rect),
    /// The field moved, appeared or went during the snapshot: the capture is refused.
    Refuse,
}

/// D-69 item 1: what a capture overpaints, from the field's rectangle read just before (`before`) and just after
/// (`after`) the snapshot: no field either time, nothing; the union of both grown by [`OVERPAINT_MARGIN`] when the two
/// agree within the margin; otherwise the capture is refused.
#[must_use]
pub fn overpaint_rect(before: Option<Rect>, after: Option<Rect>) -> Overpaint {
    match (before, after) {
        (None, None) => Overpaint::Nothing,
        (Some(a), Some(b)) => {
            let edges = |r: Rect| [r[0], r[1], r[0] + r[2], r[1] + r[3]];
            let (ea, eb) = (edges(a), edges(b));
            if ea.iter().zip(eb.iter()).any(|(x, y)| (x - y).abs() > OVERPAINT_MARGIN) {
                return Overpaint::Refuse;
            }
            let m = OVERPAINT_MARGIN;
            let (x0, y0) = (ea[0].min(eb[0]) - m, ea[1].min(eb[1]) - m);
            let (x1, y1) = (ea[2].max(eb[2]) + m, ea[3].max(eb[3]) + m);
            Overpaint::Paint([x0, y0, x1 - x0, y1 - y0])
        }
        _ => Overpaint::Refuse,
    }
}

/// Paints `rect` (CSS pixels, scaled by `scale` to the capture's device pixels, rounded outwards and clamped) with
/// [`OVERPAINT_RGBA`] in the RGBA image `px` of `width` by `height`.
pub fn overpaint(px: &mut [u8], width: u32, height: u32, rect: Rect, scale: f64) {
    let clamp = |v: f64, max: u32| (v.max(0.0) as u32).min(max);
    let x0 = clamp((rect[0] * scale).floor(), width);
    let y0 = clamp((rect[1] * scale).floor(), height);
    let x1 = clamp(((rect[0] + rect[2]) * scale).ceil(), width);
    let y1 = clamp(((rect[1] + rect[3]) * scale).ceil(), height);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * width + x) * 4) as usize;
            if let Some(p) = px.get_mut(i..i + 4) {
                p.copy_from_slice(&OVERPAINT_RGBA);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_capture_with_nothing_delivered_starts_at_once_and_nothing_is_delivered_while_it_runs() {
        let r = TokenReveals::default();
        let mut asked = false;
        let guard = r.begin_capture(MAIN, Duration::from_secs(1), || asked = true);
        assert!(guard.is_some());
        assert!(!asked, "no window to ask");
        assert_eq!(r.deliver(MAIN), None, "no delivery while a capture runs");
        drop(guard);
        assert!(r.deliver(MAIN).is_some(), "a delivery once the capture ended");
    }

    /// D-65 item 2, the reviewer's order (C44A-R1-41): Show twice before the first answer (two deliveries), the first
    /// painted, Hide acknowledges it; a capture then begins. The second delivery is still outstanding, so the capture
    /// is not granted until the window acknowledges it, and while the capture runs nothing more is delivered.
    #[test]
    fn a_late_delivery_holds_the_capture_until_acknowledged() {
        let r = Arc::new(TokenReveals::default());
        let first = r.deliver(MAIN).unwrap();
        let second = r.deliver(MAIN).unwrap();
        r.hidden(MAIN, first);
        assert!(r.any(), "the second delivery is still possibly on screen");
        assert!(r.begin_capture(MAIN, Duration::from_millis(20), || {}).is_none());
        let page = r.clone();
        let guard = r.begin_capture(MAIN, Duration::from_secs(5), move || {
            let page = page.clone();
            std::thread::spawn(move || page.hidden(MAIN, second));
        });
        assert!(guard.is_some());
        assert_eq!(r.deliver(MAIN), None, "nothing delivered during the capture");
    }

    /// D-69 item 1a, the reload schedule: a delivery painted, then the window reloads. The navigation start retires
    /// nothing (the old pixels are still on screen) and makes the window not capturable; only the new document's
    /// `page_ready` retires the delivery and lets a capture through.
    #[test]
    fn a_reload_retires_nothing_until_the_new_document_has_painted() {
        let r = Arc::new(TokenReveals::default());
        let d = r.deliver(MAIN).unwrap();
        r.navigation_started(MAIN);
        assert!(r.any(), "a navigation start retires nothing");
        assert!(r.begin_capture(MAIN, Duration::from_millis(20), || {}).is_none(), "refused while the old page shows");
        // A late acknowledgement from the old document is not needed: the new document's paint retires the delivery.
        let page = r.clone();
        let guard = r.begin_capture(MAIN, Duration::from_secs(5), move || {
            let page = page.clone();
            std::thread::spawn(move || {
                page.navigation_finished(MAIN);
                assert!(page.page_ready(MAIN));
            });
        });
        assert!(guard.is_some(), "granted once the new document has painted");
        drop(guard);
        assert!(!r.any());
        let _ = d;
    }

    #[test]
    fn a_loading_window_is_not_capturable_even_with_nothing_delivered() {
        let r = TokenReveals::default();
        r.navigation_started(MAIN);
        assert!(r.begin_capture(MAIN, Duration::from_millis(20), || {}).is_none());
        assert!(r.begin_capture("float-pen", Duration::from_millis(20), || {}).is_some(), "another window is not");
        r.navigation_finished(MAIN);
        assert!(r.page_ready(MAIN));
        assert!(r.begin_capture(MAIN, Duration::from_millis(20), || {}).is_some());
    }

    /// D-72 item 1: `page_ready` carries no identity, so the host accepts it only for a navigation it saw start and
    /// finish, once. A report with no navigation, between start and finish, or a second time changes nothing: the
    /// delivery stays outstanding and the field stays protected.
    #[test]
    fn page_ready_is_accepted_only_after_a_navigation_the_host_saw_start_and_finish() {
        let r = TokenReveals::default();
        let d = r.deliver(MAIN).unwrap();
        r.field_shown(MAIN, true);
        let protected = |r: &TokenReveals| r.any() && r.field_present();
        // No navigation at all (a window that never navigated never accepts one).
        assert!(!r.page_ready(MAIN));
        assert!(protected(&r), "a report with no navigation changes nothing");
        // Between the start and the finish of a navigation.
        r.navigation_started(MAIN);
        assert!(!r.page_ready(MAIN));
        assert!(protected(&r), "a report before the load finished changes nothing");
        assert!(r.begin_capture(MAIN, Duration::from_millis(20), || {}).is_none(), "still not capturable");
        // The legitimate order: started, finished, then the report.
        r.navigation_finished(MAIN);
        assert!(r.page_ready(MAIN));
        assert!(!r.any() && !r.field_present(), "the old document's delivery is retired");
        // A second report for the same navigation changes nothing.
        let d2 = r.deliver(MAIN).unwrap();
        r.field_shown(MAIN, true);
        assert!(!r.page_ready(MAIN));
        assert!(protected(&r), "a second report changes nothing");
        // A finish with no start is ignored too.
        r.navigation_finished(MAIN);
        assert!(!r.page_ready(MAIN));
        assert!(protected(&r));
        let _ = (d, d2);
    }

    #[test]
    fn an_acknowledgement_counts_only_for_its_own_window_and_delivery() {
        let r = TokenReveals::default();
        let d = r.deliver(MAIN).unwrap();
        r.hidden("float-pen", d);
        r.hidden(MAIN, d + 1);
        assert!(r.any());
        r.hidden(MAIN, d);
        assert!(!r.any());
    }

    /// D-69 item 1: only the main window takes part: a float's identity gets no delivery, and its reports are ignored.
    #[test]
    fn a_float_window_gets_no_delivery_and_reports_nothing() {
        let r = TokenReveals::default();
        assert!(!caller_allowed("float-pen"));
        assert!(caller_allowed(MAIN));
        assert_eq!(r.deliver("float-pen"), None);
        r.field_shown("float-pen", true);
        assert!(!r.field_present());
        r.answer_rect("float-pen", 1, Some([0.0, 0.0, 1.0, 1.0]));
        assert_eq!(r.ask_rect(Duration::from_millis(20), |_| {}), None);
    }

    #[test]
    fn a_destroyed_window_has_nothing_outstanding() {
        let r = TokenReveals::default();
        r.deliver(MAIN).unwrap();
        r.deliver(MAIN).unwrap();
        r.forget(MAIN);
        assert!(!r.any());
        assert!(r.begin_capture(MAIN, Duration::from_millis(20), || {}).is_some());
    }

    #[test]
    fn a_rectangle_request_is_answered_or_times_out() {
        let r = Arc::new(TokenReveals::default());
        let page = r.clone();
        let got = r.ask_rect(Duration::from_secs(5), move |id| {
            std::thread::spawn(move || page.answer_rect(MAIN, id, Some([10.0, 20.0, 100.0, 24.0])));
        });
        assert_eq!(got, Some(Some([10.0, 20.0, 100.0, 24.0])));
        assert_eq!(r.ask_rect(Duration::from_millis(20), |_| {}), None, "no answer: refused");
    }

    /// D-69 item 1: the overpaint covers both reads with the margin; a field that moved between them refuses.
    #[test]
    fn the_overpaint_covers_the_field_and_a_moved_field_refuses() {
        let a = [100.0, 50.0, 160.0, 24.0];
        assert_eq!(overpaint_rect(None, None), Overpaint::Nothing);
        assert_eq!(overpaint_rect(Some(a), Some(a)), Overpaint::Paint([96.0, 46.0, 168.0, 32.0]));
        assert!(matches!(overpaint_rect(Some(a), Some([102.0, 50.0, 160.0, 24.0])), Overpaint::Paint(_)), "within");
        assert_eq!(overpaint_rect(Some(a), Some([312.0, 50.0, 160.0, 24.0])), Overpaint::Refuse, "moved 212 px");
        assert_eq!(overpaint_rect(Some(a), None), Overpaint::Refuse, "went during the snapshot");
        assert_eq!(overpaint_rect(None, Some(a)), Overpaint::Refuse, "appeared during the snapshot");
    }

    /// The pixels of the field's rectangle equal the overpaint colour; the rest of the capture is unchanged.
    #[test]
    fn the_field_rectangle_is_painted_over_in_device_pixels() {
        let (w, h) = (40u32, 20u32);
        let mut px = vec![0x11u8; (w * h * 4) as usize];
        overpaint(&mut px, w, h, [5.0, 2.0, 5.0, 3.0], 2.0);
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let inside = (10..20).contains(&x) && (4..10).contains(&y);
                assert_eq!(px[i..i + 4] == OVERPAINT_RGBA, inside, "pixel {x},{y}");
            }
        }
        // Clamped at the image's edges.
        overpaint(&mut px, w, h, [-10.0, -10.0, 1000.0, 1000.0], 1.0);
        assert!(px.chunks(4).all(|p| p == OVERPAINT_RGBA));
    }
}
