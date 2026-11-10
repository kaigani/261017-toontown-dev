# C3.2 round-2 evidence

Reviewed product: `65cf4d88bdf530fe7970dfe37ac230198864c64c`.

- `workspace.txt` and `workspace.exit.txt`: final unfiltered workspace invocation; enabled finding regressions are expected to fail.
- `anim-model.txt`: all animation/model tests, including the new seeded oracles; only SI-104 fails.
- `executor-r2.txt`, `events-budget.txt`: targeted probes, history failures and the larger deadline probe. The latter records zero events on both undo/redo and a typed timeout at about 3.001 seconds.
- `executor.txt`: initial executor run; 140 per-binary summaries, 799 passed / 0 failed / 9 ignored. Its original shell wrapper did not preserve the native exit code; final workspace and targeted native exits are saved explicitly.
- `cargo-fmt*`, `clippy*`, `deny*`: engineering gates. Clippy retains only the older empty-doc-comment exemption, not the new needless-range-loop exemption.
- `sdk-matrix-results.json`, `finite-sdk-result.json`, `sdk-binary.json`, `ci-provenance.json`: independently rerun official SDK 2.0.0 probes against the downloaded, history-pinned release executable. Working packages and original generated receipts live in the system temporary directory.
- `artifact-comparison.json`, `e2-comparison.json`: independent cross-OS byte comparisons and structurally checked old/new E4 field differences. Original downloads remain in the system temporary directory; small raw E4/E2 payload copies are preserved under `ci-42714347825/` and the old comparison base under `ci-42650981858/`.
- `ci-*.json`, `ci-jobs.json`, `ci-target-timings.json`, `timings.md`, `ci-test-extracts.txt`, `ci-failed-attempts.txt`: CI execution/timing evidence, including failed first attempts and C3.1 closure.
- `golden-diff.txt`, `product-diff.txt`, `kernel-diff.txt`, `ci-diff.txt`: fixed-head source/golden audit. New golden probes independently reverse only FOV digits and recover both old complete-state hashes.
- `merge-reviewer-gate.txt`, `response-reviewer-gate.txt`: empty fixed-head ownership diffs. `c31-registers.json`: 18 byte-identical inherited registers.
- `register.json`, `register.md`: complete round-2 state inventory; no pending entries.

PowerShell may wrap native stderr in `NativeCommandError` headers even for successful compiler progress. Use explicit native exit sidecars and test-result summaries, not those wrapper headers, to classify a run. No captured cargo log is rewritten after completion.
