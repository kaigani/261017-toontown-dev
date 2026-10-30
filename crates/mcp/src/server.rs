//! The MCP server over one JSON-RPC stream (stdio in headless mode): lifecycle, dispatch, tools, resources,
//! subscriptions, events, prompts, completions, logging, progress, cancellation and elicitation (11 §4, §7, §9, §12).
//!
//! Threads: a reader turns input lines into messages; requests run in arrival order on one call thread, except
//! `job_wait`, which waits on its own thread (so a long wait does not hold up other calls); `notifications/cancelled`
//! reaches queued and waiting requests; an event pump receives the executor's events, keeps
//! the numbered event buffer of `app://events`, sends coalesced `notifications/resources/updated` (at most one per
//! resource per 100 ms), log notifications and progress, and polls the session's state (disconnected by the user,
//! typed tools changed). All output goes through one locked writer, one JSON object per line.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use tt_executor::agents::{AgentConnect, AgentSession, AgentStatus, Category, Policy};
use tt_executor::{BatchRequest, Envelope, ExecutorHandle, Invocation, InvokeOptions, Origin, Status};
use tt_registry::{CommandError, Scope};

use crate::jsonrpc::{self, Incoming};
use crate::tools::{self, Plan, ToolDef};
use crate::{docs, prompts, resources};

/// Protocol revisions this server speaks (11 §4: minimum 2025-06-18).
pub const PROTOCOL_VERSIONS: [&str; 2] = ["2025-11-25", "2025-06-18"];
/// The revision answered when the client asks for one this server does not know.
pub const DEFAULT_PROTOCOL: &str = "2025-06-18";
/// How long a confirmation elicitation waits for the client (the token's lifetime).
const ELICIT_TIMEOUT: Duration = Duration::from_secs(300);
/// Events kept for `app://events?after=`.
const EVENT_BUFFER: usize = 10_000;
/// Coalescing of resource-updated notifications (11 §9.1).
const COALESCE: Duration = Duration::from_millis(100);
/// Concurrent requests in flight.
const MAX_INFLIGHT: usize = 64;

/// How the server runs.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// `headless` (stdio) or `attached`.
    pub run_mode: String,
    /// Policy of the session (`None`: the hub's default).
    pub policy: Option<Policy>,
    /// Scopes of the session (`None`: the policy's).
    pub scopes: Option<BTreeSet<Scope>>,
    /// File-system roots granted at launch (`--allow-path`).
    pub roots: Vec<PathBuf>,
    /// Confirmation categories pre-authorised for `confirm: true`.
    pub preauthorized: BTreeSet<Category>,
    /// How long a call waits while the session is paused before failing with `agent.paused` (11 §6.4).
    pub pause_wait: Duration,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            run_mode: "headless".to_owned(),
            policy: None,
            scopes: None,
            roots: Vec::new(),
            preauthorized: BTreeSet::new(),
            pause_wait: Duration::from_secs(10),
        }
    }
}

/// Log levels (RFC 5424 order).
const LEVELS: [&str; 8] = ["debug", "info", "notice", "warning", "error", "critical", "alert", "emergency"];

#[derive(Debug, Default)]
struct State {
    session: Option<AgentSession>,
    client_caps: Value,
    protocol: String,
    log_level: Option<usize>,
    subscriptions: HashSet<String>,
    dirty: HashSet<String>,
    last_sent: HashMap<String, Instant>,
    events: VecDeque<Value>,
    next_seq: u64,
    typed_version: u64,
    progress: HashMap<String, Value>,
    inflight: usize,
}

struct Shared {
    handle: ExecutorHandle,
    opts: ServerOptions,
    out: Mutex<Box<dyn Write + Send>>,
    st: Mutex<State>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<Result<Value, Value>>>>,
    cancelled: Mutex<HashSet<String>>,
    closing: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn send(&self, v: &Value) {
        let Ok(text) = serde_json::to_string(v) else { return };
        let mut out = lock(&self.out);
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }

    fn origin(&self) -> Option<Origin> {
        let st = lock(&self.st);
        st.session.as_ref().map(|s| Origin::Agent {
            session: s.session_id.clone(),
            name: s.client.clone(),
            scopes: s.scopes.clone(),
        })
    }

    fn session_id(&self) -> Option<String> {
        lock(&self.st).session.as_ref().map(|s| s.session_id.clone())
    }

    fn status(&self) -> Option<AgentStatus> {
        let id = self.session_id()?;
        self.handle.agent_status(&id).ok()
    }

    /// The session's origin with its current grant.
    fn current_origin(&self) -> Option<Origin> {
        let base = self.origin()?;
        match (base, self.status()) {
            (Origin::Agent { session, name, .. }, Some(st)) => Some(Origin::Agent { session, name, scopes: st.scopes }),
            (o, _) => Some(o),
        }
    }

    fn is_cancelled(&self, id: &Value) -> bool {
        lock(&self.cancelled).contains(&id.to_string())
    }

    /// Sends a request to the client and waits for its answer.
    fn ask(&self, method: &str, params: Value, timeout: Duration) -> Option<Result<Value, Value>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        lock(&self.pending).insert(id, tx);
        self.send(&jsonrpc::request(id, method, params));
        let r = rx.recv_timeout(timeout).ok();
        lock(&self.pending).remove(&id);
        r
    }

    fn client_can(&self, feature: &str) -> bool {
        lock(&self.st).client_caps.get(feature).is_some()
    }
}

