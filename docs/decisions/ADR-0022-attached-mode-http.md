# ADR-0022 — Attached mode over Streamable HTTP

Status: accepted (M4 C4.1, 2026-11-12). Scope: 11 §3 (attached mode, several clients), §4 (Streamable HTTP, loopback,
Origin validation, bearer token, discovery file, off by default); ADR-0015 §10 (deferred to M4); C1.4 handoff gap 1.

## Decision

1. **Transport on `std::net`.** `tt_mcp::http` is a small HTTP/1.1 server with no async runtime, so the headless
   binary's dependency policy (`scripts/check-headless-deps.py`) is unchanged. One endpoint, `/mcp`.
2. **Security.** Bound to `127.0.0.1` by default; every request needs `Authorization: Bearer <token>` (constant-time
   comparison); a present `Origin` must be a loopback origin and `Host` must name the loopback address (DNS-rebinding
   protection). Remote binding stays off (`agent.server.allowRemote` is stored; the MCP authorization flow it needs is
   not implemented, so it is refused).
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
   readable only by the user, and also `--mcp-discovery <file>` for tests.
6. **Freshness for `inspect_ui`.** The tool waits on its own thread (2 s bound) until the front end has rendered every
   event broadcast before the call; a tree that did not catch up is marked `stale: true`.

## Consequences

- `scripts/mcp-conformance.py --http` runs the official SDK client against the running desktop application in the
  `desktop gui` CI job.
- Restarting a document thread that never returns (C1.4 gap 3) is the host's; see `reviews/M4-design-engine-restart.md`.
