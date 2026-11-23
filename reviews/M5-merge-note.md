# M5 merge note: Timeline and Exposure Sheet (draft, kept from the start)

Branch `m5-timeline-xsheet` off `main` at `7f93687e` (tag `m4`). Builder: Claude Opus 5.5. Completed at the end of
M5; until then only the table of §4 is kept current.

## 1. Checkpoints and closures

| Checkpoint | Scope | Closure |
|---|---|---|
| C5.1a | Timeline kernel: `timeline.*`, interactive playback, the timing-gesture interpreter, the `xsheet` composition | handed off on `ba8e1946` (run 45100267389, 36 of 36); `reviews/M5-C5.1a-handoff.md` |
| C5.1b | the Timeline view | in progress |
| C5.2 | the Exposure Sheet, cell commands, the Properties and Cells panels | not started |

## 4. Tests that failed on a commit later green on rerun (the M11 list)

Every test that fails on a commit whose product is unchanged, its cause and what was done (no rerun in the hope it
passes).

| Test (owner) | Runs | Done |
|---|---|---|
| `drawing byte gate (macos-latest)`: every three-way row recorded before the row "zoom (click)" (the builder's `scripts/c42-gesture-equivalence.py`) | 45082297045 (on `869a18e4`; Windows green in the same run; macOS green on `main`) | **not a margin: a product defect from M4.** The Drawing view's canvas reads its mapping (`draw.viewMapping.get`) at mount and again after reporting its size; the host serves a window's calls concurrently, so the mount read (the old 497-px height) answered last and the canvas kept the old mapping (offset 248.5 against the kernel's 242) until the zoom row's `shell.view` made it read again. The window's gestures mapped with the stale offset, so the window, the replay and the direct commands disagreed. Not macOS-specific (the slower runner made the late answer likely). Fixed with the D-89 read gate on the mapping reads (`desktop/src/canvas/mappingRead.ts`, used by `CanvasView.tsx`); the D-89 sweep had covered the panels, not the canvas's own mapping. Vitest `src/canvas/mappingRead.test.ts` answers the older (mount) read last and expects the newer read's mapping (ruling E-20) |
| `drawing byte gate (macos-latest)`: every row of "Gesture equivalence three ways for the C4.4b tools" (the builder's `scripts/c44b-gesture-equivalence.py`) | 45114345189 (on `6c68ebbf`; Windows green in the same run; green on macOS in 45100267389) | **not a margin: a race in the M4 script.** It opened the views, slept a fixed 1 s and read their sizes; on this run the Drawing view had not reported its canvas size yet, so the script read the headless virtual 1920 x 1080 (SI-069), gave engine 2 that size, and its D-32 check compared the wrong size with itself. Every replay on engine 2 then mapped the window's samples differently (the window's own mapping centres a view of about 330 x 109 px), while the window and the direct commands agreed. Fixed: a bounded wait until every view has a reported size, not the virtual default, stable over two reads 0.5 s apart; the same race fixed in `scripts/c43-gesture-equivalence.py` (the Camera view). No product change |

## 5. M4 defects found by M5

| Defect | Found | Fix |
|---|---|---|
| The Drawing view's canvas mapping read without a read gate (the row of §4, run 45082297045) | C5.1a CI | the D-89 read gate on the mapping reads (`desktop/src/canvas/mappingRead.ts`), Vitest `mappingRead.test.ts` (ruling E-20) |
| **After a handler panic the desktop never offered the restart.** The watchdog counted a miss only for an unanswered heartbeat and an answer only for an `ok` one; a dead document thread answers at once with `internal`, so neither was counted, the engine was never marked not responding, and the banner with Restart and the recovery data never appeared | C5.1a round 1, the handler-panic note §1.3 (ruling E-26 item 4) | ruling E-29 item 3: a heartbeat answered with the stopped `internal` (`engineStopped`) marks the engine at once (`desktop/src-tauri/src/watchdog.rs`, unit tests); the live check (a dev-only hook kills the document thread, the banner, Restart, Recover) is a C5.1b row and in its manual script |
