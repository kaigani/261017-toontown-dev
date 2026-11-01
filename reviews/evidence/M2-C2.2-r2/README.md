# Round-2 evidence

Read the [review report](../../M2-C2.2-r2.md) first.

- Current tests: `geometry-final.log`, `commands-final.log`, `mcp-final.log`; five failures are intentional, as numbered in the report.
- Complete broad run: `workspace-complete.log` and `workspace-summary.json`. It compiled an earlier review-test snapshot. `c01-prerequisite-rerun.log` resolves the missing ESLint prerequisite failures.
- Register: `conformance.md`, all 345 original source units; `register-deltas.json` records every disposition change.
- Local performance: `local-strokes-100k-windows.json`, `local-analysis.json`, `measurements.log`; planar raw facts in `planar-worst-case-windows.json`; `planar-analysis.json` recomputes local and CI maxima. Cache raw observations: `cache-retention.json`.
- CI: `ci-run.json`, `ci-run.log`, `ci-counts.json`, downloaded `ci-strokes-*` and `ci-planar-*` raw observations, with recomputed `ci-analysis.json`.
- Changes: `declared-tests.diff`, `c21-golden.diff`, `c21-golden-structural.json`, `c22-golden-changes.json`, `reviewer-tests-diff.txt`.
- Static and frontend receipts: `allows.log`, `release-features.log`, `review-clippy.log`, `fmt.log`, `generator.log`, `audit.log`, `pnpm-*`.
- `iterations/` retains superseded early test logs and the interrupted initial workspace attempt. These are development history, not the final test disposition.

All receipts are UTF-8 text or JSON. Raw observations remain separate from derived summaries. No quoted quantile relies solely on a stored summary.
