# C1.4 round-three evidence

Product: `39ea1b2`, Windows 11, Rust 1.98.1 MSVC. Product/spec files are unchanged. The report is [M1-C1.4-r3.md](../../M1-C1.4-r3.md).

- `workspace.log`: full original workspace sweep, 695 passed / 0 failed / 5 ignored.
- `replay-final.log`: final added replay regressions, 1 passing control / 4 deliberate failures. It supersedes `replay-only.log` and the replay section of `rust-r3.log`, which retain initial fixture refinements.
- `rust-r3.log`, `mcp-history-r{1,3}.json`, `mcp-history-r{1,3}-commands.json.gz`: both retained 10k seeds driven through MCP on saved/reopened fixtures, with a full second 10k operation replay and canonical comparisons. The command JSON is losslessly gzip-compressed by finalize.py. `build-oracles.py` preserves the original oracle body and adds the MCP transport; run rustfmt after regeneration.
- `protocol-r3.log`: exact-head CI binary; `protocol-cargo.log`: local Cargo-built binary. The final protocol failure is cancellation followed by a late accepted elicitation; the other variants pass. `python-final.log` verifies automatic discovery of both new Python files, with native exit status recorded separately.
- `frontend-rerun.log`: completed 333 Vitest + 9 tooling tests. The earlier `frontend.log` also completed with the same passing totals; its shell wrapper returned 1 despite the passing native output.
- `sdk.json`, `sdk-ci-*.json`, `sdk.log`: local and Windows/macOS official SDK 24/24. Linux SDK job/steps and raw results are in `ci-green.*`.
- `ci-green.*`, `ci-failed.*`: fresh GitHub Actions API records/logs for the exact requested runs. No PR/issue was created.
- `observations/`: exact copied local/CI raw measurements. `measurement-analysis.*` recomputes all 19 source files. `summary.json` records the CI executable hash, oracle receipts and test totals.
- `verification.json`, `reviewer-path-diff.txt`, `builder-test-diff.txt`: receipt hash checks, 18 gate statuses/totals, reviewer-owned path check, lint/dependency/generated checks and declared builder edits. `verify.py` reproduces those checks.
- `conformance.*`, `register.py`: all 226 + 32 original units reparsed and re-adjudicated, with existing plan checkpoints attached to every deferred row.
- `readiness.*`, `readiness.py`: all 51 SI numbers, including removed/reserved numbers, and all five ignored function bodies compared against their introduction commits.
- `fmt.log`, `new-tests-clippy.log`: new Rust test formatting/clippy. `sha256.json` hashes this evidence set; run `finalize.py` to refresh it after all writers stop.

Commands used: `pnpm install --frozen-lockfile`; `cargo test --workspace --locked`; targeted `cargo test -p tt-mcp --test c14_r3_replay --test c14_r3_history_r1 --test c14_r3_history_r3 --locked --no-fail-fast -- --nocapture`; `cargo test -p tt-headless --test c14_r3 --locked -- --nocapture`; Python suite runner with TT_REVIEW_BINARY; official SDK script; declaration generator; release-feature proof; measurement analyzer; `pnpm test`; targeted new-test clippy and `cargo fmt --all --check`.

Cargo used CARGO_INCREMENTAL=0 and CARGO_PROFILE_DEV_DEBUG=CARGO_PROFILE_TEST_DEBUG=0 to limit this worktree's disk use. PowerShell redirection added native-stderr presentation records to some early logs and can return a shell status of 1 even for a passing native suite; the complete test-result records, JSON receipts and explicit Python native status are retained. Completed UTF-16 shell logs were losslessly decoded to UTF-8 for review. No passing result is inferred from a truncated log or from the shell wrapper alone.

This evidence does not claim the unexecuted Cartesian crash/stage/ACL/late-root variants described in the report. New failures are deliberate regressions for scoped builder closure; the review branch is expected to be red until those are fixed or ruled on.
