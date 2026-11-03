# Final-round evidence — product 9d42034

The deliverable is [the round-3 report](../../M3-C3.1-r3.md). No product or pre-existing test/receipt was modified.

- `register-complete.json`: fixed-head 694-unit / 64-row register and deltas.
- `patch.txt`, `binary-complete.txt`, `texture-finding.txt`: six enabled failing assertions, four P1 findings.
- `round2-proving.txt`, `closure-hierarchy-final.txt`, `closure-oracles.txt`, `delegation-complete.txt`: original proving tests and independent variants. `closure-oracles.txt` retains the first corrected 4096-sequence success plus two earlier setup failures; the later closure/hierarchy log supersedes those setup failures.
- `sdk-final.json.gz`: raw official SDK sweep of all 59 commands, including events and CLI replay; 419/420 checks pass, with R3-01 retained. `sdk-clipboard.json` reproduces R3-02/03 on release. Earlier SDK attempts remain losslessly archived and labeled. Use Python `gzip.open(path, 'rt', encoding='utf-8')` to read them. `sdk-archive-manifest.json` gives raw/compressed hashes; no raw call or check was removed.
- `selection-debug.txt`, `selection-release.txt`: refusal and cancelled-scan observations with queued reads.
- `performance-complete.json`: final derived comparison, including two C3.1 baselines, M2 repeated series, 100k raw samples and planar cases. `performance.json` is the earlier narrower comparison; it was preserved rather than overwritten. Source JSON is under `ci-artifacts/`.
- `ci-run.json`, `ci-full.txt`, `ci-artifact-manifest.json`: run 41689238994 identities/log and every downloaded file's SHA-256. Relevant raw CI records are retained here. Installers, executables, frontend build machinery and unrelated downloads were moved to the temporary archive named in the manifest, not committed as review evidence. The release binary used by the SDK is identified by its downloaded artifact hash there.
- `repros/`: actual distinct-project bitmap collision inputs and the saved package missing both recovery textures, with package hashes.
- Other text files preserve initial attempts and engineering checks. Early compilation/reference/fixture/export-timestamp mistakes are setup history, not additional product findings. All corrected checks retain their substantive assertions.

Most PowerShell-captured text logs are UTF-16 with a BOM; JSON collector outputs and this file are UTF-8. Readers should honor the BOM. No collector rewrites committed historical evidence; new receipt destinations must not already exist.
