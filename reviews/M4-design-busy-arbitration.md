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
  - *Waits:* an agent `action`, `job`, batch or `pointer` call while `modal` is a dialog or a gesture; **while the user
    is mid-gesture (M4 C4.2, ruling D-12), also every agent `ui` and `toggle` command**: `tool.*`, the pen commands,
    `drawAid.*`, `view.*` and `colour.current.set` change the session state the user's gesture depends on (11 §6.4
    lines 237-241). Queries still run. The wait polls a shared flag
    every 20 ms, bounded by the session's `waitForUserMs` and by the call's watchdog budget; it ends early when the
    modal state clears or the caller cancels (cancellation answers `command.timeout` with `reason: "cancelled"`, as
    every cancelled request does, ADR-0020 decision 8).
  - *Refused at once with `ui.modalOpen`:* an agent focus-changing `ui`/`toggle` command while a user *dialog* is
    open (`window.panel.set`, `window.panel.move`, `view.active.set`, `view.type.set`, `app.mode.set`,
    `workspace.select`, `workspace.layout.set`, `workspace.reset`, `window.fullScreen.set`, `window.minimized.set`,
    `window.raiseAll.run`, `window.geometry.set`, `ui.preferences.open`, `ui.help.open`, `ui.startScreen.set`; since
    M4 C4.2 also `tool.active.set` and `tool.group.select`, since the active tool decides what the user's next input
    does). Other `ui` commands under a dialog run as in C4.1.
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

- Kernel (`crates/executor/tests/c41/busy.rs`): scripted reports through `report_shell` — an agent action waits and
  succeeds when the report clears before `waitForUserMs` (the clearing report is sent by the test after it observes
  the waiting caller through a step-ordered probe, not after a sleep); fails with `ui.busy` and nothing changed when it
  does not (`waitForUserMs` 50 ms, the assertion is on the outcome, not on elapsed time); `0` fails at once; queries
  and user calls run during a modal; a focus-changing agent `ui` command fails with `ui.modalOpen` only under a dialog;
  a cancelled waiting caller answers `reason: "cancelled"` and queues nothing.
- MCP: `ui.busy` and `ui.modalOpen` reaching an agent over HTTP through the real application in the `desktop gui`
  job (`scripts/c41-gui-e2e.py`); the transport itself, with two sessions on one executor, in
  `crates/mcp/tests/c41.rs` (as built; the note first named a separate HTTP busy test).
- Freshness: a test front end (scripted reporter thread) reports trees with sequence numbers; `inspect_ui` after a
  command returns the tree whose number covers that command's events, and marks a tree that never catches up as stale
  after the bound.
- E6 (C4.3): end to end in the running application.

## 5. Amendment for C4.3b: E6 in the running application (plan §3.7; rulings D-26, D-27)

11 lines 586-587: "while a scripted 'user' drags an element in attached mode, the agent's mutating call waits and then
succeeds; `undo ifTop` refuses to undo the user's later step." The scenario
(`scripts/c43-e6-coexistence.py`, the `desktop gui` job on both OSes):

1. The application runs attached with the sample project, the Camera view shown and active, Bob selected, the
   Transform tool current. An agent session connects over MCP (an `agent_session` with `edit` scope).
2. **The user drags Bob** in the Camera view. On Windows this is a real pointer drag by OS input (`SendInput`: press,
   moves over about 1.5 s, release; ruling D-26). On macOS the runner allows no synthetic OS input to the webview, so
   the window plays the same samples through its own input path (`pointer` routed to the window, C4.2b's P1). The
   receipt states which.
3. **While the drag is pressed**, the agent calls `anim.transform.set {nodeIds: [Bob], target: "static", position}`.
   The call waits: the window has reported the gesture as the modal state. The test records the call's send time, the
   release time and the answer time, and asserts the order: answered after the release, not before.
4. **After the release** the call succeeds (its envelope `ok`, `documentRev` advanced). The user's drag is one undo
   step (the Transform tool keys Bob's position at the current frame), and the agent's step follows it.
5. **The user's later step:** the user nudges Bob again (a second real drag), a step after the agent's.
6. **The agent's `edit.undo {ifTop: <its step id>}`** answers `undo.notTop`, naming the user's later step as the top.
   The user's step survives: `edit.history.get` shows, in order, the user's drag, the agent's call and the user's
   later step, none undone.
7. Nothing is lost or reordered: the document's history, the agent session's activity log and the change events agree
   on the three steps and their origins.

**Invariants:**
- **E1:** an agent mutation during a user gesture waits at most `waitForUserMs` and succeeds after the release when the
  wait suffices; with `waitForUserMs` shorter than the drag it fails `ui.busy` instead. The scenario runs once each way.
- **E2:** `undo ifTop` never undoes another origin's later step.

No wall-clock figure is asserted near a bound (D-27): the order of the events is asserted, and the wait is recorded
in the receipt.

### 5.1 As built (C4.3b)

- **Setup.** The script creates the project itself (`project.create`, the element Bob with a painted rectangle,
  exposed on frames 1-24): the sample project is M9 C9.3's. The default workspace shows the Camera view central; the
  script makes it active, selects Bob and chooses the Transform scene tool.
- **The user's drags** start inside Bob's box away from his pivot (a press on the pivot drags the pivot, 05 line 418).
  The points are Bob's drawing units mapped through `draw.viewMapping.get {view: cameraView}` (re-read before each
  drag: Bob moves) and the canvas's place in the window capture, scaled to device pixels with `view.list`'s logical
  size. The script is per-monitor DPI aware.
- **OS input** (Windows) is `SendInput` for the button and `SetCursorPos` for the moves. No input is sent unless the
  application's window is in front and is the root of the window under the press (the webview's own window belongs to
  its browser process); the check is recorded.
- **The played mode** (macOS; `TT_E6_MODE=played` runs it anywhere) plays the drags from a second MCP session through
  the window's input path, so the agent's calls never queue behind it. Its steps are agent steps of the other session,
  told apart by `agentSessionId`; they stand in for the user's. The played gesture's release is its commit's answer,
  a large envelope whose arrival can trail the agent's small answer by milliseconds, so in this mode the causal order
  is the history's (the stand-in's step, then the agent's) and the timing check is only "sent while playing". The
  `ui.busy` case needs a press held past `waitForUserMs` (10 s); a played gesture lasts about two seconds, so it runs
  on Windows only. The manifest names the input mode.
- **Also checked:** the modal state names the acting tool (`transform`, not the drawing tool); the activity log records
  the agent's change (ok) and the refused undo (error).
- **Found and fixed:** SI-157. The window reported a gesture's end before sending its commit, and an agent change
  waiting on the gesture could run between them. The window now reports the end once the commit has answered, and the
  host does not arbitrate the window's commit of a played agent gesture again
  (`crates/executor/tests/c43/order.rs`, `desktop/src/canvas/gestureOrder.test.ts`).
