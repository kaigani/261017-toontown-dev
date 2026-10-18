# 07 — Sound, Lip-sync and Media Elements

This section specifies everything in the application that deals with audio and time-based media: sound elements and
the sound placements they hold (import, storage, placement on the frame axis, trimming, looping, volume, fade
envelope), how sound is shown in the Timeline and the Exposure Sheet (waveforms, lip-sync mode), interactive playback,
mixing and scrubbing, the sound element editor, automatic and manual **lip-sync** (the eight-shape mouth chart,
detection, mapping of mouth shapes to drawings and applying the result to a drawing element's exposures), **media
elements** (cells that link content held elsewhere instead of copying it) and importing a movie file as an image
sequence. Section 15 defines the agent interface (namespaces `sound`, `lipsync`, `media`).

Related sections: application shell, modes, preferences and windows → `01_application_shell.md`; element model and
generic Timeline/Exposure Sheet editing (cells, exposures, selection, insert/delete blank cells, clipboard rules) →
`04_timeline_xsheet.md`; Library items and scene order (Storyline) → `08_library_storyline.md`; interactive playback,
rendered previews, SWF import and all exports → `09_playback_export.md`; persistence in the project package
(`.animpkg`) → `10_data_model_project_format.md`; MCP server and generic tools → `11_agent_interface_mcp.md`; compiled
command list → `A_command_registry.md`.

---

## Contents

1. Concepts and data model
2. Audio import, validation and storage
3. Creating sound elements
4. Importing sound: entry points and placement rules
5. Sound element in the Properties view
6. Sound in the Timeline
7. Sound in the Exposure Sheet
8. Playback, mixing and scrubbing
9. Sound element editor
10. Lip-sync
11. Media elements
12. Movie import as an image sequence
13. Sound in exported output (summary)
14. Undoable operations (summary)
15. Agent interface
16. Design decisions / open questions

---

## 1. Concepts and data model

### 1.1 Entities

| Entity | Description |
|---|---|
| **Sound element** | A scene element with no visual content that holds zero or more sound placements on the frame axis. It appears as a track in the Timeline and as a column in the Exposure Sheet. Like every element it has a name (default pattern `sound_NNN`), a display colour, visibility and lock state and a stacking position. It also carries a per-element **streamed** flag (see §13). |
| **Sound asset** | One imported audio file held inside the project package and shared by every placement that references it. Has a format (channel count, bit depth, sample rate, duration) and a cached peak summary (waveform). Identical content imported twice MUST be stored once. |
| **Sound placement** | One use of a sound asset in a sound element: start frame, stop frame, in-point and out-point in the source audio (seconds), loop count, volume, a fade envelope and an optional lip-sync track. A sound element may hold many placements; placements in the same element MUST NOT overlap in frames. |
| **Envelope** | Ordered list of gain points over a placement's source audio; fade presets (fade in, fade out, fade in and out) are generated envelopes. |
| **Lip-sync track** (mouth track, lip chart) | Per-frame mouth-shape assignment (one of A, B, C, D, E, F, G, X) for a sound placement, with a per-frame *manual* flag and an optional per-frame display label (the name of the mapped drawing). Produced by detection, by explicit setting, or by hand editing. |
| **Lip-sync mapping** | For a (sound element, drawing element) pair: which drawing of the drawing element represents each of the eight mouth shapes. Stored in the project so the mapping is remembered between sessions. |
| **Media element** | An element whose cells reference ("link") content held elsewhere — a Library item or an external file (bitmap, vector drawing, SWF, sound) — instead of copying it into the element. Its cells are organised as media sequences. |
| **Media object** | Project-wide registry entry for one linked source (Library item or external file path). Several media elements and sequences may reference the same media object without duplicating data. |
| **Media sequence** | A run of frames in a media element showing one media object: start frame, stop frame and **link offset** (which frame of the linked source is shown at the start frame). |

### 1.2 Sound placement parameters

| Parameter | Type | Default | Range / rule | Effect |
|---|---|---|---|---|
| Sound asset | id | — | MUST reference an asset in the project | Which audio is played |
| Start frame | int (1-based scene frame) | target frame of the import / drop | ≥ 1; MUST NOT overlap the previous placement in the element | First frame on which the placement plays |
| Stop frame | int | start frame + ⌈played duration × fps⌉ − 1 | start frame ≤ stop frame ≤ natural end; MUST NOT overlap the next placement | Last frame; the audio is cut after this frame |
| In-point (start time) | number, seconds | 0.000 | 0 ≤ in < out | Audio skipped at the head of the source (trim head) |
| Out-point (stop time) | number, seconds | source duration | in < out ≤ duration | Audio discarded at the tail (trim tail) |
| Loop count | int | 1 | ≥ 1 | Number of back-to-back repetitions of the trimmed segment |
| Volume | int | 0 | −50 … +50 | Global gain on the placement, applied on top of the envelope (§9.5) |
| Envelope | list of points | empty (unity gain) | point times within [0, source duration]; levels 0.0 … 1.0 | Time-varying gain, fades (§9.5) |
| Lip-sync track | per-frame record | none | one entry per frame of the placement | Mouth assignment (§10.3) |

Rules:

- **Played duration** = (out-point − in-point) × loop count.
- **Natural end** = start frame + ⌈played duration × fps⌉ − 1. The stop frame MAY truncate the placement earlier but
  MUST NOT extend it past its natural end. The out-point trims the source; the stop frame truncates on the frame axis.
- Changing the in-point, out-point or loop count changes the natural end; if the stop frame was at the old natural end
  it follows the new natural end, otherwise it is clamped to the new natural end. If the new span would overlap the
  next placement, the stop frame is clamped to the frame before that placement.

### 1.3 Time spaces and frame/time conversion

Three time spaces are used in this section and in its commands:

| Space | Unit | Meaning |
|---|---|---|
| Scene frames | int, 1-based | Frame *f* of the scene starts at scene time `(f − 1) / fps` |
| Source time | seconds (number, millisecond precision displayed) | Position inside the sound asset's audio, 0 = first sample |
| Placement time | seconds | Time since the placement's start frame began |

- Within a placement starting at frame *s*, frame *f* covers source time
  `t = in + ((f − s) / fps mod (out − in))` (the modulo applies when looping).
- The scene frame rate (scene/project property, see `01_application_shell.md`) is the only conversion factor.
- Changing the frame rate keeps every placement anchored to its start frame; the frame span of each placement is
  recomputed from its played duration. Lip-sync tracks become *stale* (§10.3) when the frame rate changes.

---

## 2. Audio import, validation and storage

### 2.1 Supported import formats

The application MUST decode at least the following and SHOULD accept any further format the platform's decoder offers:

| Format | Extensions |
|---|---|
| WAV (PCM 8/16/24/32-bit integer and 32-bit float; ADPCM variants where decodable) | `.wav` |
| AIFF / AIFF-C | `.aif`, `.aiff`, `.aifc` |
| MP3 (MPEG-1/2 Layer III) | `.mp3` |
| AAC in an MPEG-4 audio container | `.m4a` |
| μ-law (Sun/NeXT) | `.au`, `.snd` |
| Audio track of a movie file (any movie format accepted by §12) | `.mov`, `.mp4`, `.m4v`, … |
| ADPCM or MP3 sound embedded in an imported SWF file (see §13 and `09_playback_export.md`) | — |

- Mono and stereo MUST be supported; files with more channels SHOULD be down-mixed to stereo on import.
- Any sample rate from 8 kHz to 96 kHz MUST be supported; the source sample rate is kept.
- The file chooser for sound import filters to the supported sound formats and also offers an "all files" option.
- Decoding is implementation-defined but MUST be cross-platform and MUST work offline.

### 2.2 Validation and limits

| Condition | Behaviour | Error / warning code |
|---|---|---|
| File cannot be decoded or format unsupported | That file is refused; other files of a multi-file import continue | `sound.formatUnsupported` |
| File cannot be read | That file is refused | `sound.importFailed` |
| File exceeds the size limit (Design decision D1) | The user is warned that the file is very large and advised to split it, and may choose to continue or cancel | `sound.tooLarge` (warning) |
| Audio output device unavailable | Sound import and editing remain available; playback is silent and the user is informed once per session | `sound.deviceUnavailable` |
| A referenced sound asset is missing or unreadable when a project is opened | The user is warned once per project open; further identical warnings in that session are suppressed. Affected placements keep their timing, show no waveform and play silence until relinked | `sound.assetMissing` |

### 2.3 Storage in the project

- Imported audio is **copied** into the project package's sound folder (layout in `10_data_model_project_format.md`).
  The imported file's bytes are kept unchanged (no re-encoding).
- If an asset with identical content already exists, it is reused. A different file with an already-used name gets a
  unique name.
- Asset files are written when the project is saved; saving shows progress for large sound files.
- Assets no longer referenced by any placement, media object or Library item of the project are removed from the
  package on save.
- The application SHOULD store a **waveform cache** (per-bucket min/max peak pairs per channel) alongside each asset so
  waveforms need not be recomputed on load. Computing it shows progress and runs in the background; the waveform is
  drawn as soon as it is available.

### 2.4 Audio properties display

Wherever a single sound is shown in detail (sound element editor, Library preview), the application shows a one-line
summary: channel layout (Mono / Stereo), bit depth, sample rate in kHz with 3 decimals, and duration in seconds with 3
decimals.

---

## 3. Creating sound elements

| Capability | Contexts in which it MUST be reachable | Behaviour |
|---|---|---|
| Add sound element | Main menu (element creation), Exposure Sheet context menu and its element-creation control, generic "add elements" command with type = sound | Adds an empty sound element above the current selection and selects it. The Properties view shows the sound panel (§5) so a file can be imported at once. |
| Add several elements | Generic add-elements command (`04_timeline_xsheet.md`) with type, count (≥ 1) and base name | Multiple elements get numbered suffixes. |
| Import sound as new element | Main menu, Timeline context menu, sound toolbar | Chooses a file, creates a new sound element named after the file and places the sound at the current frame (§4.2). |

Rules:

- A sound element cannot be the child of a peg and cannot be a parent; it is always a top-level track.
- Sound elements have no visual content: renderers ignore them and they show nothing in the Camera or Drawing views.
- When an element that cannot be seen in the current mode (e.g. a sound element added in drawing mode) is created, the
  application MUST tell the user that the new element is not visible or editable in that mode and how to make it
  visible (switch to the scene-planning mode). Code: `element.notVisibleInMode` (owned by section 04; listed here for
  completeness).
- The default track colour for sound elements is a user preference (`01_application_shell.md`).

---

## 4. Importing sound: entry points and placement rules

### 4.1 Entry points

| Entry point (context) | Target element | Target frame |
|---|---|---|
| Generic file import (main menu) with a sound file | The selected sound element if one is selected, otherwise a new sound element | Current frame |
| Exposure Sheet: sound column cell context menu → import sound file | That column | Frame of the selected cell |
| Exposure Sheet: lip-sync-mode cell context menu → import sound, with a submenu of **recently imported sounds** for quick re-use | That column | Frame of the selected cell |
| Properties view sound panel → Import | The selected sound element | Current frame |
| Import sound as new element (toolbar / menu) | New sound element | Current frame |
| Lip-sync panel → Import sound (§10.7) | New sound element, which becomes the panel's selected sound | Current frame |
| Sound element editor → Import (§9.7) | The element being edited | Editor's current frame |
| Drag a sound item from the Library onto the Timeline or Exposure Sheet | Dropped-on sound element, or a new one when dropped on empty space | Drop frame |
| Drag audio files from the operating system's file manager | As above | Drop frame |
| Import of an SWF file containing sounds (`09_playback_export.md`) | New sound elements for the SWF's sounds | Event sounds at their trigger frames with their in/out points, loop count and envelope; stream sounds rebuilt as one continuous placement |
| Library: import a sound file into a Library folder | The Library (not the scene); see `08_library_storyline.md` | — |

Several files MAY be imported at once; they are placed one after another in the same element, each starting on the
frame after the previous one's stop frame.

### 4.2 Placement rules

1. The placement's start frame is the target frame (selected cell, current frame or drop frame).
2. Its stop frame is its natural end (§1.2).
3. **Overlap** — if the new span overlaps an existing placement in the same element:
   - importing through an Exposure Sheet cell or a menu command inserts the new placement at the target frame and
     shifts every later placement of that element later by the new placement's length (*insert and shift*);
     if the target frame falls inside an existing placement, that placement is split at the target frame first;
   - dropping in the Timeline refuses an overlapping drop: the drop preview shows the position as invalid;
   - the sound element editor's Import places the new sound at the first free frame at or after the target frame.
   The agent interface exposes these behaviours as an explicit `onOverlap` parameter.
4. The scene length is extended if needed so that the whole placement lies within the scene.
5. After import the receiving sound element is selected; in the lip-sync panel it becomes the selected sound.
6. Each import (including multi-file imports) is one undoable step labelled *Import Sound*.
7. The waveform is computed in the background after import, with progress shown, if waveform display is enabled.

---

## 5. Sound element in the Properties view

When a sound element is selected, the Properties view shows a sound panel with:

| Control | Behaviour |
|---|---|
| Element name | Read-only display; shows a "no sound selected" placeholder when none is selected. Renaming uses the generic element rename command. |
| Import | Imports a sound file into this element (§4). |
| Edit sound | Opens the sound element editor for this element (§9). |
| Lip-sync | Opens or raises the lip-sync panel with this element preselected as the sound (§10.7). |

All three buttons are disabled when no sound element is selected.

---

## 6. Sound in the Timeline

### 6.1 Track appearance

- A sound element is a track with a sound-element icon.
- Each placement is drawn as a bar from its start frame to its stop frame, in the element's colour.
- With **waveform display** on, the bar shows the waveform; with it off, the bar is a solid bar.
- Waveform drawing: for each pixel column, the min/max peak over the covered time range is drawn as a vertical line
  centred on the track's mid-line and scaled to the track height; stereo is shown as the mix of both channels.
  Waveforms are recomputed from the cache when toggled on or when the Timeline zoom changes.

### 6.2 Interaction

| Gesture | Result |
|---|---|
| Click a placement bar | Selects that placement (and its element) |
| Drag a placement bar horizontally | Moves the placement to a new start frame, keeping its length. A ghost bar previews the drop; positions that would overlap another placement of the same element are shown as invalid and refused |
| Drag the end of a placement bar | Changes the stop frame (truncate, or extend up to the natural end) |
| Drag near the Timeline edges | Auto-scrolls |
| Double-click a placement bar, or *edit sound* from its context menu | Opens the sound element editor |
| Delete command with a placement selected | Removes that placement |
| Drag the current-frame indicator with sound scrubbing on | Plays short audio slices (§8.3) |

Element-level operations (rename, delete, clone, duplicate, lock, show/hide, arrange, colour) behave as for any element
(`04_timeline_xsheet.md`), with these sound-specific rules:

- *Clone* creates a linked copy that shares its placements with the source; *Duplicate* copies the placements
  (sharing the same assets).
- Hidden sound elements are not played (§8.2). Locked sound elements still play but cannot be edited.

---

## 7. Sound in the Exposure Sheet

### 7.1 Column appearance

- A sound element is a column. Cells covered by a placement show the placement's waveform rotated so that time runs
  downward, one cell per frame. The first cell of a placement shows the sound's name; the last cell is marked as the
  end of the placement.
- In **lip-sync mode** each cell shows the frame's mouth letter (A–G, X) and/or a small reference image of the mouth
  shape (or the mapped drawing's label) instead of the waveform. Frames whose mouth was set by hand MUST be visually
  distinguishable from detected frames (recommended: a marker or italic letter).
- The column header has its own context menu (§7.4).

### 7.2 Sound column cell commands

Reachable from the cell context menu of a sound column (waveform mode):

| Capability | Behaviour |
|---|---|
| Import sound file | Imports into this column at the selected cell (§4) |
| Insert cells / insert blank cell | Inserts empty frames before the selected cell; every placement starting at or after that frame moves later by the inserted count. A placement spanning the insertion point is split there and its second part moves later (its in-point is advanced accordingly). |
| Delete blank cell | Removes an empty frame; later placements move one frame earlier. Does nothing if the selected cell is not empty. |
| Select whole sound | Selects every frame of the placement under the cursor |
| Select from start | Selects from the placement's start frame to the current frame |
| Select to end | Selects from the current frame to the placement's stop frame |
| Play | Plays only this placement's audio (no picture), from the start of the selection, or from the placement start if nothing within it is selected |
| Stop | Stops a sound started with *Play* |
| Cut / Copy / Paste / Delete | Operate on frame ranges of sound. **Delete** empties the selected frames: a placement crossing the range is split around it; the part before keeps its in-point and gets a new stop frame; the part after gets a new start frame and a later in-point so it still plays the same audio at the same scene frames. **Cut** = Copy + Delete. **Paste** inserts the copied sound portion (placement fragments with their in/out points, volume and envelope) at the selected cell using *insert and shift*. |
| Delete sound | Removes the whole placement under the cursor |
| Show lip-sync | Switches the column to lip-sync mode. Placements without a valid lip-sync track get one computed first (detection job with progress, §10.4). |
| Recompute lip-sync | Re-runs detection for the selected placement(s), **discarding manual edits**. The user MUST be warned beforehand that manual edits will be overwritten; agents pass an explicit policy (§15). |

### 7.3 Lip-sync mode cell commands

The cell context menu of a sound column in lip-sync mode offers:

- **Set mouth** ▸ A, B, C, D, E, F, G, X — assigns that mouth to every selected frame and marks them manual. The mouth
  of the frame under the cursor is shown checked. Disabled for frames without a lip-sync track.
- Import sound (from file, or from the recent-sounds list).
- Insert cells, insert blank cell, delete blank cell; select whole/from start/to end; play/stop; cut/copy/paste/delete;
  delete sound; recompute lip-sync — all as in §7.2.
- **Hide lip-sync** — returns the column to waveform mode (the toggle's label reflects its state).

### 7.4 Column-header commands

The header context menu of a sound column offers the generic element commands (arrange, display/visibility, colour,
cut/copy, clone, delete, rename, add element note) and **Edit sound**. In lip-sync mode it also offers **Modify lip-sync
mapping** (§10.8). Commands that are meaningless for sound (e.g. sending the column to a light table) MUST be disabled
or omitted for sound columns.

---

## 8. Playback, mixing and scrubbing

### 8.1 Sound toggles

| Toggle | Default | Persistence | Behaviour |
|---|---|---|---|
| Sound playback | on | per user | When off, interactive playback is silent |
| Sound scrubbing | off | per user | Plays the frame's audio slice whenever the current frame changes in the Timeline (§8.3). Effective only while sound playback is on; shown disabled while sound playback is off |
| Waveform display | on | per user | Timeline (and Exposure Sheet) show waveforms vs. solid bars |

### 8.2 Interactive playback with sound

- When the scene is played (`09_playback_export.md`), all audible placements of the current scene are mixed and played
  in sync with the picture, starting at the current frame.
- **Mixing:** the scene sound track is the sum of every placement of every audible sound element, plus sounds contained
  in media-element sources (§11). For each placement: offset to its start frame, apply in/out points and loops, apply
  the envelope, apply volume, truncate at the stop frame. The sum MUST be protected from wrap-around clipping (soft
  limiter or hard clip at full scale; Design decision D5).
- **Audibility:** hidden or disabled sound elements are muted; locking does not mute.
- **Sync:** audio is the master clock. If the picture cannot keep up, frames are dropped when the *force frame rate*
  playback option is on; otherwise the picture lags and the audio is re-positioned at each displayed frame.
- When the playback rate is set to a value other than the scene frame rate, sound is muted (Design decision D4).
- Looped playback restarts the audio together with the picture. The playback range (`09_playback_export.md`) also
  limits the audio.
- **Across scenes:** a placement whose span runs past the end of its scene keeps playing into the following scenes in
  movie-level preview and export, until its stop frame or the end of its audio; scene transitions are honoured
  (`08_library_storyline.md`).
- Exposure-sheet preview in drawing mode also plays the sound columns.

### 8.3 Scrubbing

- **Frame scrubbing:** with sound scrubbing on, each change of the current frame in the Timeline (dragging the frame
  indicator, stepping to the previous/next frame) plays that frame's audio slice `[t, t + 1/fps)`. Each slice SHOULD
  have a short (≈ 5 ms) fade-in and fade-out to avoid clicks. While the user drags quickly, pending slice requests
  older than the latest one are dropped.
- **Continuous scrubbing** (used in the sound element editor when its scrubbing option is on, §9.4): plays the audio
  around the pointer's time position as the pointer moves.

### 8.4 One-shot playback

- Play/stop a single placement from the Exposure Sheet (§7.2), without the picture.
- Play/stop the sound contained in the linked source of a media cell (§11.4).
- Preview a Library sound item (`08_library_storyline.md`).
- Starting any other playback, or the *stop all sounds* command (also bound by default to the platform's cancel key),
  stops all one-shot sounds.

### 8.5 Preview window volume

The rendered-preview window (`09_playback_export.md`) MUST provide a volume control and a mute toggle in addition to its
transport and loop controls.

---

## 9. Sound element editor

### 9.1 Purpose, access and commit model

The sound element editor edits every placement of one sound element: start/stop frames, in/out points, loop count,
volume, envelope and fades, the element's streamed flag and the per-frame mouth assignment. It can also import and
delete placements and run lip-sync detection.

It MUST be reachable from: the main menu (with a sound element selected), the Timeline context menu of a sound track,
double-clicking a placement bar, the Exposure Sheet sound-column header menu, the sound toolbar, the Properties view
sound panel (§5) and the lip-sync panel (§10.7). It is enabled only when a sound element is selected.

The editor works on a **working copy** of the element. Confirming commits all edits as one undoable step labelled
*Edit Sound*; cancelling discards them. The editor is resizable; its layout is design work. It consists of two linked
panels described below, plus a lip-sync area.

### 9.2 Element panel (frame axis)

Shows every placement of the element on a frame axis, each labelled with its start and stop frame numbers.

| Item | Behaviour |
|---|---|
| Frame ruler with current-frame indicator | Clicking a frame number or dragging the indicator sets the editor's current frame; updates the current-frame readout and the mouth preview (§9.6) |
| Placement | Click to select; the selected placement becomes the *current sound* of the lower panel |
| Drag a placement | Moves it (start frame changes, length kept) |
| Start handle | Drag to change the placement's start frame (length kept) |
| Stop handle | Drag to change the stop frame |
| Lip-sync strip | When a lip-sync track exists, each frame shows its mouth assignment |
| Horizontal scroll | Scrolls the frame axis |

Rules: placements cannot overlap — drags stop at neighbouring placements; start frame ≥ 1; stop frame ≤ natural end.

Element transport: go to first frame (of the element, or of the selected placement), previous frame, play/stop (plays
the element's mixed audio from the current frame; the indicator follows), next frame, go to last frame, loop toggle.

Fields:

| Field | Type | Default | Range | Effect |
|---|---|---|---|---|
| Start frame | int | selected placement's start frame | 1 … (next placement's start − length) | Moves the placement |
| Stop frame | int | selected placement's stop frame | start frame … min(natural end, next placement's start − 1) | Truncates the placement |
| Current frame | read-only | editor's current frame | — | Display |
| Streamed | bool | off | — | Marks the whole element for streamed export (§13) |

An invalid entry (non-numeric or out of range) reverts to the last valid value when the field loses focus; typed
values are clamped to their range.

### 9.3 Current-sound panel (source-time axis)

Shows the full waveform of the selected placement's source audio on an axis in seconds.

| Item | Behaviour |
|---|---|
| Header | Sound name and the properties line (§2.4) |
| Seconds ruler | Automatic graduation; the kept range is marked on the ruler |
| Waveform | The kept part between in-point and out-point is visually distinguished from the trimmed head and tail |
| In-point handle | Drag to trim the head |
| Out-point handle | Drag to trim the tail |
| Drag the kept region | Slides the in/out window, keeping its length |
| Envelope line and points | §9.5 |
| Playback pointer | Shows the playing position |
| Horizontal scroll | Scrolls the time axis |

Current-sound transport: rewind (pointer to in-point) and play/stop (plays only this placement, with trim, envelope and
volume applied, from the pointer).

Fields:

| Field | Type | Default | Range | Effect |
|---|---|---|---|---|
| Start time (in-point) | seconds, 3 decimals | 0.000 | 0 ≤ value < out-point | Trims the head |
| Stop time (out-point) | seconds, 3 decimals | source duration | in-point < value ≤ duration | Trims the tail |
| Loop count | int | 1 | ≥ 1 | Repetitions of the kept segment |
| Volume | slider with numeric readout | 0 (centre) | −50 … +50 | §9.5 |

Changing in/out points or loop count updates the placement's span in the element panel (§1.2 rules).

### 9.4 Zoom and scrubbing

| Control | Range | Default | Effect |
|---|---|---|---|
| Zoom | 0 … 100 | 50 | Zoom level of both panels |
| Scrubbing | bool | off | When on, dragging the current-frame indicator or the playback pointer plays the audio under it (§8.3), so mouth assignments can be checked by ear |

### 9.5 Volume, envelope and fades

- **Envelope model:** an ordered list of points `(time, leftLevel, rightLevel)`; time in source-time seconds, levels
  linear gain 0.0 … 1.0 (mono sources use the left level). With no points the gain is 1.0 throughout. Gain is
  interpolated linearly between points and held constant before the first and after the last point. This model MUST be
  expressible as an SWF sound envelope on export (§13).
- **Display:** the envelope is drawn as a line over the waveform; full level at the top of the panel.
- **Editing gestures:**
  - clicking on the envelope line adds a point at that time and level;
  - dragging a point moves it; its time is constrained between its neighbours and its level to 0 … 1; with the
    constrain modifier held, only the level changes;
  - dragging a point vertically out of the panel, or selecting it and invoking delete, removes it.
- **Fade presets** (from the waveform panel's context menu; durations default to 0.5 s, or to the selected range):
  - *Fade in (d)*: points (in, 0) → (in + d, 1);
  - *Fade out (d)*: points (out − d, 1) → (out, 0);
  - *Fade in and out (dIn, dOut)*: both;
  - *Remove envelope*: clears all points.
  Applying a preset replaces any existing points inside the affected interval.
- **Volume:** a global gain on the placement applied on top of the envelope, intended for levelling tracks so the
  mixed output does not distort. Value 0 = unchanged; gain = 1 + v/100 (−50 → 50 %, +50 → 150 %) (Design decision D2).
  The editor SHOULD show a clip indicator when the placement's peak after gain exceeds full scale.

### 9.6 Lip-sync area

| Control | Behaviour |
|---|---|
| Detect lip-sync | Runs detection (§10.4) for the selected placement, or for all placements of the element if none is selected, with progress and cancel |
| Mouth preview | Large reference image of the mouth assigned to the current frame |
| Mouth label | Letter of the current frame's mouth, or the mapped drawing's label if one is stored |
| Eight mouth buttons A–X | Each shows the shape's reference image and letter; clicking assigns that mouth to the current frame (or to the selected frame range) and marks it manual; the preview updates |

These controls are enabled only when the selected placement has a lip-sync track (except *Detect lip-sync*).

Lip correction workflow: turn scrubbing on → drag the current-frame indicator slowly while listening → on a frame with
the wrong mouth, click the correct mouth button → repeat → confirm.

### 9.7 Import and delete

| Control | Behaviour |
|---|---|
| Import | Chooses a file and adds it to this element at the editor's current frame; if that frame is occupied, the new placement starts at the first free frame after the occupying placement |
| Delete | Removes the selected placement; disabled when none is selected |

### 9.8 Edge cases

- On an empty sound element, the editor opens with empty panels and only *Import* enabled.
- All fields are disabled while no placement is selected.
- A placement whose asset is missing is shown without a waveform and cannot be played; the missing-asset warning
  (§2.2) is shown once.
- If a placement's timing changes after its lip-sync track was computed, frames trimmed away lose their mouth data,
  frames added get X, and the track is marked *stale* so the user (or agent) can re-run detection.

---

## 10. Lip-sync

### 10.1 Overview

Lip-sync analyses a voice track (a sound placement) and produces a **lip-sync track**: one of eight mouth shapes per
frame. The track can be inspected and edited, then **applied** to a drawing element that contains a drawing for each
mouth shape. Applying writes that element's exposures so that the right mouth drawing is exposed on each frame. The
workflow is **detection → mapping → apply**; each step is independently available and repeatable.

### 10.2 The mouth chart

The application uses the eight-shape mouth chart that is a widespread convention in 2-D character animation: seven
speaking shapes, labelled A to G, plus a rest shape X. The letters are labels for *shapes*, not for sounds. Each shape
covers a group of speech sounds that look alike on the lips:

| Shape | Looks like | Speech sounds it covers (approximate) |
|---|---|---|
| **A** | Lips pressed together | m, b, p |
| **B** | Mouth slightly open, teeth close together | most consonants — s, z, t, d, k, g, n, j, h, y, ch, sh, th, l — and the narrow vowel "ee"/"i" |
| **C** | Mouth open, relaxed | open-mid vowels such as "e" in *bed*, "a" in *cat* |
| **D** | Mouth wide open | wide or stressed open vowels such as "a" in *father*, "i" in *ice* |
| **E** | Lips rounded into an "o" | "o" in *go*, "aw", "er" |
| **F** | Lips small and puckered | "oo", "u", "w" |
| **G** | Upper teeth resting on the lower lip | f, v, "ph" |
| **X** | Closed, relaxed rest mouth | silence, pauses, breaths and undetermined sound |

- The canonical order everywhere (UI, data, results) is A, B, C, D, E, F, G, X (indices 0–7).
- The application MUST ship its own reference artwork for each shape (a large image for previews and a small one for
  buttons and Exposure Sheet cells).
- The sound-to-shape grouping used by detection is configurable data (§10.5, phoneme map).

### 10.3 Lip-sync track data model

For each sound placement:

- `mouth[f]` ∈ {A … G, X} for every frame *f* of the placement (stored relative to the placement start; reported in
  scene frames);
- `manual[f]`: true when set by hand or explicitly by command;
- `label[f]`: optional display label (the mapped drawing's name);
- `source`: how the track was produced (`detected`, `transcript`, `phonemes`, `explicit`);
- `stale`: true when the placement's audio, in/out points, loop count, stop frame or the scene frame rate changed since
  the track was produced. A stale track is still shown and applicable;
- per sound element: `showMouthShapes` (Exposure Sheet display mode, §7.3).

Tracks and mappings are saved with the project.

### 10.4 Running detection

| Trigger | Behaviour |
|---|---|
| Exposure Sheet *show lip-sync* | Detects for placements without a track, then shows lip-sync mode |
| Exposure Sheet *recompute lip-sync* | Re-detects; manual edits are replaced (after warning) |
| Sound element editor *detect lip-sync* | Detects for the selected placement (or all) |
| Lip-sync panel *apply* | Detects if the selected sound has no track, then applies |

Detection is a background job:

- progress is shown (overall percentage; when several placements are processed, which one of how many);
- it is **cancellable**; on cancel the previous track (if any) is left untouched;
- it works offline;
- storing the result is one undoable step.

### 10.5 Detection — functional requirements

Input: the placement's trimmed, loop-expanded audio, mixed to mono. Output: a list of time segments
`(startTime, endTime, mouth)` and the derived per-frame track.

1. **Recognition.** The audio is analysed into a time-aligned sequence of speech sounds (phonemes) or directly into
   mouth classes. The engine is implementation-defined (Design decision D6) but MUST:
   - run offline with no network access;
   - be deterministic: the same audio, frame rate and options always produce the same track;
   - process at least 5× faster than real time on a mid-range laptop;
   - fall back gracefully for any language or for unclear audio by choosing shapes from acoustic cues (openness from
     loudness → B/C/D; rounding from low second-formant energy → E/F; frication → G/B) so the result is always
     plausible.
2. **Phoneme map.** Recognised phonemes are converted to mouths through an editable **phoneme map**. The default map
   (ARPAbet notation) is:

   | Mouth | Phonemes |
   |---|---|
   | A | M, B, P |
   | B | S, Z, T, D, K, G, N, NG, CH, JH, SH, ZH, TH, DH, Y, HH, IY, IH, L |
   | C | EH, AE, EY, AH (unstressed) |
   | D | AA, AY, AW, AH (stressed) |
   | E | AO, OW, OY, ER, R |
   | F | UW, UH, W |
   | G | F, V |
   | X | silence, pause, breath, noise |

   An IPA-keyed equivalent MUST also be accepted where phonemes are supplied by the caller (§10.6).
3. **Silence.** Wherever the short-term energy is below an adaptive threshold (default 32 dB below the utterance peak)
   for at least 60 ms, the result is X, regardless of recognition.
4. **Smoothing** (animation-friendly):
   - minimum hold: default 2 frames at ≥ 24 fps, 1 frame at ≤ 12 fps; shorter segments merge into the neighbour with
     the most similar shape (A↔G, C↔D, E↔F, B↔C);
   - closed-lip consonants are never dropped: an m/b/p shorter than a frame still forces A on its frame;
   - optional anticipation: shift the whole track earlier by N frames (default 0).
5. **Frame quantisation.** For each frame *f* of the placement (duration 1/fps), the mouth is the segment covering the
   largest part of the frame's time span; ties go to the segment active at the frame's start. Frames with no segment
   get X.
6. **Store.** The track is stored with all `manual` flags false (unless the caller asked to keep manual frames, §15).

Quality targets (acceptance): ≥ 80 % frame agreement with a hand-made track on clean English dialogue at 24 fps;
every silence longer than 150 ms rendered as X; closed-lip shapes present for ≥ 95 % of m/b/p occurrences;
transcript-assisted detection (§10.6) ≥ 90 % frame agreement.

Detection options (per call, with project-level defaults, §15): minimum hold frames, anticipation frames, silence
threshold (dB), language (default automatic), phoneme map.

### 10.6 Transcript-assisted detection

Detection MAY be given extra information to improve accuracy; this is essential for agents that cannot listen:

- **Transcript text:** the spoken words. The application converts text to phonemes (pronunciation dictionary plus
  letter-to-sound rules for unknown words) and **force-aligns** them to the audio, then continues at step 2 of §10.5.
  If alignment fails for part of the audio (low confidence), that part falls back to plain recognition and the result
  reports the affected time ranges as warnings (`lipsync.alignmentWeak`).
- **Timed phoneme list:** a list of `{phoneme, start, end}` (seconds in placement time or source time, stated by the
  caller) in ARPAbet or IPA. The application skips recognition and alignment and applies §10.5 steps 2–6 directly.
  Unknown phoneme symbols are reported (`lipsync.unknownPhoneme`) and treated as B.
- **Untimed phoneme list:** a list of phonemes without times is force-aligned like a transcript.

### 10.7 Lip-sync panel

A dockable/floating panel that brings the whole workflow together. It MUST be reachable from the window menu
(show/hide), the sound toolbar, the Properties view sound panel and the Timeline (with a drawing element selected it
opens with that element as the mouth element). It is created on first use and raised if already open.

| Control | Behaviour |
|---|---|
| Sound selector | Lists the sound elements of the current scene. Default: the selected sound element, else the last used, else the first. |
| Import sound | Imports a file into a new sound element and selects it in the sound selector |
| Mouth element selector | Lists the drawing elements of the scene. Selecting one makes it the destination and runs automatic mapping (§10.8) |
| Import mouth template | Opens the mouth-template chooser (§10.11); the chosen template becomes a new drawing element with eight mouth drawings and is selected as the mouth element |
| Apply | Detects if needed, then applies (§10.9) with the current mapping and stores the mapping. Enabled only when a sound and a mouth element are chosen and every mouth shape is mapped or deliberately left empty |
| Advanced options (disclosure) | Shows/hides the advanced area; the state persists per user |
| Edit sound (advanced) | Opens the sound element editor on the selected sound |
| Mapping rows (advanced) | One row per shape A–X: reference image of the shape with a tooltip describing the sounds it covers; previous/next buttons stepping through the mouth element's drawings (in drawing order, wrapping around); a thumbnail of the currently mapped drawing (blank when unmapped) |
| Modify lip-sync mapping | Opens the mapping-by-name form (§10.8) |

The panel refreshes when the scene changes (elements added, removed, renamed; drawings added, removed or renamed).

### 10.8 Mapping

**Automatic mapping** — when a mouth element is chosen, each shape is mapped by the first rule that matches:

1. the stored mapping for this (sound element, mouth element) pair;
2. a drawing whose name equals the shape letter (case-insensitive: "A" … "G", "X");
3. a drawing tagged with that shape through the drawing's *mouth-shape* property (mouth templates tag their drawings);
4. otherwise drawings in drawing order: 1st → A, 2nd → B, …, 8th → X.

A shape with no drawing stays unmapped.

**Mapping by name** (the *modify lip-sync mapping* form, reachable from the lip-sync-mode column header and from the
lip-sync panel): shows the source sound element (read-only), a destination selector listing only drawing elements,
and one text field per shape (default: the letter itself) next to the shape's reference image, for typing the name of
the drawing to use. Confirming applies the track with that mapping (§10.9) and stores the mapping.

- If the scene has no drawing element, the user is told there is nothing to apply lip-sync to
  (`lipsync.noDrawingElement`).
- A name that matches no existing drawing is still written as a cell label; the cell shows as missing until a drawing
  with that name exists (consistent with `04_timeline_xsheet.md`).
- An empty name means that shape produces empty cells.
- The form is enabled only when the sound has a lip-sync track.

### 10.9 Apply semantics

Applying the lip-sync track of a sound element (all its placements, or one placement) to a drawing element:

- For each frame *f* covered by the applied placements, the drawing element's cell at *f* receives an exposure of the
  drawing mapped to `mouth[f]`; consecutive frames with the same mouth become one exposure held over those frames.
- **Overwrite policy** (explicit parameter for agents; the GUI uses *replace*):
  - *replace* — existing exposures inside the applied span are overwritten;
  - *emptyOnly* — only empty cells inside the span are written; occupied frames are left and reported;
  - *failIfOccupied* — nothing is written if any cell in the span is occupied; the conflicting frames are reported
    (`lipsync.conflict`).
- Frames outside the applied span are never touched.
- **Unmapped shapes** produce empty cells (default), or — by option — the X drawing, or leave the existing cell.
- If the drawing element is shorter than the span, it is extended; the scene is extended if necessary.
- The drawing element's drawings themselves are not modified, and the mapping is stored with the project.
- The whole apply is one undoable step labelled *Apply Lip-sync*.
- Editing a track afterwards does **not** automatically re-apply it. An optional per-pair setting *auto re-apply on
  edit* (default off) re-applies the track after each edit using the stored mapping and policy *replace*.

### 10.10 Manual editing

| Where | How |
|---|---|
| Exposure Sheet (lip-sync mode) | Select one or more frames → *set mouth* A … X |
| Sound element editor | Set the current frame, click one of the eight mouth buttons; use scrubbing to listen |
| Re-detection | Replaces manual edits (after a warning in the GUI; agents pass `manualEdits` = `"keep"` or `"replace"`) |

Every manual edit is an undoable step labelled *Set Mouth*.

### 10.11 Mouth templates (bundled content)

- A **mouth-template chooser** shows the mouth templates available in the Library (thumbnail and name) and imports the
  chosen one as a new drawing element containing eight mouth drawings, with its own peg. Double-clicking a template
  imports it.
- The application MUST ship at least six mouth templates, authored fresh, covering a range of styles: at least one
  each for an adult human, a child, a stylised/simple cartoon mouth, an animal muzzle and an exaggerated comic style,
  in front view; some SHOULD include teeth and tongue detail.
- Each mouth template's eight drawings MUST be named exactly `A` … `G`, `X` **and** carry the matching mouth-shape
  property so automatic mapping always succeeds.
- Users MAY save their own mouth templates to the Library (`08_library_storyline.md`); a template counts as a mouth
  template when it contains a drawing element whose drawings carry mouth-shape properties.

---

## 11. Media elements

### 11.1 Purpose

A media element references content where it already is — in the Library or on disk — instead of copying it into the
project, saving space when the same content is used repeatedly. It MUST accept any bitmap, vector, SWF or sound source
the application can import, including at least JPEG, PNG, BMP, GIF, TGA, TIFF, SWF, WAV, AIFF and MP3, and Library
items (templates).

Its cells point to **media objects** in a project-wide registry; each media object refers to one Library item or one
external file. Several media elements or sequences can reference the same media object without duplicating data.

### 11.2 Creating

A media element is created from the same contexts as other elements (main menu, Exposure Sheet context menu and its
element-creation control, generic add-elements command with type = media). The default colour for media elements is a
user preference.

### 11.3 Filling a media element

- Dragging an item (template, image, SWF, sound) from the Library onto a media column or track at a frame links it there
  as a new **media sequence** starting at that frame.
- When dropping a Library item onto the Timeline, the user may choose between *copy* (content copied into a regular
  element, `08_library_storyline.md`) and *link* (creates or uses a media element).
- A sequence's initial length is the source's natural length: its frame count for a template or SWF, 1 frame for a
  still image, the duration in frames for a sound.
- Adjacent sequences referencing the same media object with contiguous link offsets are merged into one sequence.
- Inserting at a frame inside or before existing sequences pushes later sequences later (*insert and shift*).

### 11.4 Media cell commands

| Capability | Behaviour |
|---|---|
| Add / remove exposure, set exposure 1/2/3/custom, extend exposure | As for drawing columns (`04_timeline_xsheet.md`), applied to media cells |
| Insert cells, insert blank cell, delete blank cell | As for drawing columns |
| Create cycle / advanced cycle | As for drawing columns, cycling media cells |
| **Reset media timing** | Restores the selected sequence to its natural timing: one exposure per source frame starting at its link offset, for the full remaining length of the source. Disabled when the selection is not within a media sequence |
| Cut, copy, paste, delete | Operate on media cells |
| Play sound / stop sound | Plays or stops the sound contained in the linked source of the selected cell |

### 11.5 Display and information

- Media cells in the Exposure Sheet show the linked item's thumbnail and name.
- Hovering a sequence shows: media name; location (Library path or file path); frame range in the element and length;
  the source frame shown first (link offset); the source length.
- Linked content is rendered in the Camera View like the corresponding element type; sounds contained in linked sources
  take part in the playback mix (§8.2).
- If a linked file is missing, the user is told which file could not be found and offered to locate it (relink) or
  continue; unresolved sequences render as a missing-content placeholder and play silence (`media.sourceMissing`).
- SWF export offers an option to export media-element content as reusable symbols so a source linked several times is
  stored once (`09_playback_export.md`).

---

## 12. Movie import as an image sequence

### 12.1 Behaviour

The application MUST be able to import a movie file as a sequence of bitmap images into an image element. This is
reachable from the context menu of an image column in the Exposure Sheet and from the generic import command when a
movie file is chosen.

- Accepted formats: at least MOV, MP4/M4V (H.264, H.265, MPEG-4 Part 2, ProRes, Motion JPEG where decodable) and AVI;
  SHOULD accept any other format the platform decoder offers. The file chooser filters to supported movie formats.
- Steps:
  1. The user chooses a movie file; an options form (§12.2) is shown.
  2. The movie is sampled at the **scene frame rate** by default: image *n* is the movie frame displayed at movie time
     `(n − 1) / fps`, and each image is exposed for one frame starting at the selected cell, so playback duration equals
     movie duration. Alternative sampling: *one image per source frame*, each exposed for `round(sceneFps / sourceFps)`
     frames (minimum 1).
  3. Images are stored as bitmap drawings in the selected image element, named `<movie name>-<n>`.
  4. If an opacity is set, it is applied to the imported images.
  5. If the movie has an audio track, it is imported into a new sound element starting at the same frame. Failure to
     import the audio does not abort the image import; the user is informed (`media.audioImportFailed`).
  6. Progress (decoding and storing N frames) is shown; the import is cancellable; the whole import is one undoable step
     labelled *Import Movie*.

### 12.2 Import options

The import options form shows only the groups that apply to the file being imported:

| Group | Option | Type | Default | Range | Effect |
|---|---|---|---|---|---|
| Image | Fit image to camera width | bool | off (persisted per user) | — | Scales imported images to the camera width |
| Layered image (PSD) | Preserve source layers | bool | off | — | Each layer becomes a separate element/drawing (`09_playback_export.md`); unsupported colour modes produce a warning and a flattened import |
| Vector document (AI/PDF) | Preserve source layers | bool | off | — | As above |
| Movie | Set opacity | bool | off | — | Enables the opacity field |
| Movie | Opacity | int % | 100 | 1 … 100 | Alpha applied to the imported frames |
| Movie | Sampling | enum | scene frame rate | scene frame rate / one image per source frame | §12.1 step 2 |
| Movie | Import audio | bool | on | — | §12.1 step 5 |

(Image, PSD and AI/PDF import are specified in `02_drawing.md` and `09_playback_export.md`; they share this form.)

### 12.3 Video in the scene

Movie files are not time-based elements in the scene: video enters a scene only as an image sequence (§12) or as an
SWF source in a media element. Rendered previews are played in the preview window (`09_playback_export.md`).

---

## 13. Sound in exported output (summary)

Full details are in `09_playback_export.md`. Sound-specific rules:

- **Movie export (MP4/MOV):** the sound track is the mix of all audible placements and media-element sounds (§8.2),
  built per scene and concatenated in scene order with transitions honoured. Volume, envelope, in/out points, loops and
  stop frames are applied. Placements running past the end of their scene continue into following scenes.
- **SWF export:**
  - non-streamed sound elements are exported as *event sounds* triggered at each placement's start frame, carrying the
    in-point, out-point, loop count and envelope;
  - sound elements with the **streamed** flag are exported as a *stream sound* (audio blocks interleaved per frame), so
    playback can start while downloading and the picture stays locked to the audio;
  - audio is encoded as ADPCM, or embedded as MP3 when the source is MP3;
  - the export size report includes sound data.
- Image-sequence, PDF and print outputs carry no sound.

---

## 14. Undoable operations (summary)

Each of the following is one undo step: import sound (single or multi-file); add placement from Library; remove
placement; move placement; trim / set placement properties; split placement; delete or paste a sound frame range; all
edits of one sound element editor session (*Edit Sound*); envelope point add/move/remove; apply or remove fade;
set streamed flag; show/hide lip-sync mode; detect / recompute lip-sync; set mouth (manual edit); set lip-sync track;
clear lip-sync track; set lip-sync mapping; apply lip-sync; import mouth template; add/move/remove/re-time media
sequence; reset media timing; relink asset or media object; import movie. Generic cell edits use section 04's
commands. View toggles (waveforms, playback, scrubbing) and panel state are not undoable.

---

## 15. Agent interface

### 15.1 Agent-specific requirements

- **Everything numeric.** Every gesture of this section has a command with numeric parameters: placing, moving,
  trimming (in/out points in seconds, stop frame), looping, volume, envelope points and fades, splitting and deleting
  frame ranges. No drag, file chooser or dialog is ever the only path.
- **Units and spaces.** Frames are 1-based scene frames (§1.3). Times are seconds as numbers; each time parameter states
  whether it is *source time* (position in the asset, used for in/out points and envelope points) or *placement time*
  (since the placement's start). Levels are linear gain 0.0–1.0; volume is the integer −50…+50 of §9.5; `db` values are
  dBFS (0 = full scale).
- **Addressing.** Sound assets, placements, media objects and media sequences have stable opaque ids. Path forms are
  also accepted: `scene:"Intro"/element:"Voice"/placement:2` (placements numbered 1… by start frame) and
  `asset:"line_04.wav"`.
- **Listening without ears.** `sound.waveform.get` returns per-frame peak and RMS numbers and a silence flag, and
  `sound.silence.list` returns silence/speech ranges in frames and seconds, so an agent can align action, cuts and
  dialogue precisely without audio playback. `sound.waveform.get` over a scene (`sceneId`) reads the final mix, so an
  agent can check for clipping after setting volumes.
- **Lip-sync as data.** `lipsync.detect` is a job whose result is the per-frame mouth track. The agent MAY supply a
  transcript and/or a timed or untimed phoneme list (§10.6) for higher accuracy. `lipsync.track.set` lets the agent
  write a track directly (e.g. from its own alignment). Mapping and apply are separate commands; apply takes an explicit
  **overwrite policy** and supports `dryRun` to preview exactly which exposures would be written.
- **Confirmations become parameters.** Large-file warning → `allowLarge`; re-detection over manual edits →
  `manualEdits: "replace"|"keep"` (if omitted while manual frames exist, the command returns
  `{needs_confirmation: {reason: "lipsync.manualEditsExist", manualFrameCount}}` and changes nothing); apply over
  occupied cells → `overwrite`; overlapping placements → `onOverlap`.
- **Sound element editor.** The editor's working-copy/commit model is a GUI convenience. Agent commands act directly on
  the document, each as one undo step; several edits can be made atomic with the generic `batch`. The editor can still be
  opened, driven (`sound.editor.state.set`) and closed by commands for UI-level automation; while it is open, `sound.*`
  edit commands targeting the same element act on the editor's working copy and are committed by
  `sound.editor.close {commit: true}`.
- **Jobs.** `lipsync.detect` and `media.movie.import` are jobs (progress 0–100 %, cancellable via `job_cancel`; cancel
  leaves the document unchanged). Waveform computation is internal background work: `sound.waveform.get` waits for it by
  default, or returns `{status: "pending"}` with `wait: false`.
- **Determinism.** Waveform summaries, silence detection and lip-sync detection are deterministic for the same inputs
  and options.
- **Errors** (`area.reason`): `sound.formatUnsupported`, `sound.importFailed`, `sound.tooLarge`, `sound.fileNotFound`,
  `sound.assetMissing`, `sound.deviceUnavailable`, `sound.notSoundElement`, `sound.overlap`, `sound.outOfRange`,
  `sound.placementNotFound`, `sound.editorNotOpen`, `lipsync.noTrack`, `lipsync.noDrawingElement`,
  `lipsync.notDrawingElement`, `lipsync.mappingIncomplete`, `lipsync.conflict`, `lipsync.alignmentWeak` (warning),
  `lipsync.unknownPhoneme` (warning), `lipsync.manualEditsExist`, `media.notMediaElement`, `media.sourceMissing`,
  `media.unsupported`, `media.overlap`, `media.movieDecodeFailed`, `media.audioImportFailed` (warning),
  `media.notImageElement`. Warnings are returned in a `warnings[]` array of successful results.
- Creating empty sound and media elements uses `element.create` with `type: "sound"` / `"media"` (section 04);
  `sound.import` can also create the element itself.

### 15.2 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `sound.asset.probe` | query | Inspect an audio (or movie) file without importing it | `path: path` | `{decodable, format, channels, bitDepth, sampleRate, durationSec, fileSize, exceedsLimit}` | n/a |
| `sound.import` | action | Import an audio file and place it in a sound element (Import Sound) | `path: path; elementId?: id; newElementName?: string; sceneId?: id; frame?: int = current frame (≥1); onOverlap?: enum "insertShift"\|"reject"\|"nextFree" = "insertShift"; allowLarge?: bool = false` | `{elementId, placementId, assetId, startFrame, stopFrame, durationSec, reusedAsset, warnings[]}` | yes |
| `sound.asset.import` | action | Add an audio file to the project's sound assets without placing it | `path: path; allowLarge?: bool = false` | `{assetId, reusedAsset, durationSec}` | yes |
| `sound.asset.list` | query | List sound assets of the project | `missingOnly?: bool = false; unusedOnly?: bool = false` | `{assets: [{assetId, name, channels, bitDepth, sampleRate, durationSec, missing, placementCount}]}` | n/a |
| `sound.asset.get` | query | Get one asset's format, status and users | `assetId: id` | `{assetId, name, channels, bitDepth, sampleRate, durationSec, fileSize, missing, usedBy: [{placementId, elementId}]}` | n/a |
| `sound.asset.relink` | action | Point a (missing) asset at a new file; placements keep their timing | `assetId: id; path: path` | `{assetId, durationSec, durationChanged}` | yes |
| `sound.recent.list` | query | Recently imported sounds (for quick re-import) | `limit?: int = 10 (1–50)` | `{items: [{path, assetId?, name}]}` | n/a |
| `sound.placement.add` | action | Place an existing asset or Library sound item in a sound element | `elementId: id; assetId?: id; libraryItemId?: id; frame: int (≥1); onOverlap?: enum "insertShift"\|"reject"\|"nextFree" = "reject"` | `{placementId, startFrame, stopFrame}` | yes |
| `sound.placement.list` | query | List placements of an element or a scene | `elementId?: id; sceneId?: id` | `{placements: [Placement]}` (§15.4) | n/a |
| `sound.placement.get` | query | Full parameters of one placement | `placementId: id` | `Placement` (§15.4) | n/a |
| `sound.placement.remove` | action | Remove a placement (Remove Sound) | `placementId: id` | `{}` | yes |
| `sound.placement.move` | action | Move a placement to a new start frame, keeping its length | `placementId: id; startFrame: int (≥1); onOverlap?: enum "reject"\|"clamp"\|"insertShift" = "reject"` | `{startFrame, stopFrame}` | yes |
| `sound.placement.trim` | action | Set in/out points (source seconds) and/or truncate at a stop frame | `placementId: id; inPoint?: number (≥0); outPoint?: number (≤ duration); stopFrame?: int \| "natural"` | `{inPoint, outPoint, startFrame, stopFrame, naturalEndFrame, clamped}` | yes |
| `sound.placement.set` | action | Set loop count and/or volume (and any trim/move field) of a placement | `placementId: id; loops?: int (≥1); volume?: int (−50–50); inPoint?: number; outPoint?: number; startFrame?: int; stopFrame?: int \| "natural"` | `Placement` | yes |
| `sound.placement.split` | action | Split a placement into two at a frame | `placementId: id; frame: int` | `{firstPlacementId, secondPlacementId}` | yes |
| `sound.range.delete` | action | Empty a frame range of a sound element, splitting and trimming placements (§7.2) | `elementId: id; startFrame: int; endFrame: int; closeGap?: bool = false` | `{removedPlacementIds[], createdPlacementIds[]}` | yes |
| `sound.range.select` | ui | Select frames of a placement in the Exposure Sheet/Timeline (whole, from start, to end) | `placementId: id; mode: enum "whole"\|"fromStart"\|"toEnd" = "whole"; frame?: int = current frame` | `{elementId, startFrame, endFrame}` | no |
| `sound.envelope.get` | query | Read a placement's envelope | `placementId: id` | `{points: [{time, left, right}]}` | n/a |
| `sound.envelope.set` | action | Replace the whole envelope | `placementId: id; points: [{time: number, level?: number (0–1), left?: number, right?: number}]` | `{points[]}` | yes |
| `sound.envelope.point.add` | action | Add one envelope point | `placementId: id; time: number; level?: number (0–1) = interpolated; left?: number; right?: number` | `{index, points[]}` | yes |
| `sound.envelope.point.move` | action | Move/re-level one envelope point (time clamped between neighbours) | `placementId: id; index: int; time?: number; level?: number (0–1); left?: number; right?: number` | `{points[]}` | yes |
| `sound.envelope.point.remove` | action | Remove one envelope point | `placementId: id; index: int` | `{points[]}` | yes |
| `sound.envelope.clear` | action | Remove the envelope (unity gain) | `placementId: id` | `{}` | yes |
| `sound.fade.apply` | action | Apply a fade preset | `placementId: id; type: enum "in"\|"out"\|"inOut"; inDuration?: number = 0.5; outDuration?: number = 0.5` | `{points[]}` | yes |
| `sound.element.streamed.set` | toggle | Mark a sound element for streamed SWF export | `elementId: id; enabled: bool` | `{enabled}` | yes |
| `sound.waveform.get` | query | Numeric waveform summary per frame (or per time bucket for an asset) | `placementId?: id; elementId?: id; sceneId?: id; assetId?: id; startFrame?: int; endFrame?: int; bucketsPerFrame?: int = 1 (1–32); bucketSeconds?: number = 0.04 (assets only); channels?: enum "mix"\|"separate" = "mix"; wait?: bool = true` | `WaveformSummary` (§15.4) | n/a |
| `sound.silence.list` | query | Silence and speech ranges | `placementId?: id; elementId?: id; assetId?: id; thresholdDb?: number = -40; minSilenceMs?: int = 100` | `{silences: [Range], speech: [Range]}` with `Range = {startFrame, endFrame, startTime, endTime}` | n/a |
| `sound.play` | ui | Play audio without picture: one placement, one element's mix, an asset, or the scene mix | `placementId?: id; elementId?: id; assetId?: id; sceneId?: id; fromFrame?: int; toFrame?: int; loop?: bool = false` | `{playbackId}` | no |
| `sound.stop` | ui | Stop one-shot sound playback (one or all) | `playbackId?: id; all?: bool = true` | `{}` | no |
| `sound.playback.set` | toggle | Enable/disable sound during interactive playback (per user) | `enabled: bool` | `{enabled}` | no |
| `sound.scrubbing.set` | toggle | Enable/disable Timeline sound scrubbing (per user) | `enabled: bool` | `{enabled, effective}` | no |
| `sound.waveforms.set` | toggle | Show waveforms or solid bars in Timeline/Exposure Sheet (per user) | `enabled: bool` | `{enabled}` | no |
| `sound.settings.get` | query | Read sound playback/scrubbing/waveform settings and audio device status | — | `{playback, scrubbing, waveforms, deviceAvailable, outputDevice}` | n/a |
| `sound.editor.open` | ui | Open the sound element editor on an element | `elementId: id; placementId?: id` | `{editorId}` | no |
| `sound.editor.close` | action | Close the editor, committing all its edits as one *Edit Sound* step or discarding them | `commit: bool` | `{committed}` | yes |
| `sound.editor.state.get` | query | Editor state | — | `{open, elementId, selectedPlacementId, currentFrame, zoom, scrubbing, loop, dirty}` | n/a |
| `sound.editor.state.set` | ui | Set editor current frame, selection, zoom, scrubbing, loop | `currentFrame?: int; selectedPlacementId?: id; zoom?: int (0–100); scrubbing?: bool; loop?: bool` | editor state | no |
| `sound.element.changed` | event | Placements, envelope or flags of a sound element changed | `{elementId, placementIds[]}` | — | n/a |
| `sound.asset.missing` | event | A referenced asset could not be found on open | `{assetId, path}` | — | n/a |
| `sound.waveform.ready` | event | Waveform cache computed for an asset | `{assetId}` | — | n/a |
| `lipsync.shapes.list` | query | The eight mouth shapes with descriptions, covered sounds and reference images | — | `{shapes: [{mouth: "A".."X", index, description, sounds[], referenceImage}]}` | n/a |
| `lipsync.detect` | job | Detect the mouth track for a placement (or all placements of an element), optionally transcript- or phoneme-assisted | `placementId?: id; elementId?: id; transcript?: string; phonemes?: [{phoneme: string, start?: number, end?: number}]; phonemeAlphabet?: enum "arpabet"\|"ipa" = "arpabet"; timeBase?: enum "placement"\|"source" = "placement"; language?: string = "auto"; minHoldFrames?: int; anticipationFrames?: int; silenceThresholdDb?: number; phonemeMap?: object; manualEdits?: enum "replace"\|"keep"` | `{jobId}`; on completion `{tracks: [LipTrack], warnings[]}` | yes |
| `lipsync.track.get` | query | Read the per-frame mouth track | `placementId?: id; elementId?: id; startFrame?: int; endFrame?: int; includeSegments?: bool = false; format?: enum "list"\|"string" = "list"` | `{tracks: [LipTrack]}` (§15.4) | n/a |
| `lipsync.track.set` | action | Write a mouth track explicitly (whole or part) | `placementId: id; startFrame?: int = placement start; mouths: string ("AABBX…") \| [enum "A"\|"B"\|"C"\|"D"\|"E"\|"F"\|"G"\|"X"]; markManual?: bool = true` | `{framesWritten, track: LipTrack}` | yes |
| `lipsync.track.clear` | action | Delete the lip-sync track of a placement or element | `placementId?: id; elementId?: id` | `{}` | yes |
| `lipsync.mouth.set` | action | Set one mouth on a frame range and mark it manual (Set Mouth) | `elementId: id; startFrame: int; endFrame?: int = startFrame; mouth: enum "A"\|"B"\|"C"\|"D"\|"E"\|"F"\|"G"\|"X"` | `{framesWritten}` | yes |
| `lipsync.display.set` | toggle | Show/hide lip-sync mode for a sound column (detects first if needed) | `elementId: id; enabled: bool; detectIfMissing?: bool = true` | `{enabled, jobId?}` | yes |
| `lipsync.settings.get` | query | Project defaults for detection | — | `{minHoldFrames, anticipationFrames, silenceThresholdDb, language, phonemeMap}` | n/a |
| `lipsync.settings.set` | action | Set project defaults for detection (including the phoneme map) | `minHoldFrames?: int (0–6); anticipationFrames?: int (0–4); silenceThresholdDb?: number (10–60); language?: string; phonemeMap?: object; resetPhonemeMap?: bool` | settings | yes |
| `lipsync.mapping.get` | query | Read the stored mapping, or resolve the automatic mapping, for a sound/drawing pair | `soundElementId: id; drawingElementId: id; resolve?: enum "stored"\|"auto" = "auto"` | `{mapping: {A..X: {drawingId?, drawingName?, rule: "stored"\|"name"\|"property"\|"order"\|"none"}}, complete, autoReapply}` | n/a |
| `lipsync.mapping.set` | action | Store a full or partial mapping (by drawing id or name; null = empty) | `soundElementId: id; drawingElementId: id; mapping: object {A..X: id \| string \| null}; autoReapply?: bool` | `{mapping, complete}` | yes |
| `lipsync.mapping.mouth.set` | action | Map one shape to a drawing, or step to the previous/next drawing | `soundElementId: id; drawingElementId: id; mouth: enum "A"\|"B"\|"C"\|"D"\|"E"\|"F"\|"G"\|"X"; drawing?: id \| string \| null; step?: enum "next"\|"previous"` | `{mouth, drawingId, drawingName}` | yes |
| `lipsync.apply` | action | Write the drawing element's exposures from the mouth track (Apply Lip-sync; a dry run changes nothing) | `soundElementId?: id; placementId?: id; drawingElementId: id; overwrite: enum "replace"\|"emptyOnly"\|"failIfOccupied"; mapping?: object; startFrame?: int; endFrame?: int; unmapped?: enum "empty"\|"rest"\|"keep" = "empty"; detectIfMissing?: bool = true; storeMapping?: bool = true; dryRun?: bool = false` | `ApplyResult` (§15.4) | yes |
| `lipsync.template.list` | query | Mouth templates available in the Library | — | `{templates: [{libraryItemId, name, thumbnail}]}` | n/a |
| `lipsync.template.import` | action | Import a mouth template as a new drawing element (eight tagged drawings) | `libraryItemId: id; sceneId?: id; name?: string` | `{drawingElementId, pegId, drawings: {A..X: drawingId}}` | yes |
| `lipsync.panel.open` | ui | Open/raise the lip-sync panel | `soundElementId?: id; drawingElementId?: id` | `{}` | no |
| `lipsync.panel.close` | ui | Close the lip-sync panel | — | `{}` | no |
| `lipsync.panel.state.set` | ui | Set the panel's selected sound, mouth element and advanced-area visibility | `soundElementId?: id; drawingElementId?: id; showAdvanced?: bool` | `{soundElementId, drawingElementId, showAdvanced}` | no |
| `lipsync.track.changed` | event | A mouth track was detected, set or edited | `{placementId, elementId, source}` | — | n/a |
| `lipsync.track.stale` | event | A track no longer matches its placement's timing or the frame rate | `{placementId}` | — | n/a |
| `media.sequence.add` | action | Link a Library item or external file into a media element as a new sequence | `elementId?: id; sceneId?: id; newElementName?: string; libraryItemId?: id; path?: path; frame: int (≥1); linkOffset?: int = 1; length?: int; onOverlap?: enum "insertShift"\|"reject" = "insertShift"` | `{elementId, sequenceId, mediaObjectId, startFrame, stopFrame}` | yes |
| `media.sequence.list` | query | List sequences of a media element | `elementId: id` | `{sequences: [MediaSequence]}` | n/a |
| `media.sequence.get` | query | One sequence (name, location, frames, link offset, source length) | `sequenceId: id` | `MediaSequence` (§15.4) | n/a |
| `media.sequence.remove` | action | Remove a sequence | `sequenceId: id; closeGap?: bool = false` | `{}` | yes |
| `media.sequence.move` | action | Move a sequence to a new start frame | `sequenceId: id; startFrame: int (≥1); onOverlap?: enum "reject"\|"insertShift" = "reject"` | `{startFrame, stopFrame}` | yes |
| `media.sequence.set` | action | Change a sequence's stop frame or link offset | `sequenceId: id; stopFrame?: int; linkOffset?: int (≥1)` | `MediaSequence` | yes |
| `media.sequence.naturalTiming.apply` | action | Reset a sequence to one exposure per source frame from its link offset (Reset Media Timing) | `sequenceId: id` | `MediaSequence` | yes |
| `media.object.list` | query | Project media registry (linked sources, location, missing state, usage) | `missingOnly?: bool = false` | `{objects: [{mediaObjectId, name, kind, location, missing, frameCount, durationSec?, sequenceCount}]}` | n/a |
| `media.object.relink` | action | Point a media object at a new file or Library item | `mediaObjectId: id; path?: path; libraryItemId?: id` | `{mediaObjectId, frameCount}` | yes |
| `media.sound.play` | ui | Play the sound contained in a media cell's linked source | `elementId: id; frame: int` | `{playbackId}` | no |
| `media.sound.stop` | ui | Stop media sound playback | — | `{}` | no |
| `media.movie.probe` | query | Inspect a movie file | `path: path` | `{decodable, durationSec, fps, frameCount, width, height, hasAudio, audioFormat?}` | n/a |
| `media.movie.import` | job | Import a movie as an image sequence (plus optional audio) | `path: path; elementId?: id; newElementName?: string; sceneId?: id; frame?: int = current frame; sampling?: enum "sceneRate"\|"sourceFrames" = "sceneRate"; sourceStart?: number; sourceEnd?: number; opacity?: int (1–100) = 100; importAudio?: bool = true` | `{jobId}`; on completion `{elementId, drawingIds[], startFrame, stopFrame, soundElementId?, placementId?, warnings[]}` | yes |
| `media.source.missing` | event | A linked media source could not be found | `{mediaObjectId, location}` | — | n/a |

### 15.3 Queries and events

- `sound.placement.get` / `.list` give everything needed to reason about timing: start/stop frame, natural end, in/out
  points, loops, volume, envelope, asset format, whether a lip-sync track exists and whether it is stale.
- `sound.waveform.get` is the agent's substitute for listening: one record per frame (or per bucket), with min/max
  peaks, RMS and a silence flag. Combined with `sound.silence.list`, an agent can place a placement so speech starts on a
  given frame, cut at a pause, or sync an action to a loud beat.
- `lipsync.track.get` returns the per-frame track; with `format: "string"` it returns a compact string such as
  `"XXAABBCCDD…"` starting at `startFrame`, convenient for inspection and for round-tripping through `lipsync.track.set`.
- `lipsync.apply` with `dryRun: true` returns the exposures that would be written and the conflicts, without changing
  the document. After applying, the agent can verify with section 04's cell queries or with `render_frame`.
- Events: `sound.element.changed`, `sound.asset.missing`, `sound.waveform.ready`, `lipsync.track.changed`,
  `lipsync.track.stale`, `media.source.missing` (all via `app://events`). Job progress for `lipsync.detect` and
  `media.movie.import` is reported through `job_status`.

### 15.4 Result shapes

```json
Placement = {
  "placementId": "p_19", "elementId": "e_7", "assetId": "a_3", "assetName": "line_04.wav",
  "startFrame": 25, "stopFrame": 96, "naturalEndFrame": 96,
  "inPoint": 0.250, "outPoint": 3.250, "loops": 1, "volume": 0,
  "envelope": [{"time": 0.25, "left": 0.0, "right": 0.0}, {"time": 0.75, "left": 1.0, "right": 1.0}],
  "durationSec": 3.0, "hasLipSync": true, "lipSyncStale": false, "missing": false
}

WaveformSummary = {
  "fps": 24, "startFrame": 25, "endFrame": 96, "bucketsPerFrame": 1, "channels": "mix",
  "frames": [
    {"frame": 25, "min": -0.02, "max": 0.03, "rms": 0.01, "peakDb": -30.5, "rmsDb": -40.0, "silent": true},
    {"frame": 26, "min": -0.41, "max": 0.44, "rms": 0.18, "peakDb": -7.1, "rmsDb": -14.9, "silent": false}
  ]
}

LipTrack = {
  "placementId": "p_19", "fps": 24, "startFrame": 25, "source": "transcript", "stale": false,
  "frames": [{"frame": 25, "mouth": "X", "manual": false, "label": null},
             {"frame": 26, "mouth": "A", "manual": false, "label": "A"}],
  "segments": [{"start": 0.00, "end": 0.06, "mouth": "X", "phoneme": "sil"},
               {"start": 0.06, "end": 0.14, "mouth": "A", "phoneme": "M"}]
}

ApplyResult = {
  "drawingElementId": "e_9", "span": {"startFrame": 25, "endFrame": 96},
  "exposures": [{"startFrame": 25, "length": 2, "drawing": "X"}, {"startFrame": 27, "length": 3, "drawing": "A"}],
  "framesWritten": 72, "framesSkipped": [], "conflicts": [], "unmappedMouths": [], "elementExtended": false,
  "dryRun": false
}

MediaSequence = {
  "sequenceId": "s_4", "elementId": "e_12", "mediaObjectId": "m_2", "name": "explosion",
  "location": "library:/Effects/explosion", "startFrame": 40, "stopFrame": 63,
  "linkOffset": 1, "sourceLength": 24, "missing": false
}
```

---

## 16. Design decisions / open questions

| # | Topic | Decision to make | Recommended default |
|---|---|---|---|
| D1 | Sound file size limit | Threshold for the large-file warning | Warn above 60 minutes of audio or 2 GB of decoded PCM; import still allowed after confirmation (`allowLarge`) |
| D2 | Volume unit | Mapping of the −50…+50 volume value to gain | Linear: gain = 1 + v/100 (50 %–150 %), numeric readout and clip indicator. Alternative: v/5 dB (−10…+10 dB) |
| D3 | Overlap on import/move | Behaviour when a new or moved placement overlaps another | Cell/menu import: insert and shift; Timeline drag/drop: refuse; editor import: next free frame; agents choose via `onOverlap` |
| D4 | Sound at non-native playback rates | Whether to play sound when playback fps ≠ scene fps | Mute sound |
| D5 | Mix clipping | How the summed mix is protected | Soft limiter at −1 dBFS; hard clip as fallback in exports |
| D6 | Detection engine | Technology for lip-sync detection | Offline phoneme recogniser producing time-aligned phonemes + the phoneme map (§10.5); forced alignment when a transcript is given. Alternative: a small direct 8-class classifier trained on permissively licensed, force-aligned speech data, decoded with class-transition probabilities and minimum-duration constraints. Ship pronunciation data for English at minimum; other languages SHOULD be added |
| D7 | Default apply behaviour in the GUI | Overwrite policy and unmapped shapes | *replace* within the span; unmapped shapes → empty cells |
| D8 | Automatic mapping order | Rules for mapping drawings to shapes | Stored mapping → name equals letter → mouth-shape property → drawing order |
| D9 | Track staleness | What happens to a lip-sync track after timing changes | Keep it, mark stale, emit `lipsync.track.stale`; never re-detect automatically |
| D10 | Auto re-apply | Whether editing a track updates exposures | Off by default; opt-in per sound/drawing pair |
| D11 | Hidden sound elements | Audibility | Hidden or disabled → muted; locked → audible |
| D12 | Stop-all-sounds binding | How one-shot sounds are stopped globally | Platform cancel key, and any new playback stops one-shot sounds |
| D13 | Waveform cache | Whether and how peaks are cached | Cache min/max per channel at ≈ 256 samples per bucket alongside each asset (format in section 10); rebuildable at any time |
| D14 | Movie sampling default | Scene-rate sampling vs. one image per source frame | Scene-rate sampling (duration-preserving) |
| D15 | Mouth template coverage | How many templates to ship | At least six styles (§10.11), all with drawings named and tagged A–G, X |
