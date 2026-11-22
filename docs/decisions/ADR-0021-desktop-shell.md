# ADR-0021 — Desktop shell architecture

Status: accepted (M4 C4.1, 2026-11-12). Scope: plan §4 row M4, checkpoint C4.1; M3 merge note §8 item 1 (actual-shell
command/GUI bindings, typed errors, request cancellation, settings persistence, IPC freshness); spec 01 whole, 11 §6.4.

## Context

At M3 the desktop host was the M0 shell: an engine facade without an executor, `ping` and `app_info`, a native menu
that forwarded most items as "pending", and three placeholder panels. Spec 11 P2 and plan §3.2 require that the GUI is
a client of the same command layer as agents, never a second path to project state. Spec 01 requires a full shell:
start screen, projects, menus, preferences, keymap editor, themes, workspaces and docking, history, status bar with
the agent indicator, messages, persisted settings, and the user-priority rule of 01 §12.1 rule 8.

## Decision

1. **The front end holds no authoritative state.** Everything a command can read or set lives in the executor session
   (layout, view windows, preferences, keymap, start screen, status-bar texts, modal state, agent sessions, history).
   The front end renders a mirror (`desktop/src/state/store.ts`) refreshed by engine events and sends every user action
   back as a command.
2. **One entry point.** `run_command {command, params, options?, requestToken?}` invokes a registered command as the
   user (`Origin::User`) on the executor and answers its envelope. Menus, toolbar buttons, shortcuts, docking drops,
   dialogs' OK and the start screen all go through it (`desktop/src/shell/makeActions.tsx`). Commands that need
   parameters the user must give open a dialog first (01 §0); commands that would discard the project apply the
   unsaved-changes rule and send `onUnsaved` (01 §3.5). Front-end tests assert the exact command stream of each
   gesture.
3. **Request cancellation (ADR-0020 decision 8).** A GUI call that can cancel carries a request token; the host keeps an
   `AtomicBool` per token and calls `invoke_served_until` with a probe that only loads it; `cancel_request` stores
   `true`. Calls that cannot cancel use the plain entry point. A cancelled request (`command.timeout`,
   `details.reason: "cancelled"`, SI-111) is shown as "cancelled", never as a timeout. Jobs are cancelled only by
   `job.cancel` from their progress window. A token belongs to one invocation of one window (C41-R1-07): a
   `run_command` naming a token already in flight in that window is refused before anything runs with `arg.invalid`,
   `details: {pointer: "/requestToken", reason: "requestTokenInUse"}` (round 2, C41-R2-22).
4. **Engine events.** The host subscribes to the executor and forwards every event to the webviews as JSON text with a
   delivery number. The store applies events in order and resyncs every slice when a number is missing (IPC
   freshness). The kernel counts the events it broadcasts (`ExecutorHandle::events_broadcast`).
5. **Sensor facts are a kernel API, not commands** (`ExecutorHandle::report_shell`, `reviews/M4-design-busy-
   arbitration.md`): modal state, text focus, hover description, pointer text, view canvas sizes, screens, the attached
   flag, the rendered UI tree with the last event number it reflects, and messages the GUI showed (emitted as
   `ui.message.shown`, `source: "user"`). A report never changes the document, a setting, a layout or the history, and
   agents cannot send one. `app.state.changed` follows a reported `modal` change.
   **Freshness fields (round 2, C41-R2-07; SI-121).** Two additive result fields beyond 11 §7.5 tell an agent which
   events a tree reflects: `inspect.ui.tree` answers `renderedSeq`, the engine event number the accepted GUI tree
   vouches for (`null` when the tree is not vouched for, or is the headless virtual tree); the MCP `inspect_ui` tool
   answers `stale: true` when the front end has not rendered the events before the call within 2 s (it then returns
   the last tree it has, never an error). In the desktop host the tree is vouched for only when every window counts:
   the main window, every float the host opened (from the moment it opens it) and every window that reported anything
   (C41-R2-06; reviews/M4-design-floating-windows.md §3). **The quorum invariant (round 3, C41-R3-04, -06;
   reviews/M4-design-floating-windows.md §3a):** the expected windows are the main window plus every panel the
   kernel's current arrangement shows as a floating window, from the moment the command that floated it was accepted
   (the document thread publishes the kernel's visible floats before it answers, and a tree without a shown float's
   `float-<panelId>` node vouches for nothing); a window missing, without a tree or number, older than the last event
   or of another generation leaves the result unvouched; and when no window has a tree (the main window reloads), the
   host sends `uiTreeCleared: true` and the kernel clears its stored tree and number explicitly, so an absent `uiTree`
   is never read as "the old tree still stands".
