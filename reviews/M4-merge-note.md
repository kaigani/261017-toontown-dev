# M4 merge note: desktop shell

**Complete on the builder's side; a merge candidate only once C4.4b passes review** (ruling D-30). Branch
`m4-desktop-shell`. Builder: Claude Opus 5.5. The orchestrator merges and tags; the builder does not.

M4 delivers the desktop application around the M1-M3 kernel: the shell (menus, dock, panels, dialogs, preferences,
floating windows), the Drawing view and the Camera view as live canvases on WebGL2 and WebGPU, the gesture driver that
turns pointer input into the same commands an agent sends, the parity gate that binds every surface to a command, the
panels and agent surfaces of C4.4a and the four drawing tools of C4.4b.

## 1. Checkpoints and closures

| Checkpoint | Scope | Closure |
|---|---|---|
| C4.1 | the desktop shell front end and host wiring | passed (`reviews/M4-C4.1-plan.md`; handoffs to r3) |
| C4.2 | the Drawing view canvas, the gesture driver, gesture equivalence | C4.2a and C4.2b passed (C4.2b on 2026-11-15; `reviews/M4-C4.2-plan.md`) |
| C4.3a | the Camera view: composition, coordinates, scene tools | passed (`reviews/M4-C4.3-plan.md`) |
| C4.3b | the parity gate over every surface, the flow table, the preview lifecycle | passed on product head `f5eb9e84`, run 44286651371 (36 of 36), handoff `df8fe594` (`reviews/M4-C4.3b-handoff-r5.md`); three full rounds and one closure check, rulings D-30 to D-54 |
| C4.4a | panels and settings, agent surfaces (`reviews/M4-C4.4-plan.md`, D-40) | passed on product head `87f63b5f`, run 44861150241 (36 of 36 on the first attempt), handoff `7dad9a7e` (`reviews/M4-C4.4a-handoff-r5.md`); three full rounds and two closure checks, rulings D-55 to D-74; findings 12 + 2 + 2 + 5, all closed (including the P1 on the agent token in agent captures) |
| C4.4b | the Contour Editor, Perspective, Reposition All Drawings, the Text tool; no M4 deferred entry | handed off on product head `e6f71489`, run 44916529274 (36 of 36 on the first attempt), `reviews/M4-C4.4b-handoff.md` (`57bad5ee`); round 1: fifteen findings (one P1, the lock takeover, §6; twelve P2; one P3 and the D-81 P3 observations), rulings D-75 to D-81, all fixed; round 2 on product head `bf2b2195`, run 44951926851, `reviews/M4-C4.4b-handoff-r2.md`: seven items (C44B-R2S-01 to -04, C44B-R2L-01, C44B-R2-01, -02) and D-82's grapheme clusters, rulings D-82 to D-85, all fixed; round 3 (the last full round) on product head `a67edbb1`, run 44977394689, `reviews/M4-C4.4b-handoff-r3.md`: two static findings (C44B-R3S-01, the lock's bound; C44B-R3S-02, the agent-acting snapshot) and one live (C44B-R3-01, refusals reported for keys and native menus), rulings D-86 and D-87, all fixed; **closure handoff** on product head `a7f8554a`, run 45014999591, `reviews/M4-C4.4b-handoff-r4.md`, for one scoped closure check; D-88 accepted the three static witnesses that describe the code by design, the interim `fs.ioError` answer for a lock file in use (SI-200 stays a question) and the `url` 2.5.9 bump |

Rulings across M4: the O series during C4.1 (to O-24) and the D series from C4.2 on (to D-88), recorded in
`reviews/M4-C4.1-plan.md`, `reviews/M4-C4.2-plan.md` and `reviews/M4-C4.3-plan.md` §15.

## 2. Inventory and counts (final build)

