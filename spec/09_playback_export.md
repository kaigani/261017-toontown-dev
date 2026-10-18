# 09 — Playback, Preview, Import, Export, Capture and Printing

This section specifies everything that moves frames **out of** the application — interactive playback in the working
views, rendered previews, movie / image-sequence / snapshot / PDF export, publishing to video-sharing services, and
printing — and everything that brings media **into** it: general file import and its per-format behaviour, keyed image
import, import-and-vectorize entry points, and image acquisition from cameras and scanners for stop-motion. It also
defines the agent-facing `playback`, `render`, `import`, `export`, `capture` and `print` command namespaces.

Related sections:

- `01_application_shell.md`: main window, preferences, project properties (frame rate, camera size), command binding.
- `02_drawing.md`: vectorization engine and its settings; drawing layers; onion skin and light table in the Drawing View.
- `03_colour.md`: palettes, swatches, colour identity (relevant to vector import/export and palette round-trip).
- `04_timeline_xsheet.md`: elements, cells, exposures, selection, the Timeline ruler and Exposure Sheet.
- `05_scene_planning.md`: cameras, pegs, keyframes (the camera composition).
- `06_effects.md`: effects, the render pipeline and the effect preview window.
- `07_sound_lipsync.md`: sound elements, mixing, scrubbing internals, sound import details, media (video) elements.
- `08_library_storyline.md`: library imports, storyline order, scenes and transitions.
- `10_data_model_project_format.md`: the project package (`.animpkg`) and where imported media is stored.
- `11_agent_interface_mcp.md`: generic tools (jobs, rendering, permissions/confirmation scopes); `A_command_registry.md`.

---

## 1. Architecture overview (functional)

### 1.1 Output paths

The application has five distinct output paths. They MUST be kept separate because they have different quality,
timing and determinism guarantees.

| Path | Purpose | Quality / timing | Output |
|---|---|---|---|
| **Interactive playback** | Plays or steps the current scene in the working views in real time; moves the current frame. | Live view renderer; may drop frames (§2.7). Not deterministic. | Screen only |
| **Rendered preview** | Renders a range at final quality to a temporary movie, then plays it in a built-in player at true speed. | Final quality; plays at the exact frame rate. | Temporary file + player window |
| **Export** | Renders a chosen scope and range to files with a chosen format, optionally publishes. | Final quality; deterministic. | Movie, image sequence, still image, PDF, web page, upload |
| **Print** | Lays rendered frames or drawings out on pages. | Final quality. | Printer, PDF or print file |
| **Deterministic render (agent)** | Renders a single frame or a range on demand for verification or downstream use. | Final (or draft) quality; byte-reproducible. | PNG inline or files |

The effect preview window (single-frame / range software render inside a docked window with a render cache) is owned
by `06_effects.md`; it consumes the playback range defined here.

### 1.2 Compositions

Every render-producing feature (preview, export, snapshot, print, agent render) composes a frame in one of these ways:

- **Camera composition** — the scene as seen through the active camera, in Timeline order, with pegs, camera motion
  and effects applied. This is what "Preview Scene" and "Current Scene: Camera View" produce.
- **Exposure-sheet composition** — the scene's visible elements composited in Exposure Sheet column order in drawing
  mode, without camera or peg motion. This is what "Preview Exposure Sheet" and "Current Scene: Drawing View" produce.
- **Single element** — only one selected element (column), drawing by drawing, as seen in the Drawing View
  (used by print and agent render).

### 1.3 Ranges

All exporters, previews and printing share one range model ("scene export iterator"). It MUST support:

- the **full movie** — all scenes in storyline order, either **with** storyline transitions rendered or **without**
  (straight cuts);
- the **current scene**, in camera composition or exposure-sheet composition;
- a **sub-range** of the current scene: an explicit inclusive frame interval, the current selection, or the scene's
  free playback range (§2.8).

### 1.4 Input paths

- **General file import** takes a list of files, classifies each one (bitmap image, layered image, vector art/PDF,
  vector animation, movie, sound, template), and inserts it at the current frame and element (§8).
- **Image acquisition** takes images from cameras, webcams, video devices and scanners, can key and optionally
  vectorize them, and inserts them as cells, one per frame (§9).

---

## 2. Interactive playback

### 2.1 Purpose and scope

Interactive playback plays the current scene inside every open working view that shows the current frame — Drawing
View, Camera View, Top View, Side View — and moves the playheads of the Timeline and Exposure Sheet in step. It does not
render to a file. Frame-stepping commands navigate the current frame within the playback range.

### 2.2 Commands and where they are reachable

The following commands MUST exist. They MUST be reachable from the main menu; the transport commands (first, previous,
play/stop, next, last, loop) MUST also be available as a playback control strip that can be shown in the main window
toolbar area (which items are visible by default is design work; previous/next frame MUST be addable). The playback
range commands, change-current-frame, and the sound playback/scrubbing toggles MUST also be reachable from the
Timeline ruler's context menu. Storyline playback MUST also be reachable from the Storyline view.

| Command | Behaviour summary |
|---|---|
| Play / Stop | Toggle: start playing from the current frame, or stop. |
| First / Previous / Next / Last Frame | Navigate within the playback range. |
| Previous / Next Frame (global) | Frame −1 / +1, usable from any view. |
| Previous / Next Drawing | Jump to the previous / next drawing change (exposure boundary). |
| Change Current Frame | Enter a frame number and jump to it. |
| Loop | Toggle wrap-around playback. |
| Play Storyline | Toggle playback across scenes with transitions. |
| Playback rate | Choose 1, 2, 12, 24 or 30 fps, or "use project frame rate". |
| Force Frame Rate | Toggle frame dropping to keep real time. |
| Playback Range: Automatic Fit / Free | Range mode. |
| Change Playback Start / End Frame | Numeric entry of the free range ends (enabled in Free mode only). |
| Sound Playback / Sound Scrubbing | Toggles (§2.9). |
| Preview Movie / Scene / Exposure Sheet / Exposure Sheet Selection / Quick Preview | Rendered previews (§3). |

Shortcuts: Play/Stop, single-frame stepping and previous/next drawing MUST be bindable to single keys (no modifier) and
SHOULD ship with single-key defaults; Quick Preview SHOULD have a default binding. The specific default map is design
work. All commands are rebindable (`01_application_shell.md`).

### 2.3 Play / Stop

- If stopped, Play starts from the current frame. If the current frame is outside the playback range, playback starts
  at the range start.
- If playing, Stop halts playback and leaves the current frame where playback stopped.
- Playback plays the frames of the current scene's **playback range** (§2.8).
- At the end of the range: with Loop off, playback stops on the last frame of the range; with Loop on, it wraps to the
  first frame and continues until stopped.
- A timer advances one frame per tick at the selected playback rate (§2.6); Force Frame Rate (§2.7) decides whether
  frames are dropped when rendering falls behind.
- If Sound Playback is on, all audible (visible, not muted) sound elements of the scene are mixed and played in sync
  from the current frame (§2.9).
- While playing, the Play/Stop control shows its "stop" state (checked menu item, stop icon).
- Starting playback MUST **pause the live preview** of an open capture session (§9.9); stopping playback resumes it.

### 2.4 Frame navigation

| Command | Behaviour |
|---|---|
| First Frame | Current frame := first frame of the playback range. |
| Previous Frame | Current frame − 1, not below the range start. |
| Next Frame | Current frame + 1, not above the range end. |
| Last Frame | Current frame := last frame of the playback range. |
| Previous / Next Frame (global) | Current frame ∓ 1. These are the commands bound to the single-key stepping shortcuts; there MUST be only one binding per direction. |
| Previous / Next Drawing | Move to the previous / next frame at which the drawing exposed in the current (or selected) element changes. If no element is selected, use the nearest change across all visible elements. |
| Change Current Frame | Accepts an integer frame number and jumps there. |

Rules:

- Previous at the first frame and Next at the last frame do nothing; they never wrap, even with Loop on.
- Using a navigation command while playing stops playback first.
- Change Current Frame clamps values outside 1..scene length; non-numeric input is rejected (`playback.invalidFrame`).

### 2.5 Loop and storyline playback

- **Loop** (toggle, default off): playback wraps from the last frame of the range to the first and never stops by itself.
  The state persists for the session.
- **Play Storyline** (toggle): plays across all scenes in storyline order **with transitions rendered** (dissolves,
  wipes and the other transition types of `08_library_storyline.md`). Playback starts at the Storyline view's cursor
  position. Invoking it again stops playback. During a transition, frames from the outgoing and incoming scenes are
  composited according to the transition's type and timing. Storyline playback and transition compositing are owned
  by `08_library_storyline.md` (commands `storyline.play.start` / `storyline.play.stop`, frame read-back
  `storyline.frame.render`); this section only places the command in the playback controls. Loop, playback rate,
  Force Frame Rate and Sound Playback apply to storyline playback as well.

### 2.6 Playback rate

| Choice | Effect |
|---|---|
| 1, 2, 12, 24, 30 fps | Sets the **interactive playback speed only**. It does not change the project frame rate and does not affect previews, export or printing. |
| Use project frame rate | Plays at the frame rate set in the project properties. |

Rules: the choices form a radio group with the current choice indicated; default is "use project frame rate"; the
choice remains until changed and is persisted as a user preference. The command layer additionally accepts any rate in
1–120 fps (§11).

### 2.7 Force Frame Rate

Toggle, default off.

- **On**: playback is time-based. When displaying a frame takes longer than one frame period, frames are skipped so the
  playhead stays in real time. Displayed frames may jump; sound stays in sync.
- **Off**: playback is frame-based. Every frame in the range is shown; if rendering is slow, playback runs slower than
  the selected rate. With sound on, sound may drift or restart per frame.

### 2.8 Playback range

Each scene has a playback range: a mode plus a (start, end) frame pair. It is saved in the project.

| Mode / command | Behaviour |
|---|---|
| **Automatic Fit** (default for new scenes) | The range is always frame 1 to the scene's last frame and follows changes in scene length. |
| **Free** | User-defined range. Two **range markers** appear on the Timeline ruler at the start and end; dragging them sets the ends. Example: a 60-frame scene played only over frames 20–30. |
| Change Playback Start Frame / End Frame | Enabled only in Free mode. Numeric entry of the start or end frame. |

Validation:

- 1 ≤ start ≤ end ≤ scene length.
- A new start greater than the end moves the end to the start; a new end smaller than the start moves the start to the end.
- If the scene becomes shorter than the free end, the end is clamped to the new length.
- Switching to Automatic Fit hides the markers; the last free values are remembered and restored on the next switch to Free.

Consumers of the range: first/last/previous/next frame, play and loop, the export "free playback range" option
(§4.7), the print "free playback range" option (§7), and the effect preview's "render playback range" (`06_effects.md`).

### 2.9 Sound during playback (summary; details in `07_sound_lipsync.md`)

