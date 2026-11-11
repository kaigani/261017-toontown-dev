# Charter dispositions

| Item | Final evidence / adjudication |
|---|---|
| A ownership | c33_r3_recovery positive ownership/published probe, amended r2 recovery, builder round2 all 9 tests; pass |
| A live jobs | c33_r3_live actual child export passes; builder same-process concurrent and live-child/other-host probes pass |
| A records | new cancellation error test fails (R3-01); failed records/save/open/fresh-session checks pass; dedicated MCP failed receipt passes, cancellation missing error fails |
| A rollback | independent c33_r3_transaction refused restore, preserved backup/sidecar, retry byte identity and no partial manifest pass |
| B register | every 198 row retained; two divergent for R3-01; 0 pending |
| C ownership/declarations | audit.json, amendments.json, assertion-comparison.json; exact ownership gate empty, no product edits |
| D determinism/performance | downloaded current-head artifacts, 27 direct comparisons / 18 identical; diagnostic differences recorded; unchanged goldens, inherited tests and finite measurements adjudicated in report |
| E engineering | full workspace native 101 only 3 classified targets; 52 Python suites pass; all-target and strict new-root lint, per-package formatting, dependency audit, features, generator, tooling guard pass; 28/28 fixed-head CI jobs green |
| F readiness/closure | numbered report lists ready for scoped adjudication; attached gate M4 C4.2, thumbnails M5 C5.2; no new SI |

A final disposition is not an exhaustive race proof. The report names residual coverage limits and the waiting-test ruling conflict explicitly. No charter item is left as an unreported promise of future passing evidence.
