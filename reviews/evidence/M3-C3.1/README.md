# M3 C3.1 evidence

Raw receipts; nothing here is rewritten after capture.

| File | What |
|---|---|
| `ci-41614164207/{win,mac}/c31-measurements/timing-budget.json` | Timing work-account calibration (`crates/executor/tests/c31_budgets.rs`, release, CI runners): 1001 drawing columns × 100 000 frames. In that (pre-candidate) run: remove exposure on every column 0.903 / 0.983 s (Windows / macOS), insert a cell in all columns 0.975 / 0.720 s, over-budget refusal 4 / 3 ms, scene-limit refusal 1 ms, nothing staged by any refusal. **Single observations, not bounds (C31-R1-12):** the handoff candidate's run 41621608304 recorded 1.17 s (Windows insertion), 1.24–1.50 s (macOS) and 4.6 / 6.0 ms refusals; the criterion is the 30-s watchdog. |
| `ci-41614164207/{win,mac}/c24-measurements/` | The M2 100k baselines (`c24_budgets`, `c24_aggregate`) re-run at the C3.1 head (handoff item 8). |
| `baseline-comparison.json` | Medians of the c24 series at this head against the M2 round-3 raw CI records (`reviews/evidence/M2-C2.4-r3/ci-artifacts/`). Windows ratios 0.95–1.03 for stroke add / undo / redo / dry run, RSS ×1.076, live bytes ×1.000; macOS 0.67–0.79 (faster), RSS ×0.94. Cross-run observations on shared runners, not causal attribution. C22-T2 and R1-12 stay with M11 C11.1; the C22-T2 gate is still missed and every budgeted Flatten is still a refusal, which is not completion. |
| `eslint-timing.json` | Why `check (macos-latest)` failed once in run 41614164207: the reviewer test `c01-r2-boundary` (30-s `eslint .` limit) timed out on a slow runner; locally the regenerated `commands.ts` lints in the same time as M2's (9.8 s whole tree both ways). |

Before the C3.1 fix to `crates/model/src/change.rs` (one heap string per frame when diffing/patching exposures),
the same calibration on the Windows runner (run 41610425235, `measurements release (windows-latest)`) took 54.452 s
(remove exposure on every column) and 63.726 s (insert a cell in all columns), past the 30-s watchdog, and setup of
the 1001 columns took 43.480 s (now under 1 s). The log lines are quoted in the handoff (§2 "Timing budget,
calibrated").
