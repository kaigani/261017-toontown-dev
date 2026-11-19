# M4 C4.4a design note 2: agent surfaces

Status: before code (C4.4a). Plan `reviews/M4-C4.4-plan.md` §4.6, §4.7, §6 item 2, §9 (ruling D-40 answers 5 and 6).
Covers the Preferences dialog's Agent access category, the Activity panel (its model and live updates), the confirmation
sheet and how it sits with elicitation, the agent indicator, the activity-persistence preference, the load report window
and the `clipboard` GUI-only reason. The conventions of design note 1 §1 apply (`reviews/M4-design-panels.md`): one
command layer, declared references, disabled with the reason, agent changes reflected, keyboard reach, the macOS runner.

Two findings here need a ruling before code. They are marked **[ruling]** and collected in §9.

## 1. What exists and what the kernel lacks

Facts from the code at `93cf2ea9`:

- **Commands** (11 §15.1, all registered): `agent.server.set`, `agent.server.status` (sessions with `paused`, and
  `pendingConfirmations`), `agent.session.grant`, `.pause`, `.resume`, `.disconnect`, `agent.policy.defaults.set`
  (checked against the `agent.policy.defaults` preference's schema), `agent.token.regenerate`, `agent.typedTools.set`,
  `agent.activity.list`, `agent.activity.export`, `agent.activity.persist.set`, `agent.confirmation.respond`; events
  `agent.session.connected`, `agent.session.disconnected`, `agent.confirmation.requested`. `doc.loadReport.get` (10 §7.3)
  and `edit.history.get` and `.move` (position at most 1000) are registered.
- **Front end:** the indicator (`StatusBar.tsx:92-205`) is persistent while a session is connected, shows a distinct
  acting state from the host's `shell.agentActing`, and has a popover with Pause, Resume and Stop per session and the last
  20 activity lines, fetched once when the popover opens. It has no **Activity…** button (11 §6.4). The Preferences
  dialog's Agent category edits the `agent.*` keys through `prefs.set`, and its generic object editor shows the numeric
  members of `agent.policy.defaults` as check boxes (a bug).
- **Kernel gaps**, each an SI and a registry change before the code that needs it (§8):
  1. No event tells a surface that the activity log grew. The plan's "`agent.activity.*` events" do not exist.
  2. `ActivityEntry` (`agents.rs:254-305`) has no batch id and no changed object ids, only `changedCount`. The panel
     can group neither by batch nor highlight what an entry changed.
  3. The `askInApp` member of the policy is stored and read nowhere. `AgentHub::take` (`agents.rs:430-469`) lets a
     token through unless the decision is an explicit decline, so a token the user has not answered yet is accepted.
     When an elicitation-capable client is connected, both the elicitation and an in-app request can be raised for one
     call. Nothing tells the GUI that a pending confirmation was answered elsewhere, used or expired.
  4. `agent.activity.persist.set` and `agent.typedTools.set` write no preference, so both revert at restart. 11 §11
     calls the persistence a preference (default off).
  5. `agent.token.regenerate` does not disconnect the HTTP sessions, as 11 §15.1 says it does. The agent research found
     that its rewrite of the discovery file loses the endpoint; I will check this again before the binding.
  6. `project.opened` carries no issue count. The open job's result has the non-info issues in `warnings`, and the
     report in `doc.loadReport.get`.
  7. No code touches the clipboard yet.

## 2. Preferences › Agent access (01 §7.3 lines 783-789; 11 §6.4, §15.1)

- **One command per setting, never `prefs.set`** (plan §4.6). 01 lines 787-789 also let `prefs.get` and `prefs.set` read
  and write the `agent.*` keys "like any other preference, subject to the restrictions section 11 places". These two
  rules are compatible: the category must use the commands, and `prefs.set` stays available to the user's command layer.
  But `prefs.set {agent.server.enabled: true}` writes the key without starting the server, so two commands set one state
  with two different effects. **Proposal (SI):** `prefs.set` on an `agent.*` key applies through the same path as its
  command: the server starts or stops, and the policy defaults are validated the same way. The refusal for agent callers
  stays as it is (`prefs.rs`: an agent cannot change its own access). A kernel test shows that both routes give the same
  state and the same event.
- **Controls:**

  | Control | Command | Notes |
  |---|---|---|
  | Run the agent server (check box) | `agent.server.set` | The fields below depend on it. |
  | Port (number field) | `agent.server.set` | `{enabled, port}`. |
  | Allow remote connections | `agent.server.set` | `{enabled, allowRemote}`; disabled with the reason until the MCP authorization flow exists. |
  | Default policy (observe, assist, autonomous) | `agent.policy.defaults.set` | |
  | Limits: maxJobs 1-8, maxBatchSteps 1-100000, maxInlineImagePx 256-8192, maxInlineBytes (in MB) | `agent.policy.defaults.set` | Number fields with their ranges, replacing the generic editor. |
  | Ask me in the app when an agent needs confirmation | `agent.policy.defaults.set` | `askInApp`, check box. |
  | Let the agent move my view | `agent.policy.defaults.set` | `letAgentMoveView`, check box. |
  | Highlight agent changes | `agent.policy.defaults.set` | `highlightAgentChanges`, check box. |
  | Expose commands as individual tools; namespaces | `agent.typedTools.set` | A namespace checklist from the registry's namespaces; the result's `toolCount` is shown. |
  | Keep the activity log beside the project | `agent.activity.persist.set` | Toggle. |

  The read-only status line (endpoint, discovery file, number of sessions) comes from `agent.server.status`.
- **Commit:** at OK, like the rest of the dialog. Each changed group sends its one command, in table order. The first
  refusal stops the sequence: the dialog stays open with the refusal under its field (`agent.server.set` with the port
  taken, for example), and the groups not yet sent keep their edits. The commands are `ui` kind with no undo step, so
  the groups already applied stay applied, and the error says so. Cancel sends nothing.
- **Regenerate token…** is an immediate button, outside OK and Cancel, because it acts at once. A confirmation dialog
  ("Connected HTTP clients will be disconnected") comes first. Its Regenerate button sends `agent.token.regenerate`
  after kernel gap 5 is fixed. The dialog's Cancel is `dismiss`.
- **Reflected:** the category re-reads on `prefs.changed` for the `agent.*` keys, and the status line re-reads on
  `agent.session.connected` and `.disconnected`. An agent cannot change these settings (scope `app`), but a second
  window or the command layer can, so the Vitest drives the change through events only.
- **The serving endpoint (ruling D-62 item 1, C44A-R1-40; 11 §15.1).** `agent.server.status` reports the endpoint the
  HTTP server serves on, the discovery file's `endpoint`, however the server was started: the host tells the kernel when
  it starts serving and when it stops (`ExecutorHandle::set_agent_serving`, from `Host::start_http_on` and
  `Host::stop_http`, and again on the new engine after an engine restart). A headless server's stdio session reports
  `stdio`. `enabled` is true while the server serves, while the saved preference has it on, or while a session is
  connected.
  - **The check box shows the saved preference** (`agent.server.enabled`): it is what OK writes, and an explicit
    `--mcp-http` launch serves for that run without changing it (ruling O-21; ADR-0022 decision 5).
  - **The status line says why the server is on**: "The agent server is running at <endpoint> · n sessions", followed by
    "started for this run by the --mcp-http launch option; the saved setting below is off" when the saved preference is
    off, or "on in the saved setting below" when it is on.
- **The token (ruling D-62 item 2, C44A-R1-41; 11 §4 lines 107-108: "shown and regenerable").** A Token field beside
  Regenerate token…, with Show and Copy. **No surface an agent can read ever contains the token:**
  - **Masked by default.** The field shows a fixed run of dots, nothing derived from the token. The page asks the host
    for the token (`agent_token`, a host IPC call of the page, never a registry command) only for a reveal or a Copy,
    and holds it only while it is shown.
  - **Show** reveals it only while the person asks for it: until Hide, leaving the page (another category, the dialog
    closed), the window losing focus, a regenerated token (`shell.agentToken`), or a window capture. Its reason is
    `disclose`.
  - **Copy** writes it to the system clipboard (`copyText`, reason `clipboard`) without showing it.
  - **The window tree** (`inspect_ui`) always reports the field's value as `redacted` (`data-tt-value`), revealed or not.
  - **Window captures: the reveal is cleared before the capture.** The page reports `token_shown {shown: true}` before
    it draws the token and `{shown: false}` once the mask is painted (two frames later). Every capture
    (`inspect.screenshot`, whoever asked) first takes the host's capture guard (`desktop/src-tauri/src/token_reveal.rs`):
    while a capture runs no window may start a reveal (the page is told so and keeps the mask), and the guard is
    granted only when no window shows the token. A window that shows it is asked to hide it (`token-hide`), and the
    capture waits, bounded by the capture's budget and 2 s, for its report; if the report does not come, nothing is
    captured and the call is refused (`render.failed`, `reason: "tokenShown"`).
  - **Logs and the activity export.** The token never passes through the kernel's command layer, so the activity log,
    its export and the event stream cannot contain it; the page logs nothing, and the host's `agent_token` writes no log
    line.
  - **Tests.** `agentToken.test.tsx` (masked by default, Show and Hide with their reports, the reported window tree
    redacted while revealed, `token-hide` masking it, no reveal during a capture, Copy to the clipboard only, nothing
    sent or logged carrying it, leaving the page); `token_reveal.rs` (the capture guard); the D-39 rows "token Copy"
    (the clipboard read back and compared with the discovery file by hash) and "token Show" (an agent's `inspect_ui`,
    screenshot and activity export while it is shown, and the application's log: none carries it, and the field's
    pixels in the capture equal the masked field's).

## 3. Activity panel (11 §11)

- **Where:** a new dockable panel `activity`, opened by the indicator's **Activity…** button and by Window › Activity.
  01 §5.2's panel list and 01's Window menu do not have it; an SI adds both (11 §6.4 and §11 require the panel).
- **Model.** On open, the panel reads `agent.activity.list {limit: 500}`. After that it appends entries from a new event,
  `agent.activity.recorded {entry}` (kernel gap 1; emitted by `AgentHub::record` with the entry as stored). There is no
  polling. On `project.opened` and `project.closed` the panel re-reads, because the log is per project session. If an
  entry's `seq` is not the next one after the last shown (the panel missed events while hidden), the panel re-reads the
  list with `afterSeq`, a new optional parameter (`since` is a time and can tie). Above 500 entries, the oldest are
  dropped from view; the log itself is unchanged.
- **Rows:** time, client, the command's label (from the registry, with the id as a tooltip), status (ok,
  needs_confirmation, error with its code), changed count, duration. Dry runs and cached retries are marked as such.
- **Grouping by batch.** One row per batch, expandable (`disclose`) to its steps. A batch is a single entry today, with
  its steps in `params`. Calls in one MCP `batch-script` are separate entries with nothing to join them. **Proposal (SI,
  kernel gap 2):** `ActivityEntry` gets `batchId` (shared by the entries of one script run, else absent) and
  `changedIds` (the undo step's touched ids, at most 1000, with `changedIdsTruncated`). Both are optional, so older
  `agent-activity.jsonl` lines still read.
- **Select an entry → highlight its changed objects.** A transient outline on the canvas, drawn like the selection
  outline, from `changedIds`. Selecting a row is GUI state (no command); the element is `navigate`. Ids no longer in the
  document are skipped, and the row says "n objects no longer exist".
- **Undo back to before this entry.** The entry's `undoStepId` is found in `edit.history.get`, and the item sends
  `edit.history.move {position: index - 1}`. The item is enabled only when:
  - the step is still in the history;
  - it is applied (index at most `position`);
  - no atomic batch is open (the command's own enablement rule).

  Otherwise it is disabled with the matching reason: "its step is no longer in the history", "already undone", or the
  kernel's. The history is linear, so moving back also undoes the user's later steps. A confirmation names the count
  first ("Undo 7 steps, 3 of them yours?"), unless every later step is from that agent. The panel re-reads its
  enablement on `edit.history.changed`, `doc.undone` and `doc.redone`. This is "if the steps after it can be undone in
  order" (11 §11) for a linear history.
- **Copy as batch JSON** (GUI-only, reason `clipboard`; D-40 answer 6). Writes `{steps: [{commandId, params}]}` for the
  entry (a batch's steps as they are) to the clipboard, in the form `agent.activity.export {format: batch}` writes. A
  Vitest compares the two on the same entry. Parameters whose inline media was replaced by a hash cannot be replayed:
  the copy keeps the hash, and the panel says "contains media references; not replayable as is".
- **Export…** is a save dialog, then `agent.activity.export {path, format, onExists: fail}`. Choosing to overwrite in
  the platform dialog sends `onExists: overwrite`.
- **Sessions strip** (01 §12 per-session grant). One row per session from `agent.server.status`, with:
  - client name and connected time;
  - a policy choice sending `agent.session.grant {sessionId, policy}` at once;
  - Scopes… opening a checklist that sends `agent.session.grant {sessionId, scopes}` at OK;
  - Pause or Resume, and Disconnect, the same references as the indicator's.

  It re-reads on the session events and on `agent.session.grant`'s answer. For a paused session, the kernel needs to
  announce the change; an SI adds `agent.session.changed {sessionId}` for pause, resume and grant (today only connect
  and disconnect have events).

## 4. Confirmation in the app and elicitation (11 §6.3, §6.4)

**[ruling] The decision rule.** 11 §6.3 step 1 sends an elicitation when the client supports it. Step 2 otherwise
returns `needs_confirmation`, "in attached mode the user may instead be shown the confirmation in the GUI ... the agent
receives the user's decision". Proposal:

1. **Attached, a window present, and the session's `askInApp` on:** the request goes to the app only, with no
   elicitation. The person at the application decides. The kernel records the pending confirmation as an in-app request
   (`Pending.in_app = true`) and emits `agent.confirmation.requested`.
2. **The agent receives the decision without polling.** The MCP server treats an in-app request like an elicitation it
   sent. It holds the call until the user answers or the token expires (5 minutes), then re-invokes it with the token
   (accepted) or returns `confirmation.declined`. If the token expires, the call returns `confirmation.expired`. This
   reuses the elicitation path (`run.rs:1289`): the executor is free while the call is held, so the agent's other calls
   and the user's work go on.
3. **A token of an in-app request is usable only after an accept** (kernel gap 3). `take` refuses it while the decision
   is open (`confirmation.expired` with reason "waiting for the user's answer in the app"). Today an agent that reads the
   token from the `needs_confirmation` result can retry and bypass the person. The fix and a kernel test that fails
   without it come before the sheet.
4. **Otherwise** (headless, no window, or `askInApp` off), 11 §6.3 as implemented: elicitation if the client supports
   it, else the token round trip.

**The sheet is not modal** [ruling]. Plan §4.6 calls it "non-blocking" and also "a user dialog (E-busy note: agent
mutations wait while it is up)". Those conflict. A sheet counted as a user modal makes every agent mutation wait and
fail with `ui.busy` while the question is open. The question would then block the work it is about, and with rule 2 it
would hold the agent for up to five minutes on a single call. Proposal: the sheet is a non-modal window-level panel that
is not counted in `userBusy`. While the person holds one of its buttons down, the ordinary mid-gesture rule applies.

**The sheet:**

- the client's name, the command's label, the `summary`, and the `consequences` as a list;
- Accept (default button) and Decline, each sending `agent.confirmation.respond {confirmationId, accept}`;
- a countdown to the token's expiry;
- several pending requests queue in one sheet, with "1 of 3".

On a new event, `agent.confirmation.resolved {confirmationId, outcome: accepted | declined | expired | used}` (kernel
gap 3, registry addition), the sheet drops a request decided elsewhere (a second window, or the token's expiry). On
open and after a reload it reads `agent.server.status`'s `pendingConfirmations`, so a request raised while the window
was loading is shown. Escape is Decline, never a silent dismiss. Accept and Decline are command elements, not
`answer`: the `answer` reason is for a GUI question resolved locally, and this question is the kernel's.

**macOS:** a sheet attached to the main window, not a separate window; Return is Accept, Escape is Decline.

## 5. Agent indicator (01 §12.1 rule 9; 11 §6.4)

- **Keeps** its states (connected, acting from `shell.agentActing`, server on with no session) and Pause, Resume and
  Stop.
- **Adds:**
  - **Activity…** (opens the panel; `navigate`);
  - a badge with the count of pending in-app confirmations, which opens the sheet;
  - the acting state's label: the command's label instead of its id (11 §6.4 "last command label"). As built after
    ruling D-62 item 3 (C44A-R1-42): `desktop/src/shell/commandLabel.ts` gives the registry's label, in the acting state
    and the popover's activity lines (and the Activity panel); a batch by its own label, which the acting notice now
    carries with its step count, or "Batch (n steps)".
- **The popover's activity lines** stay live while it is open, from `agent.activity.recorded`, instead of the one fetch
  on open.
- **A paused session** shows as paused, from `agent.session.changed`.
- **Port fallback.** When the configured port was taken, the indicator shows the endpoint actually used (11 §4 line
  105), from `agent.server.status`.
- **Keyboard:** the indicator is a button in the status bar's tab order. Its popover is a menu (design note 1 §9's
  mechanism).

## 6. Activity persistence (11 §11)

`agent.activity.persist.set {enabled}` keeps its behaviour (append to `agent-activity.jsonl` beside the project) and
also stages a new preference key `agent.activity.persist` (bool, default false, live, category agent). The setting then
survives a restart, and `prefs.changed` reflects it (design note 1 §2). `agent.typedTools.set` likewise stages
`agent.typedTools` (`{enabled: false, namespaces: []}`). The SI adds both keys to 01 §7.3's Agent access list by
reference to 11. With no project open nothing is persisted, and the Agent access check box says so in its tooltip.

## 7. Load report window (10 §7.3; plan §4.7)

- **When:**
  - The window is shown when `doc.loadReport.get`, read on `project.opened`, has any non-info issue, or
    `repairedCount > 0`, a non-null `migratedFrom`, or `readOnly`. It is shown once per open, keyed by `projectId` and
    the open's job id, so a re-render does not show it again.
  - It is shown whoever opened the project. For an agent's open in attached mode, the person is told too; the agent has
    the issues in `warnings` (11 §6.3).
  - It is non-modal and not counted in `userBusy`.
- **Contents:**
  - a summary line: "Opened with 3 repairs and 2 warnings", plus "migrated from format n" and "read-only: reason";
  - the issues grouped by code and severity, collapsed (`disclose`), each with its object path from the 10 §10.4
    structure;
  - **Copy details** (reason `clipboard`): the report as text;
  - **Close** (`dismiss`).

  No item is a separate message (10 §7.3).
- **Reads:** `doc.loadReport.get` becomes the window's declared read and leaves agent-only (D-40 answer 5).
  `project.opened` is enough as the trigger, so no kernel change is needed.

## 8. The `clipboard` reason and the gate (D-40 answer 6)

- **The reason.** `clipboard` joins the GUI-only reasons in `reads.json`'s reason list and the parity gate
  (`parity_gate.rs`, `parity.test.tsx`): text written to the system clipboard, with no document or application state
  changed. Its elements are Copy as batch JSON and Copy details. The canvas's Cut, Copy and Paste are commands
  (`edit.*`) and are not affected.
- **The check**, like the `answer` check of round 2. For each element declared `clipboard`, the Vitest:
  1. clicks it with the engine and the clipboard stubbed;
  2. asserts exactly one `navigator.clipboard.writeText` call and no `run` or `perform` other than queries;
  3. fails on a fixture whose handler also runs a command.
- **The front end's single writer:** one function `copyText(text)` in `desktop/src/shell/clipboard.ts`. Tauri's WebView
  supports `navigator.clipboard.writeText` on both OSes from a user gesture. The parity collector finds `clipboard`
  elements by their declared reason, and the gate fails if one does not call `copyText`.

## 9. Spec issues and rulings before code (numbers follow note 1's, from SI-173)

**For the orchestrator:**

- **R-a [ruling]:** the in-app confirmation rule of §4. In attached mode with `askInApp` on, the request goes to the app
  only; the MCP server holds the call until the answer; an in-app token is unusable before an accept.
- **R-b [ruling]:** the sheet is not modal and does not block agent mutations (§4). This replaces the plan's E-busy note.

**SIs:**

1. `prefs.set` on `agent.*` keys applies like the commands (§2).
2. The `activity` panel and Window › Activity (§3).
3. `agent.activity.recorded` event; `afterSeq` on `agent.activity.list`; `batchId` and `changedIds` on `ActivityEntry`
   (§3).
4. `agent.session.changed` event (§3, §5).
5. In-app confirmations: `Pending.in_app`, `take`'s refusal before an accept, the `agent.confirmation.resolved` event,
   the MCP server holding the call (§4).
6. `agent.token.regenerate` disconnects the HTTP sessions and keeps the discovery file's endpoint (§1, §2).
7. Preference keys `agent.activity.persist` and `agent.typedTools` (§6).
8. The `clipboard` reason (§8; D-40 answer 6 records it, the SI fixes the gate's check).

## 10. Tests and the D-39 rows

- **Kernel:** a pending in-app token refused before an accept, accepted after, declined after a decline; `resolved`
  for each outcome; `recorded` for every entry, `seq` contiguous; `prefs.set` and `agent.server.set` give one state;
  the persistence key survives a restart; token regeneration disconnects HTTP sessions.
- **MCP e2e** (plan §4.6):
  - a command needing confirmation, accepted and then declined in the sheet; the held call returns the command's result
    or `confirmation.declined`;
  - the same with `askInApp` off and an elicitation client (elicitation only, no sheet);
  - the server switched off and on from Preferences (the discovery file goes and returns);
  - the activity export written and read back;
  - Undo back to before an entry restores the document's revision.
- **Vitest:**
  - each control's declared reference;
  - the Activity panel appends from events alone and re-reads on a gap;
  - the sheet queues, drops on `resolved`, and Escape declines;
  - the load report window shows once per open;
  - the `clipboard` check.
- **D-39 rows, both forms:**
  - each Agent access control at OK;
  - Regenerate token;
  - each sessions-strip control;
  - Undo back to before an entry;
  - Export;
  - the sheet's Accept and Decline (the MCP form raises the request from a second, test-only session);
  - the indicator's Pause, Resume, Stop and Activity….

  The rows for the copy actions record the reason, not a command.
- **macOS:** the sheet's attachment and keys, checked by the manual script `reviews/manual/M4/C4.4a-agent.md` and by the
  D-39 table's MCP form on the macOS runner.

## 11. No single MCP answer or event over 1 MiB (D-51 item 2, SI-163; an item of C4.4a)

**The rule.** No single MCP message, a tool result, a resource read or a notification, may exceed 1 MiB (1 048 576
bytes of its JSON-RPC text), on stdio and on HTTP alike. A common client refuses a larger server-sent event and
reports "SSE stream ended without a response" although the server sent everything (D-49 item 4: `job_wait` on a
12 000-frame render answered 4.1 MB; `reviews/evidence/M4-C4.3b-r4/job-wait`).

**Which answers can exceed it** (they grow with the document or the job's output, not with the request):

| Kind | Commands | Today |
|---|---|---|
| Job records that list outputs | `render.range.run` (one entry per frame), the import jobs (`import.file.run`, `import.keyedImages.run`, `draw.vectorFile.import`, `draw.vectorize.run`: created ids), `draw.feather.apply` (layer ids) | unbounded |
| Whole-document reads | `doc.export.json`, `draw.drawing.get` (a large drawing with `include`), `anim.evaluate` (nodes × frames), `doc.validate`, `element.drawing.list` | unbounded |
| Inline images | `inspect.screenshot`, the contact sheet and any `png` result | bounded by `maxInlineBytes` (default 50 MB) and `maxInlineImagePx`, not by the cap |
| The window tree | `inspect_ui` with a large depth | unbounded |
| Events | `draw.drawing.changed` and the other `*.changed` events whose payload lists ids | unbounded |

Already paged and within the cap: `doc.object.find`, `edit.history.get` (`offset`, `limit` at most 1000),
`agent.activity.list` (`limit`).

**The bounded shapes.**

1. **A job record's result that enumerates outputs** keeps its other fields and replaces the list by
   `{count, location, pattern, first, last, truncated}`. `location` is the output directory or the owning element,
   `pattern` the naming pattern (for example `frame-####.png`), and `first` and `last` are the first and last entries
   in full. `truncated` is true when the list was not given; a list of at most 100 entries is still given in full, with
   `truncated: false`. The full list is read in pages with a new query, `job.outputs.list {jobId, offset = 0,
   limit = 1000 (1-1000)}`, answering `{total, offset, items}` (a registry addition: SI-181).
2. **Inline images.** The effective inline limit is the smaller of `maxInlineBytes` and the cap less the envelope. An
   image over it is refused with `limit.exceeded {limit: "answerBytes", value, size}`, whose hint names `region`, a
   smaller view, or the file-writing command (`export.snapshot.run`).
3. **Whole-document reads and the window tree.** Over the cap they answer `limit.exceeded {limit: "answerBytes"}` with
   a hint naming the narrower query (`within`, `include`, a frame range, `depth`) or the file form
   (`doc.package.export {form: "folder"}`). Nothing is cut silently.
4. **Events.** An id list over 10 000 entries becomes `{count, truncated: true}`; the client re-reads the subject.
   `draw.drawing.changed` keeps `drawingId` and `rev`, which is enough to re-read.
5. **The backstop.** At the server's writer (`crates/mcp/src/server.rs` `send`), a response over the cap is replaced by
   the same `limit.exceeded` envelope, and a notification over the cap is not sent and is recorded in the audit log.
   This holds for commands not listed above and for any future one.

**As built (C4.4a), after ruling D-56 corrected D-51 item 2.** There is no general 1 MiB cap: the spec's limits govern
answers. Kept from the design above: the range job's bounded answer (item 1, with the companion manifest file as the full
list; no paged read, which would need a spec A row, D-57) and the HTTP transport fix (a full queue drops advisory
progress, never the stream; the answer waits for room). Withdrawn: the inline cap (item 2), the `limit.exceeded`
answers (item 3), the event truncation (item 4) and the writer's backstop (item 5). New (D-56 item 3): over HTTP an answer
over 1 MiB with nothing yet sent on its request's stream is one plain `application/json` body (`http.rs` `stream_sse`).
The measurement:

- `c12_kernel_state::every_example_runs_on_the_fixture_under_the_watchdog` measures every example's answer and every
  job example's final record, each under 1 MiB less 8 KiB for the tool-result wrapper (a bound on the examples, not on
  answers);
- `c33_range::a_long_range_answers_with_its_frames_summarised` runs 150 frames and checks the summary, the answer's
  size and the full companion file;
- `http.rs` `a_large_first_answer_is_a_json_body_and_anything_else_an_event_stream`;
- the 12 000-frame wait over HTTP is checked end to end (receipt).

**The gate test as designed** (no wall-clock assertion, D-45):

- every command with a worked example (`crates/mcp/src/worked-examples.json`, `crates/registry/src/example-results.json`)
  runs on a fixture project, and its serialised answer is at most 1 MiB; the test reports the largest;
- stress fixtures for the enumerating commands: `render.range.run` of 12 000 frames at scale 0.05 gives a record of at
  most 64 KiB with `count` 12 000, `truncated` true and `first` and `last`; `job.outputs.list` pages cover all 12 000
  entries exactly once;
- a synthetic oversize answer and notification meet the backstop.

**End to end:** the reviewer's client (Python `mcp` over `httpx2`) waits for that render over HTTP and receives its
answer. **Spec issues:** SI-181 (`job.outputs.list` and the bounded record), SI-182 (`limit.exceeded` with
`answerBytes` for the reads, images and the backstop).

**An answer that cannot be delivered (ruling D-59 item 4).**

- **When it is dropped.** Over HTTP, a request's answer waits up to 10 s for room in its stream's queue (`http.rs`
  `offer_answer`, `ANSWER_WAIT`). If the client went away, or stopped reading for that long, the answer is dropped. The
  command has already run by then: it committed, refused or started its job exactly as it would have with a reader.
- **What is recorded.**
  - The call's own activity entry stands, with its status, `undoStepId`, `documentRev` and changed count.
  - A second entry follows it: tool `transport`, command `answer.undelivered`, params `{requestId}`, status `error`,
    code `transport.answerDropped` and reason "client not reading".
  - The route reports the dropped request's id (`route` returns it), and the session's protocol loop records the entry
    (`server.rs` `record_undelivered`, through `ServerOptions::undelivered`). stdio has no such queue and records
    nothing.
- **What the agent can still learn.**
  - **The activity log** (`agent.activity.list`, or the `app://activity` resource) names the call by command and
    params, with its outcome and the drop that followed. An agent that retries with the same `requestId` gets the
    cached answer and runs nothing twice (C14-R2-03).
  - **The document state.** `doc.state.get` (its revision) and a read of the objects the call named show what changed;
    `edit.history.get` shows the undo step by its label.
  - **The job record**, for a call that started a job: `job.list` and `job.wait`.
- **Tests.** `http.rs` `an_answer_nobody_reads_is_reported_undelivered` checks that the route reports the dropped id
  and nothing else. Written under the GUI hold; not yet run.

## 12. Host-only notices (ruling D-57)

Spec A is edited by the user only, so the window learns of some changes through host-only `shell.*` notices instead of
registry events. A `shell.*` notice is emitted on the executor's bus for the desktop host and its windows; the MCP server
drops it before anything is recorded or sent (`crates/mcp/src/server.rs` `pump_events`), which
`crates/mcp/tests/c44a_shell_notices.rs` checks (none in `app://events`, none in a log notification). None carries
anything an agent cannot read through a declared query:

| Notice | Emitted by | Payload | What the window does | The declared query with the same information |
|---|---|---|---|---|
| `shell.agentActivity` | every recorded agent call (`run.rs` `audit`, C4.4a) | `{entry}`, the entry as stored | the Activity panel and the indicator append it | `agent.activity.list` (the same entries) |
| `shell.agentSession` | `agent.session.grant` (C4.4a) | `{sessionId}` | the sessions strip and the indicator re-read | `agent.server.status` (`sessions`) |
| `shell.agentToken` | `agent.token.regenerate` (C4.4a) | `{}` | the desktop host restarts its HTTP server (SI-178) | none needed: it carries nothing |
| `shell.agentConfirmation` | an in-app confirmation decided, expired or used (C4.4a, D-58) | `{confirmationId}` | the confirmation sheet re-reads | `agent.server.status` (`pendingConfirmations`) |
| `shell.agentServer` | `agent.server.set` (C4.1, O-21) | `{enabled}` | the host starts or stops the server | `agent.server.status` (`enabled`) |
| `shell.agentActing` | around an agent request (C4.1) | `{sessionId, client, commandId, acting}` | the indicator's acting state | `agent.server.status` (`lastActivity`) and `agent.activity.list` once it ends |
| `shell.view`, `shell.window`, `shell.help`, `shell.preferences`, `shell.raiseAll` | the `ui.*` and `window.*` commands (C4.1) | the command's own parameters | the host applies them to the native window | the command's own state queries (`view.list`, `window.geometry.get`, `ui.help.get`) |

The registry events the design proposed instead (`agent.activity.recorded`, `agent.session.changed`,
`agent.confirmation.resolved`) are listed in `plan/spec-issues.md` under "Spec A additions proposed by M4, for the user".
