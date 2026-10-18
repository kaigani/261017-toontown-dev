# Round 3 evidence

Start with the [round-3 report](../../M0-C0.4-r3.md).
`final-audit.json` summarizes all gate receipts; `decision-results.json` contains the 35 independent cases.
Fixture `*.json.gz` and matching `*.analysis.json` files preserve real CLI inputs/results. Logs are gzip-compressed.
Fixtures are synthetic tests, not physical-pen measurements. Canvas fixture copies omit unused embedded PNG previews.

From the repository root, reproduce the analysis with:

```text
python reviews/evidence/M0-C0.4-r3/decision_probes.py
python reviews/evidence/M0-C0.4-r3/run_review_suites.py
python reviews/evidence/M0-C0.4-r3/copy_probe.py
python reviews/evidence/M0-C0.4-r3/audit_ci.py
python reviews/evidence/M0-C0.4-r3/final_audit.py
```

`run_review_suites.py` expects the new R3 tests to fail; each underlying test command itself exits nonzero.
The amended R2 suite and builder controls pass. The page runner copies only its reviewer test into Vitest's configured
tree, then removes that temporary file in `finally`; do not run other frontend suites concurrently with it.

For a fresh full gate: install frozen dependencies and build the default frontend, run `run_gates.py rust`, then
`run_gates.py frontend`, then `run_gates.py default` and `probe_gating.py default`, then `run_gates.py diagnostics`
and `probe_gating.py diagnostics`. This order preserves the matching bundle/binary for each independent probe.
Tauri commands run through `cmd /d /c`, without packaging locally; exact-head CI supplies both platforms' packages.

No physical tablet test or real-Mac test was run. No Windows Security prompt was dismissed.
