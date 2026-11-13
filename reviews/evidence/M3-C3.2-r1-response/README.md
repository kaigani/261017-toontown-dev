# M3 C3.2 round 1 — builder receipts

Local runs on Windows against the round-1 working tree (built on `970d9e7`, the merge of `review/m3-c32-r1`, before the
round-1 commit; `headSha` in the reports is that parent). Release `toontown-headless`, official `mcp==2.0.0`.

| File | What |
|---|---|
| `local-e4/` | `scripts/e4-cutout-numeric.py`: 8/8 checks, worst error 8.9e-16. `evaluate.json` differs from C3.2's `f41480ab…` only in 96 `screen.bbox` values: the new project's camera now has the exact default FOV (C32-R1-03); world, local and decomposition bytes are unchanged. |
| `local-e4-variant/` | `--variant` (the reviewer's changed middle pose and second current camera): 8/8 checks, worst error 8.9e-16. CI compares it across OSes from the round-1 commit on. |
| `finite-decomposition/` | The reviewer's `finite-decomposition.py`, unchanged: exit 0; 26 nested 1e6 pegs read back scale 1e156 (C32-R1-06). |
| `sdk-matrix/` | The reviewer's `sdk-matrix.py`, unchanged: exit 0, 40/40 records, the C32-R1-04 and both C32-R1-05 records included. |
| `workspace-totals.txt` | `cargo test --workspace` with `check`'s skip list, debug, `-j 3`: 299 binaries, 1458 passed, 0 failed, 24 ignored. |