/// Serves one MCP client on `input`/`output` until the input ends or the user disconnects the session.
///
/// # Errors
/// A message when the executor cannot be reached at all.
pub fn serve<R, W>(handle: ExecutorHandle, opts: ServerOptions, input: R, output: W) -> Result<(), String>
where
    R: BufRead + Send + 'static,
    W: Write + Send + 'static,
{
    let shared = Arc::new(Shared {
        handle: handle.clone(),
        opts,
        out: Mutex::new(Box::new(output)),
        st: Mutex::new(State { protocol: DEFAULT_PROTOCOL.to_owned(), ..State::default() }),
        next_id: AtomicU64::new(1),
        pending: Mutex::new(HashMap::new()),
        cancelled: Mutex::new(HashSet::new()),
        closing: AtomicBool::new(false),
    });
    // Reader.
    let (line_tx, line_rx) = mpsc::channel::<Option<String>>();
    std::thread::Builder::new()
        .name("mcp-reader".into())
        .spawn(move || {
            let mut input = input;
            loop {
                let mut line = String::new();
                match input.read_line(&mut line) {
                    Ok(0) | Err(_) => {
                        let _ = line_tx.send(None);
                        return;
                    }
                    Ok(_) => {
                        if line_tx.send(Some(line)).is_err() {
                            return;
                        }
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    // Event pump.
    let events = handle.subscribe();
    let pump = {
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("mcp-events".into())
            .spawn(move || pump_events(&shared, &events))
            .map_err(|e| e.to_string())?
    };
    // Requests other than `job_wait` run one at a time in arrival order, so pipelined calls apply in the order sent
    // and the audit log is reproducible.
    let (work_tx, work_rx) = mpsc::channel::<(Value, String, Value)>();
    let worker = {
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("mcp-calls".into())
            .spawn(move || {
                for (id, method, params) in work_rx {
                    answer(&shared, &id, &method, &params);
                }
            })
            .map_err(|e| e.to_string())?
    };
    let mut waits = Vec::new();
    while !shared.closing.load(Ordering::Relaxed) {
        let line = match line_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Some(l)) => l,
            Ok(None) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => continue,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let max = shared.status().map_or(50 * 1024 * 1024, |s| s.limits.max_inline_bytes);
        if line.len() as u64 > max {
            shared.send(&jsonrpc::error(
                &Value::Null,
                jsonrpc::INVALID_REQUEST,
                "message exceeds the session's maxInlineBytes",
                Some(json!({ "code": "limit.exceeded", "limit": "maxInlineBytes", "value": max })),
            ));
            continue;
        }
        match jsonrpc::parse(line) {
            Err(response) => shared.send(&response),
            Ok(Incoming::Response { id, outcome }) => {
                let key = id.as_u64().unwrap_or(0);
                if let Some(tx) = lock(&shared.pending).remove(&key) {
                    let _ = tx.send(outcome);
                }
            }
            Ok(Incoming::Notification { method, params }) => notification(&shared, &method, &params),
            Ok(Incoming::Request { id, method, params }) => {
                if method == "initialize" || method == "ping" {
                    if let Some(r) = request(&shared, &id, &method, &params) {
                        shared.send(&r);
                    }
                    continue;
                }
                {
                    let mut st = lock(&shared.st);
                    if st.inflight >= MAX_INFLIGHT {
                        drop(st);
                        shared.send(&jsonrpc::error(&id, jsonrpc::INTERNAL_ERROR, "too many requests in flight", None));
                        continue;
                    }
                    st.inflight += 1;
                }
                // `job_wait` blocks on the caller's side (11 P5) and gets its own thread; everything else runs in
                // arrival order.
                if method == "tools/call" && params.get("name").and_then(Value::as_str) == Some("job_wait") {
                    let s = shared.clone();
                    let spawned = std::thread::Builder::new()
                        .name("mcp-wait".into())
                        .spawn(move || answer(&s, &id, &method, &params));
                    match spawned {
                        Ok(h) => waits.push(h),
                        Err(e) => return Err(e.to_string()),
                    }
                    waits.retain(|h| !h.is_finished());
                } else if work_tx.send((id, method, params)).is_err() {
                    break;
                }
            }
        }
    }
    drop(work_tx);
    let _ = worker.join();
    for w in waits {
        let _ = w.join();
    }
    shared.closing.store(true, Ordering::Relaxed);
    if let Some(id) = shared.session_id() {
        let _ = handle.disconnect_agent(&id, "client closed the connection");
    }
    let _ = pump.join();
    Ok(())
}

/// Answers one request unless the client cancelled it (11 §4: no response to a cancelled request).
fn answer(shared: &Arc<Shared>, id: &Value, method: &str, params: &Value) {
    if !shared.is_cancelled(id)
        && let Some(r) = request(shared, id, method, params)
        && !shared.is_cancelled(id)
    {
        shared.send(&r);
    }
    lock(&shared.cancelled).remove(&id.to_string());
    lock(&shared.st).inflight -= 1;
}

fn notification(shared: &Arc<Shared>, method: &str, params: &Value) {
    match method {
        "notifications/initialized" | "notifications/roots/list_changed" => {
            if shared.client_can("roots") {
                let s = shared.clone();
                let _ = std::thread::Builder::new().name("mcp-roots".into()).spawn(move || refresh_roots(&s));
            }
        }
        "notifications/cancelled" => {
            if let Some(id) = params.get("requestId") {
                lock(&shared.cancelled).insert(id.to_string());
            }
        }
        _ => {}
    }
}

/// The client's roots join the launch roots of the session (11 §6.2).
fn refresh_roots(shared: &Shared) {
    let Some(Ok(r)) = shared.ask("roots/list", json!({}), Duration::from_secs(30)) else { return };
    let Some(session) = shared.session_id() else { return };
    let mut roots = shared.opts.roots.clone();
    for root in r.get("roots").and_then(Value::as_array).into_iter().flatten() {
        if let Some(path) = root.get("uri").and_then(Value::as_str).and_then(file_uri_path) {
            roots.push(path);
        }
    }
    let _ = shared.handle.grant_roots(&session, roots);
}

/// The path of a `file://` URI.
fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = resources::decode(rest.strip_prefix("localhost").unwrap_or(rest));
    // `file:///C:/x` on Windows.
    let trimmed =
        if rest.len() > 2 && rest.as_bytes()[0] == b'/' && rest.as_bytes()[2] == b':' { &rest[1..] } else { &rest[..] };
    Some(PathBuf::from(trimmed))
}

fn invalid_params(id: &Value, msg: &str) -> Value {
    jsonrpc::error(id, jsonrpc::INVALID_PARAMS, msg, None)
}

#[allow(clippy::too_many_lines, reason = "the method table")]
fn request(shared: &Arc<Shared>, id: &Value, method: &str, params: &Value) -> Option<Value> {
    if method == "ping" {
        return Some(jsonrpc::result(id, json!({})));
    }
    if method == "initialize" {
        return Some(initialize(shared, id, params));
    }
    if lock(&shared.st).session.is_none() {
        return Some(jsonrpc::error(id, jsonrpc::INVALID_REQUEST, "the session is not initialized", None));
    }
    let reg = tt_executor::registry().ok()?;
    Some(match method {
        "tools/list" => {
            jsonrpc::result(id, json!({ "tools": tool_list(shared).iter().map(ToolDef::listing).collect::<Vec<_>>() }))
        }
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(invalid_params(id, "`name` is required"));
            };
            let args = params.get("arguments").and_then(Value::as_object).cloned().unwrap_or_default();
            let token = params.get("_meta").and_then(|m| m.get("progressToken")).cloned();
            match call_tool(shared, id, name, &args, token) {
                ToolOutcome::Result(v) => jsonrpc::result(id, v),
                ToolOutcome::UnknownTool => invalid_params(id, &format!("unknown tool `{name}`")),
                ToolOutcome::Cancelled => return None,
            }
        }
        "resources/list" => jsonrpc::result(id, json!({ "resources": resources::listing() })),
        "resources/templates/list" => jsonrpc::result(id, json!({ "resourceTemplates": resources::templates() })),
        "resources/read" => {
            let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                return Some(invalid_params(id, "`uri` is required"));
            };
            read_resource(shared, id, uri)
        }
        "resources/subscribe" | "resources/unsubscribe" => {
            let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                return Some(invalid_params(id, "`uri` is required"));
            };
            if resources::resolve(uri).is_none() {
                return Some(jsonrpc::error(
                    id,
                    jsonrpc::RESOURCE_NOT_FOUND,
                    "resource not found",
                    Some(json!({ "uri": uri })),
                ));
            }
            let mut st = lock(&shared.st);
            if method == "resources/subscribe" {
                st.subscriptions.insert(uri.to_owned());
            } else {
                st.subscriptions.remove(uri);
                st.dirty.remove(uri);
            }
            jsonrpc::result(id, json!({}))
        }
        "prompts/list" => {
            jsonrpc::result(id, json!({ "prompts": prompts::PROMPTS.iter().map(prompts::listing).collect::<Vec<_>>() }))
        }
        "prompts/get" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let Some(p) = prompts::PROMPTS.iter().find(|p| p.name == name) else {
                return Some(invalid_params(id, &format!("unknown prompt `{name}`")));
            };
            let args = params.get("arguments").and_then(Value::as_object).cloned().unwrap_or_default();
            match prompts::render(p, &args) {
                Ok(v) => jsonrpc::result(id, v),
                Err(missing) => invalid_params(id, &format!("missing required argument `{missing}`")),
            }
        }
        "logging/setLevel" => {
            let level = params.get("level").and_then(Value::as_str).unwrap_or_default();
            let Some(i) = LEVELS.iter().position(|l| *l == level) else {
                return Some(invalid_params(id, "unknown log level"));
            };
            lock(&shared.st).log_level = Some(i);
            jsonrpc::result(id, json!({}))
        }
        "completion/complete" => jsonrpc::result(id, json!({ "completion": complete(shared, reg, params) })),
        _ => jsonrpc::error(id, jsonrpc::METHOD_NOT_FOUND, &format!("unknown method `{method}`"), None),
    })
}

