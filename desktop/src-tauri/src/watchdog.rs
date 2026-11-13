//! Engine-not-responding detection (reviews/M4-design-engine-restart.md; C1.4 gap 3).
//!
//! A heartbeat query runs off the main thread every [`PERIOD`]; it counts as missed when no answer arrives within
//! [`HEARTBEAT_BOUND`]. Three missed heartbeats in a row mark the engine not responding; any answer clears the mark at
//! once. The front end shows the mark and offers Wait or Restart; a restart is never automatic.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::host::Host;

/// Time between heartbeats.
pub const PERIOD: Duration = Duration::from_secs(5);
/// How long a heartbeat waits for its answer.
pub const HEARTBEAT_BOUND: Duration = Duration::from_secs(5);
/// Missed heartbeats in a row that mark the engine not responding.
pub const STRIKES: u32 = 3;
/// Event name of the engine status sent to the front end.
pub const ENGINE_STATUS_EVENT: &str = "engine-status";

/// The heartbeat state machine.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Watchdog {
    missed: u32,
    not_responding: bool,
}

impl Watchdog {
    /// Records a heartbeat outcome; returns `Some(state)` when the not-responding mark changed.
    pub fn record(&mut self, answered: bool) -> Option<bool> {
        if answered {
            self.missed = 0;
            if self.not_responding {
                self.not_responding = false;
                return Some(false);
            }
            return None;
        }
        self.missed = self.missed.saturating_add(1);
        if self.missed >= STRIKES && !self.not_responding {
            self.not_responding = true;
            return Some(true);
        }
        None
    }

    /// Whether the engine is marked not responding.
    #[must_use]
    pub fn not_responding(&self) -> bool {
        self.not_responding
    }
}

/// A heartbeat in flight (at most one: a stuck engine is not flooded).
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

fn emit(app: &AppHandle, not_responding: bool, generation: u64) {
    let payload = json!({ "notResponding": not_responding, "generation": generation });
    if let Ok(text) = tt_engine::wire::to_wire_json(&payload) {
        let _ = app.emit(ENGINE_STATUS_EVENT, text);
    }
}

/// Starts the heartbeat thread for the life of the application.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("tt-watchdog".into()).spawn(move || {
        loop {
            std::thread::sleep(PERIOD);
            let host = app.state::<Host>();
            let answered = if IN_FLIGHT.swap(true, Ordering::AcqRel) {
                // The previous heartbeat is still waiting: that is a miss.
                false
            } else {
                let exec = host.exec();
                let (tx, rx) = mpsc::channel();
                let spawned = std::thread::Builder::new().name("tt-heartbeat".into()).spawn(move || {
                    let env = exec.invoke(tt_executor::Invocation::user("app.state.get", json!({})));
                    IN_FLIGHT.store(false, Ordering::Release);
                    let _ = tx.send(env.is_ok());
                });
                if spawned.is_err() {
                    IN_FLIGHT.store(false, Ordering::Release);
                }
                rx.recv_timeout(HEARTBEAT_BOUND).unwrap_or(false)
            };
            let changed = host.watchdog.lock().unwrap_or_else(std::sync::PoisonError::into_inner).record(answered);
            if let Some(state) = changed {
                emit(&app, state, host.generation());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_misses_mark_and_one_answer_clears() {
        let mut w = Watchdog::default();
        assert_eq!(w.record(false), None);
        assert_eq!(w.record(false), None);
        assert_eq!(w.record(false), Some(true));
        assert!(w.not_responding());
        assert_eq!(w.record(false), None, "already marked");
        assert_eq!(w.record(true), Some(false));
        assert_eq!(w.record(false), None, "the count starts again");
        assert_eq!(w.record(true), None);
    }
}
