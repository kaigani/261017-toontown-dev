# M3 C3.3 evidence

Raw receipts; nothing here is rewritten after capture.

| File | What |
|---|---|
| `local-gate/` | `scripts/c33-render-gate.py` against a local release `toontown-headless` (Windows, official `mcp==2.0.0`), on the C3.3 working tree before the handoff commit: 10/10 checks — the four gate images equal the in-process hashes of `crates/executor/tests/golden/c33/gate-index.json` (recorded by a debug build, so debug and release render the same bytes), the non-horizontal path's passage frames (`gate-path.json`, IDs replaced by names), and a three-frame PNG sequence delivered with its manifest. CI repeats the gate on Windows and macOS (`c33 render release`) and compares both OSes' outputs byte for byte (`c33 cross-OS`); its artefacts are the authoritative receipts. |
| `ci-42788300642/c33-{Windows,macOS}/` | The render gate's CI artefacts at `69c5b4d` (`c33 render release`, release headless, `mcp==2.0.0`): 10/10 checks on each OS; `c33 cross-OS` compared every file byte for byte (gate images, the three-frame sequence, its manifest and `gate-path.json`): identical. |
| `staging.png`, `gate-frame-9.png` | Copies of two goldens for review at a glance (the goldens themselves are `crates/executor/tests/golden/c33/`). |