| Toggle | Behaviour |
|---|---|
| **Sound Playback** | Enables sound during interactive playback. When off, playback is silent. |
| **Sound Scrubbing** | Dragging the Timeline playhead plays short slices of sound under the playhead. **Requires Sound Playback to be on**; it is shown disabled while Sound Playback is off. |

- Stepping frame by frame with sound on plays one frame's worth of audio ("frame mode").
- Scrubbing plays a short window around the scrub position.
- The mixer combines every sound element that is visible and not muted. Hidden elements are neither played nor rendered.
- If no audio output is available or the audio device cannot be started, playback continues silently and the user is
  informed once (`playback.audioUnavailable`, `playback.audioDeviceFailed`, with the device's error detail).

### 2.10 Edge cases

- **Editing during playback** is allowed; the views show edits on the next pass. Starting a drawing stroke MAY stop playback.
- **One-frame range**: Play shows that frame; with Loop on it repeats with no visible motion.
- **Scene switch**: changing the current scene stops playback.
- **Hidden elements** (hidden via show/hide toggles or hidden in the Exposure Sheet) are not drawn in playback,
  previews, export or print.

---

## 3. Rendered previews

### 3.1 Purpose

Preview commands render a range at final quality to a temporary movie and then play it at true speed in the built-in
preview player, so the user can judge timing regardless of how heavy the scene is in the live views.

### 3.2 Preview commands

| Command | What is rendered | Composition |
|---|---|---|
| **Preview Movie** | All visible elements of **all scenes**, concatenated in storyline order into one movie, with transitions. | Camera composition. |
| **Preview Scene** | The current scene including all scene-planning work (camera, pegs, effects). | Camera composition through the active camera. |
| **Preview Exposure Sheet** | All visible elements of the current scene. | Exposure-sheet composition (no camera or peg motion). |
| **Preview Exposure Sheet Selection** | Only the selected visible elements (columns or cells) of the current scene; frame span = the selection's frames. | Exposure-sheet composition. |
| **Quick Preview** | The current scene as seen from the view the user is working in. | Drawing View active → exposure-sheet composition (as Preview Exposure Sheet). Camera, Top or Side View active → camera composition (as Preview Scene). |

Enabling: each command is disabled when it has nothing to render — empty project or scene, no selection (Selection
variant), or (Quick Preview) keyboard focus in a window that is not a working view.

### 3.3 Behaviour

1. **Render.** A progress indicator shows the number of frames being rendered and allows cancel. The preview uses
   the project frame rate (not the interactive playback rate), the camera size from the project properties, and
   includes the soundtrack if the range has audible sound elements.
2. **Store.** The result is written to a temporary location managed by the application. A new preview replaces the
   previous temporary file. Temporary previews are deleted on exit.
3. **Play.** The preview opens in the built-in preview player (§3.4), which autoplays it.

Errors: nothing to render (`render.nothingToRender`), not enough memory (`render.outOfMemory`), preview could not be
loaded (`playback.previewUnavailable`).

### 3.4 Preview player

A single built-in player window plays previews and, when requested, exported movies. It MUST NOT depend on an external
browser or third-party plug-in. Controls:

| Control | Behaviour |
|---|---|
| Movie area | Shows the movie at its aspect ratio, scaled to fit the window. Autoplays on open. |
| Position slider | Range = first..last frame. While stopped, dragging seeks to that frame; while playing, the slider follows the playhead. |
| First / Previous / Next / Last | Seek to the first frame, one frame back, one frame forward, the last frame. |
| Play / Pause | Toggle. The icon shows the action available (pause while playing). |
| Loop | Toggle, default **off**. Off: playback stops at the end and the control returns to Play. On: wraps to the first frame. |
| Frame counter | "current / total", 1-based. Shows a loading state until the movie is ready. |
| Time display | Current time and total duration as h:mm:ss.ss. |
| Volume, Mute | Volume slider (0–100 %) and mute toggle (default unmuted). |
| Reload | Re-opens the file (for example after re-rendering). |
| Size report | For vector-format outputs (§4.5): shows / hides the size report panel. |
| Close | Closes the player. |

---

## 4. Export movie

### 4.1 Purpose and entry

Export Movie renders the animation or part of it to a movie, animated image or image sequence, and optionally performs
publishing operations. It is available from the main menu whenever a project is open. If the chosen scope has no
frames, export reports `export.nothingToExport`.

### 4.2 Export parameters

The export dialog is one client of the `export.movie.run` command; its layout is design work. It MUST expose:

| Parameter | Values | Default | Rules |
|---|---|---|---|
| **Output path** | absolute file path (with a browse action using the native save dialog, filtered to the selected format) | last used path; else the user's documents folder plus a default name derived from the project name and format extension | A name without an extension gets the format's extension. An invalid path is rejected (`export.invalidPath`). |
| **Format** | the format list (§4.3) | last used; else MP4 (H.264) | Changing the format refreshes the settings presets, swaps the extension in the path, and updates which settings, ranges and publishing options are available. |
| **Settings preset** | **Default**, **Custom**, plus one preset per configured publishing service where the format supports it (§4.9) | Default | Default = factory settings for the format, adapted to project frame rate and camera size. Custom = the settings last edited for that format. A service preset = settings that meet that service's requirements, and enables publishing to it. |
| **Edit settings** | opens the format's settings (§4.4–4.6) | — | Accepting edited settings switches the preset to **Custom**. |
| **Export type** | Full Movie; Current Scene: Camera View; Current Scene: Drawing View | Full Movie | §4.7. |
| **Export range** | All (with transitions); All without transitions; Frames from … to …; Selection; Free Playback Range | All without transitions | §4.7. The from/to fields are enabled only for the frame-interval choice and are validated on edit and on focus loss. |
| **Open in player after export** | on/off | on (persisted) | After a successful export, opens the result in the preview player (§3.4); image sequences open their first frame. |
| **Publishing options** | §4.8 | — | — |

All export parameters are persisted per user and restored when the dialog opens.

### 4.3 Export formats

| Format | Extension | Status | Notes |
|---|---|---|---|
| **MP4, H.264 video + AAC audio** | `.mp4` | **MUST** (baseline) | Default format. Settings §4.4. |
| **MOV movie** | `.mov` | MUST | Codecs: H.264 (MUST); an alpha-capable codec — PNG-in-MOV or ProRes 4444 (SHOULD). Settings §4.4. |
| **Animated GIF** | `.gif` | SHOULD | Palette-reduced; settings: colours (2–256), dithering on/off, loop count (0 = forever), transparency on/off. No audio. |
| **Image sequences**: PNG, JPEG, TGA, BMP (MUST); TIFF (SHOULD); others MAY | per format | MUST | One numbered file per frame (§4.6). |
| **Vector animation (SWF)** | `.swf` | MAY (legacy, optional) | Settings §4.5. |
| Other movie containers (e.g. WebM/VP9, AVI) | per format | MAY | Settings §4.4. |

The set of formats is discoverable at run time (`export.formats.list`), with each format's capabilities (video, audio,
alpha, vector) and settings schema.

### 4.4 Video settings (MP4, MOV and other movie formats)

| Setting | Default | Range / values |
|---|---|---|
| Codec | H.264 | per container |
| Quality | high (≈ CRF 18 equivalent) | low / medium / high / lossless where supported, or numeric 0–100 |
| Bit-rate mode | quality-based | quality-based, or target bit rate (kbit/s) |
| Key-frame interval | every 2 s worth of frames | 1–600 frames |
| Frame size | camera size from project properties | width/height in pixels; "keep aspect" on by default; scale presets 25 %, 50 %, 100 %, 200 % |
| Frame rate | project frame rate | 1–120 fps (override; the timeline is resampled by frame duplication/dropping) |
| Pixel format / alpha | no alpha | alpha only for alpha-capable codecs |
| Audio | on if the range has audible sound | on/off; AAC (MP4) or PCM/AAC (MOV); sample rate 44.1 / 48 kHz (default 48); channels mono/stereo (default stereo); bit rate (default 192 kbit/s) |
| Web-optimised ("fast start") | on | on/off — index placed at the file start for progressive download |

Rules:

- The soundtrack is a stereo mix of all audible sound elements in range. Formats without an audio track disable the
  audio settings. Audio across storyline transitions is cross-faded or concatenated along with the picture.
- Multi-pass encoders show their pass in the progress indicator.
- Rendered frames MAY be cached on disk before encoding; the cache MUST be deleted afterwards.
- Settings are stored per format and restored next time (they form the "Custom" preset).

### 4.5 Vector animation settings (optional SWF format)

If the vector animation format is implemented:

| Setting | Default | Range | Effect |
|---|---|---|---|
| **Protect from import** | off | — | Sets the format's "protected" flag so editing tools — including this application — refuse to import the file. |
| **Include preloader** | off | — | Adds a preloader sequence that waits until the movie has loaded before playing. |
| **Linked templates as reusable clips** | off | — | Exports the content of each linked-template (media) element once as a reusable clip and places instances of it, instead of expanding every frame. Elements that cannot be clips are expanded normally. |
| **Embed palette information** | off | — | Embeds swatch, style and palette metadata so that re-importing several such files rebuilds one shared palette and keeps swatch identity for recolouring (§8.6). |
| **Compress** | **on** | — | Writes the compressed variant of the format. |
| **Bitmap quality** | 80 | 0–100 (out-of-range input clamped) | JPEG quality for embedded bitmaps (image elements, textures). |

Behaviour:

- Frame rate = project frame rate; frame size = camera size; background = scene background colour.
- Vector drawings become shapes: pencil lines as strokes, brush strokes and painted zones as fills. Stroke order is
  optimised. Identical drawings are stored once and reused.
- Bitmaps and textures are stored as JPEG at the chosen quality.
- Scenes are concatenated; with "All (with transitions)", transitions are rendered by compositing the two scenes.
- Effects the format cannot express natively are rasterised or converted: particle effects become per-frame placements;
  drop shadows and blurs become the format's native filters.
- Sound elements become event sounds placed at their start frames; elements marked "streamed" in the sound element
  settings become one stream sound (`07_sound_lipsync.md`).
- A **size report** is produced: file name, total size in bytes, camera size, frame rate, then the byte cost of every
  exported item grouped by kind (drawings, bitmaps, sounds, reusable clips), sorted by size. It is shown in the preview
  player and is available through `export.report.get`.

### 4.6 Image sequence settings

- Naming: `<name><nnnn>.<ext>`, frame number zero-padded to 4 digits by default (padding 1–8 configurable), numbered
  from the first exported frame; an option chooses whether numbering uses the source frame numbers or starts at 1
  (default: source frame numbers).
- Per-format options: JPEG quality 0–100 (default 90); PNG compression level; colour depth (8-bit RGB, 8-bit RGBA).
- Formats that support alpha keep transparency when RGBA is chosen: areas with no content are transparent; otherwise
  they are filled with the scene background colour.

### 4.7 Export type and range semantics

