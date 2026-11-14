# M4 design note — floating panels as separate windows (ruling O-2)

Status: decided before implementation (M4 C4.1 round 1, ruling O-2). Owner: builder. Spec: 01 §5.4 ("floated as a
separate window"; floating panels remember their last size and position; Close active panel; Toggle floating windows),
01 §5.5 (workspaces store floating geometry and visibility), 01 §11 (restored at start), 11 §7.5 (`inspect_ui`).
Replaces the in-window floats of the C4.1 handoff (a divergence from 01 §5.4).

## 1. What a float is

- **One webview window per visible floating panel**, labelled `float-<panelId>` and loading the same front-end
  bundle with `?panel=<panelId>`. It renders that panel only (title bar: the panel's title), with the same
  `PanelBody` the dock uses. Floating tool palettes stay on the rail until their tools exist (their owners are later
  checkpoints); the mechanism is the same when they arrive.
- **The kernel decides; the host reconciles.** Which panels float, whether they are shown and their geometry are kernel
  state of the current workspace arrangement (`window.panel.list`, `workspace.get` `floating`). After every state
  change the host reconciles the OS windows with it: it opens a window for each visible float, closes the windows of
  panels no longer floating or visible, and moves or resizes a window whose kernel geometry differs from the OS one.
  Restore at start and on a workspace switch (including the full-screen arrangement) are the same reconciliation.
- **The one rule holds.** No window owns state. A float's gestures run registered commands through `run_command`,
  exactly as in the main window. Moving or resizing a float ends in `window.panel.set {panelId, geometry}` when the
  gesture pauses (as the main window's `window.geometry.set`); the kernel clamps the geometry to the screens reported by
  the windows (the main window's clamp rule). The OS close button of a float runs `window.panel.set {panelId, visible:
  false}` (01 §5.4: Close active panel hides a floating window); a double-click on its title strip docks it
  (`window.panel.set {floating: false}`, as a tab double-click).
- **Geometry** of a float is in logical screen pixels, like the main window's. A float shown for the first time takes
  the kernel default (80, 80, 480 x 360) clamped to the screens.

## 2. Mirror store, events and resync per window

- Every window has its own mirror store over the same numbered engine events (`engine-event` goes to every webview).
  Each store follows the event numbers of the current engine generation and resyncs on a gap or on a new generation
  (ruling O-14), independently of the other windows. A float subscribes only to the slices its panel reads.
- The main window's startup behaviour, menus, shortcuts editor and dialogs live in the main window only. A float has
  no menu bar of its own; on macOS the application menu is global; on Windows the main window's menu applies when it is
  focused, and a float forwards its key presses to the same kernel dispatch (`dispatch_key`), so shortcuts work there
  too.

## 3. Reports from several windows

- **Owner per window.** The host keeps each window's sensor facts separately, by window label: modal state, text
  focus and the rendered UI tree with its event number. It forwards one combined report to the executor:
  - **modal**: a window's modal state (its own dialog or gesture) blocks agents like the main window's; the combined
    state is the most recent modal still open in any window, and ends when none is;
  - **text focus**: the focused window's;
  - **UI tree**: one tree, the main window's with every float's tree appended as a child `window` node
    (`id: float-<panelId>`), so `inspect_ui` reports one tree that includes floats; its event number is the smallest
    of the windows' numbers (the tree is fresh only when every window has caught up; C41-R1-14 rules apply to the
    combined value). The windows counted (round 2, C41-R2-06) are the main window, every float the host has opened
    and not seen destroyed (from the moment it opens it, before it reports), and every window that reported anything;
    one of them without a tree or without a number makes the combination unvouched (`renderedSeq` absent, and
    `inspect.ui.tree` answers `renderedSeq: null`); a closing float leaves the count at once.
- **A stale report never blocks forever (ruling O-9).** When a window is destroyed, reloaded or replaced (a float
  closed by reconciliation, the engine restart), the host drops that window's facts and forwards the recomputed
  combination. A report naming another engine generation is ignored (O-14).

## 4. Focus and the active view

- A float that gains OS focus runs `window.panel.set {panelId, focus: true}` (the kernel's focused panel; a floating
  view becomes the active view, so the mode follows it, 01 §4). Focus changes caused by reconciliation (opening or
  raising windows) are not reported back as gestures.

## 5. The main window and the floats

- **Raise all** (`window.raiseAll.run`) raises the main window and every float. **Minimise** (`window.minimized.set`)
  minimises the main window; the floats are hidden while it is minimised and shown again when it is restored (they
  are tool windows of the main window). **Full screen**: the full-screen arrangement has its own float list; the
  reconciliation shows it. **Closing the main window** runs the quit flow (unsaved-changes rule); quitting closes the
  floats with the application. **Toggle floating windows** (`window.floatingPanels.set`) hides or shows them all
  through the kernel.
- **One reconciliation at a time, from the current kernel state (round 2, C41-R2-10).** Every path that changes which
  float windows exist or are shown goes through the same reconciliation: a kernel state change, a workspace switch,
  the main window's placement (minimise, restore), Raise all, a restart and the start. Requests are numbered and
  coalesced: one pass runs at a time on its own thread; it reads `window.panel.list` for the newest request, and on
  the main thread applies it only if no newer request arrived meanwhile (a superseded snapshot is never applied; the
  next pass reads the kernel again). A pass shows exactly the floats the kernel wants (global visibility included: a
  float hidden by Toggle floating windows is not wanted, so its window closes) and hides them while the main window
  is minimised; nothing else shows a float window. Raise all is a flag the next pass honours for the floats it shows.
  The pure parts are tested with interleaved requests (`Coalescer`), and the planner with a globally hidden float.
- **Echo guard and geometry write-back (round 2).** The host's own changes (opening, showing, placing a window) are
  not reported back as gestures: for 400 ms after the host acts on a window its events are the host's own, and a move
  or resize that ends within 1 px of where the host placed it is too (`EchoGuard`, a pure value with the clock
  supplied). A gesture's geometry is sent once it has paused for 250 ms after its last event (a trailing debounce,
  `Debounce`): a long drag sends one `window.panel.set`.

## 6. Request cancellation and capabilities

- **Cancellation tokens are per window.** The host keys each in-flight request's flag by the window that sent it and
  the token; `cancel_request` from one window cannot cancel another window's request, even with the same token text.
- **Capabilities.** The float windows get the main window's capability set and nothing more (`core:event` listen and
  unlisten; the app's own commands): `capabilities/default.json` names the windows `main` and `float-*`.

## 7. Tests

- **Kernel** (`crates/executor/tests/c41/`): float geometry is clamped to the reported screens; floating, hidden and
  docked transitions; Toggle floating windows.
- **Host** (`tt-desktop` unit tests): the reconciliation plan (open, close, move) from a kernel arrangement and the
  open windows is a pure function; combining reports from several windows (modal, text focus, tree and its number);
  dropping a window's facts; per-window cancellation. Round 2: the freshness quorum includes every float the host
  opened before it reports (C41-R2-06); interleaved reconciliation requests (`Coalescer`); the echo guard and the
  trailing debounce on a supplied clock.
- **Front end** (`desktop/src`): the float route renders its panel and reports its tree; a float's geometry gesture
  runs `window.panel.set {geometry}`; its close runs `window.panel.set {visible: false}`.
- **GUI end to end on both OSes** (`scripts/c41-gui-e2e.py`): floating a panel by command opens an OS window of the
  application (counted by the OS) and the combined `inspect_ui` tree has its `float-<id>` node; a geometry change
  moves the OS window; docking it back closes the window; a restart restores a float.
- **Manual** (`reviews/manual/M4/C4.1-floating-windows.md`): floats across two monitors, scaling, minimise and raise,
  closing with the OS button, focus and the active view.
