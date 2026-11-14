# M4 C4.2b design note 2: the frame loop and the back ends

Before the code (plan §3.3, §4 item 2; ruling D-3: C4.2b). Requirements: 02 §4.2 (the nine layers of the canvas, back to
front), §10 (aids), §13.3 (the overlay of a gesture in progress); 06 §6.9 (antialiasing options, overlay buffering,
software path), §6.10; ADR-0007 decision 3 and the canvas rule set; ADR-0009 lines 70-72 (one input abstraction);
rulings D-5 (GPU quantities on the workstation with receipts, loosely on CI), D-7a (per-window frame loop), D-9.

## 1. One frame

`requestAnimationFrame` drives one frame per view window (D-7a):

1. **Apply deltas** received since the last frame, oldest first, until the queue is empty or the frame's delta budget
   is spent: at most 4 ms of decode and upload per frame, measured with `performance.now()`; work left over waits for
   the next frame (geometry already decoded is uploaded first, so a large resync spreads over frames while the
   previous revision stays visible, T1).
2. **Draw the static scene** in 02 §4.2's order, layers 1 to 8: background; grid (underlay); static light-table
   underlays; auto light table; onion skin; the current drawing; static light-table overlays; grid (overlay). Each
   resident drawing is drawn with the view matrix received in `canvas.view` (the canvas never computes a mapping,
   design note 6) and its colour slots.
3. **Draw the overlay** (layer 9): selection outline and handles, marquee or lasso, the gesture in progress, brush-size
   preview, cursor ghosts, invisible strokes and open ends when Show Strokes is on. With `overlayBuffering` on (06 §6.9,
   default) layers 1 to 8 are rendered to an offscreen target that is reused while only the overlay changes, so a
   gesture redraws one quad and the overlay.
4. **Acknowledge** the frame (`canvas.ack`), which lets the host send the next batch (design note 1 §4).

No step awaits IPC: messages arrive into a queue filled by the IPC callback and the frame only reads it.

## 2. Input

Pointer events (mouse, pen, touch) go through one input abstraction (D-9; ADR-0009 lines 70-72) producing the samples
of design note 3 §2: view point, pressure (`PointerEvent.pressure`, 0.5 for a mouse press is replaced by 1 as 02 §13.3
says), tilt, pointer kind (`pen` with the eraser button or `pointerType` eraser gives `penEraser`), buttons, modifiers
and `timeStamp`. The overlay draws the samples immediately; on release the samples go to the host in one
`gesture.commit` message, which calls the same `gesture::to_command` as the headless `pointer`, so both paths produce
the same command (G2). Escape, a lost capture or the window closing sends `gesture.cancel` (D-7c); nothing is sent
while the gesture is in progress except `gesture.begin` (the busy state of 11 §6.4). Coalesced events
(`getCoalescedEvents`) are delivered as samples so fast strokes keep every sample (G5).

## 3. Back ends

WebGPU and WebGL2 ship with the same features (ADR-0007 decision 3). Which one runs follows the canvas rule set's step 1
(the scorable back end with the higher worst-scene fps; a tie goes to WebGPU; a back end without a valid measurement is
never selected over one with): both are measured at first launch and after a GPU, driver or webview version change,
on the three rule-set scenes, for 2 s each (the spike used 10 s; the product measurement is shorter and repeated on the
next change if inconclusive); the result and the inputs are persisted in the user settings; a failed back end is
excluded until the next version change; the user can override it in preferences. The software path (06 §6.9) is the
webview's own WebGL2 on a software adapter (WARP, SwiftShader), which the Windows CI runner exercises.

Antialiasing follows 06 §6.9: MSAA when `interactiveAntialiasing` is on, analytic edge coverage for tool overlays when
`antialiasToolOverlays` is on.

## 4. Measurements (plan §6; D-5)