| Export type | What is rendered |
|---|---|
| **Full Movie** | Every scene in storyline order; all visible elements from the first to the last frame of each scene, including effects. |
| **Current Scene: Camera View** | The current scene in camera composition (same as Preview Scene). |
| **Current Scene: Drawing View** | The current scene in exposure-sheet composition, with no camera motion. Renders all visible exposure-sheet elements; to export a single column use the Selection range. Useful for motion tests or to hand one element to another program. |

| Export range | Meaning |
|---|---|
| **All (with transitions)** | All frames, including storyline transitions between scenes. |
| **All without transitions** (default) | All frames; scenes cut straight with no transitions. |
| **Frames from / to** | Inclusive, 1-based interval. For Full Movie the numbers are global movie frames; for current-scene types they are scene frames. |
| **Selection** | The frames — and, for Drawing View type, the elements — selected in the Timeline or Exposure Sheet. |
| **Free Playback Range** | The current scene's free playback range (§2.8). |

Enabling rules:

- **Full Movie**: only All (with transitions) and All without transitions are enabled.
- **Current-scene types**: All, Frames from/to and Free Playback Range are enabled (All behaves as All without
  transitions, since a single scene has none); Selection is enabled only if a selection exists; Free Playback Range only
  if the scene's playback range mode is Free.
- The from/to fields are initialised to 1 and the scene length.

Validation: 1 ≤ from ≤ to ≤ length; invalid values are refused (`export.invalidRange`). If nothing is left to export:
`export.nothingToExport`.

### 4.8 Publishing options

| Option | Default | Effect |
|---|---|---|
| **Write HTML page** | off (persisted) | Writes `<moviename>.html` next to the exported movie, embedding it with standard HTML video (or the vector player for SWF), sized to the movie, autoplay off, controls on, loop on. The page carries no application branding. Available for MP4, MOV, WebM, GIF and SWF outputs. Write failure: `export.htmlWriteFailed`. |
| **Add to media library** | off (persisted) | MAY: after export, adds the movie to the platform's media library where one exists. Unavailable formats or platforms hide the option. Failure: `export.mediaLibraryUnavailable`. |
| **Open in player after export** | mirrors §4.2 | Same flag as §4.2. |
| **Publish to service** | via service preset | §4.9. |

### 4.9 Publishing to video-sharing services

The application MUST support publishing an exported movie to **configurable video-sharing services**. Services are
defined by configuration (a service descriptor: id, display name, authorisation endpoint, upload endpoint, accepted
formats, limits, metadata fields and category source); none is hard-wired.

- **Rendering.** Selecting a service preset renders an MP4 (H.264/AAC) with settings suited to that service, then uploads it.
- **Pre-flight checks** against the service's limits, before rendering where possible and again on the produced file:
  minimum duration, maximum duration, maximum file size, and minimum frame count = ceil(minimum duration × fps).
  Each failure reports the limit and the actual value (`publish.tooShort`, `publish.tooLong`, `publish.tooLarge`,
  `publish.tooFewFrames`).
- **Metadata.** Title (required; default = project name), description (required if the service requires it), category
  (chosen from the list the service provides; a waiting state is shown while the list loads), keywords/tags (optional,
  comma or space separated), visibility (public / unlisted / private where supported; default private).
- **Authorisation.** Sign-in uses the service's standard delegated authorisation flow in the system browser; the
  application never asks for or stores the service password. Tokens are stored in the operating system's credential
  store. A "remember sign-in" option (default off) keeps the token between sessions; otherwise it is discarded on exit.
- **Upload** shows progress (0–100 %) with cancel; cancel aborts the upload. On success the user is told the upload
  completed and, where the service returns one, given the video's URL.
- **Errors:** `publish.authFailed`, `publish.serviceError` (with service detail), `publish.accountRestricted`,
  `publish.network.timeout`, `publish.network.hostNotFound`, `publish.network.tlsFailed`, `publish.network.notFound`,
  `publish.cancelled`, `publish.network.unknown`.
- Publishing is an **outward action**: through the command layer it requires explicit confirmation (§11.1).

### 4.10 Execution, progress and messages

Order of operations:

1. Validate the output path (`export.invalidPath`). If the file exists, apply the overwrite policy — in the GUI, ask
   whether to replace it; via commands, the `onConflict` parameter (§11).
2. Build the frame list for the chosen type and range; if empty, `export.nothingToExport`.
3. Render with a progress indicator (frames done / total, current phase). Export can be cancelled
   (`export.cancelled`); after a cancel, partially written files are deleted.
4. Mix and attach the soundtrack (movie formats) or embed sounds (vector format).
5. Post-operations in order: HTML page, media library, publishing, open in player.
6. Report success, or an error such as `export.outOfMemory`, `export.fileInUse` (the file might be in use by another
   application), `export.formatUnavailable`, `export.encoderFailed`.

**What is rendered** (all exporters, print and agent render):

- Only **visible** elements. Hidden elements and annotation elements are excluded.
- Effects are rendered at final quality. Disabling effects in the view affects the view only, never export.
- Output size = camera size and frame rate = project frame rate unless overridden by format settings.
- Background = scene background colour (or transparent where the format supports alpha and alpha is requested).
- High-quality (filtered, anti-aliased) texture sampling is always used for export.
- No watermark or other overlay is ever composited.

---

## 5. Export snapshot

### 5.1 Purpose

Export Snapshot saves an image of the **current frame** to a still-image file. It is available from the main menu
whenever a project is open.

### 5.2 Parameters

| Parameter | Default | Rules |
|---|---|---|
| Output path (with browse via the native save dialog) | last used path | Invalid path → `export.invalidPath`. Existing file → overwrite policy. |
| Format | **PNG** | PNG, JPEG, TGA, BMP (MUST); TIFF (SHOULD); others MAY. |
| Format options | per format | JPEG quality 0–100 (default 90); colour depth with or without alpha; compression where applicable. |

The last format and options are persisted.

### 5.3 Behaviour

- The current frame is rendered at camera size through the composition of the **active view**: camera composition when
  the Camera (or Top/Side) View is active; exposure-sheet composition of the current scene when the Drawing View is
  active, or only the selected element if one is selected there.
- Areas with no content are transparent if the format supports alpha and alpha is selected; otherwise they are filled
  with the scene background colour.
- Nothing at the current frame → `export.nothingToExport`. Write failure → `export.fileInUse`.

---

## 6. Export drawing to PDF

### 6.1 Purpose

Exports the **currently selected vector drawing** (one cell) as a single-page vector PDF. Available from the main menu
and from the drawing element's context menu.

### 6.2 Behaviour

1. Preconditions, checked in this order:
   - a drawing element must be selected in the Timeline or Exposure Sheet (`export.noDrawingSelected`);
   - the selected element must be a vector drawing element, not image, sound or other (`export.notVectorElement`);
   - the cell at the current frame must contain a drawing (`export.emptyCell`).
2. Choose the output file (native save dialog filtered to PDF); an existing file follows the overwrite policy.
3. The drawing is written as one PDF page:
   - each drawing layer (line art, colour art — `02_drawing.md`) is written in stacking order;
   - pencil strokes become stroked Bézier paths with width, join, cap and dash from the line style;
   - brush strokes and painted zones become filled Bézier paths;
   - colours are resolved from the palette(s) of the drawing's scene; RGB colour, alpha written as opacity;
   - page size: the drawing's bounding box (default) or the camera frame, at a scale factor (default 1 drawing unit
     = 1 pt; configurable);
   - linear and radial gradient swatches SHOULD be written as PDF shadings and texture swatches SHOULD be written as
     clipped images; where this is not implemented they fall back to a flat (average) colour and the warning
     `export.pdfFeatureFlattened` is reported.
4. On success the user is told the path that was written.
5. Failures: `export.fileInUse`, `export.pdfWriterFailed`.

---

## 7. Print

### 7.1 Purpose and entry

Prints frames or drawings to a printer, or to a PDF or print file, with one drawing per page, several drawings per page
in a grid, or flip-book pages. Available from the main menu; SHOULD use the platform-standard print shortcut.

### 7.2 Print parameters

| Group | Parameter | Default | Range / rules |
|---|---|---|---|
| Printer | Printer | system default printer (last used is persisted) | installed printers |
| | Page setup | — | §7.3 |
| | Print to file | off | When on, enables an output path (with browse). Invalid path → `print.invalidPath`. Existing file → overwrite policy; a read-only or locked file → `print.cannotReplace`. |
| | As PDF | off | Writes PDF. Enabled only when Print to file is on. |
| Copies | Number of copies | **1** | 1–999 |
| | Collate | **off** | Shown with an illustration of the resulting page order. |
| Layout | **One drawing per page** | **selected** | Each frame/drawing centred and scaled to fit the printable area of its own page. |
| | **Multiple drawings**, rows × columns | rows **3**, columns **2** | 1–20 each; grid filled left-to-right, top-to-bottom in frame order. Row/column fields enabled only with this layout. |
| | **Flip book** | — | Several frames per sheet arranged to be cut out and bound in sequence, each with a binding margin on the left (default 15 mm), frames in reading order, frame number printed small in the margin. |
| Export type | Full Movie | **selected** | all scenes |
| | Current Scene: Camera View | | camera composition |
| | Current Scene: Drawing View | | exposure-sheet composition |
| | Selected Element: Drawing View | | only the selected element, drawing by drawing, as in the Drawing View |
| Range | All | **selected** | all frames of the chosen type |
| | Frames from / to | 1 / length | inclusive, 1..length |
| | Selection | | the selected frames |
| | Free Playback Range | | the scene's free playback range |
| | Key frames only | off | Print only keyframe frames: frames where a visible element's exposed drawing changes (drawing-view types) or where a peg/camera keyframe exists (camera-view type). |
| | Individual drawings only | off | Print each distinct drawing once instead of once per exposed frame (held exposures skipped). Mainly useful with Selected Element. |
| Decoration | Print camera frame | off | Draws the camera frame rectangle around each printed frame. |

Range enabling rules match export (§4.7): range options other than All are disabled for Full Movie; Selection requires
a selection; Free Playback Range requires Free mode. All print parameters are persisted per user, together with the
printer's own options (colour mode, double-sided, page order, paper size and source, resolution, page range).

### 7.3 Page setup

| Setting | Default | Values |
|---|---|---|
| Paper size | the printer's default, else locale default (A4 or Letter) | A0, A1, A2, A3, A4, A5, Letter, Legal, plus any sizes the printer reports |
| Orientation | **Portrait** | Portrait, Landscape |

### 7.4 Behaviour

1. Frames are rendered with the chosen composition (as for export).
2. Frames are laid out per the layout choice on pages of the chosen size and orientation, with the camera-frame
   decoration if chosen.
3. Pages are sent to the printer or written to the file / PDF.
4. Errors: `print.deviceFailed` (the selected printer may be invalid or unsupported), `export.nothingToExport`,
   `print.cancelled`.

A preview of the laid-out pages (page count and page images) MUST be available (`print.preview.get`); the GUI SHOULD
show it in the print dialog.

---

