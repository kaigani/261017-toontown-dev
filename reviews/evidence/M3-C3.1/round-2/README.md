# M3 C3.1 round 2 — receipts

Local gate at the round-2 code (committed with the response), Windows 11. Raw output filtered to the result lines;
nothing is rewritten after capture. The CI run of the response head is named in the closing message.

| File | What |
|---|---|
| `workspace-tests.txt` | `cargo test --workspace --locked -j 4 --no-fail-fast` with the CI skips (the release-only oracles and properties that run in their own jobs): 274 test binaries, 1352 passed, 0 failed, 23 ignored. Includes every reviewer round-2 target (`c31_r2_variants` executor 21/21 and anim 3/3, `c31_r2_persistence` 3/3, `c31_r2_notes_boundary` 1/1, `c31_r2_cancellation` 2/2, `c31_r2_protocol` 1/1) and the builder's `c31_round2` 10/10. |
| `reviewer-python-suites.txt` | `scripts/run-reviewer-python-suites.py`: 46/46, including `c31-r2-audit` and `c31-r2-sdk`. `scripts/check-reviewer-ownership.py` ok. |
| `sdk-r2.json` | `tests/adversarial/c31-r2-sdk.py --binary target/debug/toontown-headless.exe` (official `mcp==2.0.0`): all five checks `ok` — RFC test before a delegated replace, RFC copy moving the hidden current column, generic cut clearing its note, 64-character clipping rows, worker available. |

Also run: `cargo fmt --all --check`; both clippy invocations of the CI `check` job (`-D warnings`); the
command-declaration generator (no diff, `plan/spec-issues.md` stable); allow inventory 98 lints / 40 files, 0 new.
