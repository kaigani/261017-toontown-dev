# ADR-0015 — The MCP server: one agent surface over the command layer

Status: accepted (M1 C1.4, 2026-10-29). Scope: plan §4 row M1, checkpoint C1.4 (generic tools, resources,
subscriptions, events; permissions and scopes, the confirmation protocol, the audit log; the headless binary,
batch-script mode, docs resources) and the C1.4 obligations of `reviews/M1-C1.3-r3.md`.

## Context

Spec 11 makes MCP the agent interface: tools with input and output schemas, resources with templates and
subscriptions, prompts, logging, completions, progress and cancellation (§4); sessions with policies, scopes and roots
(§6.1–§6.2); a confirmation protocol (§6.3); pause and limits (§6.4–§6.5); generic tools (§7); events (§9); errors
(§10); an audit log (§11); prompts (§12); conformance (§14). Plan §3.2 requires one command layer: the GUI and agents
call the same registered commands. The executor (ADR-0013) already serializes every mutation and owns undo,
transactions, idempotent requests and the never-block watchdog.

## Decision

1. **Every tool is the command layer.** The 28 generic tools of 11 §7 map onto registered commands (`get_object` →
   `doc.object.get`, `set_properties` → `doc.object.patch`, `undo` → `edit.undo`, `checkpoint` → `checkpoint.*`,
   `render_frame` → `render.frame.run`, …). `run_command` and `batch` reach every command. The tools the server
   answers itself (`list_commands`, `describe_command`, `get_docs`, `job_wait`) read the registry or call `job.wait`.
   Nothing bespoke mutates the document. `pointer`, `key` and `type_text` need the drawing tools (M2 C2.3) and the
   desktop keymap and UI tree (M4 C4.2). Until then they answer `command.notImplemented` with that milestone.
   - Every tool's `structuredContent` is the command envelope (11 §7.2), and its `outputSchema` is the envelope
     schema. The text content names the outcome, and for a confirmation the token.
   - Typed tools (`agent.typedTools.set`) expose commands as tools named by replacing `.` with `_`, with the
     command's own parameter schema. A change sends `notifications/tools/list_changed`.
2. **Sessions live in the executor** (`tt_executor::agents`), so `agent.*` are ordinary registry commands that the GUI
   will also call. A session has a client name, a policy (observe, assist, autonomous), scopes (never `app`), roots,
   pre-authorised confirmation categories and limits. The executor replaces an agent origin's scopes with the hub's
   grant on every call, so a grant or pause takes effect at once.
