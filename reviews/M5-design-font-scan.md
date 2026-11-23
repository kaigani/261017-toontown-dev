# M5 design note: commands that wait on the system-font scan (ruling E-41)

Builder: Claude Opus 5.5. Status: **for the orchestrator's answer; nothing is built.** Found by run 45206466211
attempt 1 (`c33 render release (windows-latest)`, merge note §4 and §5).

## 1. What happens today

### 1.1 When the scan starts

- The font list is one process-wide value, `tt_vector::fonts::fonts()` (`crates/vector/src/fonts.rs:208`), a `OnceLock`
  filled by `fontdb::Database::load_system_fonts()`. That call opens and parses the header of every file in the
  platform's font folders (Windows: `%WINDIR%\Fonts` and the user's fonts; macOS: the system, library and user font
  folders).
- Every engine start begins the scan on a background thread (`fonts::warm()`, called by `ExecutorHandle::spawn_full`,
  `crates/executor/src/lib.rs:339`). That covers the desktop application (the engine starts with the window, so the scan
  runs while the start screen shows), the headless server and MCP, and every test that spawns an engine.
- A command that needs the list before the scan finishes **blocks on the `OnceLock` until it finishes**, however long
  that takes. Nothing bounds or reports the wait.

### 1.2 Which commands wait, and why

The list is needed to **resolve a text run's font** (`fonts::resolve`): the family as installed, else the platform's
default sans-serif face, else a fixed fallback metric (02 §3.7, lines 215-216: a missing font displays with a fallback
and keeps its name). Its callers:

| Need | Reached from |
|---|---|
| A text node's **layout** (advances, line breaks, ascender and descender), hence its **bounds** (`drawing::node_bounds`, `drawing_bounds`, `text_layout`) | the render list (`commands/render_list.rs:293-294`: every displayed drawing element's bounds and extent), so `render.frame.run`, `render_frame`, `inspect.frame.describe`, `inspect.hitTest`, the views' frames and thumbnails; drawing bounds and selection (`draw.bounds.get`, `draw_select.rs`, `draw_transform.rs`, `draw_cut.rs`, `draw.rs`); colour and stroke operations over a drawing with text (`colour.rs`, `draw_strokeops.rs`); the animation evaluator's bounds (`anim_eval.rs`); the envelope gesture (`envelope_gesture.rs`) |
| A text node's **glyph outlines** (`ops::glyphs::shapes`) | drawing a text node (`draw_render.rs:433`), converting text to strokes (`draw_strokeops.rs:453`) |
| Editing text (`text::layout`) | `draw.text.*` (`draw_text.rs`), the Text tool's editing (`text_edit.rs`) |
| Whether a family is installed (`fonts::is_installed`) | `doc.validate`'s `missingFonts` lint (`doc_lint.rs:564`) |

**A frame without visible text still waits** when the scene holds a text node anywhere in a displayed drawing element:
the render list computes `drawing_bounds` for every displayed drawing element **before** it decides the element's
verdict (hidden, excluded, out of frame), because `inspect.frame.describe` (detail `full`) reports the bounds of hidden
entries too (`inspect.rs:265`). That is what CI hit: the sample fixture's text node "Caption" sits in Bob's drawing;
`c33_bitmap` hides Bob, yet its first `render.frame.run` and `inspect.hitTest` blocked. Reproduced locally by holding
the scan 70 s (both tests fail with `command.timeout`, exactly as on CI).

### 1.3 What a frame without text needs from the scan

Nothing. Strokes, fills, bitmaps, images and the camera need no font. A frame needs the list only for a text node it
draws (outlines) or whose bounds it uses to decide the verdict of a drawn element (in frame or out of it).

### 1.4 How long the scan takes

On this workstation the whole `c33_bitmap` binary (engine start, scan, open, render) takes about 2 s; CI's Windows
runner took 1.9 s, 9.8 s and 2.4 s in three green runs and over 60 s once (the job's first process to render; the
binaries after it, with the font files cached by the system, were fast). The scan's own duration is not measured
today. The slow case is a cold disk and an antivirus scanning each font file on first open, with several hundred files
on a runner image. A person's machine with many fonts installed, on a slow disk, can see the same.

### 1.5 What the caller sees

