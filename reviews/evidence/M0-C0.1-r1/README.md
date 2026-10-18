# Review evidence

The verdict and interpretation are in `../../M0-C0.1-r1.md`.

- `adversarial-rust.log` and `adversarial-frontend.log`: the permanent regression failures (5 Rust, 8 Vitest).
- `runtime.json` and `ipc-bench.json`: actual command arguments, exits, timings, self-test, linked imports,
  desktop negative control, binary sizes, and real WebView2 measurements.
- `static-import-lint.log` versus `dynamic-import-lint.log`: rejected static import versus accepted bypass;
  the latter also passes `dynamic-import-typecheck.log`.
- `graphics-import-proof.log`: the proof incorrectly accepts an actual DirectComposition import.
- `ci-run.json`, `ci-excerpts.txt`, `ci-logs.zip`: job metadata, indexed excerpts, and full retrieved run/macOS
  job logs. The CI implementation matches the handoff; only documentation changed after the green run.
- `browser-layout.json`: T3 browser inspection of the mock-host UI, not a native menu test.
- Remaining logs record the baseline and final tooling checks; `cargo-metadata.json` and `headless-tree.txt`
  record the dependency topology.

Logs are normalized to UTF-8/LF; ANSI color codes and trailing whitespace are removed for reviewability.
PowerShell can label ordinary native stderr output `NativeCommandError` (for example Cargo's compilation
progress) even when the command exits successfully. That wrapper alone is not evidence of a failed check.
Baseline checks and final non-test checks passed; the new regression suites intentionally fail as documented.

Runtime measurements can be regenerated with `tests/adversarial/collect_runtime_evidence.py` after release
builds. Command output includes this worktree's absolute paths as provenance, not runtime configuration.
