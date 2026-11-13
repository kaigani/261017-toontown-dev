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
    pub fn restart(&self) -> Result<u64, tt_executor::CommandError> {
        let (exec, _) = Self::spawn_executor(&self.options)?;
        let was_serving = self.http.lock().unwrap_or_else(PoisonError::into_inner).is_some();
        self.stop_http();
        // The stuck executor's autosave becomes recoverable work for the new one.
        let _ = exec.release_abandoned_recovery();
        *self.exec.write().unwrap_or_else(PoisonError::into_inner) = exec;
        *self.watchdog.lock().unwrap_or_else(PoisonError::into_inner) = crate::watchdog::Watchdog::default();
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if was_serving {
            let _ = self.start_http();
        }
        Ok(generation)
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
        match token {
            None => self.exec().invoke(inv),
            Some(t) => {
                let flag = Arc::new(AtomicBool::new(false));
                self.cancels.lock().unwrap_or_else(PoisonError::into_inner).insert(t.to_owned(), flag.clone());
                let probe = move || flag.load(Ordering::Acquire);
                let (env, _) = self.exec().invoke_served_until(inv, &probe);
                self.cancels.lock().unwrap_or_else(PoisonError::into_inner).remove(t);
                env
            }
        }
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