## 8. Import

### 8.1 Import entry points

| Command (context) | Imports |
|---|---|
| **Import File** (main menu; Exposure Sheet / Timeline background context menu) | Any supported file (§8.2); multi-select. |
| **Import Images** (image element context menu) | Bitmaps into the selected image element. |
| **Import Images with Keying** (image element context menu) | Bitmaps with colour keying (§8.9). |
| **Import Movie as Image Sequence** (image element context menu) | A movie's frames as image cells (§8.7). |
| **Import Vector Art** (drawing element / cell context menu) | PDF or AI-compatible art into the selected drawing cell, layers flattened into that one drawing. |
| **Import and Vectorize: From File** (main menu; drawing element context menu) | Bitmaps converted to vector drawings (§8.10). |
| **Import and Vectorize: From Capture Device / From Scanner** (main menu; drawing element context menu) | Acquisition in vectorize mode (§9). |
| **Acquire: From Capture Device / From Scanner** (main menu; image element context menu) | Acquisition in image mode (§9). |
| **Select Acquisition Device** (main menu; image and drawing element context menus) | Chooses the default capture / scanner device; the choice is remembered. |
| **Import Sound** (main menu and sound controls; sound and lip-sync element context menus) | Sound into a new sound element, or into the selected sound / lip-sync element (`07_sound_lipsync.md`). |
| **Convert Element to Vector** (image element context menu) | Vectorizes an existing image element (`02_drawing.md`). |
| Library import (library context menu) | Images, vector animations, sounds and templates into the library (`08_library_storyline.md`). |
| **Drag and drop** | Files from the OS file browser onto the Camera, Drawing, Top, Side, Timeline or Exposure Sheet views (§8.11). |

### 8.2 Supported formats and dispatch

The file chooser allows multiple selection and offers filters: all supported files (default), image formats, layered
image formats, vector art formats, movie formats, sound formats, vector animation (if supported), all files. The last
import folder is remembered. Classification uses file content (signature), falling back to the extension.

| Family | Formats | Result in the scene |
|---|---|---|
| **Bitmap images** | MUST: PNG, JPEG, GIF (first frame), BMP, TIFF, TGA. SHOULD: JPEG 2000, WebP. | A new **image** element (or the selected compatible one), one cell per file, at the current frame; fit-to-camera-width by default (§8.4). |
| **Layered images** | MUST: PSD in 8-bit RGB, greyscale and multichannel modes. SHOULD: layered TIFF. | With "preserve layers" on: **one image element per layer**, stacked in the file's layer order and named after the layers. Otherwise one flattened image. An unsupported colour mode imports a flattened image and reports `import.layeredModeUnsupported`, stating the supported modes. Messages from several files are collected into one summary. |
| **Vector art** | MUST: PDF and PDF-compatible AI files. SHOULD: SVG. MAY: legacy PostScript-based AI. | A new **drawing** (vector) element. With "preserve layers" on, each source layer becomes a separate drawing element; otherwise one drawing. Paths become pencil lines and fills; gradients become gradient swatches; colours are added to a palette. Each PDF **page** becomes a drawing named `<file> (p.<n>)` on consecutive frames. Details §8.5. |
| **Vector animation (SWF)** | MAY (pairs with the optional SWF export) | Converted to native elements (§8.6). |
| **Movies** | MUST: MP4/H.264, MOV. SHOULD: WebM, AVI, M4V. | Image-sequence import (§8.7). |
| **Sound** | MUST: WAV, AIFF, MP3, M4A/AAC. SHOULD: FLAC, Ogg Vorbis, AU (µ-law). | A new **sound** element starting at the current frame (`07_sound_lipsync.md`). |
| **Templates** | `.animtpl` | Through the library flow (`08_library_storyline.md`). |

### 8.3 Insertion rules

- **Insertion point**: the current scene, the current frame, and the selected element if it is compatible.
- **Compatible element selected** (image file → image element, vector art → drawing element, sound → sound element):
  content goes into that element starting at the current frame. Otherwise a **new element** is created, named after
  the file without its extension (made unique if needed).
- **Several files of the same family** go into consecutive frames of one element, in natural name order, so a numbered
  sequence (`walk_1 … walk_10`) becomes an animation in the right order.
- **Copying**: imported bitmap, movie and sound files are copied into the project package. If a file with the same name
  already exists there, the new one is stored under a suffixed name and the user is told which name was used
  (warning `import.renamed`).
- **Visibility**: if the new elements cannot be seen in the current working mode (for example in drawing-only mode),
  warn the user that the newly added elements are not visible in that mode and how to make them visible (warning
  `import.notVisibleInMode`).
- **Undo**: each import is one undoable step labelled by kind (Import Image, Import Sound, Import Vector Art, Import
  Movie, Import Vector Animation, Capture Image).
- **Progress**: long imports show progress with phases (reading file structure, building vector objects, importing
  frames) and can be cancelled; cancelling rolls back the whole import.

### 8.4 Import options

Shown after files are chosen when at least one option applies; only applicable groups are enabled. Choices are
persisted.

| Group | Option | Default | Effect |
|---|---|---|---|
| Images | **Fit image to camera width** | **on** | Scales the bitmap so its width equals the camera frame width, keeping proportions. When off, the image keeps its pixel size relative to the project resolution. |
| Layered images | **Preserve layers** | on | One element per layer (§8.2). |
| Vector art | **Preserve layers** | on | One drawing element per source layer. The cell-level Import Vector Art command always flattens into the selected drawing; the main Import File command is the way to keep layers. |
| Movies | **Set opacity** + **Opacity %** | off; 100 | Applies a fixed opacity (integer 1–100) to every imported frame. The value field is enabled only when the checkbox is on. |

Cancel aborts the whole import. Through the command layer every option is a parameter, and per-layer choices can be
made explicitly (include/exclude a layer, element name per layer, flatten groups of layers).

### 8.5 Vector art (PDF / AI-compatible / SVG) import details

- MUST import: paths (lines, Béziers, rectangles), fill and stroke colours (RGB; CMYK and greyscale converted to RGB),
  line widths, groups and layers, linear and radial gradients (as gradient swatches), embedded images (as bitmaps or
  textures).
- SHOULD import: even-odd fill and clip rules, line joins, line caps, dash patterns.
- Text: imported as outlines when the font program is embedded; otherwise skipped with a warning.
- Unsupported features (for example smooth shadings other than linear/radial, image masks, and any SHOULD feature not
  implemented) are skipped; the import continues and ends with a warning summary listing the unsupported features found
  per file (`import.unsupportedFeatures`).
- A damaged file that can be repaired (for example by rebuilding its cross-reference table) is imported with a warning;
  a file declaring a newer format version than supported is attempted with a warning.
- Errors: `import.notVectorArt` (the file is not of the expected type), `import.versionUnreadable`,
  `import.cannotOpen`, `import.cannotRead`.

### 8.6 Vector animation (SWF) import (optional)

If supported, the file's timeline is converted into native elements:

- **Shapes** become vector drawings; identical shapes are shared. Fills and lines become zones and strokes; colours are
  added to a palette. If the file carries embedded palette information (§4.5), the source palette, styles and colour
  identities are restored, and one palette is shared across several imports.
- **Bitmaps** become image or texture data.
- **Placement**: each display depth becomes a drawing element; per-frame placement matrices become **peg** elements
  with keyframes, and colour transforms become colour-transform data. Values are reduced to keyframes where they are
  constant or linear.
- **Sounds**: event and stream sounds become sound elements placed at their frames.
- **Scene**: imported either as a new scene named "Imported Scene" (numbered if needed) or into the current scene at the
  current frame. This is a parameter; the default is a new scene for main-menu imports and the current scene for
  element-targeted imports.
- **Frame rate**: no resampling — source frame N lands on scene frame N.
- **Limits**: files over 1000 frames are refused (`import.tooManyFrames`); protected files are refused
  (`import.protected`; in multi-file and library imports the others still import and a summary lists the skipped ones).
- Unsupported content (scripts, buttons, text/font definitions, shape morphs, embedded video and other non-graphic
  content) is dropped; the import continues and reports a summary (`import.unsupportedFeatures`).
- Invalid file: `import.invalidFile`.

### 8.7 Movie import as image sequence

- Every movie frame is decoded to a bitmap and stored as an image cell of an image element, one cell per frame,
  starting at the current frame.
- The movie opacity option (§8.4) is applied to every frame.
- Frames are taken one-to-one by default (no rate conversion); a "match project frame rate" option (default off)
  resamples by dropping or duplicating frames.
- The movie's audio track, if any, is imported as a sound element starting at the same frame (option, default on).
- Files beyond the configured maximum size are refused (`import.fileTooLarge`).

### 8.8 Sound import (summary; owner `07_sound_lipsync.md`)

- Formats as in §8.2.
- Above a configured size, warn that the file is very large, recommend splitting it into smaller segments, and ask
  whether to continue (command layer: `confirm` parameter or a `needs_confirmation` result).
- Errors: `import.soundUnreadable`, `import.unsupportedFormat`, `import.audioUnavailable` (no audio subsystem).

### 8.9 Import images with keying

Imports bitmaps and removes a solid background colour (green/blue screen, or paper) so it becomes transparent. It uses
the same keying engine as capture (§9.5).

| Area | Control | Default | Behaviour |
|---|---|---|---|
| Preview | Image with keying applied | first image | Transparent areas drawn over a checkerboard. Zoom in / out (default fit). |
| | Previous / Next, counter "n / N", file name | 1 / N | Step through the chosen files. |
| Files | File list + browse (multi-select) | empty | |
| Keying | Keying type | Chroma (RGB) | Chroma (RGB), Chroma (YUV), Luma (YUV) |
| | Pick colour | none | Eyedropper on the preview sets the key colour; clicking the swatch opens a colour chooser. |
| | Luma correction | off | Evens out lighting before keying (meaningful for luma keying). |
| | Show matte | off | Shows the matte instead of the image. |
| | Hide checkerboard | off | Shows plain white behind transparent pixels. |
| | Minimum | 0 | integer 0–100 (invalid entry → `import.valueOutOfRange`) |
| | Maximum | **99** | integer 0–100 |
| | Matte filter | None | None, Mean, Gaussian, Erode, Dilate, Distance |
| | Filter intensity | Low | Low, Medium-Low, Medium, Medium-High, High |
| | Apply to all | — | Saves the current keying parameters and applies them to all chosen images (otherwise parameters may differ per image). |

OK imports all files with their keying into the selected image element at the current frame, one cell per file, with
progress. Keying parameters persist between sessions.

### 8.10 Import and vectorize (summary; engine owned by `02_drawing.md`)

- Files: path list + browse (multi-select); preview pane with refresh and zoom.
- Vectorization type: **Black and white**, or **With texture** (keeps the bitmap colour as a texture fill).
- Pre-filter: none; smoothing with slight loss of sharpness; smoothing with greater loss of sharpness; edge
  enhancement; sharpening.
