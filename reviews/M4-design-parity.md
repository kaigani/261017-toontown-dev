# M4 C4.3b design note 9: the parity gate over every surface

Status: before code (C4.3b). Plan `reviews/M4-C4.3-plan.md` §3.6, part E; ruling D-25 ("the parity gate covers every
surface a person can act on: float windows, context menus, dialogs' buttons, the start screen, the status bar's
controls, canvas gestures (each tool's gesture as an action bound to its command or commands) and keyboard shortcuts;
the generated manifest is compared both ways in `check` and in Vitest; a GUI action with no declaration fails the
build. The parity note says how gestures are represented and how an action that sends several commands is
declared.").

## 1. What exists

11 P1 (line 19) and §14.1 (lines 560-563): the build fails on a GUI action without a binding or with a binding to an
unregistered id, and every command is reachable from a GUI location or declared `agentOnly` with a justification. C4.1
built this for the main menu (`desktop/src/ui-bindings/menu.json`). `crates/executor/tests/c12_registry.rs`
`ui_binding_manifest_parity` checks both directions, but its reverse direction accepts a command whose declaration
merely names a GUI location in prose (`gui = "..."`), whether or not a person can reach it. The toolbars, tool rail,
view chrome, panel actions, dialogs, start screen, status bar, floats, keymap and canvas gestures are in no manifest.
The front end marks its actionable elements with `data-tt-command` (and `data-tt-params`), not all of them.

## 2. The manifest

`desktop/src/ui-bindings/parity.json` (generated, never edited by hand; PA4). It has one entry per GUI action:

| Field | Meaning |
|---|---|
| `id` | the action's stable id: its element's `data-tt-id` (a button, menu item, control), `key:<chord>` for a shortcut, `gesture:<view type>:<tool>` for a canvas gesture |
| `surface` | `menu`, `toolbar`, `toolRail`, `toolOptions`, `viewChrome`, `panel`, `contextMenu`, `float`, `dialog`, `startScreen`, `statusBar`, `keymap`, `canvas` |
| `commands` | the command ids the action sends, in order: one for most actions; several for an action that sends a sequence (a gesture whose `tool` selects the tool first, then its command; a dialog's OK that sets and saves), each id once |
| `params` | the fixed parameters, when the action always sends the same ones (a menu item's), else absent |
| `when` | for a gesture, the condition that chooses among its commands (`click`, `drag`, `handle:<name>`, `modifier:<name>`) |

**Gestures** (D-25). A canvas gesture is an action of the canvas surface: one entry per (view type, tool), listing
every command its gesture can end in, each with its `when`. The table comes from the kernel's gesture driver (the
gestures note's tables: C4.2 tools, scene tools, the temporary overrides), dumped by a test-only Rust function, so the
manifest cannot claim a command the driver does not send, nor miss one it does.

**Several commands.** An action that sends several commands lists them all in `commands`. Each must be registered, and
each counts as bound.

## 3. How it is generated

`scripts/gen-parity-manifest.mjs` writes the manifest from four sources:
1. **The rendered front end.** In Vitest's DOM, the shell renders each surface: the main window with every panel, the
   start screen, each dialog and each float, the status bar, each context menu opened. The script collects every
   element a person can act on (a `button`, `menuitem`, `input`, `select`, an element with a click handler, a role of
   `button`/`menuitem`/`tab`/`checkbox`), and its `data-tt-command` and `data-tt-params`.
2. **The menu manifest** (`menu.json`), included as the `menu` surface.
3. **The keymap's defaults** (`keymap.list`'s default bindings), as the `keymap` surface.
4. **The gesture table** from the kernel (§2).

`--check` regenerates in memory and fails when the file differs (the `lint` job and a Vitest test run it).

## 4. The gate (both ways, twice)

- **Vitest** (`desktop/src/ui-bindings/parity.test.ts`):
  - **PA1:** every actionable element of every rendered surface is in the manifest with its commands. An actionable
    element without a `data-tt-command` fails the build, unless it is declared GUI-only (§5).
  - **PA4:** the manifest equals the generator's output.
- **`check`** (`crates/executor/tests/c43.rs`, replacing the reverse half of `ui_binding_manifest_parity` and keeping
  the C4.1 test as a subset):
  - **PA2:** every manifest command is registered and not an event; fixed `params` validate against its schema.
  - **PA3:** every implemented command is bound in the manifest, or declared `agentOnly` with a justification, or
    declares a `gui` location that a later milestone builds: its prose names an owner checkpoint after M4, and the
    merge note lists it. A `gui` declaration that names M4 or earlier and is not bound fails.

## 5. What is not a command (SI-155)

Some things a person can act on are not commands by design: moving and resizing a float window, dragging a dock
splitter, scrolling, a text field's typing before its form is submitted, hover, and focus. C4.1 declared some of them
as `window.*` commands, the geometry ones (01 §12). The manifest marks such an element `guiOnly` with a reason from a
fixed list: `geometry`, `scroll`, `typing`, `hover`, `focus`. A `guiOnly` element with any other reason fails. Window
geometry that ends in a `window.*` command is bound to it, not `guiOnly`. SI-155 records the list.

## 6. Invariants

PA1-PA4 above, each one test. The gate fails on a deliberately unbound button (a fixture surface in the Vitest test)
and on a manifest naming an unregistered id (a fixture manifest in the Rust test), so a gate that checked nothing would
fail its own test.