- **Final product head:** `a7f8554a`, run 45014999591, 36 of 36 green (the C4.4b closure handoff; round 3's head was
  `a67edbb1`, run 44977394689; round 2's `bf2b2195`, run 44951926851; round 1's `e6f71489`, run 44916529274).
- **Commands in the parity gate:** 347 = 161 action + 41 read + 59 agent-only + 86 deferred
  (`tests/adversarial/c43b-r2s-static.py`; C4.3b ended at 154 bound, 61 agent-only, 132 deferred).
- **Availability inventory:** 288 commands in scope, 1,230 (command, code) pairs: 13 asks-user, 1 deferred, 257
  execution, 716 parameter, 219 precheck, 24 unreachable (the 257th execution pair: a project command refused because a
  program outside the lock protocol keeps a lock file open, D-86 item 1) (`scripts/check-availability-inventory.py`,
  `docs/availability-inventory.md`).
- **D-39 table** (one row per control: its command and parameters, its effect, its undo steps): 309 rows, the whole
  table green in the MCP form on the final build (and in every CI run's `desktop gui`); the OS form (real input) run on
  each checkpoint's rows, C4.4b's on the final build.
- **D-44 table** (one row per choice of every dialog or message box): 73 rows (66 flows and the confirmation sheet's
  7), green on the final build, 5 named (recovery open's three choices, the engine restart's two: covered by Vitest).
- **Gesture equivalence three ways** (the window, the headless replay of its samples, the commands sent directly):
  C4.2's drawing tools, C4.3's scene tools in the Camera view, C4.4b's 20 rows (the four tools in both views).
- **Vitest** 99 files, 820 tests. **Reviewer Python suites** 86 of 86. **Allowed lints** 96 in 40 product files (the
  count `scripts/check-product-allows.py` gates); no new `unsafe` in C4.4b.

## 3. The deferred list by owner (D-31): none owned by M4

86 entries in `desktop/src/ui-bindings/deferred.json`, each with its spec section and owner: **M5 51, M6 18, M7 6,
M8 5, M10 4, M9 2.** Recorded besides the list:

- **Feather Edges** in the views' context menus and the Drawing menu: `draw.feather.apply` is declared but not
  implemented (owner M7 C7.2, or the first checkpoint after the SI-077 decision); its owner adds the item in both menus
  (D-40 answer 11).
- **The Camera view's scene-family context menu grows in M6 C6.1**: the items of 05 §3.2 lines 326-328 keep their
  `deferred` owners (D-30, D-40 answer 12); C4.4a ships the general edit items of 01 §6.
- **Toolbar items with fixed parameters** (01 §5.7): left to the milestone that changes the workspace format (SI-170).
- **The outline colour list of the Text panel** comes with the palette (M7; SI-169).
- **The gestures of tools outside M4** (SI-129): `pointer` answers "not built" with the tool's structured command;
  the Texture Editor's are M7 C7.1's.

## 4. Tests that failed on a commit later green on rerun, and the CI margins (the M11 list)

Every test that failed on a commit and passed on a rerun or a later commit without a change to it during M4, and
what was done (ruling D-74's request):

| Test (owner) | Runs | Done |
|---|---|---|
| `c34_r1_contract` `sequential_wait_timeout_retains_job_identity` (reviewer, M3) | 43548393377, 43605840535, 43608320184, 43617667731, 43632661801, 43690187580, 43779448685, 44592525640 | C4.1: a heartbeat lock in my code (fixed); later the 50 ms margin: D-64 edit (5 s budget), retired |
| `c34_round4` `a_queued_batch_wait_…_enqueue_not_from_its_start` (M3) | 43605840535 to 43632661801, 43772579796, 44668936063 | budget 5,000 ms (D-45), retired |
| `c42a_r2_review` `cancelled_capture_jobs_finish_after_request_returns` (reviewer) | 43632661801 attempt 2 | a probed-wait defect in my code, fixed (C4.2a r5); the race skipped by D-17 |
| `c41/availability` `passes_stay_constant_on_a_large_project` (mine) | 43617667731, 43698460055, 44809024880 | D-34/D-41 ratio form (C4.3b); D-71 item 2: the selection first pass's ratio 40 |
| `c23_r2_timeout` `smooth_watchdog_reaches_mcp_and_next_read_answers` (reviewer) | 44715875251 | D-68 item 6: setup on a 5 s executor, retired |
| `c24_aggregate` `aggregate_break_apart_and_size_limit_render_raw_observations` (mine) | 44809024880 | D-71 item 3: setup on a 120 s executor; `ok()` prints the envelope |
| `c14_measurements` autosave (mine) | 44823278999 | the order observed instead of assumed |
| `drawing byte gate (macos)`: `fs.outsideRoots` on its fixture (headless session) | 43728082844 | the roots race: D-68 item 4 and D-73 item 1 |
| `c01-r2-boundary` lint `spawnSync ETIMEDOUT`; `App.test.tsx` sensor report (mine) | 43390751786 | the first a runner timeout, rerun; the second fixed with an event-driven wait (C4.1) |
| `c41-r2-desktop-floats` initial geometry (reviewer probe) | 44668936063 | a product defect: the float is built hidden (C4.4a r2) |
| `c13_r1_persistence` `concurrent_stale_lock_takeover_has_only_one_owner` (reviewer, M1) | 44894231140 (macOS `check`) | **not a margin: a product defect** in the lock takeover (removal by name after the protection was released could leave every contender told "held by another"); diagnosed in the C4.4b handoff §6, fixed under D-77 (§6 below) |

A security advisory published during the closure: RUSTSEC-2026-0592 (`url` 2.5.8, a stack overflow in `Url::origin()`
on nested `blob:` URLs, through `tauri` and `wry`) failed `cargo deny` in run 45009040573; the lock file moved to `url`
2.5.9 (`a7f8554a`), nothing else changed.

Failures that were not reruns but deterministic on CI's machines, fixed on the next commit: `c21_example_results`
(a pending confirmation's `expiresInMs` differed by 1 ms; now a time key, `dfe9cea7`); `c14-r3-measurements.py` on
Windows (a non-ASCII test literal read as cp1252); the C4.4b three-way script on CI's Windows runner (its window
capture was 1 x 1 px when the Camera view was looked for: that view's rows use agent input there).

`c14_r1_autosave` and `c23_r4_closure` / `c23_round3` / `c34_round3` were listed as flakes in the C4.1 and C4.2a
handoffs but did not fail in a CI run after them.

The retired margins:

- **D-64:** `crates/executor/tests/c34_r1_contract.rs::sequential_wait_timeout_retains_job_identity` raced a 50 ms
  margin (500 ms budget) on the 3-core macOS runner (run 44592525640). It now runs with a 5 s budget (500 ms margin),
  `timeoutMs <= 4500` and a job that outlasts the wait on any runner.
- **D-45 (a builder test):** `crates/executor/tests/c34_round4.rs::a_queued_batch_wait_is_bounded_from_its_callers_enqueue_not_from_its_start`
  raced the 100 ms margin of a 1,000 ms budget on the macOS runner (run 44668936063); it now runs with 5,000 ms.
- **D-68 item 6 (a reviewer test):** `crates/mcp/tests/c23_r2_timeout.rs::smooth_watchdog_reaches_mcp_and_next_read_answers`
  built its 20,000-point stroke under its 600 ms budget and timed out in that setup on the macOS runner (run
  44715875251; once before in M3 C3.1 round 2). The setup now builds it with an executor of the product's 5,000 ms
  budget; the timed part and assertions are unchanged.

## 5. Known limits, in plain words

- **Opening a project can wait while another program holds its old lock file open** (Windows). When a stale lock is
  taken over, the old file is renamed `<project>.lock.removed-<code>` and deleted as soon as every other program that
  had it open (a virus scanner, an indexer, a backup tool) closes it; the project opens at once. If such a program
  keeps the project's arbitration file open while it is being removed, the open waits up to 12 seconds and then says
  which file is in use, changing nothing; the person can try again once that program lets go, and an agent gets
  `fs.ioError` with `reason: "lockFileInUse"` and the file's path (D-84, D-86; SI-200).
- **A very large stroke is slow to edit.** A pencil stroke of 50,000 points drawn as straight segments, on its own
  layer, makes a 17.3 MB drawing for the canvas, which takes the kernel 4.0 s to build. After an edit of that stroke
  the window shows the result 19 to 36 s after the mouse is released (Contour Editor 35.9 s and 18.7 s, Perspective
  about 20 s, Reposition All about 21 s, on this workstation, both back ends). Ordinary drawings commit in about 35 ms.
  Accepted as reported for M11 C11.1 (D-75 items 2 and 6, D-76 item 6).
- **Typing lands on the screen exactly at its budget on a 60 Hz display.** A typed character is drawn within two
  display frames of the key, with one or two milliseconds to spare at most (95th percentile 34.1 ms against 35.6 ms on
  WebGL2 in round 2; in round 1 35.1 against 35.2 on WebGL2, 34.5 against 35.4 on WebGPU). The engine's own work per keystroke is about 2 ms; the rest is the webview delivering each answer
  just after a frame. A person repeats the measurement on a display without a frame cap (§8); if it fails there, M11
  C11.1 owns the option of pushing the glyph's geometry through the event channel (not built).
- **There is no font list.** The Text panel takes a typed font family name; no command lists the installed fonts
  (`draw.font.list` is proposed to the user; D-57 forbids adding it without the user).
- **No font is bundled, so text can look different on Windows and macOS.** Text lays out with the computer's own font
  files; a missing font and the default use the platform's default sans-serif face. The Text panel says so for every
  font. Bundling one open-licensed face is a proposal for the user (SI-169); it would change the layout of existing
  text in the default font.
- **What an agent cannot read about text being edited.** A headless agent cannot read caret positions, line boxes or
  the edit session; it places the caret with `pointer` and `key`, types with `type_text`, and reads the text with
  `draw.drawing.get`. An attached agent reads the session in `inspect_ui`. No agent can read an input method's
  pre-edit (composition) text: the webview holds it until it is committed. `draw.text.layout.get` is proposed.
- **Snap to Contour cannot close one stroke on itself** (SI-070 as decided; D-76 item 1): an end dragged to its own
  start does not snap there. A question for the user (§7).
- **The live previews are close, not exact** (SI-154, SI-190): a transform or envelope preview lands within one view
  pixel of the committed result on the sample drawings; pencil widths and text under a strong projective map are
  approximate until the commit.
- **Some tools have no gestures yet** (SI-129): their commands work, their canvas gestures come with their owners.

## 6. The lock takeover fix (rulings D-77, D-78, D-83, D-84, D-86)

`crates/persist/src/lock.rs`: the lock's arbitration file (`<lock>.takeover`) is now removed while its protection is
still held (unlinked before the `flock` is released; on Windows deleted while the handle, which shares deletion, is
open), and a contender removes such a file only after proving it abandoned under the file's own protection (Windows:
an open with delete access that refuses write sharing, deleted on close; elsewhere: an exclusive `flock` on the opened
file and the same device and inode as the path). Safety first: never two owners. Deterministic tests under the
`test-hooks` feature: `crates/executor/tests/c44b_lock_takeover.rs` (a releasing arbiter suspended with its file
protected, a contender removing nothing; an abandoned file proven and suspended, exactly one owner after). Written
under the GUI hold.

D-78 item 1 (the static review's P1, C44B-R1S-07: a contender's stale decision, resumed late, could remove a live
owner's lock by name) completed it, design note first (`reviews/M4-design-lock-takeover.md`): no file is removed by
name on the strength of an earlier decision. A non-owner removes a lock or an arbitration file only under that file's
own protection, re-validated by identity and content after the last pause point (`remove_proven`); an owner removes
its own file while still protecting it (`remove_own`). Tests: five in `c44b_lock_takeover.rs` (the releasing arbiter,
the abandoned arbiter, the zero-owner and two-owner schedules, 40 randomised trials). **Results:** the reviewer's
`c13_r1_persistence::concurrent_stale_lock_takeover_has_only_one_owner`, unchanged, **200 of 200 runs** on Windows
(each run its 64 trials; `reviews/evidence/M4-C4.4b-r2/lock-loop.json`); green on macOS in every CI run since
(`check`), run 44951926851 included. The limit: on a file system that cannot lock (some network file systems) protection
cannot be proven, and M1's age rule stays.

Round 2 (rulings D-83 item 1 and D-84; findings C44B-R2S-01 and C44B-R2L-01) completed it again: a handle confers a
role only while the path still names it (a creator confirms its file's identity under the protection it just took; a
lost file is contention, and a creator that cannot take the protection removes nothing); on Windows the proven lock
file is renamed aside under its protection, so its name is free at once even while another program holds it open; a
pending deletion or sharing error is contention, never "not writable" (reported only when the folder cannot be
written); "held by another" is answered only when true at that moment. The 200-run receipt above is of round 1's
code. **Round 2's receipts** (`reviews/evidence/M4-C4.4b-r3/lock-loops-idle.json`, `lock-loops-busy.json`, on `50e9ed28`): the randomised lock
test and the M1 test each looped 2,000 times with every core busy (one CPU burner per logical core, 32, and eight
loops at once): 2,000 of 2,000 each, zero failures; and 2,000 times each on an idle machine: 2,000 of 2,000 each, zero
failures. Every run's seed is printed with its verdict.

Round 3 (ruling D-86 item 1; finding C44B-R3S-01): an open always answers within a bound. When a program outside the
protocol keeps a lock file open so that a stale lock cannot be replaced, the opener decides for at most 12 s
(`ACQUIRE_BOUND`, `TAKEOVER_STALE` plus a round), then answers that the file is in use (`Locked::InUse`, its path),
changing nothing; a cancelled opener answers at once. I3 is stated with its condition (no program outside the protocol
keeps a lock file open); the next open after that program lets go proceeds. **Closure receipts**
(`reviews/evidence/M4-C4.4b-r4/`, on `3e5d5605`): the randomised lock test and the M1 test each 2,000 runs idle and 2,000 with every core busy: 8,000 runs, zero failures.

## 7. Questions for the user, in one place

Interim defaults are built and stated in `plan/spec-issues.md`; each line is the question to answer.

| Issue | Question |
|---|---|
| SI-070 | Should one stroke close on itself by Snap to Contour (a moved stroke's unmoved anchors as candidates), against 02 line 799's "shapes close"? |
| SI-120 | Confirm the answer to a request cut off by an engine restart, or define a dedicated code in 11 §10. |
| SI-121 | Confirm the UI tree's freshness fields (11 §7.5), or specify another way. |
| SI-122 | One zoom ladder for all step-wise zooming: 02 §4.3's or 01 §6.5's? |
| SI-123 | Add a rotate-view tool to 02 §5.1, or drop it from 01 §6.5? |
| SI-124 | Correct 11 §7.6's `tool.*.select` to the registry's names. |
| SI-125 | Specify `pointer`'s spaces and tilt in 02 §13.3, or restrict `pointer`. |
| SI-126, SI-127 | Confirm the static light table before the Exposure Sheet, and the Drawing-view conventions. |
| SI-128 | Persist view transforms (in which scope); register `ui.view.create` for agent views? |
| SI-129 | Which checkpoints own the gestures of the tools outside M4? |
| SI-130 | Confirm `pointer`'s event cap, or define it in 11 §6.5. |
| SI-131, SI-141 | Confirm the drawing-tool and window-routing details, or state them in 02 and 11. |
| SI-132 | Add a view event to spec A, or confirm `shell.view`; state the headless screenshot targets. |
| SI-133 | A view-space rectangle in a rotated view: convert to a polygon, or refuse? |
| SI-134 | Does a point selection have a hit tolerance, and which? |
| SI-135 | Confirm the code for an agent gesture interrupted by the user. |
| SI-136, SI-137, SI-140 | Confirm the back-end measurement, tessellation buckets and where the back-end choice lives. |
| SI-138, SI-139, SI-153 | Confirm the overlay's, onion skin's and the Camera view's appearance. |
| SI-142 | Should `RenderResult` name the view of an `activeView` render; should a click activate a view window? |
| SI-143 | Which command is canonical for a view's type: `view.type.set` or `anim.view.type.set`? |
| SI-144 | Confirm the scene tools' undo labels. |
| SI-145 | True projection for rotated planes; clipping at the near plane? |
| SI-146 | What a scene-space point addressed to a drawing means. |
| SI-147, SI-152 | What a Camera view without a camera shows; the start-up tool and family. |
| SI-148 | How a request names the drawing Select in the Camera view. |
| SI-149 | What `anim.view.coords.get`'s `depth` means. |
| SI-150, SI-151 | Confirm the scene tools' furniture, the multi-node drag and the temporary Skew. |
| SI-154, SI-190 | Should previews be exact (a kernel-computed preview per sample)? |
| SI-155 | Confirm what a person can act on that is not a command. |
| SI-158 | Confirm the Camera three-way's tolerance through a headless view of another size. |
| SI-162 | Should Delete Workspace ask before deleting? |
| SI-163 | Should the product do more for clients that refuse a server-sent event over 1 MiB? |
| SI-164 | Should a failed or cancelled Open return to the previous project? |
| SI-169 | Bundle one open-licensed default font (a layout change for existing text)? |
| SI-175 | Accept the Activity panel's registry additions (`agent.activity.recorded`, `afterSeq`, entry fields)? |
| SI-177 | Should 11 §6.3's confirmation order change for attached mode (the app asked first)? |
| SI-186 | Is "flat/smooth" collinear handles (as built), or zero handles? |
| SI-187 | Is a kernel-computed snap preview wanted? |
| SI-188 | Perspective's side drags (another reading mirrors the opposite side); the `mapping` option's command. |
| SI-189 | Reposition All's reference box: the current frame's drawing (as built) or the union of all drawings? |
| SI-196 | `draw.stroke.set` on the Contour Editor's Tool options (as built), or a Line section for every selection tool? |
| SI-198 | Confirm `element.hidden` as the code of a gesture on an element the Camera view does not draw, and that an outline drawn by "Show outline on selection" is not editable. |
| SI-199 | Should the kernel refuse an envelope apply that carries geometry onto or beyond a perspective map's vanishing line, instead of clamping it to a very large coordinate? |
| SI-200 | Should a lock file kept open by a program outside the lock protocol have a dedicated error code (for example `project.lockFileInUse`) instead of `fs.ioError` with `reason: "lockFileInUse"`? |
| Spec A | The additions M4 proposes (`plan/spec-issues.md`, "Spec A additions proposed by M4, for the user"): `draw.text.layout.get`, `tool.perspective.options.set`, `draw.font.list`, `job.outputs.list`, `agent.activity.recorded`, `agent.activity.list {afterSeq}`, the `ActivityEntry` members, `agent.session.changed`, `agent.confirmation.resolved`. |

SI-184, SI-185, SI-191 and SI-193 to SI-195 are interim decisions without a question of their own (confirm or
specify).

## 8. Checks only a person can make

Manual scripts in `reviews/manual/M4/` (index: `README.md`), each a few minutes, with a "Result" section to fill in:

| Area | Scripts |
|---|---|
| Menus, shortcuts and keyboard | `C4.1-macos-menu-and-shortcuts.md`, `C4.1-windows-menu-and-shortcuts.md`, `C4.1-keyboard-and-focus.md`, `C4.1-dialog-then-shortcut.md`, `C4.4a-keyboard.md` |
| Windows, monitors, scaling, themes | `C4.1-window-geometry-and-full-screen.md`, `C4.1-floating-windows.md`, `C4.1-display-scaling.md`, `C4.2-display-scaling.md`, `C4.1-theme-follow-system.md` |
| OS integration | `C4.1-native-dialogs.md`, `C4.1-single-instance-and-open-document.md`, `C4.4a-open-recent-full-path.md` |
| A real agent | `C4.1-attached-agent.md`, `C4.3-coexistence.md`, `C4.4a-agent.md` |
| The canvas | `C4.2-visual-fidelity.md`, `C4.2-feel.md`, `C4.2-navigation.md`, `C4.2-gpu-less.md`, `C4.3-camera-view.md` |
| A pen or tablet | `C4.2-tablet.md`, `C4.3-scene-tools.md`, `C4.4b-tools-pen-and-macos.md` |
| Text input | `C4.4b-text-input-method.md` (input methods, dead keys, composition; Windows and macOS) |
| Timing on an uncapped display | keystroke to glyph: `python scripts/c44b-measure.py <toontown> <out> --backend webgl2 --text-only`, then `--backend webgpu`; the receipt's `keyToGlyphBound` gives the control median, the bound and the margin (D-76 item 2). The D-33 frame measurement on an uncapped 60 Hz display (C4.3b). |

## 9. Receipt folders over 5 MB, for the trimming decision (D-42)

M4's receipts total 82 MB. Its folders over 5 MB:

| Folder | Size | Files | The largest files |
|---|---|---|---|
| `reviews/evidence/M4-C4.3b-r1` | 25.3 MB | 618 | `case8/rows.json` 4.1 MB, `case8/setup-audit.json` 2.5 MB, `case7/legacy-r1/d2-cases.json` 1.8 MB |
| `reviews/evidence/M4-C4.2b-r1` | 22.4 MB | 155 | `d1-long-webgpu.json` 7.0 MB, `d1-long-webgl2.json` 7.0 MB, `d2-cases.json` 1.8 MB |
| `reviews/evidence/M4-C4.2b-r3` | 5.7 MB | 230 | `c2/c2-gestures.json` 1.2 MB |

Receipts from C4.3b round 2 on are D-42 summaries (C4.4b's: 396 KB in round 1, 341 KB in round 2, 556 KB in round 3, 560 KB at the closure, the lock loops' per-run lists included). Folders over 5 MB from earlier milestones, for the
same decision: `M1-C1.4-r2` 100.2 MB, `M2-C2.4-r3` 87.7 MB, `M1-C1.1-r2` 86.8 MB, `M3-C3.1-r3` 22.4 MB, `M1-C1.2-r5`
20.2 MB, `M1-C1.2-r4` 19.0 MB, `M1-C1.2-r3` 18.2 MB, `M0-C0.3-r1` 17.2 MB, `M1-C1.2-r2` 17.1 MB, `M3-C3.2-r3` 16.8 MB,
`M1-C1.3-r1` 14.3 MB, `M2-C2.3-r3` 11.1 MB, `M2-C2.1-r2` 10.7 MB, `M3-C3.1-r2` 10.3 MB, `M0-C0.3` 9.4 MB, `M1-C1.4-r1`
9.1 MB, `M3-C3.2-r1` 9.0 MB, `M3-C3.3-r4` 8.6 MB, `M1-C1.2-r1` 6.8 MB, `M2-C2.2-r3` 6.8 MB, `M2-C2.3-r1` 6.3 MB,
`M1-C1.1-r1` 5.8 MB, `M3-C3.1-r1` 5.6 MB.
