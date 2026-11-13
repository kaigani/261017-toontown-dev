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
   `job.cancel` from their progress window.
4. **Engine events.** The host subscribes to the executor and forwards every event to the webviews as JSON text with a
   delivery number. The store applies events in order and resyncs every slice when a number is missing (IPC
   freshness). The kernel counts the events it broadcasts (`ExecutorHandle::events_broadcast`).
5. **Sensor facts are a kernel API, not commands** (`ExecutorHandle::report_shell`, `reviews/M4-design-busy-
   arbitration.md`): modal state, text focus, hover description, pointer text, view canvas sizes, screens, the attached
   flag, the rendered UI tree with the last event number it reflects, and messages the GUI showed (emitted as
   `ui.message.shown`, `source: "user"`). A report never changes the document, a setting, a layout or the history, and
   agents cannot send one. `app.state.changed` follows a reported `modal` change.
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
   (`complete_reference`).
9. **Menus.** `desktop/src/ui-bindings/menu.json` (version 2) is the single source of the native menu and the parity
   test: command references (id plus fixed parameters), check-mark sources, submenus, and the dynamic Open Recent and
   workspace lists. Enablement (registered, implemented, 01 §1.3, undo/redo availability), check marks, undo/redo labels
   naming the step and its agent, and accelerators from the keymap are refreshed from executor state on the events that
   can change them. On macOS the menu owns its key equivalents; on Windows and Linux the shortcut is shown in the label
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
11. **Themes.** `light`, `dark`, `system` (follows the OS appearance live), and user theme files in
    `<settings>/Themes/*.json` with a closed token set; invalid files are ignored. A theme is a set of CSS custom
    properties applied at once.
12. **Docking.** The kernel `Layout` is rendered with dockview: `corners` decides which areas run the full window length;
    areas stack their tab groups along their long side. Every drop is vetoed and replaced by the one
    `window.panel.move` an agent would send; a tab's close button is `window.panel.set {visible: false}`; double-click
    floats. Floating panels are separate OS windows (round 1, ruling O-2; reviews/M4-design-floating-windows.md): the
    kernel decides, the host reconciles one webview window per visible float, every window reports its own facts and
    the host combines them. A splitter drag ends in the same-place `window.panel.move` where that command can express
    it (SI-118, interim ruling O-3).
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
