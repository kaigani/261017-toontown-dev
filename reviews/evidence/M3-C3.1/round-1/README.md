# M3 C3.1 round 1 — receipts

Local gate at the round-1 code head (`20788e0`; the response commit adds documents only), Windows 11. Raw output,
filtered to the result lines; nothing is rewritten after capture. The CI run of the response head is named in the
closing message.

| File | What |
|---|---|
| `workspace-tests.txt` | `cargo test --workspace --locked -j 4 --no-fail-fast` with the CI skips (the release-only oracles and properties that run in their own jobs): 267 test binaries, 1311 passed, 0 failed, 23 ignored. |
| `reviewer-python-suites.txt` | `scripts/run-reviewer-python-suites.py`: 44/44, including `c31-r1-audit`, `c31-r1-register`, `c31-r1-sdk`. `scripts/check-reviewer-ownership.py` ok. |
| `frontend.txt` | `pnpm lint`, `typecheck`, `test` (40 files, 333 tests), `format:check`. |

Also run: `cargo fmt --all --check`; both clippy invocations of the CI `check` job (`-D warnings`); the
command-declaration generator (no diff); allow inventory 98 lints / 40 files, 0 new.
