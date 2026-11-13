# ADR-0022 — Attached mode over Streamable HTTP

Status: accepted (M4 C4.1, 2026-11-12). Scope: 11 §3 (attached mode, several clients), §4 (Streamable HTTP, loopback,
Origin validation, bearer token, discovery file, off by default); ADR-0015 §10 (deferred to M4); C1.4 handoff gap 1.

## Decision

1. **Transport on `std::net`.** `tt_mcp::http` is a small HTTP/1.1 server with no async runtime, so the headless
   binary's dependency policy (`scripts/check-headless-deps.py`) is unchanged. One endpoint, `/mcp`.
2. **Security.** Bound to a loopback address only: `tt_mcp::http::start` refuses any other address with the typed
   `StartError::RemoteBind` before binding, and `--mcp-http` refuses one at argument parsing (C41-R1-02); the
   preference path can pass only a port (`agent.server.port`, 1024..65535) on `127.0.0.1`; `agent.server.allowRemote`
   is stored but never used, because the MCP authorization flow remote binding needs is not implemented. Every request
   needs `Authorization: Bearer <token>` (constant-time comparison). `Host` must be an authority (host and optional port;
   no scheme, path, userinfo or whitespace; case-insensitive; `localhost.` with its trailing dot is the same name) naming
   the loopback (`localhost`, `127.0.0.1`, the IPv6 loopback in any spelling); a present `Origin` must be `http` or
   `https` with such an authority and nothing after it; the opaque origin `null` is refused (C41-R1-06).
   - **Strict requests (C41-R1-03, -05).** The head is read first (16 KiB, 10 s from the connection being accepted) and
     parsed strictly: `METHOD SP target SP HTTP/1.1`, CRLF lines only, no folded lines, a colon in every header line,
     `Authorization`, `Host`, `Origin`, `Content-Length` and `Mcp-Session-Id` at most once, no `Transfer-Encoding`,
     `Content-Length` decimal digits only; each refusal is a `400` with no session or tool side effect. The path,
     `Host`, `Origin`, token, method, declared length (`413` above the session's inline payload limit, 50 MiB by
     default, the limit of 11 §6.5 that stdio frames with too) and session are checked before a single body byte is
     read or reserved; the body then arrives incrementally within 60 s of the connection being accepted.
   - **Admission.** 64 connections at once, 16 of them before their token check; over either cap, `503` with
     `Retry-After: 1`, nothing read. An SSE subscriber that stops reading is dropped (1024 queued messages, 10 s write
     timeout). Every answer ends with a bounded lingering close (256 KiB, 1 s) so an early refusal reaches the client.
   - **Credentials fail closed (ruling O-6).** The token and session ids come from the operating system's random source
     only; when it fails, no token is made, the server does not start and the window says why (`agent.serverFailed`).
     A token read from the discovery file must have the generated form (48 lower-case hex characters) or a new one is
     made: a planted file with another token is not trusted (ruling O-8).
   - **Sessions (ruling O-7).** A session with no open stream and no request in flight expires after 30 minutes; its
     id then answers `404` and the client initialises again. The session cap check and its insertion are one critical
     section. A holder of the installation token can use any session id it learns: the token already grants every
     session's powers to its holder (one user, one installation), so a session id is a routing key, not a credential.
3. **Protocol.** `POST` carries one JSON-RPC message. A request is answered with an SSE stream that carries the
   messages belonging to it (progress for its token, the server's own requests such as `elicitation/create`) and ends
   with its response; notifications and responses are accepted with `202`. `GET` opens the session's stream for
   messages that belong to no request (resource updates, logs, tool-list changes). `DELETE` ends the session.
   `Mcp-Session-Id` is assigned at `initialize` and required afterwards. Batched messages are refused (removed from
   the protocol in 2025-06-18).
4. **One protocol loop.** Each HTTP session runs `tt_mcp::serve` — the stdio loop — over an in-memory reader (one
   message per line, EOF when the session ends) and a routing writer. Sessions, scopes, confirmations, pause, the audit
   log, request cancellation and typed tools are therefore identical to stdio. Each session is its own agent session
   (`transport: "http"`) over the one executor (11 §3: several clients share the document).
5. **Lifecycle in the desktop host.** Off by default (11 §4). The host starts the server when `agent.server.enabled`
   is true or `--mcp-http <address>` is given, and stops it when the preference is cleared. The documented port is
   47215 (`agent.server.port`); the next free ports are tried. The token is the discovery file's
   (`agent.token.regenerate` creates it); the host writes `<settings>/agent-discovery.json` with the endpoint and token,
   readable only by the user, and also `--mcp-discovery <file>` for tests. On POSIX the file's mode is `0600`; on
   Windows it lives in the per-user settings folder (`%APPDATA%`) and inherits that folder's per-user ACL, with no ACL
   of its own. When the server stops, at a graceful quit, and at start while the server is off, the file is rewritten
   without endpoint and process id, keeping the token (ruling O-8): it never advertises a dead endpoint; a file left by
   a process that was killed is replaced at the next start.
6. **Freshness for `inspect_ui`.** The tool waits on its own thread (2 s bound) until the front end has rendered every
   event broadcast before the call; a tree that did not catch up is marked `stale: true`.

## Consequences

- `scripts/mcp-conformance.py --http` runs the official SDK client against the running desktop application in the
  `desktop gui` CI job.
- Restarting a document thread that never returns (C1.4 gap 3) is the host's; see `reviews/M4-design-engine-restart.md`.