The host's watchdog cancels the call at its budget (60 s) with `command.timeout` (SI-020; "cancelled
unless it had already committed") and the generic hint to read `doc.state.get` and retry. Nothing tells the person or
the agent that the fonts are the cause, or when to retry.

## 2. Options

| Option | What it does | Cost and risk |
|---|---|---|
| **(a) Background start at engine start** | already built (`fonts::warm()`) | does not help when the first command arrives before the scan ends (headless and MCP: a render right after start; tests), nor when the scan outlasts the budget |
| **(b) No wait without text** | the render list computes a text node's bounds only when it needs them: for an element it may draw (not hidden, not excluded), and lazily for `inspect.frame.describe`'s `full` detail of hidden entries (that one read still waits). Commands over drawings without text never touch the list (already true) | small, local to `render_list.rs` and `inspect.rs`; output unchanged (the same bounds are computed when read). Covers every frame without a drawn text node, which is the CI case and most frames |
| **(c) Bounded wait with a structured answer** | a command that needs the list waits for it within a stated bound (proposed 20 s, under the 60 s budget), then answers before it changes anything: `command.timeout` with `details.reason: "fontsLoading"` and the hint "the system fonts are still being read; retry in a few seconds". The window shows "Reading the system fonts" on the status line while it waits. The spec's error list has no code for "not ready"; `command.timeout` (SI-020) already carries reasons by ruling (`cancelled`, SI-111; `engineRestarted`, SI-120), so no new code is needed | moderate: the wait becomes a timed wait on the list (a condition variable beside the `OnceLock`), checked at each entry point in the table of §1.2 (one function, `fonts::ready_within(bound)`, called by the handlers before their first text layout). Determinism is kept: a command either lays out with the full list or refuses, never with a partial list |
| **(d) Cache the scan across runs** | the font list (paths, families, styles, file sizes and modification times) is saved in the user's cache folder; at start the cached list is used at once and checked against the folders in the background; a changed folder triggers a full scan, and the cache is replaced when it ends | largest: `fontdb`'s face records are not serialisable as they are, so the cache needs its own format and validation; and between start and the check a font installed since the last run is missing, so layout could differ from the same document's layout a minute later. The determinism question (02 §3.7, ADR-0016: layout bounds depend on the machine's fonts) needs a ruling before it is built |
| (e) Resolve only the named families | ask the platform for a family's file (DirectWrite, Core Text) instead of scanning every file | platform code on two systems, a dependency or `unsafe` bindings; out of proportion |

## 3. Recommendation

**(b) and (c) together, (a) kept, (d) not now.**

- **(b)** removes the wait from every frame that draws no text, which is the case CI hit and the common one. The
  render list's output does not change; only when bounds are computed.
- **(c)** turns the remaining wait (a frame that draws text, text editing, the lint) into a bounded wait with a
  structured, retryable answer that names its cause, for the person (the status line) and for the agent (the reason and
  the hint). It needs no spec A member and no new error code (a reason on `command.timeout`, as SI-111 and SI-120 did).
- **(a)** stays: in the application the scan usually ends before the first frame anyway.
- **(d)** only if measurements show slow scans on real machines: the change (b) and (c) make also records the scan's
  duration (a field of `doc.state.get`'s diagnostics is a spec A addition, so stderr and the application log only), so
  the question can be answered with numbers. The determinism question of (d) goes to the user if it is taken up.

**Tests, if approved:** a dev-only `test-hooks` hold on the scan (as in the reproduction), then: a frame with a hidden
text node and none drawn renders, hit-tests and describes (except the hidden entry's bounds) while the scan is held; a
frame that draws text answers `command.timeout` with `reason: "fontsLoading"` after the bound and changes nothing; both
answer normally once the scan is released; the same for `draw.bounds.get` on a drawing with text and for
`doc.validate`. `c33_bitmap` then no longer depends on the scan (its frame draws no text), and its row leaves the merge
note's table only once the change is in a green run.

## 4. Questions

1. Approve (b) and (c), and the bound (20 s)?
2. The reason's name (`fontsLoading`) on `command.timeout`, recorded as an SI beside SI-111 and SI-120?
3. Which checkpoint: C5.1b (it touches the render list the Timeline's thumbnails and the views use), or a separate fix
   at C5.1b's handoff?
