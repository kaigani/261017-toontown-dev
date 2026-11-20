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
| C4.4a | panels and settings (`reviews/M4-C4.4-plan.md`, D-40) | in progress |
| C4.4b | the four drawing tools; the zero-M4 deferred check | not started |

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

- **Retired (D-64):** `crates/executor/tests/c34_r1_contract.rs::sequential_wait_timeout_retains_job_identity` raced a
  50 ms margin (500 ms budget) on the 3-core macOS runner (run 44592525640, `53a1014e`). The test now runs with a 5 s
  budget (500 ms margin), `timeoutMs <= 4500` and a job that outlasts the wait on any runner; the margin is no longer an
  M11 item.
- **Retired (D-45, a builder test):** `crates/executor/tests/c34_round4.rs::a_queued_batch_wait_is_bounded_from_its_callers_enqueue_not_from_its_start`
  raced the 100 ms margin of a 1,000 ms budget on the macOS runner (run 44668936063); it now runs with 5,000 ms.
