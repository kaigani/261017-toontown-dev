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