6. **Shell effects.** Native-window commands (`window.fullScreen.set`, `window.geometry.set`, `window.minimized.set`,
   `window.raiseAll.run`) change kernel state and publish `shell.window` / `shell.raiseAll` notices after acceptance;
   requests only the shell can carry out (`ui.preferences.open`, `ui.help.open`) publish `shell.preferences` /
   `shell.help`; `shell.agentActing` brackets agent requests for the indicator. `shell.*` notices go to the host only:
   the MCP event pump does not forward them. In headless mode the same state is a virtual window and virtual views. When
   the user moves or resizes the window, the host invokes `window.geometry.set` (the gesture produces the command);
   equal values publish nothing, so there is no echo loop; geometry is clamped to the screens (01 §5.4).
7. **User priority** runs on the caller's side of an agent call before anything is queued: agent actions, jobs and
   batches wait while the user is in a dialog or a gesture (`waitForUserMs`, default 10 s, `set_wait_for_user`; SI), then
   fail with `ui.busy`; focus-changing agent `ui` commands under a user dialog fail with `ui.modalOpen`; user calls and
   queries are never held.
8. **Keymap and dispatch.** Bindings are data (`crates/executor/src/keymap-default.json`, set `default`) plus user
   overrides stored as deltas (`keymap.json`). Sequences are platform-neutral (`Primary+Ctrl+Alt+Shift+<Key>`; `Ctrl` is
   `Primary` on Windows and Linux). One resolution function (`ExecutorHandle::resolve_key`) serves real key presses
   (`dispatch_key`) and the MCP `key` tool: mode context first, then `global`; a dialog suspends shortcuts; a focused text
   field keeps unmodified keys; a toggle the binding leaves open gets its next state; missing required parameters mean a
   GUI dialog (an agent's call fails with `arg.missing`). The front end decides synchronously only whether to stop the
   webview's own default for a key; the kernel decides what runs. Menu clicks are completed by the same function
   (`complete_reference`). **Refusals (round 2, C41-R2-09; 11 §7.6, §10, §6.4).** A key bound to a command that cannot
   run now answers that command's `command.disabled` (`details: {commandId, reason, keys}`, the reason the menu shows);
   `{handled: false}` is only for a key bound to nothing. The GUI consumes a refused key silently (at most a status
   line, never a message box). An agent's `key` call while the user's dialog has the keyboard fails with
   `ui.modalOpen`, and while the user edits a text field (unmodified keys) with `ui.busy`; the GUI's own keys are
   suspended as before.
9. **Menus.** `desktop/src/ui-bindings/menu.json` (version 2) is the single source of the native menu and the parity
   test: command references (id plus fixed parameters), check-mark sources, submenus, and the dynamic Open Recent and
   workspace lists. Enablement (registered, implemented, 01 §1.3, undo/redo availability), check marks, undo/redo labels
   naming the step and its agent, and accelerators from the keymap are refreshed from executor state on the events that
   can change them. **Availability (ruling O-10; closure ruling O-23; reviews/M4-design-availability.md).**
   `ExecutorHandle::availability` answers whether a command reference can run now, for every surface that offers a
   command (native menu, toolbars, tool rail, key dispatch, the MCP `key` tool, context actions). It evaluates, in the
   pipeline's order (11 §8): registered, not an event, a project when the declaration needs one, the declared
   enablement rule, implemented (the static checks); then, for a complete reference (no required parameter missing),
   the command's declared **precheck** (`#[command(precheck = ...)]`): a cheap pure function of the session state and
   the raw parameters returning the state refusal the command gives with no parameter missing (an empty clipboard,
   nothing selected, no current drawing, a peg as the current element, the central view, the last camera, a reference
   that must name one of several parameters). The transaction pipeline runs the same precheck before every handler,
   so availability and execution are one predicate by construction (the pipeline runs it immediately before the
   handler, after `expectRev`, where the handler gave the same refusal before). A precheck makes no copy and runs no
   pipeline or integrity gate; availability lends the session to one read-only context per chunk (each chunk its own
   request on the document thread, ending after 32 references or once it has held the thread 2 ms) and takes it back;
   a panicking precheck is contained (the reference reports unavailable with an internal reason; an invocation answers
   `internal`). Two refusals ask the user and keep the reference enabled: `project.pathRequired` (Save asks for a
   location) and `project.unsavedChanges` (01 §3.5). References with required parameters missing are completed by a
   dialog and get the static checks only. **Completeness (closure ruling O-24):** `docs/availability-inventory.md`
   classifies every (command, refusal code) pair that `scripts/check-availability-inventory.py` finds in the handler
   of every GUI command and the executor functions it calls (`parameter`, `precheck`, `asks-user`, `execution`,
   `deferred <checkpoint>`, `unreachable`, each with a reason); the `lint` job fails when a pair is missing, a row is
   stale, or a `precheck` pair cannot be reached from the command's precheck. Every state refusal whose state C4.1
   builds through commands is `precheck`; `deferred` is only for a state that needs a view or facility of a later
   checkpoint (its owner; the M4 merge note lists them). A refusal that availability reports as runnable but the
   handler gives for a state reason is P3 once the pair is in the inventory (the user gets the typed refusal); one
   that availability reports as unavailable while the command would succeed stays P2. The dry run is the test
   oracle: a sweep of the 269 GUI references in eight constructed states asserts that availability equals a dry run of
   each reference, in both directions, and a second sweep builds, for every `precheck` row, the state the row names in
   which the refusal occurs and one in which it does not (or the row says why the state cannot be built through
   commands). Selections, the clipboard and exposure columns can be large: what a precheck needs to know of them (the
   kinds of the selected elements, whether a copied element is still in the project, the cells of a column, the
   cameras of a scene) is computed once per value and document revision, during availability only (the pipeline runs
   prechecks inside transactions and computes directly), so a pass after a change reads such a value once and the
   passes after it read nothing. Cost bound, tested: a full pass on a project of 10,000 elements, with 10,101 elements
   selected, with them on the clipboard, with a current column of 10,000 runs and with 991 cameras costs no more than
   on an empty project plus 20 ms and under 50 ms; no availability request holds the document thread more than 20 ms,
   including the first pass after such a change; a user command during continuous passes completes within 100 ms.
   Passes run when the menu refreshes (once per burst of state-changing events), when a toolbar renders, and per key
   press. **Form validity is local:** a dialog's
   OK button is also disabled while its draft is incomplete or invalid (an empty name, a frame rate out of range),
   because it judges parameters that have not been sent and changes with every keystroke; the command validates them
   again when it runs and its typed error is shown if they are refused. On macOS the menu owns its key equivalents; on Windows and Linux the shortcut is shown in the label
   and the webview's dispatch runs it, so a key never runs twice.
10. **Preferences** are a data catalogue (`crates/executor/src/prefs.json`: 01 §7.3, the agent keys of 11, keys earlier
    commands use) validated per key; `prefs.set` is atomic; only non-default values are stored; a stored value that no
    longer validates falls back to the default for that key; agents cannot set `agent.*`. The Preferences window stages
    edits and applies them on OK with `prefs.reset` (restored categories), one `prefs.set` and the keymap edits in
    order; Cancel discards everything; theme changes preview live.
    Object-typed keys (the four `edit.*` timeline and motion-point option sets, `agent.policy.defaults`), round 2:
    - **A supplied object is a partial update (ruling O-16).** `prefs.set` merges its members over the key's present
      effective value, then validates the merged object as a whole (member names, member types, the nested domain),
      atomically with the call's other keys; `agent.policy.defaults.set` with members omitted does the same. Members
      not supplied keep their present value; `prefs.reset` is how members return to their defaults. `prefs.changed`
      names the key only when the merged value differs.
    - **Stale values fall back per member (ruling O-17).** For an object-typed key the fallback above applies to each
      member: a stored member that is missing, of the wrong type or outside its domain takes its default; valid members
      are kept; unknown members are ignored. A stored value that is not an object at all falls back whole.
    **Message help coverage (ruling O-11; round 3, ruling O-19).** Every message id has help, checked in `check` by
    `scripts/check-message-help.py`: a text scanner of the product sources (code literals at constructor sites,
    constants named there, the kernel's shown messages, the front end's message ids, the host's notices), whose own
    coverage is tested with fixtures. Its limit: a code built at run time (formatted, computed or passed in) is not
    seen by a text scanner. The durable fix is deferred to **M11 C11.1** (owner): error codes constructed only from one
    catalogue type, so a Rust test iterates the catalogue and checks that every code has help.
11. **Themes.** `light`, `dark`, `system` (follows the OS appearance live), and user theme files in
    `<settings>/Themes/*.json` with a closed token set; invalid files are ignored. A theme is a set of CSS custom
    properties applied at once.
12. **Docking.** The kernel `Layout` is rendered with dockview: `corners` decides which areas run the full window length;
    areas stack their tab groups along their long side. Every drop is vetoed and replaced by the one
    `window.panel.move` an agent would send; a tab's close button is `window.panel.set {visible: false}`; double-click
    floats. Floating panels are separate OS windows (round 1, ruling O-2; reviews/M4-design-floating-windows.md): the
    kernel decides, the host reconciles one webview window per visible float, every window reports its own facts and
    the host combines them. A splitter drag ends in the same-place `window.panel.move` where that command can express
    it (SI-118, interim ruling O-3). Round 2 (C41-R2-22, -10): a float shown for the first time takes the kernel's
    default geometry, (80, 80) and 480 x 360 logical pixels, clamped to the reported screens; when the host cannot open
    a float's window it sends the main window the host notice `window.floatFailed` (`{messageId, severity: "warning",
    text}` with the OS reason in `text`, shown in the message area) and the panel stays floating in the kernel; one
    reconciliation runs at a time from the current kernel state, and every path that shows floats respects
    `window.floatingPanels.set` (reviews/M4-design-floating-windows.md §5).
13. **Settings persistence.** The executor writes versioned settings files (`preferences`, `keymap`, `state` with
    suppressed tips, first-run flags and the main window geometry, `recent`, `workspaces`) at its acceptance boundary,
    so dry runs, failed batches and cancelled requests write nothing; a corrupt file is set aside and its defaults apply.
    Round 1 (C41-R1-04, -08, -09; rulings O-12, O-13):
    - **Version policy, as for documents.** The same major version is read (a newer minor version's unknown members are
      tolerated); a missing or malformed `formatVersion` makes the file corrupt (`badVersion`); another major version is
      not read (`unsupportedVersion`). Either is kept aside with its defaults used; the other files are unaffected.
    - **Corrupt file versus stale value.** A file is corrupt when it cannot be read, is not a settings envelope of its
      kind, fails the version policy, or its whole `data` is not the kind's shape (preferences: an object; recent: a list
      of `{path, name, lastOpened}`; state: an object whose `lastUsed`, `suppressedMessages`, `firstRun` and `window`
      members have their types; keymap: an object with `baseSet`, `added`, `removed` of their types; workspaces: an
      object with a `workspaces` list of named entries). Inside a well-shaped file, a value that fails its own
      validation is stale, not corrupt: a preference falls back to its default (decision 10), a binding or a workspace
      entry is skipped.
    - **What happened is told.** `prefs.corrupt` names the file, the reason and where the copy was kept, or why it could
      not be kept aside. Copies are created with create-new semantics (`<name>.corrupt-<time>.json`, then `-1`, `-2`):
      an earlier copy is never replaced, and copies are never pruned. A settings write that fails (or a per-user folder
      that cannot be created) is told once per file per session with the path and the OS reason (`prefs.saveFailed`,
      SI-119 interim); the change stays in memory.
    - **Interrupted writes.** Writers hold `<settings>/.settings.lock` shared while they write; at start the executor
      removes `.<kind>.json.<hex>.tmp` files only when it holds that lock exclusively, so a temporary file another live
      process of the application is writing is never removed.
14. **Single instance** (01 §1.1): a loopback port and secret in `<settings>/instance.json`; a second instance forwards
    its documents and exits.

## Consequences

- A GUI path that changes project state, a setting or a layout without a command is a defect; the front-end tests check
  the command stream of every gesture, and `inspect_ui` shows the command id each widget invokes.
- The kernel API grows by `report_shell`, `resolve_key`, `complete_reference`, `set_wait_for_user`,
  `events_broadcast`, `await_rendered` and `attached`; none is reachable by agents.
- `view.*` stay with M4 C4.2 as registered; the kernel already keeps the view windows (types, per-type transforms,
  sizes), so C4.2 adds the commands and the canvas.
