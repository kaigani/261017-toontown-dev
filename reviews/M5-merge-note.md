# M5 merge note: Timeline and Exposure Sheet (draft, kept from the start)

Branch `m5-timeline-xsheet` off `main` at `7f93687e` (tag `m4`). Builder: Claude Opus 5.5. Completed at the end of
M5; until then only the table of §4 is kept current.

## 1. Checkpoints and closures

| Checkpoint | Scope | Closure |
|---|---|---|
| C5.1a | Timeline kernel: `timeline.*`, interactive playback, the timing-gesture interpreter, the `xsheet` composition | not handed off |
| C5.1b | the Timeline view | not started |
| C5.2 | the Exposure Sheet, cell commands, the Properties and Cells panels | not started |

## 4. Tests that failed on a commit later green on rerun (the M11 list)

Every test that fails on a commit whose product is unchanged, its cause and what was done (no rerun in the hope it
passes).

| Test (owner) | Runs | Done |
|---|---|---|
| `drawing byte gate (macos-latest)`: every three-way row recorded before the row "zoom (click)" (the builder's `scripts/c42-gesture-equivalence.py`) | 45082297045 (on `869a18e4`; Windows green in the same run; macOS green on `main`) | **not a margin: a product defect from M4.** The Drawing view's canvas reads its mapping (`draw.viewMapping.get`) at mount and again after reporting its size; the host serves a window's calls concurrently, so the mount read (the old 497-px height) answered last and the canvas kept the old mapping (offset 248.5 against the kernel's 242) until the zoom row's `shell.view` made it read again. The window's gestures mapped with the stale offset, so the window, the replay and the direct commands disagreed. Not macOS-specific (the slower runner made the late answer likely). Fixed with the D-89 read gate on the mapping reads (`desktop/src/canvas/CanvasView.tsx`); the D-89 sweep had covered the panels, not the canvas's own mapping |