Frame time p95 over 300 frames idle and during a scripted gesture, input-to-ink, release-to-committed and the delta
queue length are recorded by the attached e2e on both CI OSes (loose bounds on CI: the runners have no GPU) and on the
workstation with receipts (the plan's bounds). The kernel-side holds stay in `c42.rs`.

## 5. Invariants (one builder test each)

- **F1** A frame's delta work stays within its budget (4 ms of decode and upload); leftover work is carried over, none
  is dropped (Vitest with a fake clock).
- **F2** No frame awaits IPC (the frame function is synchronous over the queue; a test with a stalled channel keeps
  drawing frames).
- **F3** The draw order is 02 §4.2's (a recording back end in Vitest lists the layers drawn).
- **F4** The back end is the rule set's output for the measured cells (the rule table's own Vitest cases, now run on
  the product selector).
- **F5** A cancelled gesture sends no command and leaves the overlay empty (D-7c).

## 6. Amendments after C4.2a (before the code)

Where this section and §1 to §5 differ, this section holds.

- **Gesture messages, concretely.** They reuse the paths C4.2a built, so equivalence holds by construction (G2, D-4):
  - **begin:** the C4.1 shell report with `modal {kind: "gesture", viewId}`. The kernel's mid-gesture user priority
    (ruling D-12) applies from that moment.
  - **commit:** the host command `gesture_commit {viewId, events}`. Its `events` are the `pointer` request's events, so
    `PointerRequest` is the one schema.
    - `dt` is the difference of consecutive `PointerEvent.timeStamp` values, rounded to whole milliseconds.
    - The pointer kind comes from `pointerType`, the pen's eraser button and `pointerType: "eraser"`.
    - A mouse press has pressure 1 (02 §13.3). A pen reports its own pressure; its hover samples are not sent.
    - Tilt is carried (SI-125).
    - The host runs the same gesture driver as `ExecutorHandle::pointer`, with the user's origin, and answers the
      `pointer` envelope.
  - **cancel:** the report with the modal cleared. No command is sent.
- **The attached `pointer` (P1, P2, P4).**
  - In attached mode, an agent's `pointer` call for a view shown in a window is routed to that window's input queue.
    The routing is a host message carrying the request id and the events.
  - The events are played through the same sample collection and overlay as the user's, with simulated time.
  - They are committed through `gesture_commit` with the agent's origin, and the envelope answers the MCP call.
  - The queue runs one gesture at a time, so P1 holds.
  - A user press during an agent gesture discards the agent's gesture with nothing committed, and its call answers
    the SI-135 refusal (P2).
  - While the user is mid-gesture, the agent's call waits up to `waitForUserMs` and then answers `ui.busy` (P4, the
    kernel rule of D-12).
  - A view not shown in any window keeps the headless path.
- **Back-end measurement in the product (SI-136).**
  - Each back end is measured on the rule set's three scenes for 2 s per cell, on an offscreen canvas, once after the
    first launch and after a GPU, driver or webview version change.
  - Until a result exists the canvas runs WebGL2, the mandatory path.
  - The selector is `desktop/src/canvas/backend.ts`. It applies step 1 of the rule set, and its table-driven Vitest
    cases are the rule table's own.
- **Overlay appearance** (selection outline and handles, marquee, gesture ink, cursor ghosts): SI-138.
- **Matrices.**
  - Every covered tool's gesture through the real window equals the headless `pointer` gesture and the direct command,
    in one table over all 18 tools of design note 3.
  - Every overlay kind is absent from `draw.drawing.render` and `render.frame.run` of the same state, in one table
    over the overlay kinds (06 §6.10).
  - Every view-dependent tolerance used by the window's gestures follows the announced zoom (already C4.2a's kernel
    rule; the window path joins the table).
- **Invariants added:**
  - F6 (D-7a, D-7c): a float closing mid-gesture, Escape, or a lost pointer capture leaves the document, the history and
    the selection untouched and the overlay empty.
  - F7 (D-7b): an engine restart mid-gesture discards the gesture, and the new generation reloads every resident
    drawing.
