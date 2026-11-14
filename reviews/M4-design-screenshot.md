# M4 C4.2 design note 5: `screenshot`

Before the code (plan §4; rulings D-3, D-6: the headless half in C4.2a, attached capture in C4.2b). Requirements:
11 tool `screenshot` (line 335), §15.2 (`inspect.screenshot`), §6.5 (inline caps 2048 px, 8 MB), §3 lines 88-90
(headless renders use the software rasteriser); 06 §6.10.

## 1. Headless (C4.2a)

`inspect.screenshot {target?, region?}` renders the target virtual view through the final software rasteriser at its
virtual size: the Drawing view is the current drawing through the view's mapping (design note 6) on the Drawing view
background, the same pixels `draw.drawing.render` produces for that drawing and region. `target` is `window` (the active
view, else the Drawing view), a view id or a panel id of a view; any other panel or `dialogs` refuses with
`view.notFound` headless (there is no virtual chrome to capture). The display aids (grid, onion skin, light tables) are
drawn by the interactive renderer only (06 §6.10): headless screenshots show the drawing, and the receipt says so.

### 1a. Which thread does what (C42A-R1-02)

- **Document thread** (the handler, a few hundred microseconds): resolves the target and the region, checks the side
  cap, reads the view mapping and the Drawing-view background, and takes the snapshot: the document by reference count,
  the current drawing's id, the planar cache's handle and the blob store. It submits the capture as a job and answers
  `{…, jobId}` with `png` and `sha256` still `null`.
- **Job thread:** rasterises through the final software rasteriser (`draw_render::rasterise_with`, shared with
  `draw.drawing.render`, S4), encodes the PNG, checks the byte cap and stores the finished result. The job's
  cancellation stops the rasteriser between work slices.
- **Caller's thread** (`ExecutorHandle::serve`): waits for the job as `wait: true` does for render jobs, bounded by the
  call's watchdog budget and its cancellation, and answers the finished result (with the job's id). Inside a batch the
  step answers the pending result and the job id (`job.wait` collects it).

### 1c. Bound and cancellation (round 2)

- **At most two captures run at once in one executor** (`inspect::MAX_CAPTURES`); each holds one job thread from its
  submission until its PNG is encoded. A further capture **waits for a slot on its caller's own thread** (no thread is
  added; `ExecutorHandle::serve`, before the request is queued), bounded by the call's budget and its cancellation; at
  most **eight** callers wait (`host::MAX_WAITING_CAPTURES`). One more caller, or a wait that reaches the budget,
  answers `limit.exceeded` with `details: {limit: "concurrentCaptures", max: 2, maxWaiting: 8, running, waiting}`
  before anything is snapshotted or started. The handler also refuses when two capture jobs are still running (a
  cancelled capture finishing in the background, below), so the job threads never exceed two. The isolated test
  `c42_isolated::concurrent_captures_are_capped_and_their_threads_counted` races fourteen callers and counts the
  process's OS threads while they run. The reviewer's `concurrent_captures_finish_and_budget_is_bounded` (six
  callers, all answered) holds because six is within two running plus eight waiting.
- **Waiting** honours the waiting call's own budget and cancellation: a cancelled waiter answers `command.timeout`
  (`reason: "cancelled"`) and a waiter at its budget answers `limit.exceeded`, and neither starts a capture afterwards
  (`c42_isolated::a_waiting_capture_honours_its_cancellation_and_budget_and_starts_nothing_afterwards`). **Waiters are
  not served in arrival order:** a freed slot goes to whichever waiting caller the condition variable wakes first.
- **A capture job that panics** ends `failed` with a typed `internal` error (the job runner catches the panic, for
  every job), so it never keeps a slot (`c42_isolated::a_panicking_capture_job_releases_its_slot`); the caller's slot is
  released by its guard on any return or unwind.
- **A cancelled request does not stop its capture:** ADR-0020 decision 8 lets a job continue after its request is
  cancelled, so the capture finishes in the background (its job's cancellation stops the rasteriser between work
  slices only when the job itself is cancelled); its thread counts against the bound until it ends.

### 1b. Regions and caps

A `region` is a rectangle of the view's pixel plane: it is not clipped to the window, so a region outside the window
shows what the canvas would show if it extended there (the drawing through the same mapping). The side cap (2048 px)
is checked from the requested size before anything is rendered (S2); the 8 MB byte cap can only be known after
encoding and is checked then (ADR-0020 decision 8 permits this for encoded-byte limits).

## 2. Attached (C4.2b, D-6)

A capture of what the user sees, through the host: Windows, `ICoreWebView2::CapturePreview` on the window's webview;
macOS, `WKWebView.takeSnapshot(with:)`; for a float window, that window's webview. It includes the canvas, the chrome,
an open dialog and the gesture overlay. A test opens a dialog and holds a gesture overlay and finds both in the PNG.

## 3. Result and limits

`{png (base64), width, height, sha256}`; `region {x, y, w, h}` in the target's view pixels crops before encoding. The
inline caps are checked before encoding: a long side over 2048 px or a PNG over 8 MB refuses with
`render.inlineTooLarge` (no file output exists for screenshots).

## 4. Invariants (one builder test each)

- **S1** The PNG decodes, has the stated width and height, and its sha256 matches.
- **S2** The caps are enforced before encoding (a target larger than 2048 px refuses without rendering).
- **S3** Headless and attached take the same parameters and answer the same shape (wire tests through real
  serialisation; attached in C4.2b).
- **S4** Headless pixels equal `draw.drawing.render` of the same drawing, mapping and size (one rasteriser).
- **S5** A capture never holds the document thread beyond the hold budget (plan §6: 20 ms); a user query during a
  capture answers within it (C42A-R1-02).
