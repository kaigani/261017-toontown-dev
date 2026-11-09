# M3 C3.2 evidence

Raw receipts; nothing here is rewritten after capture.

| File | What |
|---|---|
| `ci-42650981858/e4-{windows,macos}/report.json` | E4 numeric (`scripts/e4-cutout-numeric.py`, official `mcp==2.0.0`, release binary) at `7cb971a`: 8/8 checks on each OS; worst absolute error against the script's own formulas 8.9e-16 (tolerance 1e-6); head SHA, platform, Python and SDK versions. |
| `ci-42650981858/e4-{windows,macos}/evaluate.json` | The `anim.evaluate` answer with IDs replaced by names. SHA-256 `f41480ab4ce32929e8cae8368f78f39a52b391d4ddb8999908a6b4350f987aab` on both; the `e4 cross-OS` job compared the two byte for byte. A local debug run on Windows gave the same hash. |
| `ci-42650981858/e4-{windows,macos}/inputs.json` | Every request the scenario sent. |
| `ci-42650981858/c32-measurements-{windows,macos}/evaluator-budget.json` | `c32_budgets` (release, CI runners): 24 nested pegs, 1 000-point paths, three 1 000-key channels each. Windows / macOS seconds: deepest node over 10 000 frames 0.685 / 0.836; over 100 000 frames 6.253 / 8.422; all 24 nodes over 10 000 frames with every part 4.556 / 6.026; path sample over the bar 5.535 / 7.608; function sample 0.092 / 0.108; path read-back with passage frames 0.005 / 0.006. All completed, no revision changed. Single observations on shared runners, not bounds; the criterion is the 30-second watchdog. |

Run 42650981858: the first attempt of `check (macos-latest)` failed one reviewer test, `c31_r2_cancellation::r02_r14_timeout_after_solo_write_rolls_back_and_queued_read_answers`. Its queued read has 100 ms between the batch's cancellation and its own deadline while each charge spins 20 ms. It passed 6/6 locally and nothing on that path changed in C3.2, so the job was re-run once and passed. All 25 jobs green.

The run at the docs head (42665072550) failed the other test of the same kind on its first macOS `check` attempt:
`c31_r3_selection::measured_refusal_and_cancelled_scan_have_no_late_mutation`, whose queued read must answer within 700 ms
when issued 120 ms into a call that is cancelled at 700 ms. Measured locally (debug, Windows), the read answers in
0.596–0.620 s, so the cancelled call's tail after its deadline is about 20–40 ms against a margin of about 100 ms. Each of
these two reviewer tests failed once on macOS and passed in the other run, and neither code path changed in C3.2 (the only
`run.rs` change merges `anim.changed` events of accepted calls). The job was re-run once. The margins of these two tests
on shared macOS runners are worth a reviewer's look.
