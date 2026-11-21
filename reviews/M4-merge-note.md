# M4 merge note: desktop shell (draft)

**Draft, not a merge candidate.** The merge note and the M4 merge to `main` come after C4.4b passes (ruling D-30).
Branch `m4-desktop-shell`. Builder: Claude Opus 5.5. The orchestrator merges and tags; the builder does not.

## 1. Checkpoints and closures

| Checkpoint | Scope | Closure |
|---|---|---|
| C4.1 | the desktop shell front end and host wiring | passed (`reviews/M4-C4.1-plan.md`) |
| C4.2 | the Drawing view canvas, the gesture driver, gesture equivalence (C4.2b passed 2026-11-15) | passed (`reviews/M4-C4.2-plan.md`) |
| C4.3a | the Camera view: composition, coordinates, scene tools | passed (`reviews/M4-C4.3-plan.md`) |
| C4.3b | the parity gate over every surface, the flow table, the preview lifecycle | **passed** on product head `f5eb9e84`, run 44286651371 (36 of 36 green), handoff `df8fe594` (`reviews/M4-C4.3b-handoff-r5.md`); three full rounds and one closure check, rulings D-30 to D-54 |
| C4.4a | panels and settings (`reviews/M4-C4.4-plan.md`, D-40) | **passed** on product head `87f63b5f`, run 44861150241 (36 of 36 green on the first attempt), handoff `7dad9a7e` (`reviews/M4-C4.4a-handoff-r5.md`); three full rounds and two closure checks, rulings D-55 to D-74. Findings: round 1 twelve, round 2 two new, round 3 two new, closures five; all closed, including the P1 on the agent token in agent captures (closed by the host's overpaint, 486 live captures with none revealed, and the `page_ready` guard); final static check `reviews/M4-C4.4a-r5-closure-static.md` |
| C4.4b | the four drawing tools; the zero-M4 deferred check | in progress (from `87f63b5f`) |

## 2. Still to write at the end of C4.4b

The inventory and counts, the deferred list by owner (none owned by M4, D-31), the receipt folders over 5 MB for the
orchestrator's trim (D-40), the open spec issues, and the final head with its run.

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
