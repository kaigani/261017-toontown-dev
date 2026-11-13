# M3 C3.1 round 3 — receipts

Local gate at the round-3 code (committed with the response), Windows 11. Raw output filtered to the result lines;
nothing is rewritten after capture. The CI run of the response head is named in the closing message.

| File | What |
|---|---|
| `workspace-tests.txt` | `cargo test --workspace --locked -j 4 --no-fail-fast` with the CI skips (the release-only oracles and properties that run in their own jobs): 282 test binaries, 1374 passed, 0 failed, 23 ignored. Includes every reviewer round-3 target (`c31_r3_patch` 4/4, `c31_r3_binary` 5/5, `c31_r3_closure` 4/4, `c31_r3_delegation` 2/2, `c31_r3_hierarchy` 1/1, `c31_r3_selection` 1/1, `c31_r3_sequences` 1/1) and the builder's `c31_round3` 4/4. |
| `reviewer-python-suites.txt` | `scripts/run-reviewer-python-suites.py`: 50/50, including `c31-r3-audit` and the three round-3 SDK suites in their default (helper) mode. `scripts/check-reviewer-ownership.py` ok. |
| `sdk-r3.json` | `tests/adversarial/c31-r3-sdk.py --binary target/debug/toontown-headless.exe` (official `mcp==2.0.0`): ordered ordinary-source copy `ok`, worker available `ok`. |
| `sdk-r3-clipboard.json` | `c31-r3-clipboard-sdk.py` with the reviewer's `repros/seed-3225905-21.animpkg` (source) and `seed-3225905-199.animpkg` (destination), copied to a scratch directory: cross-project colliding IDs keep source artwork `ok`; atomic paste then copy captures staged bytes `ok`. |
| `sdk-r3-texture.json` | `c31-r3-texture-sdk.py` with the reviewer's `repros/texture-41.animpkg`, `texture-209.animpkg` (copied): collision remaps references `ok`; both blobs readable before save `ok`; both survive save/open `ok`. |

Also run: `cargo fmt --all --check`; both clippy invocations of the CI `check` job (`-D warnings`); the
command-declaration generator (no diff, `plan/spec-issues.md` stable); allow inventory 98 lints / 40 files, 0 new.
