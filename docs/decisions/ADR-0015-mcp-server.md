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

11. **Changes from review round 1** (`reviews/M1-C1.4-r1.md`; response `reviews/M1-C1.4-r1-response.md`). These
    replace the corresponding parts of decisions 1–9.
    - **Wrapper input is validated before conversion** (C14-R1-01). `arguments` must be an object (`-32602`
      otherwise). A generic tool's complete input (options, projections, pagination, types, unknown fields) is
      validated against its published input schema, and violations are envelopes with `arg.*` codes. Nothing reaches
      a command with a safety option dropped.
    - **Elicitation answers are strict** (C14-R1-02). Only `action: "accept"` with the boolean `content.confirm: true`
      authorises. Decline, cancel, a missing or non-boolean `confirm`, an error answer or anything else declines.
      Closing the session, or no answer at all, leaves `needs_confirmation`.
    - **One logical call** (C14-R1-04, -12). An answered elicitation re-invokes the same call with its token and its
      `requestId`; a decline first marks the token declined (`agent.confirmation.respond` as the user). The executor
      settles a remembered `needs_confirmation` when a retry carries its token, so the requestId remembers the final
      outcome without a second transaction. The server audits the logical call once, with its final status and whole
      duration.
    - **Session limits at command admission** (C14-R1-11). `maxBatchSteps` and `maxJobs` are checked by the executor
      for every agent submission (invocations, batches, typed tools, batch scripts), not by the server's wrappers.
      Saved policy defaults and limits are restored when an executor starts (C14-R1-06).
    - **File effects are staged** (C14-R1-03). `agent.activity.export` defers its write (`Effect::WriteFile`). It is
      staged beside the target only when every step succeeded, moved into place after the commit, and discarded on
      rollback.
    - **Replayable export** (C14-R1-15, SI-051). `format: "batch"` contains committed edits only, in order. A
      committed batch contributes its steps, with `$<n>` references renumbered and clashing `as` names renamed. Dry
      runs, queries, discovery tools, incomplete calls and redacted parameters are left out, and the result lists
      them under `omitted` with the reason. Activity entries record `dryRun`.
    - **Transport** (C14-R1-13, -14). Frames are cut at `maxInlineBytes` without allocating more; an over-long frame
      is skipped to its newline and answered with an error. Invalid UTF-8 is a parse error of its frame only. One
      writer thread owns the output behind a bounded queue (4096 messages). A write or flush failure, or a queue that
      stays full for 10 s, closes the session. Nothing writes under the state lock: progress for a `job_wait` token is
      queued under the lock that registers the token, and the final notification and the response are queued after
      it is unregistered. Closing (input EOF, output failure, user disconnect) fails pending requests to the client,
      skips queued requests, waits a bounded time for the running one, and never waits for a blocked writer.
    - **Lifecycle** (C14-R1-17). `initialize` happens once (a second one is `-32600`). Cancellation applies only to
      live requests (admitted, not yet answered; at most 64), and an unknown or finished id is ignored. A duplicate
      id while the first is in progress is refused.
    - **Server-owned work** (C14-R1-05). `list_commands`, `describe_command` and `get_docs` run under a 5 s watchdog
      on their own thread (`command.timeout` when it passes), so the call thread is never held. The glob matcher is
      iterative and quadratic at worst. Work inside the executor keeps the executor's never-block budget; restarting
      a document thread that never returns is still M4 (gap 3 of the handoff).
    - **One state shape** (C14-R1-16). `app.state.get` reports the calling agent session (`agent`: session, client,
      policy, scopes, paused, limits). `get_state` returns the command's data unchanged and narrows it only when
      `include` is given. `edit.undo` and `edit.redo` emit `doc.undone` / `doc.redone {undoStepId, label}`
      (11 §9.2).
    - **Release graph** (C14-R1-10). `test-hooks` is enabled only on `tt-executor`'s dev-dependency;
      `scripts/check-release-features.py` proves the product graphs (`tt-headless`, `tt-desktop`) do not contain it.
    - **The SDK used for conformance is pinned** (`mcp==2.0.0`, SI-049 adjudication).

12. **Changes from review round 2** (`reviews/M1-C1.4-r2.md`; response `reviews/M1-C1.4-r2-response.md`).
    - **Every file a command produces is a staged effect** (C14-R2-05). `agent.token.regenerate`, `doc.export.json`
      with a `path`, `doc.asset.export` and `agent.activity.export` defer their write (`Effect::WriteFile`, with
      `private` for the owner-only discovery file). Each file is staged beside its target only when every step
      succeeded, moved into place after the commit, and discarded on rollback. Handlers write nothing themselves;
      the remaining direct writes are the session's internal cache and the post-call activity log.
    - **The contract when the transaction committed but its file could not be published** (C14-R2-01). The call is
      an error, `fs.ioError` with `details.reason: "publish"`, `details.path` and `details.committed`.
      - The envelope keeps the commit's `changes`, `undo` and `documentRev`. The document change stays (undoable),
        so nothing is silently rolled back after other effects applied.
      - The target keeps what it had and the stage is removed.
      - The activity entry records the error.
    - **Replay is the committed edits, all of them, in order** (C14-R2-02, -03; SI-051).
      - Activity entries of batches record `committedSteps`: all or none for an atomic batch, the successful steps of
        a sequential batch. Replay keeps exactly those steps, with `$<n>` references remapped.
      - A call answered from the `requestId` cache is recorded once, as the client's call, with `cachedRetry: true`.
        It executed nothing, so replay leaves it out (reason `cachedRetry`).
      - The executor reports cache answers through `ExecutorHandle::invoke_served` without changing the envelope, so
        a retry still returns the identical first answer.
    - **Cancelling a request withdraws its elicitation** (C14-R2-04). A pending ask belongs to the request that made
      it. When the client cancels that request, the wait ends at once, the server sends `notifications/cancelled`
      for its own `elicitation/create`, and the request gets no response. The token stays pending until it expires,
      and nothing is authorised.
    - **Every `onUnsaved: "save"` boundary confirms external changes for agents** (C14-R2-07, the SI-047 ruling).
      `project.close`, `app.quit`, `project.open`, `project.create`, `project.recovery.open` and
      `project.backup.open` save through `resolve_unsaved`, which uses the same guard as `project.save`. Agents get
      the bound `externalChange` token of that call; users get `project.changedOnDisk`. The load path's internal
      save, which has no call of its own to confirm, refuses a changed file.

## Consequences

- A command added to the registry is reachable by agents at once (through `run_command`, the docs and the typed
  tools), with its scopes, confirmations and audit. No tool needs its own code.
- The same confirmation and audit machinery serves the GUI's agent controls in M4.
- The server speaks 2025-06-18 and 2025-11-25. The SDK's newer revision (2026-07-28) replaces resource subscriptions
  and deprecates the logging capability. Following it is an open question for the spec owner (SI-049), because
  11 §4 requires both.