3. **Confirmations** (11 §6.3) are raised by handlers through `Cx::confirm(category, summary, consequences,
   binding)`. When the session's policy confirms the category, the call changes nothing and returns
   `needs_confirmation` with a `confirmToken` (`cf_` + random), a summary and the consequences.
   - The token is single-use and lives five minutes. It is bound to the session, the command, a fingerprint of the
     parameters, the `documentRev` and an optional binding (a file's identity, a lock).
   - Re-invoking the same call with the token proceeds once. `confirm: true` proceeds only for categories
     pre-authorised at launch (`--preauthorize`). A dry run never asks.
   - Clients that declare `elicitation` are asked directly (`elicitation/create` with a boolean schema). Accept
     replays the call with the token; decline answers `confirmation.declined`.
   - Categories: overwrite and destructive (asked under observe and assist), and discard, network, print and
     externalChange (asked under every policy).
   - Inside an atomic batch a confirmation fails the batch (`confirmation.required`); the step can then be run with
     its token through `run_command`.
4. **Audit log** (11 §11): exactly one activity entry per tool call and one per batch, appended by the executor
   whatever the outcome. Each entry records:
   - `seq`, time, session and client, tool, command, redacted parameters (data URLs, base64 and strings over 4 KiB
     become `sha256:`), status and error code;
   - the undo step, the changed count, `documentRev` and the duration.

   Resource reads are not audited. `agent.activity.persist.set` appends the entries to `agent-activity.jsonl` beside
   the project. A committed fixture (`crates/mcp/tests/fixtures/audit-golden.jsonl`) reproduces the entries of a fixed
   script once times, durations, sessions, generated ids and temporary paths are normalised.
5. **Protocol implementation.** The server in `tt-mcp` is a hand-written JSON-RPC 2.0 server over newline-delimited
   stdio, not the `rmcp` SDK:
   - the executor is synchronous;
   - the needed subset is small and fully specified (revisions 2025-06-18 and 2025-11-25);
   - it adds no async runtime to the headless binary, whose linked libraries are policed (ADR-0005,
     `scripts/check-headless-deps.py`);
   - conformance is proven against a real client: the official Python SDK (`scripts/mcp-conformance.py`, in CI on
     every OS and with no display server) and an in-process scripted client over OS pipes
     (`crates/mcp/tests/protocol.rs`).
6. **Concurrency.** A reader thread parses lines, and requests run in arrival order on one call thread, so pipelined
   calls apply in the order sent and the audit log is deterministic.
   - `job_wait` waits on its own thread, in 250 ms slices, sending progress for its token and a final notification
     when it ends.
   - `notifications/cancelled` drops a queued or waiting request without a response. A cancelled call never cancels
     document-thread work (11 P5).
   - An event pump keeps the numbered event buffer of `app://events` (10,000 events), sends
     `notifications/resources/updated` at most once per resource per 100 ms and forwards events as log messages at
     `info`. It also polls the session (disconnected by the user, typed tools changed).
   - `initialize` and `ping` are answered inline.
7. **Resources**: `app://state`, `app://project`, `app://events?after=`, `app://activity` and the docs topics, plus
   ten templates (objects, the exposure sheet, drawings and their PNGs, frame PNGs, palettes, libraries, command
   declarations, schemas, namespace docs). Each read is a registry command (the render ones arrive with M3).
   - The `initialize` instructions are exactly `app://docs/agent-guide`.
   - Prompts: the ten workflow templates of 11 §12; a test checks that every command they name is registered.
   - Completions: command ids, object types, doc topics, and scene, drawing, palette and element ids or names.
8. **Pause and limits.** A paused session's calls wait (10 s by default), then fail with `agent.paused`; a
   disconnected session ends the server.
   - `maxBatchSteps` and `maxJobs` answer `limit.exceeded`.
   - A message larger than `maxInlineBytes` is refused.
9. **Headless binary** (11 §3).
   - `toontown-headless --mcp-stdio` serves one client on stdin and stdout; diagnostics go to stderr.
   - `--run <batch.json>` replays a `batch` input as an agent session named `batch-script`. It prints the batch
     envelope and writes a JSON-lines log (`start`, one `step` per step, `end`) to `--out`. The exit status is 0 when
     every step succeeded and 5 otherwise; 2 means the file is not a batch.
   - Agent options: `--policy`, `--grant`, `--allow-path` (roots), `--preauthorize`, `--typed-tools` and `--user-dir`
     (default: the platform's user folder). `--project` opens a project first and waits for the open to finish.
10. **Deferred to M4** (declared, not narrowed):
    - Streamable HTTP and attached mode (`--mcp-http`, the discovery file's endpoint, several clients on one
      instance), with the desktop shell;
    - the agent indicator and in-app confirmations (`askInApp`) need that shell. `agent.server.set` stores the
      preference and warns `agent.httpUnavailable`.

## Consequences

- A command added to the registry is reachable by agents at once (through `run_command`, the docs and the typed
  tools), with its scopes, confirmations and audit. No tool needs its own code.
- The same confirmation and audit machinery serves the GUI's agent controls in M4.
- The server speaks 2025-06-18 and 2025-11-25. The SDK's newer revision (2026-07-28) replaces resource subscriptions
  and deprecates the logging capability. Following it is an open question for the spec owner (SI-049), because
  11 §4 requires both.
