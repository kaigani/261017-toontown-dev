# M4 merge note: desktop shell

**Complete on the builder's side; a merge candidate only once C4.4b passes review** (ruling D-30). Branch
`m4-desktop-shell`. Builder: Claude Opus 5.5. The orchestrator merges and tags; the builder does not.

## 1. Checkpoints and closures

| Checkpoint | Scope | Closure |
|---|---|---|
| C4.1 | the desktop shell front end and host wiring | passed (`reviews/M4-C4.1-plan.md`) |
| C4.2 | the Drawing view canvas, the gesture driver, gesture equivalence (C4.2b passed 2026-11-15) | passed (`reviews/M4-C4.2-plan.md`) |
| C4.3a | the Camera view: composition, coordinates, scene tools | passed (`reviews/M4-C4.3-plan.md`) |
| C4.3b | the parity gate over every surface, the flow table, the preview lifecycle | **passed** on product head `f5eb9e84`, run 44286651371 (36 of 36 green), handoff `df8fe594` (`reviews/M4-C4.3b-handoff-r5.md`); three full rounds and one closure check, rulings D-30 to D-54 |
| C4.4a | panels and settings (`reviews/M4-C4.4-plan.md`, D-40) | **passed** on product head `87f63b5f`, run 44861150241 (36 of 36 green on the first attempt), handoff `7dad9a7e` (`reviews/M4-C4.4a-handoff-r5.md`); three full rounds and two closure checks, rulings D-55 to D-74. Findings: round 1 twelve, round 2 two new, round 3 two new, closures five; all closed, including the P1 on the agent token in agent captures (closed by the host's overpaint, 486 live captures with none revealed, and the `page_ready` guard); final static check `reviews/M4-C4.4a-r5-closure-static.md` |
| C4.4b | the four drawing tools; the zero-M4 deferred check | **handed off** (`reviews/M4-C4.4b-handoff.md`; the product head and its green run are named there), rulings D-75 and D-76 |

## 2. Inventory, counts and what M4 leaves

- **Final head:** `54767666` (product; receipts in `d4ffed00`); its CI run is named in the C4.4b handoff's §1.1.
- **Commands in the parity gate:** 347 = 161 action + 41 read + 59 agent-only + 86 deferred (the reviewer's probe
  `tests/adversarial/c43b-r2s-static.py`).
- **Availability inventory:** 288 commands in scope, 1,229 (command, code) pairs: 13 asks-user, 1 deferred, 256
  execution, 716 parameter, 219 precheck, 24 unreachable (`scripts/check-availability-inventory.py`).
- **Tables:** D-39 302 rows (the whole table green in the MCP form on the final build; the OS form on the rows of
  each checkpoint, C4.4b's 105); D-44 66 rows and the confirmation sheet's 7 (C4.4a; C4.4b adds no dialog); three-way
  equivalence: C4.2's drawing tools, C4.3's scene tools, C4.4b's 20 rows.
- **Deferred list by owner (D-31): none owned by M4.** 86 entries: M5 51, M6 18, M7 6, M8 5, M10 4, M9 2
  (`desktop/src/ui-bindings/deferred.json`, each with its spec section), plus §3 below.
- **Receipt folders over 5 MB, for the orchestrator's trim (D-40):** `reviews/evidence/M4-C4.3b-r1` (26 MB),
  `M4-C4.2b-r1` (23 MB), `M4-C4.2b-r3` (6 MB), `M4-C4.2b-r2` (5 MB). C4.4b's receipts are summaries (284 KB).
- **Open spec issues raised in M4** (interim defaults in `plan/spec-issues.md`; the user decides): SI-120 to SI-155,
  SI-158 (interim); SI-162 and SI-164 (open); SI-163's open question; SI-175 (a ruling asked); SI-184, SI-185, SI-187,
  SI-188, SI-190 to SI-196 (C4.4b, interim). The spec A additions M4 proposes are listed under "Spec A additions
  proposed by M4, for the user" (with `draw.text.layout.get` and `tool.perspective.options.set` from D-75).

## 3. Deferrals recorded during C4.4a (for the deferred list)

- **Feather Edges** in the views' context menus and the Drawing menu: `draw.feather.apply` is declared but not
  implemented (owner M7 C7.2, or the first checkpoint after the SI-077 decision); its owner adds the item in both menus
  when it implements the command (D-40 answer 11).
- **The Camera view's scene-family context menu grows in M6 C6.1**: the items of 05 §3.2 lines 326-328 keep their
  `deferred` owners (D-30, D-40 answer 12); C4.4a ships the general edit items of 01 §6 only.
- **Toolbar items with fixed parameters** (01 §5.7): left to the milestone that changes the workspace format (SI-170).

## 4. Known CI margins (the M11 list)

**Every test that failed on a commit later green on rerun during M4, and what was done** (ruling D-74's request; from
the C4.4a round-5 handoff, kept here):

| Test (owner) | Runs | Done |
|---|---|---|
| `c34_r1_contract` `sequential_wait_timeout_retains_job_identity` (reviewer, M3) | 43548393377, 43605840535, 43608320184, 43617667731, 43632661801, 43690187580, 43779448685, 44592525640 | C4.1: a heartbeat lock in my code (fixed); later the 50 ms margin: D-64 edit (5 s budget), retired |
| `c34_round4` `a_queued_batch_wait_…_enqueue_not_from_its_start` (M3) | 43605840535 to 43632661801 (as above), 43772579796, 44668936063 | budget 5,000 ms (D-45), retired |
| `c42a_r2_review` `cancelled_capture_jobs_finish_after_request_returns` (reviewer) | 43632661801 attempt 2 | a probed-wait defect in my code, fixed (C4.2a r5); the race skipped by D-17 |
| `c41/availability` `passes_stay_constant_on_a_large_project` (mine) | 43617667731, 43698460055, 44809024880 | D-34/D-41 ratio form (C4.3b); D-71 item 2: the selection first pass's ratio 40 |
| `c23_r2_timeout` `smooth_watchdog_reaches_mcp_and_next_read_answers` (reviewer) | 44715875251 | D-68 item 6: setup on a 5 s executor, retired |
| `c24_aggregate` `aggregate_break_apart_and_size_limit_render_raw_observations` (mine) | 44809024880 | D-71 item 3: setup on a 120 s executor; `ok()` prints the envelope |
| `c14_measurements` autosave (mine) | 44823278999 | the order observed instead of assumed (above) |
| `drawing byte gate (macos)`: `fs.outsideRoots` on its fixture (headless session) | 43728082844 | the roots race: D-68 item 4 and D-73 item 1 |
| `c01-r2-boundary` lint `spawnSync ETIMEDOUT`; `App.test.tsx` sensor report (mine) | 43390751786 | the first a runner timeout, rerun; the second fixed with an event-driven wait (C4.1) |
| `c41-r2-desktop-floats` initial geometry (reviewer probe) | 44668936063 | a product defect: the float is built hidden (C4.4a r2) |
| `c13_r1_persistence` `concurrent_stale_lock_takeover_has_only_one_owner` (reviewer, M1) | 44894231140 (macOS `check`) | not a margin: a race in the lock takeover's arbiter (removal by name after the protection is released) can leave every contender told "held by another"; diagnosed with a proposed fix in the C4.4b handoff §6 (D-76 item 4) |
| `c21_example_results` (mine) | 44886086598, 44894231140 | a pending confirmation's `expiresInMs` differed by 1 ms: now a time key (`dfe9cea7`), retired |

`c14_r1_autosave` and `c23_r4_closure`/`c23_round3`/`c34_round3` were listed as flakes in C4.1/C4.2a handoffs but did
not fail in a CI run after them.

The retired margins:

- **Retired (D-64):** `crates/executor/tests/c34_r1_contract.rs::sequential_wait_timeout_retains_job_identity` raced a
  50 ms margin (500 ms budget) on the 3-core macOS runner (run 44592525640, `53a1014e`). The test now runs with a 5 s
  budget (500 ms margin), `timeoutMs <= 4500` and a job that outlasts the wait on any runner; the margin is no longer an
  M11 item.
- **Retired (D-45, a builder test):** `crates/executor/tests/c34_round4.rs::a_queued_batch_wait_is_bounded_from_its_callers_enqueue_not_from_its_start`
  raced the 100 ms margin of a 1,000 ms budget on the macOS runner (run 44668936063); it now runs with 5,000 ms.
- **Retired (D-68 item 6, a reviewer test):** `crates/mcp/tests/c23_r2_timeout.rs::smooth_watchdog_reaches_mcp_and_next_read_answers`
  built its 20,000-point stroke under its 600 ms budget and timed out in that setup on the macOS runner (run
  44715875251, first attempt; once before in M3 C3.1 round 2). The setup now builds it with an executor of the product's
  5,000 ms budget, and the 600 ms executor the test times opens that document; the timed part and assertions unchanged.

## 5. Known limits (stated for M11 C11.1 and the user)

- **A very large stroke is slow to edit in the window.** A pencil stroke of 50,000 points with straight segments, on
  its own layer, is a 17.3 MB canvas payload that takes the kernel 4.0 s to build; after a Contour Editor anchor drag
  on it, the committed drawing appeared in the window 36 s after the release (one drag, WebGL2, this workstation).
  The plan's 150 ms is far away for such a drawing; accepted as reported for M11 C11.1 (D-75 items 2 and 6, D-76
  item 6).
- **Keystroke to glyph sits at its bound on a 60 Hz display.** The glyph is drawn within two display frames of the key
  (D-76 item 2), with a margin under one millisecond at 60 Hz: the webview delivers each IPC answer just after a frame,
  so the glyph lands in the second frame. The engine's own work per keystroke is about 2 ms. A person repeats the
  measurement on an uncapped display; if it fails there, M11 C11.1 owns the design option of pushing the glyph's
  geometry through the event channel (events arrive in 3-7 ms).
- **Snap to Contour cannot close a single stroke on itself** (SI-070 as decided; D-76 item 1): a question for the
  user, with the alternative written out in SI-070.