- Threshold %: integer 0–100.
- Result: one vector drawing per bitmap, in the selected or a new drawing element, one cell per file from the current
  frame. A vectorization palette is created for the colours. Progress is shown.
- The same settings are used when capture or scanning runs in vectorize mode (§9.11).

### 8.11 Drag-and-drop import

- Files dropped from the OS file browser onto the Camera, Drawing, Top, Side, Timeline or Exposure Sheet views go
  through the same dispatcher as Import File, at the drop location: the frame and element under the pointer in the
  Timeline or Exposure Sheet, or the current frame for canvas views.
- Dropping onto the Timeline offers a choice: **Copy** (copy the file into the project), **Link** (create a media/link
  element that references the file in place), **Move** (move the file into the project). Undo label "Insert Files".
- Drag-and-drop between the application's own views is specified in `04_timeline_xsheet.md`.

### 8.12 Library imports (summary; owner `08_library_storyline.md`)

Images, vector animations, sounds and templates can be imported into the library as library items rather than scene
content. Errors are reported as for scene import, plus `import.protected` for protected vector animations.

### 8.13 Generic import errors

`import.cannotImport` (with file name), `import.unsupportedFormat`, `import.unsupportedSize` (image dimensions outside
supported limits; lists the files), `import.partial` (some files could not be loaded and were not imported; lists them),
`import.imageUnreadable`, `import.fileNotFound`. Multi-file imports collect all errors into one report and import the
rest.

---

## 9. Image acquisition and stop-motion capture

### 9.1 Purpose

Image acquisition captures still frames from a connected camera, webcam, video device or scanner straight into the
exposure sheet, one capture per frame. It is used for **stop-motion** and for capturing paper drawings. A capture
session can optionally:

- **colour-key** a green/blue screen or paper background in real time;
- show an **onion-skin / light-table** overlay of an existing drawing or the previous capture over the live feed;
- **auto-capture** at a fixed interval (time-lapse);
- **vectorize** each capture.

### 9.2 Modes and entry points

| Entry (context) | Mode |
|---|---|
| Acquire from capture device (main menu; image element context menu) | **Image capture**: captures are stored as bitmaps in an image element. |
| Import and vectorize from capture device (main menu; drawing element context menu) | **Vectorize capture**: captures are vectorized into a drawing element. |
| Acquire / import and vectorize from scanner (same contexts) | Same two modes, with a scanner as the device (§9.12). |

The capture window title reflects the mode. Only one capture window exists at a time; invoking another entry point
raises the existing window and switches its mode and target. The window is non-modal and stays open while the user
works in the Timeline and Exposure Sheet.

### 9.3 Device model

The application MUST provide generic **camera and scanner device acquisition** through the platform's device
interfaces:

- **Motion cameras** (webcams, video and DV cameras, capture cards): provide a continuous live preview.
- **Photo cameras** (tethered still cameras): may provide a live preview and a slower full-resolution capture.
- **Scanners**, including document feeders that return several images per capture.

Devices connecting and disconnecting are detected while the application runs (hot-plug); the device list updates
without a restart.

Each device publishes a descriptor:

| Property | Meaning |
|---|---|
| Id, display name, type | Stable id; name; motion camera / photo camera / scanner / unknown. |
| Sources | Named capture sources, e.g. **Full-resolution photo** (slower, full detail) and **Live preview** (fast, lower quality, for rough tests). |
| Resolutions / image sizes | Available capture resolutions or pixel sizes; default preview and default capture resolution; maximum width, height and colour depth. |
| Maximum preview rate | Frames per second of the live preview. |
| Has settings | Whether the device exposes adjustable settings (exposure, ISO, white balance mode and values, focus near/far, zoom and zoom-preview panning) and/or its own settings dialog. |
| Feeder | Whether one capture can return several images. |
| Shows own UI | Whether the device presents its own interface when previewing or capturing. |
| Asynchronous | Whether capture completes asynchronously. |

### 9.4 Capture window: source settings

The window has a large capture preview area, a collapsible options panel (shown/hidden by a disclosure control), and
the Capture and Close actions.

| Control | Default | Behaviour |
|---|---|---|
| **Device** | last used, else the first detected device | Lists connected devices by name. If none, shows that no device is connected and disables capture. Changing the device opens a session on it. |
| **Source** | device default | The device's sources (§9.3). |
| **Resolution / image size** | device default capture resolution | Changing it updates the preview. |
| **Noise reduction** | off | Smooths colour noise on captured images; leave off for maximum detail. |
| **Device settings** | — | Adjusts the device's own settings (exposure, ISO, white balance, focus, zoom…); applied to the device immediately. Disabled if the device has none. May open the device's own dialog where it provides one. |
| **Refresh preview** | — | Requests a fresh preview frame; needed for devices without a continuous live feed. |

### 9.5 Keying

| Control | Default | Range | Behaviour |
|---|---|---|---|
| **Keying type** | **None** | None, **Chroma (RGB)**, **Chroma (YUV)**, **Luma (YUV)** | Selects the algorithm. When None, the other keying controls are disabled. Users are advised to pick the type that separates the background most cleanly. |
| **Pick colour** | — | — | Arms an eyedropper on the preview; a click samples the key colour (sampling a mid-tone area of the background is recommended). |
| Key colour swatch | none | — | Shows the key colour; clicking it opens a colour chooser. |
| **Luma correction** | off | — | With luma keying, evens out image brightness so drawings separate more easily from the paper. |
| **Show matte** | off | — | Shows the matte instead of the colour image: kept parts black, keyed parts white. Display aid only; does not change captured data. |
| **Hide background** | off | — | Shows keyed areas as transparent (checkerboard or white) to show what will be removed. |
| **Minimum** | 0 | 0–100 | Lower threshold. |
| **Maximum** | **99** | 0–100 | Upper threshold. Adjust both until the background is solid white/transparent. Minimum ≤ Maximum is enforced (moving one past the other pushes the other). |
| **Matte filter** | None | None, Mean, Gaussian, Erode, Dilate, Distance | Cleans up remaining fringes. |
| **Filter intensity** | Low | Low, Medium-Low, Medium, Medium-High, High | Filter strength / radius. |

**Keying algorithm** (functional):

1. **Luma correction** (optional, luma mode): normalise brightness, e.g. by dividing by a heavily blurred copy of the
   image, to flatten uneven lighting.
2. **Distance** per pixel, d ∈ [0, 100]:
   - Chroma (RGB): Euclidean distance between the pixel colour and the key colour in RGB, normalised to 0–100.
   - Chroma (YUV): distance in the chrominance (U/V) plane only, ignoring brightness — more robust to shadows on the screen.
   - Luma: difference in luminance (Y) between the pixel and the key.
3. **Alpha**: d ≤ Minimum → fully transparent; d ≥ Maximum → fully opaque; in between alpha rises linearly 0 → 1.
4. **Matte filter** on alpha: Mean or Gaussian blur feathers edges; Erode shrinks the opaque area (removes fringes);
   Dilate grows it; Distance feathers by distance from the edge. Intensity sets the radius: Low = 1 px … High = 5 px
   (in steps of 1 px).
5. **Display**: matte shows alpha as black (kept) / white (removed); hide background composites over checkerboard or
   white; otherwise the image is shown normally.
6. **Captured images** keep the computed alpha and are stored as RGBA bitmaps with keyed areas transparent.

Keying parameters persist between sessions and are shared with keyed image import (§8.9).

### 9.6 Light table (onion skin on the live feed)

| Control | Default | Behaviour |
|---|---|---|
| **Reference** | None | The image overlaid on the live preview: None; the previously captured image; or a drawing of an element of the current scene, identified by element and drawing/frame. The reference follows the current frame (it shows the content at, or just before, the frame the next capture will go to). |
| **Opacity** | 50 % | 0–100 % mix between the live feed and the reference. |
| **Aspect ratio** | Keep aspect ratio | **Keep aspect ratio** (fit inside), **Keep aspect ratio by expanding** (fill and crop), **Ignore aspect ratio** (stretch). Controls how the live image is fitted to the preview area and to the reference. |

### 9.7 Auto capture

| Control | Default | Range | Behaviour |
|---|---|---|---|
| Interval | **1** | 1–999 | Time between automatic captures. |
| Unit | seconds | milliseconds, seconds, minutes, hours, days | |
| **Start / Stop** | Start | — | Start begins time-lapse capture: one capture per interval until Stop or until the window is closed. While running, the control reads Stop, the other capture controls are disabled, and a status indicates auto capture is active. If a capture is still in progress when the next tick is due, that tick is skipped. |
| **Lock input during auto capture** | off | — | When on, pointer clicks in the application are ignored while auto capture runs, so the session is not disturbed accidentally; Stop (and its shortcut) remain available. |

### 9.8 Capture and close

- **Capture** takes one capture (§9.10), showing progress phases (capturing → applying filters → importing) with
  Cancel. Cancel discards the in-flight capture.
- **Close** ends the session and closes the window; a running auto capture is stopped first.

### 9.9 Live preview rules

- The preview shows the live feed at the device's preview rate, with keying and the light-table blend applied in real time.
- Interactive playback in the main window **pauses** the preview session; it resumes when playback stops.
- While a slider is being dragged, expensive matte filters MAY be suspended to keep the preview responsive.
- If the device disconnects, the window tries to reconnect and reports `capture.deviceDisconnected` if it cannot.

### 9.10 Where captures go

1. **Target element**: image capture mode → the selected image element of the current scene; vectorize mode → the
   selected drawing element. If no compatible element is selected, a new element is created and named automatically
   (e.g. "Capture", numbered if needed); later captures in the session go to that element.
2. **Target cell**: the capture is inserted at the **current frame** as a new drawing, named with the next available
   drawing number, exposed for one frame. The **current frame then advances by one**, so repeated captures fill
   consecutive frames — each frame represents one capture.
3. **Occupied cell**: by default the new capture is inserted before the existing content and later cells shift down
   (insert mode); an overwrite mode replaces the cell content instead.
4. **Stored image**: stored at the chosen resolution, scaled to the camera width as for imported images, keeping any
   keying alpha.
5. **Vectorize mode**: each capture is run through the vectorizer with the current import-and-vectorize settings
   (§8.10) and the resulting vector drawing is stored.
6. **Undo**: each capture is one undoable step ("Capture Image"). Failure: `capture.failed`.
7. **Feeder devices**: one capture may return several images, inserted on successive frames.

### 9.11 Vectorize on capture

In vectorize mode, the window shows a control to open the vectorization settings (§8.10); captures use the current
settings.

### 9.12 Scanner acquisition

- **Select acquisition device** chooses the default scanner (or camera); the choice is remembered.
- **Acquire from scanner** uses the device's scan settings; where the scanner provides its own interface (settings,
  prescan), it MAY be shown, but every setting the device exposes programmatically MUST also be settable without it
  (§11). Each transferred image is inserted as a cell at the current frame and the frame advances; multi-page feeders
  produce several frames. In vectorize mode each image is vectorized (§8.10).