fn initialize(shared: &Arc<Shared>, id: &Value, params: &Value) -> Value {
    let reg = match tt_executor::registry() {
        Ok(r) => r,
        Err(e) => return jsonrpc::error(id, jsonrpc::INTERNAL_ERROR, &format!("registry: {}", e.0), None),
    };
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(DEFAULT_PROTOCOL);
    let protocol = if PROTOCOL_VERSIONS.contains(&asked) { asked } else { DEFAULT_PROTOCOL };
    let client = params
        .get("clientInfo")
        .and_then(|c| c.get("title").or_else(|| c.get("name")))
        .and_then(Value::as_str)
        .unwrap_or("agent")
        .to_owned();
    let caps = params.get("capabilities").cloned().unwrap_or_else(|| json!({}));
    let mut st = lock(&shared.st);
    if st.session.is_none() {
        let o = &shared.opts;
        let connect = AgentConnect {
            client,
            policy: o.policy,
            scopes: o.scopes.clone(),
            roots: o.roots.clone(),
            preauthorized: o.preauthorized.clone(),
            transport: "stdio".to_owned(),
        };
        match shared.handle.connect_agent(connect) {
            Ok(s) => st.session = Some(s),
            Err(e) => return jsonrpc::error(id, jsonrpc::INTERNAL_ERROR, &e.message, None),
        }
    }
    st.client_caps = caps;
    protocol.clone_into(&mut st.protocol);
    let session = st.session.clone();
    drop(st);
    let version = tt_engine::Engine::new(tt_engine::EngineConfig { run_mode: tt_engine::RunMode::Headless })
        .map_or(env!("CARGO_PKG_VERSION"), |e| e.info().version);
    jsonrpc::result(
        id,
        json!({
            "protocolVersion": protocol,
            "capabilities": {
                "tools": { "listChanged": true },
                "resources": { "subscribe": true, "listChanged": false },
                "prompts": { "listChanged": false },
                "logging": {},
                "completions": {},
            },
            "serverInfo": { "name": "toontown", "title": "Toontown", "version": version },
            "instructions": docs::agent_guide(reg),
            "_meta": {
                "toontown": {
                    "registryVersion": tt_registry::REGISTRY_VERSION,
                    "formatVersion": tt_model::FORMAT_VERSION,
                    "runMode": shared.opts.run_mode,
                    "sessionId": session.as_ref().map(|s| s.session_id.clone()),
                    "policy": session.as_ref().map(|s| s.policy.as_str()),
                    "scopes": session.as_ref().map(|s| s.scopes.iter().map(|x| x.as_str()).collect::<Vec<_>>()),
                }
            },
        }),
    )
}

