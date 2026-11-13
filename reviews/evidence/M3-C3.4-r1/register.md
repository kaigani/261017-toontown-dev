# C3.4 round 1 register

{'implemented': 41, 'divergent': 1, 'missing': 0, 'deferred-with-owner': 3, 'pending': 0}

| ID | Clause | Requirement | State | Evidence / owner |
|---|---|---|---|---|
| E1-01 | 11 §14.8; 10 §3–4 | Fresh 1920x1080, 24 fps project | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-02 | 11 §14.8; 04 §2 | Ball drawing element and readback | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-03 | 11 §14.8; 02 §6 | Circle geometry and pen width | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-04 | 11 §14.8; 03 §8 | Paint via resolved swatch | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-05 | 04 §9, §17 | Peg parenting and exposure 1–30 | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-06 | 05 §9.1 | Reflected endpoint TCB at E1 zero parameters | implemented | tests/adversarial/c34-r1-numeric.py; independent-artifacts.json |
| E1-07 | 05 §9.2 | 64-chord distance inversion | implemented | independent-artifacts.json; expectations.log |
| E1-08 | 05 §10.4–10.5 | Preset Bezier inversion and velocity pins | implemented | independent-artifacts.json; expectations.log |
| E1-09 | 05 §2.10 | Extrema, monotonic segments and 25–30 hold | implemented | tests/adversarial/c34-r1-numeric.py; local-a/report.json |
| E1-10 | 11 §7.5; SI-108 | 5-column labelled sheet layout | implemented | independent-artifacts.json |
| E1-11 | 11 §7.5; SI-108 | 24 cells equal independent CLI single-frame pixels | implemented | independent-cell-parity.json |
| E1-12 | 09 §11.1(1–2) | PNG sequence and deterministic single frames | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-13 | 09 §11.1(3); 11 §6.5 | Inline dimensions and file policy | implemented | workspace.log (c33_r1_limits and c33_r2_wait); local-a/report.json |
| E1-14 | 09 §11.1(4); 11 §7.3 | Sequential waited range job and fresh-process replay | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-15 | 09 §11.1(5) | Fail and rename policies preserve originals | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-16 | 09 §11.1(5); ADR-0020 | Late read-only target rollback; no partial manifest | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-17 | ADR-0020 decisions 10–11 | Sidecar ownership and recovery | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-18 | 09 §11.1(4); 11 §15 | Cancelled jobs and list/wait/status records | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-19 | 09 §11.4 | 24 names/order/count/dimensions/hashes/fps/duration | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-20 | 11 §7.5, §7.8 | Describe, hit test and convert at integer frame 12 | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-21 | 11 §14.3; 10 §3–4 | Undo/redo and save-close-open canonical equality | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-22 | 11 §14.5; 11 §5, §7.3 | Replay symbolic script with canonical and PNG equality | implemented | local-a/report.json; local-b/report.json; independent-artifacts.json |
| E1-23 | 05 §1.4, §2.11 | Parent rotation/scale and positive camera roll; exact default FOV | implemented | independent-variant.json; local-a/report.json |
| E1-24 | 11 §5; 03 §8 | Swatch index/name acceptance and typed refusal | implemented | executor.log (c34_e1); workspace.log (c34_r1_contract) |
| E1-25 | 11 §7.3 | Atomic job rejection; no document mutation | implemented | workspace.log (c34_r1_contract) |
| E1-26 | 09 §11.1(4) | Shared direct/batch wait envelope and remaining budget | implemented | workspace.log (c34_r1_contract); crates/executor/src/run.rs; crates/executor/src/lib.rs |
| E1-27 | ADR-0020; host ruling | Case-insensitive sidecar host liveness; lock unchanged | implemented | sdk-environment.log; executor.log; workspace.log; product-diff-audit.json |
| E1-28 | 11 §9; 05 §19 | Changed-frame event coverage and one evaluation/node/frame | implemented | executor.log (c34_e1) |
| E1-29 | 11 §14; CI ruling | Both-OS release E1 and inherited suites | implemented | ci-summary.json; routing-proof.json; release-oracles.log; workspace.log |
| E1-30 | Engineering | Product lint/deny/generator/example/ownership and historical pins | implemented | allows.log; deny.log; generator.log; clippy-product.log; clippy-tests.log; historical-registers.json; workspace.log |
| E1-31 | 11 §14.8; ADR-0008 | MP4 movie encoding | deferred-with-owner | M3-merge-note.md §3 / M10 C10.1 (sidecar M8 C8.3) |
| E1-32 | 11 §14.5; readiness 12 | Attached/headless byte parity | deferred-with-owner | plan/traceability.csv row 41; M3-merge-note.md §3 / M4 C4.2 |
| E1-33 | Merge-note audit | Accurate open-SI inventory and ruled user actions | divergent | C34-R1-01; plan/spec-issues.md headings SI-001 and SI-008 |
| READY-01 | C3.3 closure readiness 1 | Readiness item 1 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-02 | C3.3 closure readiness 2 | Readiness item 2 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-03 | C3.3 closure readiness 3 | Readiness item 3 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-04 | C3.3 closure readiness 4 | Readiness item 4 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-05 | C3.3 closure readiness 5 | Readiness item 5 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-06 | C3.3 closure readiness 6 | Readiness item 6 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-07 | C3.3 closure readiness 7 | Readiness item 7 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-08 | C3.3 closure readiness 8 | Readiness item 8 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-09 | C3.3 closure readiness 9 | Readiness item 9 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-10 | C3.3 closure readiness 10 | Readiness item 10 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-11 | C3.3 closure readiness 11 | Readiness item 11 | implemented | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log |
| READY-12 | C3.3 closure readiness 12 | Readiness item 12 | deferred-with-owner | M3-C3.4-r1.md; local-a/report.json; independent-artifacts.json; workspace.log / M4 C4.2 |
