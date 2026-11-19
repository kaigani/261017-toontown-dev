//! Which windows show the agent token in Preferences › Agent access, and the window capture's guard (ruling D-62
//! item 2, C44A-R1-41; design note 2 §2): no surface an agent can read ever contains the token.
//!
//! The page reports `shown: true` before it draws the token and `shown: false` once the mask is painted. A capture
//! (`inspect.screenshot`, whoever asked for it) first takes a [`CaptureGuard`]: while one is held no window may start
//! showing the token (its report is refused and it keeps the mask), and the guard is granted only when no window shows
//! it, after asking the windows that do to hide it (`token-hide`) and waiting, bounded, for their reports. So the
//! capture never contains the revealed token: if a window does not hide it in time, the capture is refused.

use std::collections::HashSet;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The host event that asks every window to hide a shown token now.
pub const TOKEN_HIDE_EVENT: &str = "token-hide";

#[derive(Debug, Default)]
struct State {
    /// Labels of the windows that show the token.
    shown: HashSet<String>,
    /// Captures in progress.
    capturing: usize,
}

/// The windows showing the token, and the captures in progress.
#[derive(Debug, Default)]
pub struct TokenReveals {
    state: Mutex<State>,
    changed: Condvar,
}

/// Held while a capture runs: no window starts showing the token until it is dropped.
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

    /// Window `window` shows the token (`true`, reported before it draws it) or has painted the mask (`false`).
    /// Answers whether the window may show it: never while a capture runs.
    pub fn report(&self, window: &str, shown: bool) -> bool {
        let mut s = self.lock();
        if shown {
            if s.capturing > 0 {
                return false;
            }
            s.shown.insert(window.to_owned());
        } else {
            s.shown.remove(window);
        }
        drop(s);
        self.changed.notify_all();
        true
    }

    /// A window that closed or reloaded shows nothing.
    pub fn forget(&self, window: &str) {
        self.report(window, false);
    }

    /// Whether any window shows the token now.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.lock().shown.is_empty()
    }

    /// Starts a capture. When a window shows the token, `ask_hide` asks the windows to hide it, and the call waits up to
    /// `bound` for every one of them to report the mask painted. `None` (the capture is refused) when one still shows
    /// it after `bound`.
    pub fn begin_capture(&self, bound: Duration, ask_hide: impl FnOnce()) -> Option<CaptureGuard<'_>> {
        let mut s = self.lock();
        s.capturing += 1;
        let guard = CaptureGuard { reveals: self };
        if s.shown.is_empty() {
            return Some(guard);
        }
        drop(s);
        ask_hide();
        let deadline = Instant::now() + bound;
        let mut s = self.lock();
        while !s.shown.is_empty() {
            let now = Instant::now();
            if now >= deadline {
                drop(s);
                // Dropping the guard ends the capture.
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
    fn a_capture_with_nothing_shown_starts_at_once_and_blocks_a_reveal_while_it_runs() {
        let r = TokenReveals::default();
        let mut asked = false;
        let guard = r.begin_capture(Duration::from_secs(1), || asked = true);
        assert!(guard.is_some());
        assert!(!asked, "no window to ask");
        assert!(!r.report("prefs", true), "no reveal while a capture runs");
        assert!(!r.any());
        drop(guard);
        assert!(r.report("prefs", true), "a reveal once the capture ended");
        assert!(r.any());
    }

    #[test]
    fn a_shown_token_is_hidden_before_the_capture_starts() {
        let r = Arc::new(TokenReveals::default());
        assert!(r.report("main", true));
        let page = r.clone();
        // The window hides the token when asked, and reports the mask painted from its own thread.
        let guard = r.begin_capture(Duration::from_secs(5), move || {
            let page = page.clone();
            std::thread::spawn(move || {
                page.report("main", false);
            });
        });
        assert!(guard.is_some());
        assert!(!r.any());
    }

    #[test]
    fn a_window_that_does_not_hide_the_token_refuses_the_capture() {
        let r = TokenReveals::default();
        assert!(r.report("main", true));
        assert!(r.begin_capture(Duration::from_millis(20), || {}).is_none());
        // The refused capture ended: the window may report again, and a later capture may start once it hides.
        assert!(r.report("main", false));
        assert!(r.begin_capture(Duration::from_millis(20), || {}).is_some());
    }

    #[test]
    fn a_closed_window_shows_nothing() {
        let r = TokenReveals::default();
        assert!(r.report("float:x", true));
        r.forget("float:x");
        assert!(!r.any());
    }
}