/// The tools of the session: the generic set, plus the typed tools when enabled (`agent.typedTools.set`).
fn tool_list(shared: &Shared) -> Vec<ToolDef> {
    let mut out = tools::generic();
    if let (Some(st), Ok(reg)) = (shared.status(), tt_executor::registry()) {
        if let Some(namespaces) = st.typed_tools {
            for e in reg.entries() {
                if e.spec.kind == tt_registry::Kind::Event {
                    continue;
                }
                let ns = e.id().split('.').next().unwrap_or_default();
                if namespaces.is_empty() || namespaces.iter().any(|n| n == ns) {
                    out.push(tools::typed(&e.describe()));
                }
            }
        }
        lock(&shared.st).typed_version = st.typed_tools_version;
    }
    out
}

enum ToolOutcome {
    Result(Value),
    UnknownTool,
    Cancelled,
}

fn envelope_error(code: &str, message: String, details: Value, hint: Option<&str>) -> Envelope {
    let mut e = CommandError::with_details(code, message, details);
    if let Some(h) = hint {
        e = e.hint(h);
    }
    Envelope::error(e, None)
}

/// The text summary of an envelope (11 §13: names in text, ids in structured content).
fn summary(tool: &str, command: &str, env: &Value) -> String {
    match env.get("status").and_then(Value::as_str) {
        Some("ok") => {
            let n = env.get("changes").and_then(Value::as_array).map_or(0, Vec::len);
            let rev =
                env.get("documentRev").and_then(Value::as_u64).map_or_else(|| "none".to_owned(), |r| r.to_string());
            let dry = if env.get("dryRun").and_then(Value::as_bool).unwrap_or(false) { " (dry run)" } else { "" };
            format!("{tool}: `{command}` ok{dry}; {n} change(s); documentRev {rev}")
        }
        Some("needs_confirmation") => format!(
            "{tool}: `{command}` needs confirmation: {}. Re-invoke the same call with confirmToken `{}`.",
            env.pointer("/result/summary").and_then(Value::as_str).unwrap_or_default(),
            env.pointer("/result/confirmToken").and_then(Value::as_str).unwrap_or_default()
        ),
        _ => format!(
            "{tool}: `{command}` failed: {} — {}{}",
            env.pointer("/error/code").and_then(Value::as_str).unwrap_or("error"),
            env.pointer("/error/message").and_then(Value::as_str).unwrap_or_default(),
            env.pointer("/error/hint").and_then(Value::as_str).map(|h| format!(" (hint: {h})")).unwrap_or_default()
        ),
    }
}

fn tool_result(tool: &str, command: &str, env: &Envelope) -> Value {
    let v = serde_json::to_value(env).unwrap_or_else(|_| json!({ "status": "error" }));
    let text = summary(tool, command, &v);
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": v,
        "isError": env.status == Status::Error,
    })
}

