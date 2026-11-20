//! The agent token's deliveries to the windows, and the window capture's guard (ruling D-62 item 2, C44A-R1-41; D-65
//! item 2; design note 2 §2): no surface an agent can read ever contains the token.
//!
//! The host is the authority, not the page. Every answer of `agent_token` is a numbered delivery, counted as possibly
//! on screen until the window acknowledges that delivery hidden (`token_hidden`: the mask painted, or the token never
//! painted, as for Copy or an answer that came too late). A capture (`inspect.screenshot`, whoever asked for it) first
//! takes a [`CaptureGuard`]: it is granted only when no delivery is outstanding, after asking the windows to hide what
//! they show (`token-hide`) and waiting, bounded, for their acknowledgements; while a guard is held `agent_token`
//! delivers nothing. So the capture never contains the token: if a delivery is still outstanding at the bound, the
//! capture is refused.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The host event that asks every window to hide a shown token now.
pub const TOKEN_HIDE_EVENT: &str = "token-hide";

#[derive(Debug, Default)]
struct State {
    /// Deliveries not yet acknowledged hidden, with the window each went to.
    outstanding: HashMap<u64, String>,
    /// The last delivery's number.
    next: u64,
    /// Captures in progress.
    capturing: usize,
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

    /// A delivery of the token to window `window`: its number, or `None` while a capture runs (nothing is delivered).
    pub fn deliver(&self, window: &str) -> Option<u64> {
        let mut s = self.lock();
        if s.capturing > 0 {
            return None;
        }
        s.next += 1;
        let id = s.next;
        s.outstanding.insert(id, window.to_owned());
        Some(id)
    }

    /// Window `window` acknowledges delivery `id` hidden (the mask painted, or the token never painted). A delivery of
    /// another window, or an unknown one, is left as it is.
    pub fn hidden(&self, window: &str, id: u64) {
        let mut s = self.lock();
        if s.outstanding.get(&id).is_some_and(|w| w == window) {
            s.outstanding.remove(&id);
        }
        drop(s);
        self.changed.notify_all();
    }

    /// A window that closed or reloaded shows nothing: its deliveries are no longer outstanding.
    pub fn forget(&self, window: &str) {
        let mut s = self.lock();
        s.outstanding.retain(|_, w| w != window);
        drop(s);
        self.changed.notify_all();
    }

    /// Whether any delivery is outstanding now.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.lock().outstanding.is_empty()
    }

    /// Starts a capture: from now no delivery is made. When a delivery is outstanding, `ask_hide` asks the windows to
    /// hide the token, and the call waits up to `bound` for every outstanding delivery to be acknowledged. `None` (the
    /// capture is refused, and ended) when one is still outstanding after `bound`.
    pub fn begin_capture(&self, bound: Duration, ask_hide: impl FnOnce()) -> Option<CaptureGuard<'_>> {
        let mut s = self.lock();
        s.capturing += 1;
        let guard = CaptureGuard { reveals: self };
        if s.outstanding.is_empty() {
            return Some(guard);
        }
        drop(s);
        ask_hide();
        let deadline = Instant::now() + bound;
        let mut s = self.lock();
        while !s.outstanding.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_capture_with_nothing_delivered_starts_at_once_and_nothing_is_delivered_while_it_runs() {
        let r = TokenReveals::default();
        let mut asked = false;
        let guard = r.begin_capture(Duration::from_secs(1), || asked = true);
        assert!(guard.is_some());
        assert!(!asked, "no window to ask");
        assert_eq!(r.deliver("prefs"), None, "no delivery while a capture runs");
        drop(guard);
        assert!(r.deliver("prefs").is_some(), "a delivery once the capture ended");
    }

    /// D-65 item 2, the reviewer's order (C44A-R1-41): Show twice before the first answer (two deliveries), the first
    /// painted, Hide acknowledges it; a capture then begins. The second delivery is still outstanding, so the capture
    /// is not granted until the window acknowledges it (a late answer of an old reveal, dropped without painting), and
    /// while the capture runs nothing more is delivered.
    #[test]
    fn a_late_delivery_holds_the_capture_until_acknowledged() {
        let r = Arc::new(TokenReveals::default());
        let first = r.deliver("main").unwrap();
        let second = r.deliver("main").unwrap();
        r.hidden("main", first);
        assert!(r.any(), "the second delivery is still possibly on screen");
        // No acknowledgement within the bound: refused.
        assert!(r.begin_capture(Duration::from_millis(20), || {}).is_none());
        // The window drops the late answer and acknowledges it when asked.
        let page = r.clone();
        let guard = r.begin_capture(Duration::from_secs(5), move || {
            let page = page.clone();
            std::thread::spawn(move || page.hidden("main", second));
        });
        assert!(guard.is_some());
        assert_eq!(r.deliver("main"), None, "nothing delivered during the capture");
    }

    #[test]
    fn an_acknowledgement_counts_only_for_its_own_window_and_delivery() {
        let r = TokenReveals::default();
        let d = r.deliver("main").unwrap();
        r.hidden("float:x", d);
        r.hidden("main", d + 1);
        assert!(r.any());
        r.hidden("main", d);
        assert!(!r.any());
    }

    #[test]
    fn a_closed_window_has_nothing_outstanding() {
        let r = TokenReveals::default();
        r.deliver("float:x").unwrap();
        r.deliver("float:x").unwrap();
        r.forget("float:x");
        assert!(!r.any());
        assert!(r.begin_capture(Duration::from_millis(20), || {}).is_some());
    }
}
