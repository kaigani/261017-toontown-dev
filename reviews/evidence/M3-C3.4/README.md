# M3 C3.4 builder evidence

- `local-e1-windows/`: E1 on the local release binary (Windows 11, SDK `mcp==2.0.0`): `report.json` (53/53 checks),
  `hashes.json`, the canonical `evaluate.json`, `manifest.normalized.json`, `contact-sheet.json` and `.png`, and the
  recorded `e1-commands.json`. The run was on the working tree committed as `a0f0034`; the report's `headSha` reads
  `7216cb9`, the commit checked out at the time. A second local run produced byte-identical outputs (every file
  except `report.json`, the packages and the delivered manifest).
- `ci-e1-windows/`, `ci-e1-macos/`: `report.json` and `hashes.json` of the E1 step in `c33 render release`, run
  42961834557 at `46ba65e` (53/53 on each OS, SDK 2.0.0; 31.1 s on Windows, 16.5 s on macOS); `c33 cross-OS` compared all 36
  E1 outputs byte for byte (identical).
- Local gates at `a0f0034`:
  - per-package `cargo fmt --check`;
  - both clippy invocations;
  - `check-product-allows.py` (97 / 40 / 0 new);
  - no bytes 0x81/0x8D/0x8F/0x90/0x9D in changed sources;
  - `gen-command-declarations.py` (495, unchanged);
  - the CI-equivalent workspace run: 353 binaries, 1593 passed / 0 failed / 24 ignored.
- CI budget routing (`7216cb9`, run 42953847540, green): Windows `check` 41 min 40 s (from 49 min 19 s in run
  42938979758).