### 9.13 Errors

`capture.noDevice` (no device connected), `capture.sessionFailed` (cannot open a session on the device),
`capture.deviceBusy` (device may be in use by another application — close it and reselect the device),
`capture.noImage` (no image received), `capture.deviceNotReady`, `capture.outOfMemory` (suggest reducing image size or
resolution), `capture.deviceDisconnected`, `capture.cancelled`, `capture.failed`.

---

## 10. Persisted settings

| Scope | Persisted | Where |
|---|---|---|
| Export movie | Last output path, format, settings preset, export type and range choice, open-in-player, HTML page, media library | User preferences |
| Format settings | "Custom" settings per format (video, image sequence, GIF, vector) | User preferences |
| Snapshot | Last format and options, last path | User preferences |
| Print | Print to file, as PDF, copies, collate, layout and grid size, export type, range options, key frames only, individual drawings only, camera frame; printer and its options; page setup | User preferences |
| Import | Last import folder; import options; keying parameters; vectorize settings | User preferences |
| Capture | Last device, source, resolution, noise reduction, keying, light-table settings, auto-capture interval and unit, input lock | User preferences |
| Publishing | Configured services; per-service sign-in tokens only when "remember" is on (OS credential store) | User preferences / credential store |
| Playback | Playback rate choice, loop, force frame rate, sound playback and scrubbing | User preferences |
| Playback range | Mode and start/end per scene | **Project** (`10_data_model_project_format.md`) |

---

## 11. Agent interface

### 11.1 Agent-specific requirements

1. **Playback vs. render.** `playback.*` commands drive the UI-visible, real-time playback of the current scene and
   the preview player; they are not deterministic and MUST NOT be used to obtain pixels. Pixels for verification come
   from `render.frame.run` / `render.range.run` (and the generic `render_frame` tool of section 11, which MUST produce
   the same result as `render.frame.run` with default parameters). **Storyline** playback across scenes is
   `storyline.play.start` / `storyline.play.stop`, and a movie frame including transitions is rendered with
   `storyline.frame.render` — both defined in `08_library_storyline.md`, not redefined here. `playback.stop` and
   `playback.state.get` nevertheless report and stop any interactive playback, including storyline playback.
2. **Deterministic render.** For the same document revision and parameters, `render.frame.run` MUST produce identical
   pixels and, for PNG, a byte-identical file (no timestamps or variable metadata) on the same build and platform.
   Random-looking effects use their stored seeds (`06_effects.md`). Interactive settings (playback rate, view toggles,
   onion skin, effect visibility in the view) MUST NOT affect render output.
3. **Inline vs. file output.** Render commands return PNG/JPEG **inline as base64** when the image is within the
   session's inline limits (section 11 §6.5; defaults: long side ≤ 2048 px and encoded image ≤ 8 MB); larger requests MUST use `output: "file"` and otherwise fail with
   `render.inlineTooLarge`. Every image result includes width, height, byte size and SHA-256.
4. **Jobs.** Export, preview, range render, import, print and publish run as **jobs**: the command returns `{jobId}`
   immediately (or, with `wait: true`, blocks up to a timeout and returns the final result). Progress is available via
   `job_status` and job events (section 11) as `JobProgress` (§11.4). `job_cancel` cancels: partially written files are
   deleted; a cancelled import rolls back completely.
5. **Output paths and overwrite policy.** Output paths MUST be absolute; the parent directory must exist unless
   `createDirs: true`. Every file-writing command takes `onConflict: "fail" | "overwrite" | "rename"` (default
   `"error"`, reported as `export.fileExists` with the conflicting paths). `"rename"` appends ` (n)` before the
   extension. Writes are subject to the file-access policy of section 11.
6. **Manifests.** Every finished export/render job returns a **manifest** (§11.4) listing each file written with role,
   path, byte size, SHA-256, pixel size, frame number (sequences) and duration (movies/audio), plus frame count, fps,
   total duration, and warnings. `deterministic: true` on movie export strips creation timestamps and encoder-version
   metadata so repeated exports produce identical checksums.
7. **Dialogs become parameters.** Every field of the export, snapshot, PDF, print, page setup, import options, keying,
   vectorize and capture dialogs is a command parameter; the dialogs' persisted values are readable/settable via the
   `*.defaults.*`, `*.settings.*` and `*.options.*` commands, and agent calls use those persisted values only for
   parameters they omit. Commands MUST NOT open dialogs or file pickers; GUI-only confirmations ("replace file?",
   "large sound file — continue?") become parameters (`onConflict`, `confirmLarge`) or a structured
   `needs_confirmation` result.
8. **Outward actions require confirmation.** `export.publish.run` (upload to a service) requires the session to hold
   the `network` permission scope (section 11) **and** a confirmation (a `confirmToken` from a previous
   `needs_confirmation` result, an elicitation accepted by the user, or `confirm: true` where the session policy
   pre-authorises outward actions);
   otherwise it returns `needs_confirmation` with a summary (service, account, title, visibility, file, size,
   duration) and does nothing. Printing to a physical printer likewise requires the `print` scope and a confirmation;
   printing to a file does not. Agents can never supply service passwords: `export.publish.account.open` starts the
   browser sign-in which the user completes; the agent can only query the result.
9. **Imports are explicit.** Imports take explicit absolute paths, explicit target (scene, element or "new", frame)
   and explicit per-file / per-layer / per-page options; `import.file.get` lets an agent inspect a file (family,
   layers, pages, frames, audio) before importing. Import results return all created element, drawing and cell ids,
   the stored file names, and warnings (unsupported features, renamed files, not visible in mode).
10. **Capture without hardware.** Every capture-session capability is available by command. `capture.frame.addFromFile`
    feeds stop-motion from supplied image files through exactly the same pipeline as a device capture (keying, scale to
    camera width, vectorize mode, target/insert/advance rules, undo step "Capture Image") — with or without an open
    session. `capture.preview.get` returns the processed live preview (with keying and light table) as PNG for
    verification; `capture.keyColour.set` accepts either a colour or a sample point in **preview pixel space**.
