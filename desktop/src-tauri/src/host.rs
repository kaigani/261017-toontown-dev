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
    /// Each window's sensor facts (ruling O-2, O-9), combined into the one report the executor sees.
    windows: Mutex<Windows>,
}

/// The sensor facts of one window.
#[derive(Debug, Default, Clone)]
struct WindowFacts {
    /// Its open modal state (`{kind, title}`), with the order it was reported in.
    modal: Option<(u64, Value)>,
    /// A text field of it has focus.
    text_focus: bool,
    /// Its rendered UI tree and the engine event number it vouches for.
    tree: Option<(Value, Option<u64>)>,
}

/// The windows' facts and which window reported text focus last.
#[derive(Debug, Default)]
struct Windows {
    facts: std::collections::BTreeMap<String, WindowFacts>,
    order: u64,
    focused: Option<String>,
    /// The float windows the host opened and has not seen destroyed (the main window is always expected): with every
    /// window that reported anything, the freshness quorum (C41-R2-06).
    expected: std::collections::BTreeSet<String>,
}

impl Windows {
    /// The combined facts as report members: `modal` (the most recent open modal of any window, or null), `textFocus`
    /// (the focused window's) and, when any window reported a tree, `uiTree` (the main window's tree with each other
    /// window's tree as a child) with `renderedSeq`: the smallest number over the quorum (the main window, every float
    /// the host opened, every window that reported anything), absent when any of them has no tree or no number yet, so
    /// the combination is fresh only when every window has caught up (C41-R2-06).
    fn combined(&self) -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        let modal =
            self.facts.values().filter_map(|f| f.modal.as_ref()).max_by_key(|(o, _)| *o).map(|(_, v)| v.clone());
        m.insert("modal".into(), modal.unwrap_or(Value::Null));
        let focus = self.focused.as_ref().and_then(|l| self.facts.get(l)).is_some_and(|f| f.text_focus);
        m.insert("textFocus".into(), Value::Bool(focus));
        let trees: Vec<(&String, &(Value, Option<u64>))> =
            self.facts.iter().filter_map(|(l, f)| f.tree.as_ref().map(|t| (l, t))).collect();
        if !trees.is_empty() {
            let mut root = self
                .facts
                .get("main")
                .and_then(|f| f.tree.as_ref())
                .map_or_else(|| json!({ "role": "window", "id": "main" }), |(t, _)| t.clone());
            for (l, (t, _)) in trees.iter().filter(|(l, _)| l.as_str() != "main") {
                let mut node = t.clone();
                if let Some(o) = node.as_object_mut() {
                    o.insert("id".into(), json!(l));
                }
                if let Some(a) = root
                    .as_object_mut()
                    .map(|o| o.entry("children").or_insert_with(|| json!([])))
                    .and_then(Value::as_array_mut)
                {
                    a.push(node);
                }
            }
            m.insert("uiTree".into(), root);
            let quorum: std::collections::BTreeSet<&str> = std::iter::once("main")
                .chain(self.expected.iter().map(String::as_str))
                .chain(self.facts.keys().map(String::as_str))
                .collect();
            let seqs: Option<Vec<u64>> =
                quorum.iter().map(|l| self.facts.get(*l).and_then(|f| f.tree.as_ref()).and_then(|(_, s)| *s)).collect();
            if let Some(min) = seqs.and_then(|v| v.into_iter().min()) {
                m.insert("renderedSeq".into(), json!(min));
            }
        } else {
            // No window has a tree (the main window reloads, every window went away): the kernel's stored tree is
            // cleared explicitly; an absent `uiTree` would leave the old tree vouched for (C41-R3-04).
            m.insert("uiTreeCleared".into(), Value::Bool(true));
        }
        m
    }
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
            windows: Mutex::new(Windows::default()),
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
        // The new engine starts without any window's facts; the windows report them again (ruling O-9). The open float
        // windows stay expected: the new engine is not fresh until they have reported (C41-R2-06).
        {
            let mut w = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
            let expected = std::mem::take(&mut w.expected);
            *w = Windows { expected, ..Windows::default() };
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if was_serving {
            let _ = self.start_http();
        }
        Ok((generation, true))
    }

    /// Runs one command as the user. With a token, the request carries a cancellation flag that `cancel` sets; the
    /// probe only loads it. Without one, the plain entry point (no link, no cost).
    pub fn run(&self, command: &str, params: Value, dry_run: bool, token: Option<&str>) -> Envelope {
        self.run_from("main", command, params, dry_run, token)
    }

    /// [`Host::run`] for the window `window` (its label): a token is that window's own, so another window cannot
    /// cancel the request even with the same token text (ruling O-2).
    pub fn run_from(&self, window: &str, command: &str, params: Value, dry_run: bool, token: Option<&str>) -> Envelope {
        let owned = token.map(|t| format!("{window}\u{1f}{t}"));
        let token = owned.as_deref();
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
                            format!(
                                "the request token `{}` is already in flight",
                                t.rsplit('\u{1f}').next().unwrap_or(t)
                            ),
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
    pub fn report(&self, value: Value) -> Result<Value, String> {
        self.report_from("main", value)
    }

    /// [`Host::report`] from the window `window` (its label): its modal state, text focus and UI tree are kept as that
    /// window's and the executor gets the combination of every window's (ruling O-2; design note §3).
    ///
    /// # Errors
    /// The JSON text of `arg.invalid` for a malformed report.
    pub fn report_from(&self, window: &str, mut value: Value) -> Result<Value, String> {
        let generation = value.as_object_mut().and_then(|m| m.remove("generation"));
        if let Some(g) = generation
            && g.as_u64() != Some(self.generation())
        {
            return Ok(json!({ "ignored": "staleGeneration", "generation": self.generation() }));
        }
        if let Some(m) = value.as_object_mut() {
            let (modal, focus, tree, seq) =
                (m.remove("modal"), m.remove("textFocus"), m.remove("uiTree"), m.remove("renderedSeq"));
            if modal.is_some() || focus.is_some() || tree.is_some() {
                let mut w = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
                w.order += 1;
                let order = w.order;
                let f = w.facts.entry(window.to_owned()).or_default();
                if let Some(v) = modal {
                    f.modal = if v.is_null() { None } else { Some((order, v)) };
                }
                if let Some(v) = &focus {
                    f.text_focus = v.as_bool().unwrap_or(false);
                }
                if let Some(t) = tree {
                    f.tree = Some((t, seq.and_then(|s| s.as_u64())));
                }
                if focus.is_some() {
                    w.focused = Some(window.to_owned());
                }
                m.extend(w.combined());
            }
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
        self.cancel_from("main", token);
    }

    /// [`Host::cancel`] from the window `window`: only that window's requests (ruling O-2).
    pub fn cancel_from(&self, window: &str, token: &str) {
        let key = format!("{window}\u{1f}{token}");
        if let Some(f) = self.cancels.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
            f.store(true, Ordering::Release);
        }
    }

    /// A float window the host is opening: it joins the freshness quorum at once, before it has reported anything, so
    /// the combined tree is not fresh until it has (C41-R2-06).
    pub fn expect_window(&self, label: &str) {
        let combined = {
            let mut w = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
            if !w.expected.insert(label.to_owned()) {
                return;
            }
            w.combined()
        };
        let report: tt_executor::host::ShellReport =
            serde_json::from_value(Value::Object(combined)).unwrap_or_default();
        self.exec().report_shell(report);
    }

    /// Clears the sensor facts a window reported (ruling O-9): its modal state and text focus end with it when it is
    /// destroyed, reloaded or replaced, so a report whose reporter went away cannot block agents or shortcuts. It
    /// leaves the freshness quorum at once (a wait pending on it is released by the recomputed combination). The
    /// other windows' facts stay; the executor gets the new combination (ruling O-2).
    pub fn forget_window_reports(&self, label: &str) {
        let combined = {
            let mut w = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
            w.facts.remove(label);
            w.expected.remove(label);
            if w.focused.as_deref() == Some(label) {
                w.focused = None;
            }
            w.combined()
        };
        let mut combined = combined;
        // The status-bar texts a window reported (hover description, pointer position) end with it too: cleared
        // explicitly, not left as they were (C41-R3-04 audit).
        combined.insert("description".into(), json!(""));
        combined.insert("pointer".into(), json!(""));
        let report: tt_executor::host::ShellReport =
            serde_json::from_value(Value::Object(combined)).unwrap_or_default();
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
    use std::time::Instant;

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
        let held = TokenGuard::claim(&h.cancels, "main\u{1f}busy");
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
    fn reports_of_several_windows_combine_and_a_closed_window_releases_its_facts() {
        let (h, dir) = host();
        let state = |h: &Host| h.run("app.state.get", json!({}), false, None).result.unwrap_or_default();
        let tree = |h: &Host| h.run("inspect.ui.tree", json!({}), false, None).result.unwrap_or_default();
        h.exec().report_shell(serde_json::from_value(json!({ "attached": true })).unwrap_or_default());
        let now = h.exec().events_broadcast();
        assert_eq!(
            h.report_from(
                "main",
                json!({ "uiTree": { "role": "window", "id": "main", "children": [] }, "renderedSeq": now })
            ),
            Ok(json!({}))
        );
        assert_eq!(
            h.report_from(
                "float-history",
                json!({ "uiTree": { "role": "window", "label": "History" }, "renderedSeq": now })
            ),
            Ok(json!({}))
        );
        let t = tree(&h);
        assert_eq!(t["root"]["children"][0]["id"], "float-history", "one tree with the float: {t}");
        // A dialog in the float blocks like one in the main window; the main window's own null does not end it.
        let _ = h.report_from("float-history", json!({ "modal": { "kind": "dialog", "title": "Rename" } }));
        let _ = h.report_from("main", json!({ "modal": null }));
        assert_eq!(state(&h)["modal"]["title"], "Rename");
        // The float goes away: its facts go with it (ruling O-9); the tree loses its node.
        h.forget_window_reports("float-history");
        assert_eq!(state(&h)["modal"], Value::Null);
        assert!(tree(&h)["root"]["children"].as_array().is_some_and(Vec::is_empty));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// C41-R2-06: every window the host expects is in the freshness quorum: a float the host opened that has not
    /// reported a tree (or reported one without a number) keeps the combination unvouched, however fresh the main
    /// window is; a wait pending on it ends at once when the float closes, and a restart keeps the open floats
    /// expected.
    #[test]
    fn a_float_that_has_not_reported_keeps_the_combination_unvouched() {
        let (h, dir) = host();
        h.exec().report_shell(serde_json::from_value(json!({ "attached": true })).unwrap_or_default());
        let now = h.exec().events_broadcast();
        let main = json!({ "uiTree": { "role": "window", "id": "main" }, "renderedSeq": now });
        h.expect_window("float-history");
        assert_eq!(h.report_from("main", main.clone()), Ok(json!({})));
        assert!(!h.exec().await_rendered(Duration::ZERO), "the opened float has not reported");
        let seq = h.run("inspect.ui.tree", json!({}), false, None).result.unwrap_or_default()["renderedSeq"].clone();
        assert_eq!(seq, Value::Null, "an unvouched tree says so");
        // A tree without a number does not vouch either.
        let _ = h.report_from("float-history", json!({ "uiTree": { "role": "window" } }));
        assert!(!h.exec().await_rendered(Duration::ZERO));
        let _ = h.report_from("float-history", json!({ "uiTree": { "role": "window" }, "renderedSeq": now }));
        assert!(h.exec().await_rendered(Duration::ZERO), "every window caught up");
        // A waiter on a float that never reports is released when the float closes.
        h.expect_window("float-colour");
        let exec = h.exec();
        let waiter = std::thread::spawn(move || exec.await_rendered(Duration::from_secs(2)));
        let started = Instant::now();
        h.forget_window_reports("float-colour");
        assert!(waiter.join().unwrap_or(false));
        assert!(started.elapsed() < Duration::from_millis(1500));
        // A restart keeps the open floats in the quorum.
        h.expect_window("float-library");
        assert!(h.restart().is_ok());
        let now = h.exec().events_broadcast();
        h.exec().report_shell(serde_json::from_value(json!({ "attached": true })).unwrap_or_default());
        let mut main = main;
        main["renderedSeq"] = json!(now);
        main["generation"] = json!(h.generation());
        let _ = h.report_from("main", main);
        assert!(!h.exec().await_rendered(Duration::ZERO), "the float has not reported to the new engine");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The quorum invariant's host clauses (reviews/M4-design-floating-windows.md §3a), on the pure combination: the
    /// main window always counts; an expected window without a tree, a tree without a number, or no tree at all leave
    /// the combination unvouched (and no tree at all clears the kernel's tree explicitly); the smallest number counts.
    #[test]
    fn the_quorum_rule_clause_by_clause() {
        let tree =
            |seq: Option<u64>| WindowFacts { tree: Some((json!({ "role": "window" }), seq)), ..Default::default() };
        let mut w = Windows::default();
        assert_eq!(w.combined().get("uiTreeCleared"), Some(&json!(true)), "an empty set is unvouched and clears");
        w.facts.insert("float-history".into(), tree(Some(9)));
        assert!(w.combined().get("renderedSeq").is_none(), "the main window is always expected");
        w.facts.insert("main".into(), tree(Some(7)));
        assert_eq!(w.combined().get("renderedSeq"), Some(&json!(7)), "the smallest number of the quorum");
        w.expected.insert("float-colour".into());
        assert!(w.combined().get("renderedSeq").is_none(), "an expected window without a tree");
        w.facts.insert("float-colour".into(), tree(None));
        assert!(w.combined().get("renderedSeq").is_none(), "a tree without a number");
        w.facts.insert("float-colour".into(), tree(Some(8)));
        assert_eq!(w.combined().get("renderedSeq"), Some(&json!(7)));
        assert!(w.combined().get("uiTreeCleared").is_none());
    }

    #[test]
    fn a_window_cannot_cancel_another_windows_request() {
        let (h, dir) = host();
        let mine = TokenGuard::claim(&h.cancels, "float-history\u{1f}t1");
        h.cancel_from("main", "t1");
        assert!(mine.as_ref().is_some_and(|g| !g.flag.load(Ordering::Acquire)), "the main window's cancel missed it");
        h.cancel_from("float-history", "t1");
        assert!(mine.as_ref().is_some_and(|g| g.flag.load(Ordering::Acquire)), "its own window cancels it");
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
