//! The host's engine connection (M4 C4.1; ADR-0021): the executor the window is a client of, request-cancellation
//! flags for cancellable GUI calls, the numbering of engine events forwarded to the webviews, and the attached-mode
//! HTTP server.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tt_executor::{Envelope, ExecutorHandle, Invocation, InvokeOptions};

/// What the host was started with (command line; ADR-0022).
#[derive(Debug, Clone, Default)]
pub struct HostOptions {
    /// Per-user folder (`--user-dir`); the platform's when absent.
    pub user_dir: Option<PathBuf>,
    /// Serve attached mode on this address (`--mcp-http 127.0.0.1:0`), whatever the preference says.
    pub mcp_http: Option<std::net::SocketAddr>,
    /// Write the discovery file here as well (`--mcp-discovery <file>`; tests read the endpoint and token).
    pub discovery: Option<PathBuf>,
    /// Documents to open (command line or OS open-document requests before the window exists).
    pub open: Vec<PathBuf>,
}

/// The engine connection, managed Tauri state.
pub struct Host {
    /// The executor (document thread); every GUI command goes through it. Replaced only by [`Host::restart`].
    exec: std::sync::RwLock<ExecutorHandle>,
    /// Restarts so far (the event pump of an earlier generation stops forwarding).
    generation: AtomicU64,
    /// Heartbeat state (reviews/M4-design-engine-restart.md).
    pub watchdog: Mutex<crate::watchdog::Watchdog>,
    /// Cancellation flags of cancellable requests in flight, by request token. A probe reads only the flag
    /// (ADR-0020 decision 8: probes must not block).
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// The next engine-event number (delivery order, starting at 1).
    seq: AtomicU64,
    /// The attached-mode server.
    pub http: Mutex<Option<tt_mcp::http::HttpServer>>,
    /// Launch options.
    pub options: HostOptions,
    /// The per-user settings folder.
    pub settings_dir: PathBuf,
    /// Documents to open once the front end is ready (taken once).
    pending_open: Mutex<Vec<PathBuf>>,
    /// Serialises restarts (ruling O-14): a second one waits for the first and does not run.
    restart_lock: Mutex<()>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host").finish_non_exhaustive()
    }
}

impl Host {
    /// Starts the executor over the user folders.
    ///
    /// # Errors
    /// The executor's start error.
    pub fn start(options: HostOptions) -> Result<Self, tt_executor::CommandError> {
        let (exec, settings_dir) = Self::spawn_executor(&options)?;
        Ok(Self {
            exec: std::sync::RwLock::new(exec),
            generation: AtomicU64::new(0),
            watchdog: Mutex::new(crate::watchdog::Watchdog::default()),
            cancels: Mutex::new(HashMap::new()),
            seq: AtomicU64::new(0),
            http: Mutex::new(None),
            pending_open: Mutex::new(options.open.clone()),
            restart_lock: Mutex::new(()),
            options,
            settings_dir,
        })
    }

