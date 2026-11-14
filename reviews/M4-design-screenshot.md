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
