## Spec issues SI-001–051

“Decided” is an implementation decision recorded in plan/spec-issues.md, not a claim that the frozen spec was edited or the spec owner ratified it. The four open issues remain with the spec owner. SI-051’s representation now also needs the orchestrator’s C14-R3-02 ruling.

| Issue | Status | M1 dependency / next owner |
|---|---|---|
| SI-001 | open with spec owner | No: encoder quality mapping is M10; M1 has no encoder. Owner: spec owner with builder M10 C10.1. |
| SI-002 | closed (reserved; no active issue) | Reserved for measured deterministic-format incompatibility, M10 C10.1. |
| SI-003 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-004 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-005 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-006 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-007 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-008 | open with spec owner | Yes: M1 preserves opaque importOptions without interpreting it; explicit interim contract. Owner: spec owner with builder M10 C10.2. |
| SI-009 | closed (removed) | No active obligation; removed by C1.1 round-one ruling. |
| SI-010 | closed (removed) | No active obligation; removed by C1.1 round-one ruling. |
| SI-011 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-012 | closed (removed) | No active obligation; removed by C1.1 round-one ruling. |
| SI-013 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-014 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-015 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-016 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-017 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-018 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-019 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-020 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-021 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-022 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-023 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-024 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-025 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-026 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-027 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-028 | closed | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-029 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-030 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-031 | closed | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-032 | closed | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-033 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-034 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-035 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-036 | closed | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-037 | closed | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-038 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-039 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-040 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-041 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-042 | decided | Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler. |
| SI-043 | closed | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-044 | decided | M1 uses an un-undoable repair base to preserve integrity. Spec-owner ratification remains; builder M1 C1.3 decision stands. |
| SI-045 | closed | M1 confirmation delivered; C14-R2-07 closes save-boundary bypass. Reload is a user action under SI-050. |
| SI-046 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-047 | open with spec owner | Yes: binding interim ruling requires content hashes at save; watch remains size+mtime. Owner: spec owner with builder M1 C1.4 / M4 C4.1. |
| SI-048 | decided | Recorded decision applies where referenced; no undecided M1 dependency. |
| SI-049 | open with spec owner | Yes: supported older revisions are explicit; spec owner must decide the newer subscription protocol. Owner: spec owner with builder M1 C1.4 / M4 C4.1. |
| SI-050 | decided | M1 keeps app scope unavailable to agents; reload belongs to the user; keep-mine uses a bound token. |
| SI-051 | decided | M1 depends on replay semantics. C14-R3-01/02 block implementation closure; builder/orchestrator C1.4. |

## Ignored tests and carried tickets

| Test / ticket | Owner and deadline | Body unchanged since introduction |
|---|---|---|
| `every_result_lists_its_effective_params` — C14-R1-16 effectiveParams | builder M4 C4.1; before dialogs/result contracts are completed | True (`5983f29`) |
| `namespace_docs_carry_a_worked_example_with_executed_results` — C14-R1-16 executed examples | builder before M2 C2.1 begins | True (`5983f29`) |
| `an_entry_over_4_gib_round_trips_with_zip64_sizes` — large ZIP64 release-only scenario (not a defect ticket) | builder M1 C1.3; release scenario retained, rerun before changing ZIP64 writer | True (`04cc687`) |
| `child_lock_holder` — crash child process (not a defect ticket) | builder M1 C1.4; parent must execute it in every ordinary workspace run | True (`4e34520`) |
| `c03_r1_04_simd_dispatch_must_not_change_final_pixels` — C0.4 SIMD ticket (outside C1.1–C1.4) | builder M3 C3.3; before final-renderer SIMD dispatch | True (`f0f55d8`) |