/// Waits while the user has paused the session (11 §6.4). `Err` is the `agent.paused` envelope.
fn wait_unpaused(shared: &Shared, id: &Value) -> Result<(), Option<Box<Envelope>>> {
    let deadline = Instant::now() + shared.opts.pause_wait;
    loop {
        if shared.is_cancelled(id) {
            return Err(None);
        }
        match shared.status() {
            Some(st) if !st.connected => {
                return Err(Some(Box::new(envelope_error(
                    "agent.disconnected",
                    "the user disconnected this agent session".to_owned(),
                    json!({}),
                    None,
                ))));
            }
            Some(st) if st.paused => {
                if Instant::now() >= deadline {
                    return Err(Some(Box::new(envelope_error(
                        "agent.paused",
                        "the user paused this agent session".to_owned(),
                        json!({ "waitedMs": shared.opts.pause_wait.as_millis() }),
                        Some("wait and call again; the user resumes the session in the app"),
                    ))));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => return Ok(()),
        }
    }
}

#[allow(clippy::too_many_lines, reason = "the tool dispatch")]
fn call_tool(
    shared: &Arc<Shared>,
    rid: &Value,
    name: &str,
    args: &Map<String, Value>,
    progress: Option<Value>,
) -> ToolOutcome {
    let Ok(reg) = tt_executor::registry() else { return ToolOutcome::UnknownTool };
    let generic = tools::generic().iter().any(|t| t.name == name);
    let typed_id = if generic {
        None
    } else {
        let st = shared.status();
        let namespaces = st.and_then(|s| s.typed_tools);
        let found = reg
            .entries()
            .filter(|e| e.spec.kind != tt_registry::Kind::Event)
            .find(|e| tools::typed_name(e.id()) == name)
            .map(|e| e.id().to_owned());
        match (namespaces, found) {
            (Some(ns), Some(id)) if ns.is_empty() || ns.iter().any(|n| Some(n.as_str()) == id.split('.').next()) => {
                Some(id)
            }
            _ => return ToolOutcome::UnknownTool,
        }
    };
    if let Err(e) = wait_unpaused(shared, rid) {
        return match e {
            None => ToolOutcome::Cancelled,
            Some(env) => ToolOutcome::Result(tool_result(name, name, &env)),
        };
    }
    let Some(origin) = shared.current_origin() else { return ToolOutcome::UnknownTool };
    let started = Instant::now();
    let plan = match typed_id {
        Some(id) => Plan::Command {
            id,
            params: Value::Object(args.clone()),
            dry_run: false,
            expect_rev: Map::new(),
            confirm_token: None,
            confirm: false,
            request_id: None,
        },
        None => match tools::plan(name, args) {
            Ok(p) => p,
            Err(msg) => {
                let env = envelope_error("arg.invalid", msg, json!({ "tool": name }), None);
                shared.handle.audit_call(&origin, name, name, &Value::Object(args.clone()), &env, started.elapsed());
                return ToolOutcome::Result(tool_result(name, name, &env));
            }
        },
    };
    // `audited`: whether the executor already appended the call's activity entry (11 §11: exactly one per call).
    let (command, env, audited) = match plan {
        Plan::Unavailable { milestone, reason } => {
            let env = envelope_error(
                "command.notImplemented",
                format!("the `{name}` tool is not available yet: {reason}"),
                json!({ "tool": name, "milestone": milestone }),
                Some("use the structured commands (run_command) instead"),
            );
            (name.to_owned(), env, false)
        }
        Plan::Command { id, params, dry_run, expect_rev, confirm_token, confirm, request_id } => {
            let mut expect = std::collections::BTreeMap::new();
            for (k, v) in expect_rev {
                match v.as_u64() {
                    Some(n) => {
                        expect.insert(k, n);
                    }
                    None => {
                        let env = envelope_error(
                            "arg.invalid",
                            format!("expectRev of `{k}` is not a revision"),
                            json!({ "pointer": format!("/expectRev/{k}") }),
                            None,
                        );
                        shared.handle.audit_call(&origin, name, &id, &params, &env, started.elapsed());
                        return ToolOutcome::Result(tool_result(name, &id, &env));
                    }
                }
            }
            if let Some(env) = job_limit(shared, reg, &origin, &id) {
                shared.handle.audit_call(&origin, name, &id, &params, &env, started.elapsed());
                return ToolOutcome::Result(tool_result(name, &id, &env));
            }
            let options = InvokeOptions { dry_run, expect_rev: expect, request_id, confirm_token, confirm };
            let inv = Invocation { command: id.clone(), params, options, origin: origin.clone() };
            let mut env = shared.handle.invoke_audited(inv.clone(), name);
            if name == "get_state" {
                env = with_agent_state(shared, env, args);
            }
            if env.status == Status::NeedsConfirmation
                && let Some(decided) = elicit(shared, &env)
            {
                env = if decided {
                    let token = env
                        .result
                        .as_ref()
                        .and_then(|r| r.get("confirmToken"))
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let mut again = inv;
                    again.options.confirm_token = token;
                    again.options.request_id = None;
                    shared.handle.invoke_audited(again, name)
                } else {
                    envelope_error(
                        "confirmation.declined",
                        "the user declined the action".to_owned(),
                        json!({ "commandId": id }),
                        None,
                    )
                };
            }
            (id, env, true)
        }
        Plan::Server => match name {
            "list_commands" => ("list_commands".to_owned(), list_commands(shared, &origin, args), false),
            "describe_command" => {
                let cid = args.get("id").and_then(Value::as_str).unwrap_or_default();
                let env = match shared.handle.describe_command(cid) {
                    Some(d) => Envelope::ok(d, None),
                    None => envelope_error(
                        "command.unknown",
                        format!("no command `{cid}`"),
                        json!({ "id": cid }),
                        Some("list_commands lists the ids"),
                    ),
                };
                ("describe_command".to_owned(), env, false)
            }
            "get_docs" => {
                let topic = args.get("topic").and_then(Value::as_str).unwrap_or("agent-guide");
                let env = match docs::topic(reg, topic) {
                    Some(md) => Envelope::ok(json!({ "topic": topic, "markdown": md }), None),
                    None => envelope_error(
                        "arg.invalid",
                        format!("no documentation topic `{topic}`"),
                        json!({ "pointer": "/topic", "allowed": docs::TOPICS.iter().map(|s| (*s).to_owned()).chain(docs::namespaces(reg)).collect::<Vec<_>>() }),
                        None,
                    ),
                };
                ("get_docs".to_owned(), env, false)
            }
            "batch" => {
                let req: Result<BatchRequest, _> = serde_json::from_value(Value::Object(args.clone()));
                let env = match req {
                    Err(e) => envelope_error("arg.invalid", format!("not a batch: {e}"), json!({}), None),
                    Ok(req) => {
                        let max = shared.status().map_or(1000, |s| s.limits.max_batch_steps);
                        if req.steps.len() > max as usize {
                            envelope_error(
                                "limit.exceeded",
                                format!("a batch of this session has at most {max} steps"),
                                json!({ "limit": "maxBatchSteps", "value": max, "actual": req.steps.len() }),
                                None,
                            )
                        } else {
                            return ToolOutcome::Result(tool_result(
                                name,
                                "batch",
                                &shared.handle.batch_audited(req, origin, name),
                            ));
                        }
                    }
                };
                ("batch".to_owned(), env, false)
            }
            "job_wait" => match job_wait(shared, rid, &origin, args, progress) {
                Some(env) => ("job.wait".to_owned(), env, false),
                None => return ToolOutcome::Cancelled,
            },
            _ => return ToolOutcome::UnknownTool,
        },
    };
    if !audited {
        shared.handle.audit_call(&origin, name, &command, &Value::Object(args.clone()), &env, started.elapsed());
    }
    mark(shared, "app://activity");
    ToolOutcome::Result(tool_result(name, &command, &env))
}

/// `limit.exceeded` when a job command would exceed the session's concurrent jobs (11 §6.5).
fn job_limit(shared: &Shared, reg: &tt_registry::Registry, origin: &Origin, id: &str) -> Option<Envelope> {
    if reg.get(id).map(|e| e.spec.kind) != Some(tt_registry::Kind::Job) {
        return None;
    }
    let max = shared.status().map_or(2, |s| s.limits.max_jobs);
    let listed = shared.handle.invoke(Invocation {
        command: "job.list".into(),
        params: json!({}),
        options: InvokeOptions::default(),
        origin: origin.clone(),
    });
    let running = listed.result.as_ref().and_then(Value::as_array).map_or(0, |a| {
        a.iter().filter(|j| matches!(j.get("state").and_then(Value::as_str), Some("queued" | "running"))).count()
    });
    (running >= max as usize).then(|| {
        envelope_error(
            "limit.exceeded",
            format!("this session already runs {running} job(s); the limit is {max}"),
            json!({ "limit": "maxJobs", "value": max }),
            Some("wait for a job with job_wait, or cancel one"),
        )
    })
}

/// `get_state` with the session (11 §7.1: "session scopes and policy") and the `include` projection.
fn with_agent_state(shared: &Shared, mut env: Envelope, args: &Map<String, Value>) -> Envelope {
    let Some(Value::Object(m)) = env.result.as_mut() else { return env };
    let session = lock(&shared.st).session.clone();
    if let (Some(s), Some(st)) = (session, shared.status()) {
        m.insert(
            "agent".into(),
            json!({
                "sessionId": s.session_id, "client": s.client, "policy": st.policy.as_str(),
                "scopes": st.scopes.iter().map(|x| x.as_str()).collect::<Vec<_>>(), "paused": st.paused, "limits": st.limits,
            }),
        );
    }
    if let Some(include) = args.get("include").and_then(Value::as_array) {
        let keys: BTreeSet<&str> = include
            .iter()
            .filter_map(Value::as_str)
            .flat_map(|i| match i {
                "project" => &["project"][..],
                "scene" => &["scene"][..],
                "frame" => &["frame", "element", "drawing"][..],
                "selection" => &["selection"][..],
                "tool" => &["activeTool"][..],
                "views" => &["activeView", "focusedPanel", "workspace", "mode", "fullScreen"][..],
                "playback" => &["playback"][..],
                "jobs" => &["jobs"][..],
                "ui" => &["modal", "startScreenVisible", "focusedPanel", "workspace", "fullScreen"][..],
                "agent" => &["agent", "agentSessions"][..],
                _ => &[][..],
            })
            .copied()
            .collect();
        m.retain(|k, _| keys.contains(k.as_str()));
    }
    env
}

/// Asks the user through the client (MCP elicitation) when the client offers it (11 §6.3 rule 1). `None`: no
/// elicitation (the agent gets `needs_confirmation`).
fn elicit(shared: &Shared, env: &Envelope) -> Option<bool> {
    if !shared.client_can("elicitation") {
        return None;
    }
    let r = env.result.as_ref()?;
    let summary = r.get("summary").and_then(Value::as_str).unwrap_or("Confirm the action");
    let consequences = r.get("consequences").cloned().unwrap_or_else(|| json!([]));
    let message = format!("{summary}\n\nConsequences: {consequences}");
    let answer = shared.ask(
        "elicitation/create",
        json!({
            "message": message,
            "requestedSchema": {
                "type": "object",
                "properties": { "confirm": { "type": "boolean", "title": "Proceed", "description": summary } },
                "required": ["confirm"],
            },
        }),
        ELICIT_TIMEOUT,
    )?;
    let v = answer.ok()?;
    match v.get("action").and_then(Value::as_str) {
        Some("accept") => Some(v.pointer("/content/confirm").and_then(Value::as_bool).unwrap_or(true)),
        Some("decline" | "cancel") => Some(false),
        _ => None,
    }
}

fn list_commands(shared: &Shared, origin: &Origin, args: &Map<String, Value>) -> Envelope {
    let listed = match shared.handle.list_commands(origin.clone()) {
        Ok(l) => l,
        Err(e) => return Envelope::error(e, None),
    };
    let ns = args.get("namespace").and_then(Value::as_str);
    let kind = args.get("kind").and_then(Value::as_str);
    let enabled_only = args.get("enabledOnly").and_then(Value::as_bool).unwrap_or(false);
    let filter = args.get("filter").and_then(Value::as_str);
    let with_schema = args.get("includeSchema").and_then(Value::as_bool).unwrap_or(false);
    let mut picked: Vec<Value> = listed
        .into_iter()
        .filter(|c| {
            let id = c.get("id").and_then(Value::as_str).unwrap_or_default();
            ns.is_none_or(|n| id.split('.').next() == Some(n))
                && kind.is_none_or(|k| c.get("kind").and_then(Value::as_str) == Some(k))
                && (!enabled_only || c.get("enabled").and_then(Value::as_bool) == Some(true))
                && filter.is_none_or(|f| matches_filter(id, f))
        })
        .collect();
    let offset: usize = args
        .get("page")
        .and_then(|p| p.get("cursor"))
        .and_then(Value::as_str)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let limit = args
        .get("page")
        .and_then(|p| p.get("limit"))
        .and_then(Value::as_u64)
        .map_or(500, |l| usize::try_from(l).unwrap_or(500));
    let total = picked.len();
    let page: Vec<Value> = picked
        .drain(..)
        .skip(offset)
        .take(limit)
        .map(|mut c| {
            if with_schema
                && let Some(d) = c.get("id").and_then(Value::as_str).and_then(|id| shared.handle.describe_command(id))
                && let Some(m) = c.as_object_mut()
            {
                m.insert("paramsSchema".into(), d.get("paramsSchema").cloned().unwrap_or(Value::Null));
                m.insert("resultSchema".into(), d.get("resultSchema").cloned().unwrap_or(Value::Null));
            }
            c
        })
        .collect();
    let next = (offset + page.len() < total).then(|| (offset + page.len()).to_string());
    Envelope::ok(json!({ "commands": page, "total": total, "nextCursor": next }), None)
}

/// A substring, or a glob with `*` and `?`.
fn matches_filter(id: &str, f: &str) -> bool {
    if !f.contains(['*', '?']) {
        return id.contains(f);
    }
    fn glob(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => glob(&p[1..], s) || (!s.is_empty() && glob(p, &s[1..])),
            (Some(b'?'), Some(_)) => glob(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a == b => glob(&p[1..], &s[1..]),
            _ => false,
        }
    }
    glob(f.as_bytes(), id.as_bytes())
}

/// `job_wait`: waits in short slices (so cancellation and progress work), reporting progress to the client.
fn job_wait(
    shared: &Shared,
    rid: &Value,
    origin: &Origin,
    args: &Map<String, Value>,
    progress: Option<Value>,
) -> Option<Envelope> {
    let job = args.get("jobId").and_then(Value::as_str).unwrap_or_default().to_owned();
    let total = Duration::from_millis(args.get("timeoutMs").and_then(Value::as_u64).unwrap_or(60_000));
    if let Some(t) = &progress {
        lock(&shared.st).progress.insert(job.clone(), t.clone());
    }
    let deadline = Instant::now() + total;
    let mut last;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let slice = left.min(Duration::from_millis(250));
        last = shared.handle.invoke(Invocation {
            command: "job.wait".into(),
            params: json!({ "jobId": job, "timeoutMs": u64::try_from(slice.as_millis()).unwrap_or(250) }),
            options: InvokeOptions::default(),
            origin: origin.clone(),
        });
        let timed_out = last.result.as_ref().and_then(|r| r.get("timedOut")).and_then(Value::as_bool).unwrap_or(false);
        if last.status != Status::Ok || !timed_out || left.is_zero() {
            break;
        }
        if shared.is_cancelled(rid) {
            lock(&shared.st).progress.remove(&job);
            return None;
        }
    }
    lock(&shared.st).progress.remove(&job);
    Some(last)
}

fn read_resource(shared: &Shared, id: &Value, uri: &str) -> Value {
    let Ok(reg) = tt_executor::registry() else { return jsonrpc::error(id, jsonrpc::INTERNAL_ERROR, "registry", None) };
    let Some(read) = resources::resolve(uri) else {
        return jsonrpc::error(id, jsonrpc::RESOURCE_NOT_FOUND, "resource not found", Some(json!({ "uri": uri })));
    };
    let Some(origin) = shared.current_origin() else {
        return jsonrpc::error(id, jsonrpc::INVALID_REQUEST, "not initialized", None);
    };
    let text = |mime: &str, t: String| {
        jsonrpc::result(id, json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": t }] }))
    };
    match read {
        resources::Read::Command(cmd, params) => {
            let env = shared.handle.invoke(Invocation {
                command: cmd.into(),
                params,
                options: InvokeOptions::default(),
                origin,
            });
            match (env.status, env.result, env.error) {
                (Status::Ok, Some(r), _) => {
                    text("application/json", serde_json::to_string_pretty(&r).unwrap_or_default())
                }
                (_, _, Some(e)) => {
                    let code =
                        if e.code == "ref.notFound" { jsonrpc::RESOURCE_NOT_FOUND } else { jsonrpc::INTERNAL_ERROR };
                    jsonrpc::error(id, code, &e.message.clone(), Some(serde_json::to_value(&e).unwrap_or(Value::Null)))
                }
                _ => jsonrpc::error(id, jsonrpc::INTERNAL_ERROR, "no result", None),
            }
        }
        resources::Read::Describe(cid) => match shared.handle.describe_command(&cid) {
            Some(d) => text("application/json", serde_json::to_string_pretty(&d).unwrap_or_default()),
            None => jsonrpc::error(id, jsonrpc::RESOURCE_NOT_FOUND, "no such command", Some(json!({ "uri": uri }))),
        },
        resources::Read::Docs(topic) => match docs::topic(reg, &topic) {
            Some(md) => text("text/markdown", md),
            None => jsonrpc::error(id, jsonrpc::RESOURCE_NOT_FOUND, "no such topic", Some(json!({ "uri": uri }))),
        },
        resources::Read::Events(after) => {
            let st = lock(&shared.st);
            let events: Vec<&Value> =
                st.events.iter().filter(|e| e.get("seq").and_then(Value::as_u64).unwrap_or(0) > after).collect();
            let body = json!({ "events": events, "lastSeq": st.next_seq, "oldestSeq": st.events.front().and_then(|e| e.get("seq")).cloned() });
            drop(st);
            text("application/json", serde_json::to_string_pretty(&body).unwrap_or_default())
        }
        resources::Read::Activity => {
            let session = shared.session_id().unwrap_or_default();
            let env = shared.handle.invoke(Invocation {
                command: "agent.activity.list".into(),
                params: json!({ "sessionId": session, "limit": 10_000 }),
                options: InvokeOptions::default(),
                origin,
            });
            let mut t = String::new();
            for e in env.result.as_ref().and_then(Value::as_array).into_iter().flatten() {
                t.push_str(&e.to_string());
                t.push('\n');
            }
            text("application/jsonl", t)
        }
    }
}

/// Argument completion (11 §4): element and scene names, ids by type, command ids, object types, doc topics.
fn complete(shared: &Shared, reg: &tt_registry::Registry, params: &Value) -> Value {
    let arg = params.pointer("/argument/name").and_then(Value::as_str).unwrap_or_default();
    let value = params.pointer("/argument/value").and_then(Value::as_str).unwrap_or_default();
    let names = |ty: &str, by_id: bool| -> Vec<String> {
        let Some(origin) = shared.current_origin() else { return Vec::new() };
        let env = shared.handle.invoke(Invocation {
            command: "doc.object.find".into(),
            params: json!({ "type": ty, "limit": 1000 }),
            options: InvokeOptions::default(),
            origin,
        });
        env.result
            .as_ref()
            .and_then(|r| r.get("items"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|i| i.get(if by_id { "id" } else { "name" }).and_then(Value::as_str).map(str::to_owned))
            .collect()
    };
    let lower = arg.to_ascii_lowercase();
    let mut values: Vec<String> = match lower.as_str() {
        "commandid" => reg.entries().map(|e| e.id().to_owned()).collect(),
        "type" => tt_model::ObjectType::ALL.iter().map(|t| t.as_str().to_owned()).collect(),
        "namespace" | "topic" => docs::TOPICS.iter().map(|s| (*s).to_owned()).chain(docs::namespaces(reg)).collect(),
        "sceneid" => names("scene", true),
        "drawingid" => names("drawing", true),
        "paletteid" => names("palette", true),
        "libraryid" => vec!["library:local".to_owned()],
        "id" => ["scene", "element", "palette", "drawingBank"].iter().flat_map(|t| names(t, true)).collect(),
        a if a.contains("element") => names("element", false),
        a if a.contains("scene") => names("scene", false),
        _ => Vec::new(),
    };
    values.retain(|v| v.starts_with(value));
    values.sort();
    values.dedup();
    let total = values.len();
    values.truncate(100);
    json!({ "values": values, "total": total, "hasMore": total > 100 })
}

fn mark(shared: &Shared, uri: &str) {
    let mut st = lock(&shared.st);
    let subscribed: Vec<String> =
        st.subscriptions.iter().filter(|s| s.split('?').next() == Some(uri)).cloned().collect();
    st.dirty.extend(subscribed);
}

/// The event pump: event buffer, coalesced resource updates, log notifications, progress, session polling.
fn pump_events(shared: &Shared, rx: &mpsc::Receiver<tt_executor::Event>) {
    let mut last_poll = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(ev) => on_event(shared, &ev),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        flush_updates(shared);
        if last_poll.elapsed() >= Duration::from_millis(250) {
            last_poll = Instant::now();
            if lock(&shared.st).session.is_some() {
                match shared.status() {
                    Some(st) if !st.connected => {
                        shared.send(&jsonrpc::notification(
                            "notifications/message",
                            json!({ "level": "warning", "logger": "agent", "data": { "message": "the user disconnected this agent session" } }),
                        ));
                        shared.closing.store(true, Ordering::Relaxed);
                    }
                    Some(st) => {
                        let changed = {
                            let mut s = lock(&shared.st);
                            let c = s.typed_version != st.typed_tools_version;
                            s.typed_version = st.typed_tools_version;
                            c
                        };
                        if changed {
                            shared.send(&jsonrpc::notification("notifications/tools/list_changed", json!({})));
                        }
                    }
                    None => {}
                }
            }
        }
        if shared.closing.load(Ordering::Relaxed) {
            break;
        }
    }
}

fn on_event(shared: &Shared, ev: &tt_executor::Event) {
    let origin = ev
        .payload
        .get("origin")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| (ev.payload.get("by").and_then(Value::as_str) == Some("user")).then(|| "user".to_owned()))
        .unwrap_or_else(|| "system".to_owned());
    let changed: Vec<String> = ev
        .payload
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect();
    let (record, log, progress) = {
        let mut st = lock(&shared.st);
        st.next_seq += 1;
        let record = json!({ "seq": st.next_seq, "time": tt_persist::time::now_iso(), "type": ev.name, "origin": origin, "payload": ev.payload });
        st.events.push_back(record.clone());
        while st.events.len() > EVENT_BUFFER {
            st.events.pop_front();
        }
        let dirty: Vec<String> =
            st.subscriptions.iter().filter(|u| resources::affected(u, &ev.name, &changed)).cloned().collect();
        st.dirty.extend(dirty);
        let log = st.log_level.is_some_and(|l| l <= 1);
        let progress = if ev.name == "job.progress" || ev.name == "job.finished" {
            ev.payload.get("jobId").and_then(Value::as_str).and_then(|j| st.progress.get(j).cloned())
        } else {
            None
        };
        (record, log, progress)
    };
    if log {
        shared.send(&jsonrpc::notification(
            "notifications/message",
            json!({ "level": "info", "logger": "events", "data": record }),
        ));
    }
    if let Some(token) = progress {
        let p = ev.payload.get("progress").and_then(Value::as_f64).unwrap_or(if ev.name == "job.finished" {
            1.0
        } else {
            0.0
        });
        let mut n = json!({ "progressToken": token, "progress": (p * 1000.0).round(), "total": 1000 });
        if let (Some(m), Some(msg)) = (n.as_object_mut(), ev.payload.get("message").and_then(Value::as_str)) {
            m.insert("message".into(), json!(msg));
        }
        shared.send(&jsonrpc::notification("notifications/progress", n));
    }
}

fn flush_updates(shared: &Shared) {
    let due: Vec<String> = {
        let mut st = lock(&shared.st);
        let now = Instant::now();
        let due: Vec<String> = st
            .dirty
            .iter()
            .filter(|u| st.last_sent.get(*u).is_none_or(|t| now.duration_since(*t) >= COALESCE))
            .cloned()
            .collect();
        for u in &due {
            st.dirty.remove(u);
            st.last_sent.insert(u.clone(), now);
        }
        due
    };
    for uri in due {
        shared.send(&jsonrpc::notification("notifications/resources/updated", json!({ "uri": uri })));
    }
}
