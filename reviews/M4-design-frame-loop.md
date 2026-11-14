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
