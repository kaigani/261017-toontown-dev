# M4 design note — user priority, sensor reports and UI freshness

Status: decided before implementation (M4 C4.1). Owner: builder. Spec: 01 §12.1 rule 8, §12.4 `modal`; 11 §6.4;
11 §7.5 `inspect_ui`; 11 §7.6 `key`. Feeds E6 (C4.3).

## 1. Problems

1. **User priority.** While the GUI user has a modal dialog open or is mid-gesture, document-changing agent commands
   that conflict must wait up to `waitForUserMs` (default 10 000 ms; 0 = fail at once) and then fail with `ui.busy`
   (`details.blockedBy`); queries and non-conflicting commands still run. While a user dialog is open, agent `ui`
   commands that would change focus fail with `ui.modalOpen`. `app.state.get` reports `modal`.
2. **Facts only the shell knows.** Modal state, gesture in progress, text-field focus, hover description, pointer
   text, view canvas sizes, screens and the rendered UI tree exist in the webview or the OS, not in the kernel. No spec
   command sets them (spec A has none, and the inventory test keeps the registry equal to spec A).
3. **UI freshness for agents.** `inspect_ui` after a command must describe the GUI *after* that command, not a frame
   earlier. The tree is produced by the front end, asynchronously.

## 2. Contract

- **Sensor reports** are a kernel API of `ExecutorHandle` (`report_shell`), not commands. A report replaces the facts
  it carries; it never changes the document, a setting, a layout or the undo history, emits no undo step, and is not
  audited. Agents cannot send one. Reports reach the document thread as posted work (never a blocking call) and are
  applied between requests.
- **Busy arbitration** runs on the caller's side of an agent call (the MCP request thread), never on the document
  thread, before the request is queued:
  - *Waits:* an agent `action`, `job` or batch while `modal` is a dialog or a gesture. The wait polls a shared flag
    every 20 ms, bounded by the session's `waitForUserMs` and by the call's watchdog budget; it ends early when the
    modal state clears or the caller cancels (cancellation answers `command.timeout` with `reason: "cancelled"`, as
    every cancelled request does, ADR-0020 decision 8).
  - *Refused at once with `ui.modalOpen`:* an agent focus-changing `ui`/`toggle` command while a user *dialog* is
    open (`window.panel.set`, `window.panel.move`, `view.active.set`, `view.type.set`, `app.mode.set`,
    `workspace.select`, `workspace.layout.set`, `workspace.reset`, `window.fullScreen.set`, `window.minimized.set`,
    `window.raiseAll.run`, `window.geometry.set`, `ui.preferences.open`, `ui.help.open`, `ui.startScreen.set`).
  - *Never blocked:* queries, and every user-origin call (the GUI's own commands run; the modal dialog that is open is
    the user's).
  - On expiry: `ui.busy` with `details: {blockedBy: {kind, title}, waitedMs}`. Nothing was queued, so nothing ran.
- **`waitForUserMs`** has no command in spec A (spec issue). It is an executor setting (`set_wait_for_user`, default
  10 000 ms) that the host and tests set.
- **The rendered UI tree** is a sensor fact with a sequence number: the front end reports the tree together with the
  number of the last engine event it has rendered. The kernel counts the events it broadcasts. The `inspect_ui` tool
  waits on its caller's thread, bounded (2 s, below a third of the default budget), until the reported number reaches
  the number of events broadcast before the call, then reads the tree through `inspect.ui.tree`. In headless mode the
  kernel synthesises the tree from its own state (panels, views, start screen, status bar), so the tool never waits.
  A stale answer after the bound is marked `stale: true`, never presented as fresh.

## 3. Alternatives considered

1. **Wait on the document thread** (the handler checks `modal` and sleeps). Rejected: it would hold every caller,
   including the user's own commands that end the modal state, and break never-block (11 P5).
2. **Register sensor commands** (`ui.modal.set`, …) with an `app` scope. Rejected: not in spec A (inventory test),
   they would appear in `list_commands` and the activity log, and an `app`-scoped command is still a command an
   embedding could call; a handle API states the boundary in the type system.
3. **Ask the front end synchronously for the UI tree** from the `inspect.ui.tree` handler. Rejected: a handler may not
   wait for input (P5), and the webview may be busy or gone.

## 4. Deterministic tests

- Kernel (`crates/executor/tests/c41_busy.rs`): scripted reports through `report_shell` — an agent action waits and
  succeeds when the report clears before `waitForUserMs` (the clearing report is sent by the test after it observes
  the waiting caller through a step-ordered probe, not after a sleep); fails with `ui.busy` and nothing changed when it
  does not (`waitForUserMs` 50 ms, the assertion is on the outcome, not on elapsed time); `0` fails at once; queries
  and user calls run during a modal; a focus-changing agent `ui` command fails with `ui.modalOpen` only under a dialog;
  a cancelled waiting caller answers `reason: "cancelled"` and queues nothing.
- MCP (`crates/mcp/tests/c41_http.rs`): the same through the HTTP transport with two clients.
- Freshness: a test front end (scripted reporter thread) reports trees with sequence numbers; `inspect_ui` after a
  command returns the tree whose number covers that command's events, and marks a tree that never catches up as stale
  after the bound.
- E6 (C4.3): end to end in the running application.
