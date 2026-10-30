# Round-two evidence

The review is `../../M1-C1.4-r2.md`. Product baseline: `74fe3a9`.

Current completed results:

- `workspace-resumed.log`, `workspace-rest.log`, `executor-remaining.log`: combined complete Rust regression sweep. `sweep.py` compares every pre-existing case with the builder's gate log. Expected new failing targets: c14_r2, c14_r2_effects and c14_r2_replay.
- `effects-contracts-final.log`: final 3 passing contracts, 1 passing staged-restart case, 3 failing direct-write rollback cases.
- `replay-final.log`: canonical script replay, 1 pass / 2 failures.
- `protocol-final.log`: final 14-method real-process suite. Six failing methods produce nine unittest failures including malformed-evidence and package/folder subcases. Eight methods pass. `protocol-final-status.json` records its real exit status.
- `protocol-r1-resumed.log`: original 23/23 protocol cases. `frontend-resumed.log`: 333 Vitest + 9 license-tool tests. `sdk-resumed.json`: official SDK 24/24. `python-resumed.log`: existing suites pass; new measurement-regression suite fails as expected. `tickets-ignored.log`: the two owned tickets still fail explicitly.
- `conformance.json` / `.md`: current register; regenerate with `python register.py` from any directory.
- `verification.json`: receipt hashes, fresh CI download comparisons, unchanged ticket bodies across their rename, unchanged dependencies, original reviewer path diff. Regenerate with `python verify.py` after downloading the two headless artifacts into workspace `target/review-ci-verify` using `gh run download 41041163280 -n headless-Windows -n headless-macOS -D target/review-ci-verify`.
- `ci-artifacts.zip`: every original downloaded artifact file, unmodified, including both headless executables/observations and other artifacts inspected during the first run. `ci-artifact-manifest.json` records SHA-256 of each member. `bundle.py` created and byte-verified this archive before moving the expanded downloads beneath workspace `target/`. To rerun it, extract the archive to `ci/` and decompress the dev graph to `dev-feature-tree.log` first. The evidence archive replaces only the expanded evidence copies in the committed view.
- `local-recomputed`, `windows-recomputed`, `macos-recomputed` JSON/log pairs: recomputed builder/CI quantiles and environment fields. `reviewer-observations/` preserves the resumed local run's raw measurements and `reviewer-measurements-recomputed.json` checks them separately.
- `release-features-resumed.log`, `product-feature-summary.log`, `dev-feature-summary.log`: graph checks. The full inherited dev graph is losslessly compressed as `dev-feature-tree.log.gz`; it expands to about 196 MiB of repeated dependency edges.

Earlier `*-local`, `round-one`, `round-two`, `workspace-tests`, `script-replay`, `other-effects`, `frontend-tests` and similar logs are the interrupted run's observations. They are retained for provenance, including its disk-exhaustion build failure and frontend timeout, and are superseded by the named completed runs. Some PowerShell-captured logs/metadata use UTF-16LE with a BOM; `sweep.py` detects it. Raw logs and archives are marked `-text` for Git byte preservation.

No CI success claim applies to the reviewer branch with intentionally failing new regressions. The green CI run is the product handoff `74fe3a9` only. No GitHub issues or product fixes were created in this review.