11. **Coordinate spaces.** Render outputs are in **output pixels**: origin at the top-left of the camera frame (or of
    the drawing's bounds for `composition: "element"` with a `drawingId`), x right, y down. Capture preview points are
    in **preview pixel space**: pixels of the device's current preview image, origin top-left, y down. Frames are
    1-based scene frames unless a parameter says "movie frame" (Full Movie ranges).
12. **Errors** are returned as `area.reason` codes (listed throughout this section; areas `playback`, `render`,
    `export`, `publish`, `import`, `capture`, `print`) with a human-readable message and structured details
    (e.g. `{limit, actual}` for publishing limits, file lists for import errors).

### 11.2 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `playback.start` | ui | Start interactive playback of the current scene in the views (Play); storyline playback is `storyline.play.start` | `sceneId?: id; fromFrame?: int` | `{playing: true, sceneId, frame}` | n/a |
| `playback.stop` | ui | Stop interactive playback (scene or storyline); current frame stays where stopped | — | `{frame}` | n/a |
| `playback.state.get` | query | Read playback state and options | — | `PlaybackState` | n/a |
| `playback.loop.set` | toggle | Loop playback on/off | `enabled: bool` | `{enabled}` | n/a |
| `playback.rate.set` | ui | Interactive playback rate (does not change project fps) | `fps: number (1–120) \| "project" = "project"` | `{fps, effectiveFps}` | n/a |
| `playback.forceRate.set` | toggle | Force frame rate (drop frames to keep real time) | `enabled: bool` | `{enabled}` | n/a |
| `playback.range.get` | query | Read a scene's playback range | `sceneId?: id` | `{mode: "auto"\|"free", start, end, sceneLength}` | n/a |
| `playback.range.mode.set` | action | Set range mode Automatic Fit / Free (restores last free values) | `sceneId?: id; mode: "auto"\|"free"` | `{mode, start, end}` | no |
| `playback.range.set` | action | Set free range ends (switches to Free; validated and adjusted) | `sceneId?: id; start?: int; end?: int` | `{mode, start, end, adjusted: bool}` | no |
| `playback.preview.run` | job | Render a preview (movie / scene / exposure sheet / selection / quick) and optionally open the player | `type: "movie"\|"scene"\|"xsheet"\|"xsheetSelection"\|"quick"; openPlayer: bool = true; wait: bool = false` | `{jobId}` → `{previewId, manifest}` | n/a |
| `playback.player.open` | ui | Open the preview player on a preview or a movie file | `previewId?: id; path?: path; autoplay: bool = true` | `{playerId, frameCount, fps, durationSec}` | n/a |
| `playback.player.set` | ui | Control the player (play/pause, seek, loop, volume, mute, size report) | `playerId?: id; playing?: bool; frame?: int; loop?: bool; volume?: int (0–100); mute?: bool; showSizeReport?: bool` | `PlayerState` | n/a |
| `playback.player.get` | query | Read player state | `playerId?: id` | `PlayerState` | n/a |
| `playback.player.close` | ui | Close the player | `playerId?: id` | `{closed: true}` | n/a |
| `playback.started` | event | Playback started | — | `{sceneId, frame, mode}` | n/a |
| `playback.stopped` | event | Playback stopped | — | `{sceneId, frame, reason: "user"\|"end"\|"navigation"\|"sceneChanged"}` | n/a |
| `playback.range.changed` | event | Playback range or mode changed | — | `{sceneId, mode, start, end}` | n/a |
| `render.frame.run` | query | Deterministically render one frame to PNG/JPEG, inline or to file | `sceneId?: id; frame?: int; composition: "camera"\|"xsheet"\|"element"\|"activeView" = "camera"; elementIds?: [id]; drawingId?: id; width?: int; height?: int; scale: number = 1 (0.05–8); quality: "final"\|"draft" = "final"; background: "scene"\|"transparent"\|colour = "scene"; cameraFrame: bool = false; format: "png"\|"jpeg" = "png"; jpegQuality: int = 90 (0–100); output: "inline"\|"file" = "inline"; path?: path; onConflict: "fail"\|"overwrite"\|"rename" = "fail"` | `RenderResult` | n/a |
| `render.range.run` | job | Render a frame range of one scene to an image sequence (movie ranges with transitions: `export.movie.run` with a sequence format) | `sceneId?: id; from?: int; to?: int; step: int = 1; composition: "camera"\|"xsheet"\|"element" = "camera"; elementIds?: [id]; width?: int; height?: int; scale: number = 1; quality: "final"\|"draft" = "final"; background: "scene"\|"transparent"\|colour = "scene"; format: "png"\|"jpeg"\|"tga"\|"bmp" = "png"; directory: path; namePattern: string = "{name}_{frame:4}"; onConflict = "fail"; createDirs: bool = false; wait: bool = false` | `{jobId}` → `Manifest` | n/a |
| `render.limits.get` | query | Inline size limits, max output size, supported render formats | — | `{maxInlinePixels, maxInlineBytes, maxWidth, maxHeight, formats[]}` | n/a |
| `export.formats.list` | query | List export formats with capabilities and settings schemas | `kind?: "movie"\|"sequence"\|"still"\|"vector"\|"document"` | `{formats: [{id, name, extensions[], kind, video, audio, alpha, optional, settingsSchema, presets[]}]}` | n/a |
| `export.settings.get` | query | Read stored settings of a format and active preset | `format: string` | `{preset, settings}` | n/a |
| `export.settings.set` | ui | Store Custom settings for a format, or reset to Default | `format: string; preset?: "default"\|"custom"; settings?: object` | `{preset, settings}` | n/a |
| `export.movie.defaults.get` | query | Read persisted export-dialog values (path, format, type, range, publishing options) | — | `ExportMovieParams` | n/a |
| `export.movie.defaults.set` | ui | Write persisted export-dialog values | `values: object` | `ExportMovieParams` | n/a |
| `export.movie.validate` | query | Dry-run an export: resolve frames, path, conflicts, estimates, enabled ranges | same as `export.movie.run` | `{frameCount, durationSec, outputPaths[], conflicts[], estimatedBytes?, enabledRanges[], warnings[], errors[]}` | n/a |
| `export.movie.run` | job | Export movie / animated GIF / image sequence / vector animation | `path: path; format: string = "mp4"; preset?: "default"\|"custom"\|string; settings?: object; type: "wholeMovie"\|"currentSceneCamera"\|"currentSceneXsheet" = "wholeMovie"; range: "all"\|"allNoTransitions"\|"frames"\|"selection"\|"playbackRange" = "allNoTransitions"; from?: int; to?: int; sceneId?: id; elementIds?: [id]; width?: int; height?: int; fps?: number; audio: bool = true; alpha: bool = false; writeHtml: bool = false; addToMediaLibrary: bool = false; openPlayer: bool = false; deterministic: bool = false; onConflict = "fail"; createDirs: bool = false; wait: bool = false` | `{jobId}` → `Manifest` | n/a |
| `export.snapshot.run` | job | Export the current (or given) frame as a still image, active-view composition by default | `path: path; format: "png"\|"jpeg"\|"tga"\|"bmp"\|"tiff" = "png"; quality?: int (0–100); alpha: bool = true; sceneId?: id; frame?: int; composition: "activeView"\|"camera"\|"xsheet"\|"element" = "activeView"; elementId?: id; onConflict = "fail"; wait: bool = true` | `{jobId}` → `Manifest` | n/a |
| `export.drawingPdf.run` | job | Export one vector drawing to a single-page PDF | `path: path; drawingId?: id; elementId?: id; frame?: int; page: "bounds"\|"camera" = "bounds"; scale: number = 1; onConflict = "fail"; wait: bool = true` | `{jobId}` → `Manifest` (+ `warnings` e.g. `export.pdfFeatureFlattened`) | n/a |
| `export.report.get` | query | Size report of a finished vector-format export | `jobId: id` | `{file, totalBytes, cameraSize, fps, groups: [{kind, items: [{name, bytes}]}]}` | n/a |
| `export.publish.service.list` | query | Configured publishing services, limits and metadata requirements | — | `{services: [{id, name, formats[], limits: {minSec, maxSec, maxBytes}, fields: [{name, required}], visibilities[], connected}]}` | n/a |
| `export.publish.categories.list` | query | Categories offered by a service | `serviceId: id` | `{categories: [{id, label}]}` | n/a |
| `export.publish.account.get` | query | Sign-in status for a service | `serviceId: id` | `{connected: bool, accountName?, remembered: bool, pending: bool}` | n/a |
| `export.publish.account.open` | ui | Start browser sign-in for a service (user completes it) | `serviceId: id; remember: bool = false` | `{pending: true}` | n/a |
| `export.publish.account.close` | ui | Sign out and forget the stored token | `serviceId: id` | `{connected: false}` | n/a |
| `export.publish.validate` | query | Check a file or export parameters against a service's limits and required metadata | `serviceId: id; path?: path; export?: object; metadata?: object` | `{ok: bool, problems: [{code, limit?, actual?}]}` | n/a |
| `export.publish.run` | job | Upload a movie to a service (outward action; needs scope and confirm) | `serviceId: id; path?: path; export?: object; metadata: {title: string; description?: string; tags?: [string]; category?: string; visibility?: "public"\|"unlisted"\|"private" = "private"}; confirm?: bool; confirmToken?: string` | `{jobId}` or `needs_confirmation` → `{remoteId, url?, manifest}` | n/a |
| `export.job.finished` | event | An export/render/print/publish job finished | — | `{jobId, command, status, outputPaths[], errorCode?}` | n/a |
| `import.formats.list` | query | Supported import formats by family | — | `{families: [{family, formats: [{id, extensions[], optional}]}]}` | n/a |
| `import.file.get` | query | Inspect a file before import | `path: path` | `ImportFileInfo` | n/a |
| `import.options.get` | query | Read persisted import options | — | `{fitToCamera, preserveImageLayers, preserveVectorLayers, movieOpacity?, lastFolder}` | n/a |
| `import.options.set` | ui | Write persisted import options | `fitToCamera?: bool; preserveImageLayers?: bool; preserveVectorLayers?: bool; movieOpacity?: int (1–100) \| null` | same as get | n/a |
| `import.file.run` | job | Import files (any family) at a target; one undo step | `paths: [path]; sceneId?: id; elementId?: id \| "new"; frame?: int; mode: "copy"\|"link"\|"move" = "copy"; fitToCamera?: bool; preserveLayers?: bool; layers?: [{path?: path; index?: int; name?: string; include: bool = true; elementName?: string}]; pages?: [int]; flatten?: bool = false; movieOpacity?: int (1–100); vectorAnimationTarget?: "newScene"\|"currentScene"; onNameConflict: "rename"\|"error" = "rename"; confirmLarge: bool = false; wait: bool = false` | `{jobId}` → `ImportResult` | yes |
| `import.movie.run` | job | Import a movie as an image sequence (and its audio) | `path: path; sceneId?: id; elementId?: id \| "new"; frame?: int; opacity?: int (1–100); matchProjectRate: bool = false; includeAudio: bool = true; sourceFrom?: int; sourceTo?: int; wait: bool = false` | `{jobId}` → `ImportResult` | yes |
| `import.keyedImages.run` | job | Import images with keying, one cell per file | `paths: [path]; keying: Keying; perFile?: [{path: path; keying: Keying}]; sceneId?: id; elementId?: id \| "new"; frame?: int; wait: bool = false` | `{jobId}` → `ImportResult` | yes |
| `import.keyedImage.get` | query | Preview keying on one image (PNG) | `path: path; keying: Keying; display: "keyed"\|"matte"\|"checkerboard"\|"white" = "checkerboard"; maxSize: int = 1024` | `{data: base64, width, height, transparentFraction}` | n/a |
| `import.vectorize.settings.get` | query | Read import-and-vectorize settings | — | `VectorizeSettings` | n/a |
| `import.vectorize.settings.set` | ui | Write the remembered import-and-vectorize settings (defaults for `draw.vectorize.run`, 02) | `type?: "blackAndWhite"\|"withTexture"; filter?: "none"\|"smooth"\|"smoothMore"\|"edgeEnhance"\|"sharpen"; threshold?: int (0–100)` | `VectorizeSettings` | n/a |
| `import.completed` | event | An import job finished | — | `{jobId, status, elementIds[], warnings[]}` | n/a |
| `capture.device.list` | query | List connected capture devices and scanners | `type?: "motionCamera"\|"photoCamera"\|"scanner"` | `{devices: [Device]}` | n/a |
| `capture.device.get` | query | Device descriptor, current settings and settings schema | `deviceId: id` | `Device & {settings, settingsSchema}` | n/a |
| `capture.device.select` | ui | Set the default acquisition device (remembered) | `deviceId: id` | `{deviceId}` | n/a |
| `capture.device.settings.set` | ui | Set device settings (exposure, ISO, white balance, focus, zoom, scan options…) | `deviceId: id; values: object` | `{applied: object, rejected: [{name, reason}]}` | n/a |
| `capture.device.dialog.open` | ui | Show the device's own settings interface, if it has one (user-facing) | `deviceId: id` | `{opened: bool}` | n/a |
| `capture.session.open` | ui | Open a capture session (and optionally the capture window) | `deviceId?: id; mode: "image"\|"vectorize" = "image"; sceneId?: id; elementId?: id \| "new"; showWindow: bool = true` | `{sessionId, deviceId, target}` | n/a |
| `capture.session.close` | ui | Close the session (stops auto capture) | `sessionId?: id` | `{closed: true}` | n/a |
| `capture.session.get` | query | Read session state | `sessionId?: id` | `CaptureSession` | n/a |
| `capture.source.set` | ui | Set source, resolution / image size, noise reduction | `sessionId?: id; source?: string; resolution?: string; imageSize?: {w: int, h: int}; noiseReduction?: bool` | `CaptureSession` | n/a |
| `capture.keying.set` | ui | Set keying parameters of the session (persisted) | `sessionId?: id; keying: Keying` | `{keying}` | n/a |
| `capture.keyColour.set` | ui | Set key colour directly or by sampling the preview | `sessionId?: id; colour?: colour; sample?: point; sampleRadius: int = 2 (0–20)` | `{colour}` | n/a |
| `capture.display.set` | ui | Preview display aids | `sessionId?: id; matte?: bool; hideBackground?: bool` | `{matte, hideBackground}` | n/a |
| `capture.lightTable.references.list` | query | Available light-table references for the current target frame | `sessionId?: id` | `{references: [{refId, label, elementId?, drawingId?, frame?}]}` | n/a |
| `capture.lightTable.set` | ui | Light-table reference, opacity and aspect-ratio mode | `sessionId?: id; reference?: "none"\|"previousCapture"\|id; opacity?: int = 50 (0–100); aspectRatio?: "fit"\|"fill"\|"stretch"` | `{reference, opacity, aspectRatio}` | n/a |
| `capture.target.set` | ui | Target element/frame and insert/advance behaviour | `sessionId?: id; elementId?: id \| "new"; frame?: int; insertMode: "insert"\|"overwrite" = "insert"; advance: bool = true` | `{elementId, frame, insertMode, advance}` | n/a |
| `capture.preview.get` | query | Current processed preview as PNG (keying, display aids, light table) | `sessionId?: id; display: "processed"\|"raw"\|"matte" = "processed"; includeLightTable: bool = true; refresh: bool = true; maxSize: int = 1024` | `{data: base64, width, height, capturedAt}` | n/a |
| `capture.frame.add` | action | Capture from the session's device into the target (Capture Image) | `sessionId?: id; count: int = 1; useDeviceUi: bool = false` | `CaptureResult` | yes |
| `capture.frame.addFromFile` | action | Feed supplied image files through the capture pipeline (Capture Image) | `paths: [path]; sessionId?: id; keying?: Keying \| "session" \| "none" = "session"; vectorize?: bool; sceneId?: id; elementId?: id \| "new"; frame?: int; insertMode?: "insert"\|"overwrite"; advance: bool = true` | `CaptureResult` | yes |
| `capture.auto.start` | job | Start auto capture (each capture is its own undo step) | `sessionId?: id; interval: int = 1 (1–999); unit: "ms"\|"s"\|"min"\|"h"\|"day" = "s"; maxCount?: int; lockInput: bool = false` | `{jobId}` → `{captures: int, cellIds[]}` | yes |
| `capture.auto.stop` | ui | Stop auto capture | `sessionId?: id` | `{captures: int}` | n/a |
| `capture.inputLock.set` | toggle | Ignore pointer input during auto capture | `enabled: bool` | `{enabled}` | n/a |
| `capture.device.added` | event | A device was connected | — | `{device: Device}` | n/a |
| `capture.device.removed` | event | A device was disconnected | — | `{deviceId}` | n/a |
| `capture.frame.captured` | event | A capture was inserted | — | `{sessionId?, elementId, drawingId, cellId, frame, nextFrame}` | n/a |
| `capture.session.error` | event | Session error (busy, disconnected, no image…) | — | `{sessionId, code, message}` | n/a |
| `print.printer.list` | query | Installed printers and their capabilities | — | `{printers: [{id, name, isDefault, paperSizes[], duplex, colour}]}` | n/a |
| `print.pageSetup.get` | query | Read page setup | `printerId?: id` | `{paperSize, orientation, printableArea}` | n/a |
| `print.pageSetup.set` | ui | Set paper size and orientation | `printerId?: id; paperSize?: string; orientation?: "portrait"\|"landscape"` | `{paperSize, orientation}` | n/a |
| `print.settings.get` | query | Read persisted print parameters | — | `PrintParams` | n/a |
| `print.settings.set` | ui | Write persisted print parameters | `values: object` | `PrintParams` | n/a |
| `print.preview.get` | query | Lay out pages and return page count and one page image | `PrintParams; page: int = 1; maxSize: int = 1024` | `{pageCount, pages: [{page, frames[]}], data: base64, width, height}` | n/a |
| `print.run` | job | Print to a printer (needs scope + confirm) or to a file/PDF | `PrintParams; destination: "printer"\|"file" = "file"; printerId?: id; path?: path; pdfOutput: bool = true; onConflict = "fail"; confirm?: bool; confirmToken?: string; wait: bool = false` | `{jobId}` or `needs_confirmation` → `{pagesPrinted}` or `Manifest` | n/a |

`PrintParams` = `copies: int = 1 (1–999); collate: bool = false; layout: "single"|"grid"|"flipbook" = "single"; rows:
int = 3 (1–20); columns: int = 2 (1–20); bindingMarginMm: number = 15; type: "wholeMovie"|"currentSceneCamera"|"currentSceneXsheet"|
"element" = "wholeMovie"; elementId?: id; range: "all"|"frames"|"selection"|"playbackRange" = "all"; from?: int; to?: int;
keyDrawingsOnly: bool = false; individualDrawingsOnly: bool = false; cameraFrame: bool = false; paperSize?: string;
orientation?: "portrait"|"landscape"`.

### 11.3 Queries and events

- `playback.state.get` and `playback.player.get` give the full UI playback state; `timeline.currentFrame.changed` (04),
  `playback.started`, `playback.stopped` and `playback.range.changed` notify changes.
- `render.frame.run` is the canonical read-back for "what does frame N look like"; `capture.preview.get`,
  `import.keyedImage.get` and `print.preview.get` are the read-backs for capture, keying and page layout.
- `export.movie.validate`, `export.publish.validate` and `import.file.get` let an agent check before acting.
- Job lifecycle uses the generic job events of section 11; `export.job.finished`, `import.completed` and
  `capture.frame.captured` are area-specific notifications.

### 11.4 Result shapes

```json
// PlaybackState
{ "playing": false, "mode": "scene", "sceneId": "sc_1", "frame": 12,
  "range": { "mode": "free", "start": 10, "end": 40, "sceneLength": 60 },
  "rate": { "setting": "project", "fps": 24 }, "loop": false, "forceRate": false,
  "sound": true, "scrubbing": false, "measuredFps": 23.8, "droppedFrames": 0 }

// RenderResult
{ "mimeType": "image/png", "width": 1280, "height": 720, "bytes": 183422,
  "sha256": "…", "data": "<base64>" /* or */ , "path": "C:/out/f012.png",
  "documentRevision": 418 }

// JobProgress
{ "jobId": "job_7", "command": "export.movie.run", "state": "running",
  "phase": "rendering" /* queued|rendering|encoding|mixingAudio|writing|postProcessing|uploading|done */,
  "framesDone": 120, "framesTotal": 480, "percent": 25.0, "etaSec": 31 }

// Manifest
{ "jobId": "job_7", "status": "succeeded", "format": "mp4", "outputPath": "C:/out/intro.mp4",
  "frameCount": 480, "fps": 24, "durationSec": 20.0, "width": 1920, "height": 1080,
  "range": { "type": "wholeMovie", "range": "allNoTransitions", "from": 1, "to": 480 },
  "files": [
    { "role": "video", "path": "C:/out/intro.mp4", "bytes": 5321123, "sha256": "…",
      "width": 1920, "height": 1080, "durationSec": 20.0,
      "audio": { "codec": "aac", "sampleRate": 48000, "channels": 2 } },
    { "role": "html", "path": "C:/out/intro.html", "bytes": 812, "sha256": "…" }
  ],
  "warnings": [ { "code": "export.pdfFeatureFlattened", "message": "…" } ],
  "elapsedMs": 18342 }
// image sequences: one files[] entry per frame with "role": "frame", "frame": n

// Keying
{ "type": "chromaRgb" /* none|chromaRgb|chromaYuv|luma */, "keyColour": "#00B140FF",
  "min": 0, "max": 99, "lumaCorrection": false,
  "filter": "none" /* none|mean|gaussian|erode|dilate|distance */, "intensity": 1 /* 1=Low … 5=High */ }

// VectorizeSettings
{ "type": "blackWhite", "filter": "none", "threshold": 50 }

// ImportFileInfo
{ "path": "…", "family": "layeredImage", "format": "psd", "width": 2048, "height": 1536,
  "layers": [ { "index": 0, "name": "BG", "visible": true, "mode": "rgb8" } ],
  "pages": null, "frames": null, "durationSec": null, "fps": null, "hasAudio": false,
  "protected": false, "supported": true, "warnings": [] }

// ImportResult
{ "sceneId": "sc_1", "elementIds": ["el_9"], "drawingIds": ["dr_31"], "cellIds": ["ce_77"],
  "frames": { "from": 1, "to": 12 },
  "storedFiles": [ { "source": "C:/in/bg.png", "storedAs": "media/bg (2).png" } ],
  "warnings": [ { "code": "import.renamed", "file": "C:/in/bg.png" } ], "errors": [] }

// Device
{ "id": "dev_3", "name": "USB Camera", "type": "motionCamera",
  "sources": ["Live preview", "Full-resolution photo"], "resolutions": ["1920x1080", "1280x720"],
  "maxPreviewFps": 30, "hasSettings": true, "hasOwnDialog": false, "feeder": false, "async": false }

// CaptureSession
{ "sessionId": "cap_1", "deviceId": "dev_3", "mode": "image", "source": "Live preview",
  "resolution": "1920x1080", "noiseReduction": false, "keying": { /* Keying */ },
  "display": { "matte": false, "hideBackground": false },
  "lightTable": { "reference": "previousCapture", "opacity": 50, "aspectRatio": "fit" },
  "target": { "sceneId": "sc_1", "elementId": "el_4", "frame": 37, "insertMode": "insert", "advance": true },
  "auto": { "running": false, "interval": 1, "unit": "s", "captures": 0 }, "previewPaused": false }

// CaptureResult
{ "elementId": "el_4", "inserted": [ { "drawingId": "dr_52", "cellId": "ce_140", "frame": 37 } ],
  "nextFrame": 38, "warnings": [] }
```

---

## 12. Design decisions / open questions

1. **Baseline formats.** Decision: MP4 (H.264/AAC) is the default and baseline export; MOV (H.264 + an alpha-capable
   codec) and PNG/JPEG/TGA/BMP sequences are required; animated GIF SHOULD ship; SWF export/import is optional and
   MAY be omitted entirely. Recommended: implement SWF only if round-trip with external vector tools is a target.
2. **Preview player.** Decision: one built-in player window for previews and exported movies (§3.4); no external
   browser or plug-in.
3. **"Current Scene: Drawing View" scope.** Decision: renders all visible exposure-sheet elements; single-element
   output uses the Selection range (export) or Selected Element type (print).
4. **Capture insertion.** Decision: insert at the current frame, shift later cells, advance by one; overwrite is a
   parameter (`insertMode`). Revisit after user testing.
5. **Filter intensity levels.** Decision: five levels (Low … High) mapping to radius 1–5 px.
6. **Playback range and undo.** Decision: playback range changes are saved with the project but are not undoable
   (they behave like view state). Alternative: undoable, with marker drags coalesced into one step.
7. **Ownership overlaps.** Storyline playback and transition-aware movie-frame rendering are owned by
   `08_library_storyline.md` (`storyline.play.*`, `storyline.frame.render`); this section only consumes them.
   Likewise, sound-during-playback and scrubbing switches are `sound.playback.set` / `sound.scrubbing.set` (07),
   current-frame navigation is `timeline.currentFrame.set` / `.move` (04), and vectorizing imported bitmaps is
   `draw.vectorize.run` (02). This section owns only `import.vectorize.settings.*`, the remembered defaults for that
   command.
8. **Publishing services.** Decision: services are configured via descriptors; the application ships with none
   enabled by default. Open: whether descriptors can be installed as plug-ins and how service limits are refreshed.
9. **Physical printing confirmation.** Decision: printing to a physical printer is treated as an outward action
   (`print` scope), printing to a file is not (section 11 §6.1).
10. **Size limits.** Recommended defaults: warn for sound files > 200 MB or > 30 minutes; refuse movie imports
    > 4 GB or > 20,000 frames; vector-animation import limit 1000 frames. All configurable.
11. **Defaults without strong precedent.** Recommended: SWF bitmap quality 80; keying Minimum 0; light-table opacity
    50 %; auto-capture unit seconds; JPEG quality 90 for stills and sequences; flip-book binding margin 15 mm; grid
    rows/columns up to 20.
12. **Vector import breadth.** Open: whether SVG import is MUST rather than SHOULD; recommended SHOULD in the first
    release, MUST later.
13. **Auto-capture minimum interval.** Decision: minimum 1 unit; in practice the effective interval is never shorter
    than the device's capture time (overlapping ticks are skipped).
14. **Frame-rate override on export.** Decision: allowed (1–120 fps) by frame duplication/dropping, not
    interpolation; open whether to offer frame blending.
