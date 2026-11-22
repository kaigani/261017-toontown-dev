# C4.1 round 1 register

Status covers the charter item, including explicit remaining limitations.

| Item | Status | Evidence and remaining scope |
|---|---|---|
| 1 | pending | sampled host/action-router and sensor paths; exhaustive mutation scan not completed |
| 2 | pending | mechanical full registry-to-GUI and native enablement audit not completed |
| 3 | pending | validation/staging code read; full catalogue comparison and adversarial values not completed |
| 4 | pending | import/canonicalization code sampled; cross-platform hostile import probes not completed |
| 5 | divergent | C41-R1-04 unsupported settings versions; crash/read-only/shape matrix pending |
| 6 | pending | bounded theme validation read; full UI/help/theme persistence adjudication pending |
| 7 | pending | declared in-window floats and splitter loss confirmed by source; native-window permanent probe pending |
| 8 | divergent | C41-R1-01 JSON null cannot clear modal; stale-report expiry and arbitration races pending |
| 9 | pending | host AtomicBool probe confirmed by inspection; lifetime/cross-window tests pending |
| 10 | divergent | C41-R1-02/03 and C41-R1-05 real socket failures; admission allocation confirmed; remaining security charter pending (http-continuation.json) |
| 11 | pending | main-only event capability and CSP inspected; full hardening matrix pending |
| 12 | divergent | C41-R1-01 modal suspension cannot clear; freshness race testing pending |
| 13 | pending | watchdog and restart code read; live heartbeat/recovery races not probed |
| 14 | implemented | independent embedded debug GUI 28/28 and HTTP 24/24; script limitations in report |
| 15 | pending | SI-112 through SI-118 read and provisional adjudication recorded; exhaustive code agreement pending |
| 16 | pending | allow count 94/38 and reviewer fmt/clippy pass; all-target clippy blocked by inherited test comments; generator/schema audit pending |
| 17 | implemented | D-1 own commit/profile/backtrace locations; D-2 exactly two candidate builder binaries |
| 18 | divergent | CI structure unchanged except GUI job; cold Windows check 58:14 exceeds ceiling; live cache inventory pending |
| 19 | pending | supplied evidence 39 files, no oversize/JSON token fields; manual scripts read, full secret/name/count audit pending |
| 20 | divergent | item 1 refuted by modal/settings defects; item 6 structure retained but cold ceiling missed; item 7 risks retained |

| Part C item | Charter | Final status | Evidence |
|---|---|---|---|
| A HTTP remainder | 10 | divergent | C41-R1-06; authority_matrix; part-c-http.txt; part-c-lifecycle.json; source reasoning and residual risks in report |
| B Bypass | 1 | implemented | part-c-bypass.md; part-c-invokes.txt; trusted-webview and diagnostic exceptions explicitly documented |
| C User priority | 8 | divergent | existing C41-R1-01; two part_c_priority tests pass, measured 80 ms waits; caller-side ordering limitation recorded |
| D Cancellation | 9 | divergent | C41-R1-07 P3; host.rs:121-152; App.test.tsx:184-201; collision/unwind source trace |
| E Settings remainder | 5 | divergent | C41-R1-08/09; part_d_settings (2 expected failures); part-d-settings.txt; source matrix in report |
| F Preferences | 3 | divergent | C41-R1-10/11; complete 79-key part-d-preferences.md; 2 passing matrix tests, 2 expected failures |

Part D final validation: workspace fmt and executor all-target clippy with test-hooks and required lint flags pass; frontend cancellation Vitest 1/1 passes. D/E/F all adjudicated; cumulative P0 0 / P1 1 / P2 9 / P3 1. Prior pending rows retain unrelated review scope only.

| Part E item | Verdict | Evidence |
|---|---|---|
| G Keymap | divergent | C41-R1-12/13; part_e_keymap; report lists unexecuted matrix |
| H1 Freshness | divergent | C41-R1-14; part_e_freshness; concurrent bound passes |
| H2 Restart | divergent | C41-R1-15; part_e_restart; clause table in report; host runtime gaps remain |