    fn spawn_executor(options: &HostOptions) -> Result<(ExecutorHandle, PathBuf), tt_executor::CommandError> {
        let dirs = match &options.user_dir {
            Some(d) => tt_executor::UserDirs::under(d),
            None => tt_executor::UserDirs::platform()
                .unwrap_or_else(|| tt_executor::UserDirs::under(&std::env::temp_dir().join("Toontown"))),
        };
        let settings_dir = dirs.settings.clone();
        let exec = ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
            config: tt_executor::ExecutorConfig::default(),
            persist: tt_executor::PersistOptions { user_dirs: Some(dirs), ..Default::default() },
        })?;
        Ok((exec, settings_dir))
    }

    /// The executor handle (a cheap clone).
    pub fn exec(&self) -> ExecutorHandle {
        self.exec.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The executor generation (0 at start, +1 per restart).
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Replaces a stuck executor with a new one over the same user folders (reviews/M4-design-engine-restart.md). The
    /// stuck document thread is abandoned; its autosave recovery data is offered by the new executor. The HTTP server is
    /// restarted over the new executor. Returns the new generation.
    ///
    /// # Errors
    /// The new executor's start error.
    pub fn restart(&self) -> Result<(u64, bool), tt_executor::CommandError> {
        self.restart_from(self.generation())
    }

    /// [`Host::restart`] for a caller that saw generation `asked`: serialised (ruling O-14), so a restart asked while
    /// another runs waits for it and answers with the new generation without restarting again.
    fn restart_from(&self, asked: u64) -> Result<(u64, bool), tt_executor::CommandError> {
        let _serial = self.restart_lock.lock().unwrap_or_else(PoisonError::into_inner);
        if self.generation() != asked {
            return Ok((self.generation(), false));
        }
        let (exec, _) = Self::spawn_executor(&self.options)?;
        // Once its replacement runs, the stuck engine is fenced and named, so only its recovery data is released and it
        // writes nothing if it wakes up (C41-R1-15).
        let owner = self.exec().abandon();
        let was_serving = self.http.lock().unwrap_or_else(PoisonError::into_inner).is_some();
        self.stop_http();
        // The stuck executor's autosave becomes recoverable work for the new one.
        let _ = exec.release_recovery_of(&owner);
        *self.exec.write().unwrap_or_else(PoisonError::into_inner) = exec;
        *self.watchdog.lock().unwrap_or_else(PoisonError::into_inner) = crate::watchdog::Watchdog::default();
        // The new generation numbers its events from 1; the old one's cancellation flags are dropped (ruling O-14).
        self.seq.store(0, Ordering::Release);
        self.cancels.lock().unwrap_or_else(PoisonError::into_inner).clear();
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if was_serving {
            let _ = self.start_http();
        }
        Ok((generation, true))
    }

    /// Runs one command as the user. With a token, the request carries a cancellation flag that `cancel` sets; the
    /// probe only loads it. Without one, the plain entry point (no link, no cost).
    pub fn run(&self, command: &str, params: Value, dry_run: bool, token: Option<&str>) -> Envelope {
        let inv = Invocation {
            command: command.to_owned(),
            params,
            options: InvokeOptions { dry_run, ..InvokeOptions::default() },
            origin: tt_executor::Origin::User,
        };
        // A request answers from the generation it started on (ruling O-14): the probe also sees a restart, so a
        // request in flight on a replaced engine is answered at once and its late result is never shown as applied.
        let generation = self.generation();
        let exec = self.exec();
        let env = match token {
            None => {
                let probe = || self.generation() != generation;
                exec.invoke_served_until(inv, &probe).0
            }
            Some(t) => {
                let Some(guard) = TokenGuard::claim(&self.cancels, t) else {
                    return Envelope::error(
                        tt_executor::CommandError::with_details(
                            "arg.invalid",
                            format!("the request token `{t}` is already in flight"),
                            json!({ "pointer": "/requestToken", "reason": "requestTokenInUse" }),
                        ),
                        None,
                    );
                };
                let flag = guard.flag.clone();
                let probe = || flag.load(Ordering::Acquire) || self.generation() != generation;
                let env = exec.invoke_served_until(inv, &probe).0;
                drop(guard);
                env
            }
        };
        answer_of(env, generation, self.generation())
    }

    /// Applies a sensor report from the window (`report_shell`): one that names another engine generation than the
    /// current one is ignored (ruling O-14). Returns what the window is told.
    ///
    /// # Errors
    /// The JSON text of `arg.invalid` for a malformed report.
    pub fn report(&self, mut value: Value) -> Result<Value, String> {
        let generation = value.as_object_mut().and_then(|m| m.remove("generation"));
        if let Some(g) = generation
            && g.as_u64() != Some(self.generation())
        {
            return Ok(json!({ "ignored": "staleGeneration", "generation": self.generation() }));
        }
        let report: tt_executor::host::ShellReport = serde_json::from_value(value).map_err(|e| {
            serde_json::to_string(&json!({ "code": "arg.invalid", "message": format!("malformed sensor report: {e}"),
                "details": { "pointer": "" } }))
            .unwrap_or_default()
        })?;
        self.exec().report_shell(report);
        Ok(json!({}))
    }

    /// The documents still to open at start (empty after the first call).
    pub fn take_pending_open(&self) -> Vec<PathBuf> {
        std::mem::take(&mut *self.pending_open.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Marks a cancellable request cancelled (unknown or finished tokens are ignored).
    pub fn cancel(&self, token: &str) {
        if let Some(f) = self.cancels.lock().unwrap_or_else(PoisonError::into_inner).get(token) {
            f.store(true, Ordering::Release);
        }
    }

    /// Clears the sensor facts a window reported (ruling O-9): its modal state and text focus end with it when it is
    /// destroyed, reloaded or replaced, so a report whose reporter went away cannot block agents or shortcuts.
    /// Today every report comes from the main window; per-window composition arrives with the float windows (ruling
    /// O-2; `reviews/M4-design-floating-windows.md`), where `label` selects the window's own reports.
    pub fn forget_window_reports(&self, _label: &str) {
        let report: tt_executor::host::ShellReport =
            serde_json::from_value(json!({ "modal": null, "textFocus": false })).unwrap_or_default();
        self.exec().report_shell(report);
    }

    /// The next engine-event number.
    pub fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// The token of the HTTP endpoint: the discovery file's when it has the generated form (48 lower-case hex
    /// characters, ruling O-8: a planted file with another token is not trusted), else a new one written through
    /// `agent.token.regenerate` from the operating system's random source (ruling O-6).
    ///
    /// # Errors
    /// Why no token could be made (the random source failed, or the settings folder cannot be written).
    fn token(&self) -> Result<String, String> {
        let read = || {
            std::fs::read_to_string(self.settings_dir.join("agent-discovery.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.get("token").and_then(Value::as_str).map(str::to_owned))
                .filter(|t| tt_executor::is_secret_token(t, tt_executor::AGENT_TOKEN_BYTES))
        };
        if let Some(t) = read() {
            return Ok(t);
        }
        let env = self.run("agent.token.regenerate", json!({}), false, None);
        if let Some(e) = env.error {
            return Err(format!("cannot create the agent token: {}", e.message));
        }
        read().ok_or_else(|| "cannot create the agent token: the discovery file was not written".to_owned())
    }

    /// Starts the attached-mode server if it is not running (11 §4: off until the user enables it or a flag asks).
    ///
    /// # Errors
    /// A message when the token cannot be made or no port can be bound.
    pub fn start_http(&self) -> Result<String, String> {
        let mut slot = self.http.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(s) = slot.as_ref() {
            return Ok(s.url());
        }
        let token = self.token()?;
        let pref_port = self
            .run("prefs.get", json!({ "keys": ["agent.server.port"] }), false, None)
            .result
            .and_then(|r| r.pointer("/values/agent.server.port").and_then(Value::as_u64))
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(47215);
        let candidates: Vec<std::net::SocketAddr> = match self.options.mcp_http {
            Some(a) => vec![a],
            // The documented port, then the next free ones (11 §4).
            None => {
                (0..16).map(|i| std::net::SocketAddr::from(([127, 0, 0, 1], pref_port.saturating_add(i)))).collect()
            }
        };
        let mut last = String::new();
        for bind in candidates {
            let opts = tt_mcp::http::HttpOptions {
                bind,
                token: token.clone(),
                server: tt_mcp::ServerOptions { run_mode: "attached".to_owned(), ..Default::default() },
            };
            match tt_mcp::http::start(self.exec(), opts) {
                Ok(server) => {
                    let url = server.url();
                    self.write_discovery(Some(&url), Some(&token));
                    *slot = Some(server);
                    return Ok(url);
                }
                Err(e) => last = e.to_string(),
            }
        }
        Err(format!("cannot bind the agent server: {last}"))
    }

    /// Stops the attached-mode server; its sessions end and the discovery file no longer advertises an endpoint
    /// (ruling O-8). Also called when the server is off at start, so a stale file of a dead process is replaced.
    pub fn stop_http(&self) {
        if let Some(s) = self.http.lock().unwrap_or_else(PoisonError::into_inner).take() {
            s.stop();
        }
        self.clear_discovery();
    }

    /// Rewrites the discovery file without endpoint and process id, keeping a well-formed token (the
    /// per-installation token stays where ADR-0022 stores it).
    fn clear_discovery(&self) {
        let path = self.settings_dir.join("agent-discovery.json");
        let Some(v) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
            return;
        };
        let live = !v.get("endpoint").is_none_or(Value::is_null) || !v.get("processId").is_none_or(Value::is_null);
        let token = v
            .get("token")
            .and_then(Value::as_str)
            .filter(|t| tt_executor::is_secret_token(t, tt_executor::AGENT_TOKEN_BYTES));
        if live || token.is_none() {
            self.write_discovery(None, token);
        }
    }

    /// The discovery file (11 §4): endpoint, token and process id while the server runs (`url` given), only the token
    /// otherwise; readable only by the user (POSIX mode 0600; on Windows the per-user settings folder's inherited ACL).
    fn write_discovery(&self, url: Option<&str>, token: Option<&str>) {
        let v = json!({
            "format": "toontown-agent-discovery", "formatVersion": "1.0",
            "endpoint": url, "token": token,
            "processId": url.map(|_| std::process::id()),
        });
        let text = serde_json::to_vec_pretty(&v).unwrap_or_default();
        let mut targets = vec![self.settings_dir.join("agent-discovery.json")];
        targets.extend(self.options.discovery.iter().cloned());
        for t in targets {
            if let Some(parent) = t.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = write_private(&t, &text);
        }
    }
}

/// The answer to a request that started on engine generation `started` when generation `now` is current (ruling
/// O-14): its own envelope, or, after a restart, `command.timeout` with `reason: "engineRestarted"`, so a late result of
/// the replaced engine is never shown as applied.
fn answer_of(env: Envelope, started: u64, now: u64) -> Envelope {
    if now == started {
        return env;
    }
    Envelope::error(
        tt_executor::CommandError::with_details(
            "command.timeout",
            "the engine was restarted while this request ran; its outcome is unknown and nothing of it is shown",
            json!({ "reason": "engineRestarted", "generation": started, "currentGeneration": now }),
        ),
        None,
    )
}

/// One request's cancellation flag under its token (C41-R1-07): claimed only when the token is not in flight, and
/// removed by its own owner only, also when the invocation unwinds.
struct TokenGuard<'a> {
    map: &'a Mutex<HashMap<String, Arc<AtomicBool>>>,
    token: String,
    flag: Arc<AtomicBool>,
}

impl<'a> TokenGuard<'a> {
    fn claim(map: &'a Mutex<HashMap<String, Arc<AtomicBool>>>, token: &str) -> Option<Self> {
        let mut m = map.lock().unwrap_or_else(PoisonError::into_inner);
        if m.contains_key(token) {
            return None;
        }
        let flag = Arc::new(AtomicBool::new(false));
        m.insert(token.to_owned(), flag.clone());
        Some(Self { map, token: token.to_owned(), flag })
    }
}

impl Drop for TokenGuard<'_> {
    fn drop(&mut self) {
        let mut m = self.map.lock().unwrap_or_else(PoisonError::into_inner);
        if m.get(&self.token).is_some_and(|f| Arc::ptr_eq(f, &self.flag)) {
            m.remove(&self.token);
        }
    }
}

/// Writes a file atomically, readable by its owner only (POSIX; the per-user folder is private on Windows).
fn write_private(target: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// How long the host waits for the document thread at quit.
pub const QUIT_GRACE: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> (Host, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tt-host-test-{}", tt_executor::random_token(8)));
        let options = HostOptions { user_dir: Some(dir.clone()), mcp_http: None, discovery: None, open: Vec::new() };
        match Host::start(options) {
            Ok(h) => (h, dir),
            Err(e) => std::panic::resume_unwind(Box::new(e.message)),
        }
    }

    #[test]
    fn a_request_token_is_owned_by_its_invocation() {
        let map: Mutex<HashMap<String, Arc<AtomicBool>>> = Mutex::new(HashMap::new());
        let first = TokenGuard::claim(&map, "t1");
        assert!(first.is_some());
        assert!(TokenGuard::claim(&map, "t1").is_none(), "a token in flight is refused");
        // Another owner's entry under the same name is never removed by a stale guard.
        if let Some(g) = first {
            map.lock().unwrap_or_else(PoisonError::into_inner).insert("t1".into(), Arc::new(AtomicBool::new(false)));
            drop(g);
        }
        assert!(map.lock().unwrap_or_else(PoisonError::into_inner).contains_key("t1"), "kept: not the guard's own");
        map.lock().unwrap_or_else(PoisonError::into_inner).clear();
        // An unwinding invocation still releases its own entry.
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = TokenGuard::claim(&map, "t2");
            std::panic::resume_unwind(Box::new("unwind"));
        }));
        assert!(unwound.is_err());
        assert!(map.lock().unwrap_or_else(PoisonError::into_inner).is_empty(), "released while unwinding");
    }

    #[test]
    fn run_refuses_a_token_already_in_flight() {
        let (h, dir) = host();
        let held = TokenGuard::claim(&h.cancels, "busy");
        let env = h.run("app.state.get", json!({}), false, Some("busy"));
        let e = env.error.map(|e| (e.code, e.details["reason"].clone()));
        assert_eq!(e, Some(("arg.invalid".to_owned(), json!("requestTokenInUse"))));
        drop(held);
        assert!(h.run("app.state.get", json!({}), false, Some("busy")).is_ok());
        assert!(h.cancels.lock().unwrap_or_else(PoisonError::into_inner).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn restarts_are_serialised_and_a_waiting_one_does_not_restart_again() {
        let (h, dir) = host();
        let h = std::sync::Arc::new(h);
        // Both callers saw generation 0; the lock is held so both are waiting when the first one runs.
        let held = h.restart_lock.lock().unwrap_or_else(PoisonError::into_inner);
        let callers: Vec<_> = (0..2)
            .map(|_| {
                let h = h.clone();
                std::thread::spawn(move || h.restart_from(0).map_err(|e| e.message))
            })
            .collect();
        drop(held);
        let mut outcomes: Vec<(u64, bool)> =
            callers.into_iter().filter_map(|c| c.join().ok()).filter_map(Result::ok).collect();
        outcomes.sort_unstable();
        assert_eq!(outcomes, vec![(1, false), (1, true)], "one restart; the other waited and answered with it");
        assert_eq!(h.generation(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_restart_drops_old_flags_renumbers_events_and_answers_in_flight_requests_as_restarted() {
        let (h, dir) = host();
        let _ = h.next_seq();
        let _ = h.next_seq();
        let held = TokenGuard::claim(&h.cancels, "old");
        assert!(held.is_some());
        assert_eq!(h.restart().map_err(|e| e.message), Ok((1, true)));
        assert!(h.cancels.lock().unwrap_or_else(PoisonError::into_inner).is_empty(), "old flags dropped");
        assert_eq!(h.next_seq(), 1, "the new generation numbers its events from 1");
        drop(held);
        // A request that started on generation 0 and answers now is not shown as applied.
        let late = answer_of(Envelope::ok(json!({ "applied": true }), None), 0, 1);
        let e = late.error.map(|e| (e.code, e.details["reason"].clone(), e.details["currentGeneration"].clone()));
        assert_eq!(e, Some(("command.timeout".to_owned(), json!("engineRestarted"), json!(1))));
        assert!(answer_of(Envelope::ok(json!({}), None), 1, 1).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_report_of_another_generation_is_ignored() {
        let (h, dir) = host();
        let state = |h: &Host| h.run("app.state.get", json!({}), false, None).result.unwrap_or_default();
        assert_eq!(
            h.report(json!({ "generation": 0, "modal": { "kind": "dialog", "title": "Preferences" } })),
            Ok(json!({}))
        );
        assert_eq!(state(&h)["modal"]["kind"], "dialog");
        assert_eq!(h.restart().map_err(|e| e.message), Ok((1, true)));
        // The new engine starts without the old window's facts; a report still naming generation 0 is ignored.
        assert_eq!(state(&h)["modal"], Value::Null);
        let r = h.report(json!({ "generation": 0, "modal": { "kind": "dialog", "title": "Preferences" } }));
        assert_eq!(r.map(|v| v["ignored"].clone()), Ok(json!("staleGeneration")));
        assert_eq!(state(&h)["modal"], Value::Null);
        assert_eq!(h.report(json!({ "generation": 1, "modal": null })), Ok(json!({})));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_planted_token_is_replaced_and_a_stale_endpoint_is_cleared() {
        let (h, dir) = host();
        let file = h.settings_dir.join("agent-discovery.json");
        let _ = std::fs::create_dir_all(&h.settings_dir);
        let planted = json!({ "format": "toontown-agent-discovery", "formatVersion": "1.0",
            "endpoint": "http://127.0.0.1:1/mcp", "token": "letmein", "processId": 1 });
        let _ = std::fs::write(&file, planted.to_string());
        let token = h.token().unwrap_or_default();
        assert!(tt_executor::is_secret_token(&token, tt_executor::AGENT_TOKEN_BYTES), "{token}");
        // A file advertising an endpoint with no server running is cleared, the well-formed token kept.
        let live = json!({ "format": "toontown-agent-discovery", "formatVersion": "1.0",
            "endpoint": "http://127.0.0.1:1/mcp", "token": token, "processId": 1 });
        let _ = std::fs::write(&file, live.to_string());
        h.stop_http();
        let after: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap_or_default()).unwrap_or_default();
        assert_eq!(after["endpoint"], Value::Null);
        assert_eq!(after["processId"], Value::Null);
        assert_eq!(after["token"], json!(token));
        let _ = std::fs::remove_dir_all(dir);
    }
}
