# C3.1 round-1 evidence

Product under review: `8f44969741f8662831e25a01a8a024b686daf02e`. Comparison base: `0b8fd0b`.
The review changes only new review tests, reports and evidence. Entry point: [review report](../../M3-C3.1-r1.md).

## Candidate CI and raw observations

- `ci-run.json`, `ci-run.log`: metadata and complete downloaded console logs from run **41621608304**, including both OSes' release properties, history/geometry oracles, examples, feature exclusions and conformance checks.
- `ci-artifact-index.json`, `raw-provenance.json`: source artifact identities and the hashes of extracted bytes.
- `raw-candidate-artifacts.zip`: byte-preserving copy of the downloaded/extracted artifact files. Git normalizes text line endings in the convenient extracted `ci-artifacts/` copies; use this archive for original JSON bytes. PNG bytes are unchanged in both locations.
- `ci-artifacts/measurements-{Windows,macOS}/`: actual candidate C3.1 timing and inherited C2.4 budgets/aggregate observations. No refused aggregate is treated as completed work.
- `ci-artifacts/e2-{Windows,macOS}/`: seven PNGs plus hashes, replay exports and reports on each OS.
- `independent-analysis.json`: reviewer recomputation from the candidate raw observations and **M2 C2.4 round-3** raw records already retained in the repository; includes RSS, allocation, queue and maintenance values.
- `ci-property-log-extract.txt`: convenient subset of the unmodified complete log, showing 4096-case environment/command and the seven property outcomes on each OS.

The collector `tests/adversarial/c31-r1-audit.py` rejects malformed timing rows (including nonfinite/negative/boolean/oversized numbers, missing or duplicate cases, wrong identities, outcomes and revision behavior). Its default mode is a read-only unittest. `--collect` requires an explicit destination with the downloaded inputs and refuses to overwrite an existing derived analysis. It never modifies raw evidence.

## Local tests

- `anim-initial.txt`: initial per-crate build/test.
- `anim-final.txt`: seven full 4096-case canonical-domain properties, 4800 independent columnate-formula combinations, worked/edge controls and the two retained failures. `anim-oracle.txt` is the earlier run before the extra canonical-input control/formula enumeration.
- `hierarchy-review.txt`: independent 4096-case permutation/delete-reattachment oracle plus invalid order/detach-attach controls.
- `contract-oracle.txt`, `contract-final.txt`, `contract-extra.txt`, `contract-complete.txt`: successive test-development runs, retained rather than rewritten. Early batch fixture setup missed its required label; the corrected batch still demonstrates the session leak. The first opaque-copy setup used an invalid extension namespace; that setup history is not the C31-R1-13 reproduction.
- `contract-reviewed.txt`: latest complete contract probe binary: 28 tests, 6 controls pass and 22 assertions demonstrate findings.
- `clipboard-opaque.txt`: corrected opaque-data reproduction (`review.example` namespace), failing at the actual data-loss assertion. That correction is also present in `contract-reviewed.txt`.
- `contract-complete.txt`: also contains the passing 44-action/toggle common-contract matrix, including exact created/removed IDs, revision and document-event checks, dry run, undo/redo, stale guard, atomic failure and save/open equality.
- `selection-budget.txt`: large canceled selection leaves the document worker busy; following read times out. Isolated test binary avoids contaminating unrelated controls.
- `notes-shift-matrix.txt`: all 12 shifting timing/transfer operators leave the note at its original frame rather than its independently computed destination.
- `copy-settle.txt`: passing control for overwriting an existing drawing through the settle route; all copied nodes and local IDs match the source. It is separate from the failing generic-paste opaque-data test.
- `mcp-initial.txt`, `mcp-review.txt`: inherited per-crate MCP tests and independent C3.1 protocol confirmation/retry, edit/undo/ref codes and dry-run leakage.
- `sdk-review.json`, `sdk-review.txt`, `sdk-install.txt`: independent official `mcp==2.0.0` client against the debug headless binary. Five checks pass; session dry-run isolation and exact columnate total fail. Project/settings files were confined to a temporary directory. The SDK environment is helper material under ignored `target/review-sdk/`.
- `executor-initial.txt`, `workspace-final.txt`: long inherited per-crate/workspace validation. The final report addendum identifies expected reviewer failures and any other failure; green CI is not substituted for the local outcome.
- `inherited-python.txt`: 41/41 original Python suites. `all-python-final.txt`: 44/44 including the three new reviewer suites, after frozen pnpm installation.
- `audit-*.txt`, `register-*.txt`, `sdk-helper-tests.txt`: new history-pinned/static/helper unit results.
- `pnpm-install.txt`, `generator.txt`, `fmt*.txt`, `cargo-deny.txt`, `allows-final.txt`, `ownership-final.txt`: environment/freshness/engineering checks.

`change-at-0b8fd0b.txt` is the pinned pre-change M1 algorithm. `c31_r1_model_diff.rs` reconstructs its string path independently, checks 4096 exposure-diff cases and a 100000-frame case; its passing receipt is in `contract-oracle.txt`. `declared-diffs.txt` and `builder-file-diff.txt` record the candidate comparison against M2. Some PowerShell native-command logs contain a `NativeCommandError` formatting wrapper around ordinary stderr; the Rust test summary/exit outcome determines pass/fail.

## Reproduction entry points

```text
cargo test -p tt-anim --locked -j 4 --test c31_r1_oracle --test c31_r1_hierarchy --no-fail-fast
cargo test -p tt-executor --locked -j 4 --test c31_r1_contract --test c31_r1_mutations --test c31_r1_model_diff --test c31_r1_selection_budget --no-fail-fast
cargo test -p tt-mcp --locked -j 4 --test c31_r1_protocol
python scripts/run-reviewer-python-suites.py
python tests/adversarial/c31-r1-sdk.py --binary target/debug/toontown-headless.exe --output <new-receipt.json>
```

Failures are intentionally enabled and must not be filtered out of a future fixing round. Default audit/register tests compare fixed historical SHAs, never a future working tree. No historical M1/M2 receipt was rewritten, and `TT_COMMIT_REVIEW_RECEIPTS` was unset.

Final additions: `availability.txt` retains the image drawing-lock refusal failure; `workspace-summary.json` indexes the completed single workspace run (1374 passed, 22 reviewer failures, 23 ignored). Latest targeted review totals are 51 tests: 23 pass and 28 retained failures. The final source register separates media/sound availability portions into owned deferrals under SI-088.
