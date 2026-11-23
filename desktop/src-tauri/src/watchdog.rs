//! Engine-not-responding detection (reviews/M4-design-engine-restart.md; C1.4 gap 3; ruling O-14).
//!
//! A heartbeat query runs off the main thread on a fixed [`PERIOD`] cadence measured with a monotonic clock: tick `n`
//! is due at `start + n * PERIOD`, whatever the previous tick did. A heartbeat still unanswered at a tick is one miss;
//! while it stays unanswered no new one is sent and each further tick is another miss. [`STRIKES`] misses mark the
//! engine not responding (about 15 s); any answer clears the mark at once, also one that arrives late. A heartbeat
//! answered by an engine whose document thread has ended (`internal` with reason `engineStopped`: it died, or it
//! stopped itself after a panic outside an invocation) marks it at once (ruling E-29 item 3: before, such an answer
//! counted as neither a miss nor an answer, so the restart was never offered). The front end shows the mark and offers
//! Wait or Restart; a restart is never automatic.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::host::Host;

/// Time between heartbeats.
pub const PERIOD: Duration = Duration::from_secs(5);
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

    /// Records a heartbeat answered by a stopped engine (ruling E-29 item 3): marked at once; returns `Some(true)` when
    /// the mark changed. Only a real answer (the restarted engine's) clears it.
    pub fn stopped(&mut self) -> Option<bool> {
        self.missed = self.missed.max(STRIKES);
        if self.not_responding {
            return None;
        }
        self.not_responding = true;
        Some(true)
    }

    /// Whether the engine is marked not responding.
    #[must_use]
    pub fn not_responding(&self) -> bool {
        self.not_responding
    }
}

/// The due time of tick `n` (1-based) of a cadence that started at `start`.
#[must_use]
pub fn due(start: Instant, n: u32) -> Instant {
    start + PERIOD * n
}

/// The heartbeat in flight: the engine generation it was sent to (a heartbeat of a replaced engine is forgotten).
static IN_FLIGHT: Mutex<Option<u64>> = Mutex::new(None);

fn emit(app: &AppHandle, not_responding: bool, generation: u64) {
    let payload = json!({ "notResponding": not_responding, "generation": generation });
    if let Ok(text) = tt_engine::wire::to_wire_json(&payload) {
        let _ = app.emit(ENGINE_STATUS_EVENT, text);
    }
}

fn record(app: &AppHandle, answered: bool) {
    let host = app.state::<Host>();
    let changed = host.watchdog.lock().unwrap_or_else(PoisonError::into_inner).record(answered);
    if let Some(state) = changed {
        emit(app, state, host.generation());
    }
}

fn record_stopped(app: &AppHandle) {
    let host = app.state::<Host>();
    let changed = host.watchdog.lock().unwrap_or_else(PoisonError::into_inner).stopped();
    if let Some(state) = changed {
        emit(app, state, host.generation());
    }
}

/// Whether an envelope is a stopped engine's answer (`internal`, reason `engineStopped`; ruling E-29).
#[must_use]
pub fn is_stopped(env: &tt_executor::Envelope) -> bool {
    env.error.as_ref().is_some_and(|e| {
        e.code == "internal" && e.details.get("reason").and_then(|r| r.as_str()) == Some(tt_executor::ENGINE_STOPPED)
    })
}

/// Starts the heartbeat thread for the life of the application.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("tt-watchdog".into()).spawn(move || {
        let start = Instant::now();
        let mut n: u32 = 0;
        loop {
            n = n.saturating_add(1);
            let next = due(start, n);
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
            let host = app.state::<Host>();
            let generation = host.generation();
            let outstanding = {
                let mut slot = IN_FLIGHT.lock().unwrap_or_else(PoisonError::into_inner);
                match *slot {
                    Some(g) if g == generation => true,
                    _ => {
                        *slot = Some(generation);
                        false
                    }
                }
            };
            if outstanding {
                // Still unanswered at this tick: a miss.
                record(&app, false);
                continue;
            }
            let exec = host.exec();
            let a = app.clone();
            let spawned = std::thread::Builder::new().name("tt-heartbeat".into()).spawn(move || {
                let env = exec.invoke(tt_executor::Invocation::user("app.state.get", json!({})));
                let mut slot = IN_FLIGHT.lock().unwrap_or_else(PoisonError::into_inner);
                // Only the current generation's heartbeat counts; any answer clears the mark, a late one too.
                if *slot == Some(generation) {
                    *slot = None;
                    drop(slot);
                    // Misses are counted by the ticks; the reply side records answers only (a timeout is no answer).
                    if a.state::<Host>().generation() == generation {
                        if env.is_ok() {
                            record(&a, true);
                        } else if is_stopped(&env) {
                            record_stopped(&a);
                        }
                    }
                }
            });
            if spawned.is_err() {
                *IN_FLIGHT.lock().unwrap_or_else(PoisonError::into_inner) = None;
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

    /// Ruling E-29 item 3: a heartbeat answered by a stopped engine marks it at once (no three strikes); further such
    /// answers change nothing; the restarted engine's first answer clears it; ordinary errors are neither.
    #[test]
    fn a_stopped_engine_is_marked_at_once_and_a_real_answer_clears_it() {
        let mut w = Watchdog::default();
        assert_eq!(w.stopped(), Some(true));
        assert!(w.not_responding());
        assert_eq!(w.stopped(), None, "already marked");
        assert_eq!(w.record(false), None, "a miss after it changes nothing");
        assert_eq!(w.record(true), Some(false), "the restarted engine answers");
        assert!(!w.not_responding());
        // After one miss, a stopped answer still marks at once.
        assert_eq!(w.record(false), None);
        assert_eq!(w.stopped(), Some(true));
    }

    #[test]
    fn only_the_stopped_internal_counts_as_a_stopped_engine() {
        let stopped = tt_executor::Envelope::error(
            tt_executor::CommandError::with_details(
                "internal",
                "the document thread has stopped",
                json!({ "reason": tt_executor::ENGINE_STOPPED }),
            ),
            None,
        );
        assert!(is_stopped(&stopped));
        let contained = tt_executor::Envelope::error(
            tt_executor::CommandError::with_details("internal", "failed", json!({ "reason": "handlerPanicked" })),
            None,
        );
        assert!(!is_stopped(&contained), "a contained panic is one failed call: the engine keeps serving");
        let refused = tt_executor::Envelope::error(tt_executor::CommandError::new("project.none", "no project"), None);
        assert!(!is_stopped(&refused));
    }

    #[test]
    fn the_cadence_is_fixed_and_three_misses_take_about_fifteen_seconds() {
        let start = Instant::now();
        // Tick n is due at start + n * PERIOD whatever the previous tick did (no drift from waiting).
        assert_eq!(due(start, 3) - start, Duration::from_secs(15));
        assert_eq!(due(start, 4) - due(start, 3), PERIOD);
        // A heartbeat sent at tick 1 and never answered is missed at ticks 2, 3 and 4: marked 15 s after it was sent.
        let mut w = Watchdog::default();
        let marks: Vec<Option<bool>> = (2..=4).map(|_| w.record(false)).collect();
        assert_eq!(marks, vec![None, None, Some(true)]);
        // A late answer (after its tick) clears the mark at once.
        assert_eq!(w.record(true), Some(false));
    }
}
