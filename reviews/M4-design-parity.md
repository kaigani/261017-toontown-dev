# M4 C4.3b design note 9: the parity gate over every surface

Status: built (C4.3b); §2-§6 describe the gate as built, after rulings D-30 and D-31. Plan `reviews/M4-C4.3-plan.md` §3.6, part E; ruling D-25 ("the parity gate covers every
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

`desktop/src/ui-bindings/parity.json` is generated and never edited by hand (PA4). Its sections:

| Section | Source | Entry |
|---|---|---|
| `dom` | the rendered front end (§3) | `{id, surface, commands, params?}`: an element with a `data-tt-command` (`data-tt-params` gives `params`) |
| `guiOnly` | the rendered front end | `{id, surface, reason}`: an element declared `data-tt-gui-only` (§5) |
| `menu` | `menu.json` | `{id: menu:<path>, surface: menu, commands, params?}`: each command item |
| `keymap` | `crates/executor/src/keymap-default.json` | `{id: key:<chord>@<context>[platforms], commands, params?}`: each default binding |
| `toolbar` | `crates/executor/src/toolbar-default.json` (the kernel's default main toolbar, `ui::default_toolbar`) | `{id: toolbar:<item>, commands, params}`: each item through the front end's `itemRef` |
| `gestures` | `crates/executor/src/gesture-table.json` | `{view, tool, commands}`: one row per (view, gesture tool), every command its gestures end in |
| `uses` | the source of `desktop/src` and `desktop/src-tauri/src` (not tests, test support or generated code) | `{command, files}`: every quoted string shaped like a command id |

**Gestures** (D-25). A row lists every command the gestures of one tool in one view can end in: the Drawing and Camera
views' drawing tools, the Camera view's scene tools (`scene:select`, `scene:transform`, `scene:skew`; SI-148, SI-151),
and a `*` row per view for what any tool's gesture sends first (`tool.active.set`, `anim.tool.set`) and the wheel's
`view.zoom.set`. The table is data that the kernel test checks against the driver both ways (§4), so the manifest
cannot claim a command the driver does not send, nor miss one it does.

**Several commands.** An action that sends several commands lists them all in `commands`; each must be registered,
and each counts as bound.

## 3. How it is generated

`pnpm --dir desktop parity:gen` (`scripts/gen-parity-manifest.mjs`) runs `desktop/src/ui-bindings/parity.test.tsx` through
the pinned `test` script with `TT_PARITY_WRITE=1`. In Vitest's
DOM, with a scripted host, the test renders the start screen, the main window with a project (the default toolbar,
the tool palettes, the dock's panels and their view chrome, the status bar), and each dialog the menus open (About,
Project Info, Check Project, Open Backup, New Project, Project Properties, New Workspace, Preferences). The collector
(`desktop/src/ui-bindings/parity.ts`) takes every element a person can act on: a `button`, `input`, `select` or
`textarea`, an element whose role is `button`, `menuitem`, `tab`, `checkbox` or `option`, and an element whose
`data-tt-role` is `button`, `tab` or `field`. A field inside a `form` is `typing` (SI-155); a third-party wrapper
around a declared element belongs to that element and is not counted twice. Without `TT_PARITY_WRITE` the same test
compares (PA4).

## 4. The gate (both ways, twice)

- **Vitest** (`parity.test.tsx`, in the `desktop` job):
  - **PA1:** every actionable element of every rendered surface sends a command (`data-tt-command`) or is GUI-only
    for a listed reason (§5); any other fails.
  - **PA4:** the committed manifest equals the generator's output.
- **`check`** (`crates/executor/tests/c43/parity.rs`):
  - **PA2:** every command of the `dom`, `menu`, `keymap`, `toolbar` and `gestures` sections is registered and not an
    event. Each action's parameters are completed as the GUI runs them (`complete_reference`: a toggle the reference
    leaves open gets its next state) in a fixture with a project open, then validated against the schema. When
    required parameters are still missing, the GUI collects them with a dialog (01 §0), and the given ones are
    validated without `required`. A gesture's parameters are the driver's, checked by the battery.
  - **PA3** (rulings D-25, D-31): every implemented, non-event command is exactly one of
    - **bound:** an action sends it, or the source reads it (`uses`) and it is not declared agent-only;
    - **agent-only:** `agent_only = "<justification>"` in its declaration. A command declared agent-only that the
      source names is agent-only: the read is plumbing (the state stores, the host's tree and geometry reads). One
      that an action sends is in two states and fails;
    - **deferred:** listed in `desktop/src/ui-bindings/deferred.json` with `owner` (a checkpoint that has not passed:
      not M1-M3, not M4 C4.1-C4.3), `section` (the spec section that defines the location, D-30) and `location`.

    A command in no state, or in two or three, fails. So does a deferred entry whose owner has passed, that has no
    section, or that is not registered. The test prints the counts.
  - **Freshness:** the `keymap` section's commands are the kernel's defaults (`keymap::defaults`, Windows and macOS)
    both ways; the `toolbar` section is the kernel's default toolbar's command items both ways; the `gestures` section
    is the gesture table verbatim.
  - **The gesture table is the driver's:** every registered command named in `gesture.rs` or `scene_gesture.rs` is in
    the table. A battery on fresh fixtures then makes every drawing tool's G10 gesture in both views (the points mapped
    into the Camera view through Bob's plane and the camera), the drawing Select's move and pivot drags, the Zoom and
    Grabber drags, the wheel, and the scene tools' click, drag and alternate drag. Each row of the table must equal the
    commands its gestures produced, and the battery must cover every row. The battery found SI-156 (the Camera view's
    drawing Select handles).

`crates/executor/tests/c12_r1_contracts.rs` is a reviewer test, kept as committed. It still checks that every entry
declares a GUI location or an agent-only justification; PA3 is the stronger check and supersedes its reverse half.

## 5. What is not a command (SI-155)

Some things a person can act on are not commands by design. The manifest marks such an element `guiOnly` with a reason
from a fixed list:

| Reason | What |
|---|---|
| `geometry` | moving or resizing a float, dragging a dock splitter (geometry that ends in a `window.*` command is bound to it) |
| `scroll` | scrolling a list or a view's scroll bars |
| `typing` | a field's typing before its form is submitted (automatic for a field inside a `form`) |
| `hover` | hover feedback |
| `focus` | moving focus |
| `dismiss` | a dialog's Cancel or close: nothing changes |
| `navigate` | moving between the pages of one dialog (Preferences' categories) |
| `placeholder` | a control 01 places whose function a later checkpoint builds, shown disabled with its owner in the tooltip (the start screen's sample project, M9 C9.3) |

A `guiOnly` element with any other reason fails PA1.

## 6. Invariants and their failing fixtures

PA1-PA4, freshness and the gesture table are each a test. Each check fails on a fixture built to break it, in the
same test:
- PA2: a menu action naming an unregistered id, one naming an event, one with a parameter outside its enum;
- PA3: a command removed from `deferred.json` (no state), a bound command added to it (two states), an agent-only
  command that a menu sends (two states), an owner that has passed (M4 C4.2), an entry without a section;
- the gesture battery and the SI-156 test fail with the Camera view's handle mapping disabled (checked by hand).

PA1's own failing case is the collector's: an actionable element with neither attribute is listed as unbound, and the
test's `expect(unbound).toEqual([])` fails.
