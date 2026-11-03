# C2.4 R1 conformance register

Reviewed `0ded6fa454ecf4a191fceeb3638d116f102c8873`. Counts: `{"requirements": {"total": 63, "implemented": 47, "divergent": 6, "missing": 0, "deferred-with-owner": 10}, "readiness": {"total": 9, "implemented": 5, "divergent": 2, "missing": 0, "deferred-with-owner": 2}, "commands": {"total": 9, "implemented": 4, "divergent": 1, "missing": 0, "deferred-with-owner": 4}}`.

## Requirements

| id | clause | requirement | state | evidenceOrOwner |
| --- | --- | --- | --- | --- |
| U001 | 02 8.5 | selected brush outline becomes pencil lines; source removed | implemented | c24_centre_line; c24_r1_lifecycle font Y/T/O/X |
| U002 | 02 8.5 | triangulation and medial axis of spineless shapes | implemented | ops/centre; c24_centre bars/plus/frame/ring |
| U003 | 02 8.5; decision 12 | short-branch pruning and junction merging | implemented | c24_centre plus and frame; independent thin-neck/T probes |
| U004 | 02 8.5 | holes and branches; no-spine refusals stated | implemented | c24_centre and c24_r1_geometry; refusal conditions in ADR-0016 |
| U005 | 02 8.5 | source colour and texture matrix retained; colour splits by source | implemented | draw_strokeops; c24_centre_line; c23_strokeops |
| U006 | 02 8.5 | default thickness equals independent area / axis length | implemented | c24_r1_geometry: 0.25/0.75 du bars, T and thin neck, 0.05 relative ceiling |
| U007 | 02 8.5 | explicit thickness override | implemented | c24_centre_line::an_explicit_thickness_overrides_the_average_width |
| U008 | 02 8.5 | fill-only centreLineOnFill warning | implemented | retained c23_strokeops explicit/default painted-zone tests |
| U009 | 02 8.5 | one labelled undo; redo and save/open | implemented | c24_centre_line vectorized frame/ring/triangle lifecycle |
| U010 | 02 11.1 | menu/cell entry points | deferred-with-owner | Owner M4 C4.1; registered GUI locations, full drawing UI not M2 |
| U011 | 02 11.2 | PNG/JPEG/BMP/GIF/TIFF/TGA/flattened PSD; CMYK to RGB | implemented | c24_image; c24_decode_fuzz; 20,000 independent cases, 212 fixtures |
| U012 | 02 11.2; ADR-0018 | decoder typed errors, dimensions and allocation limits | implemented | c24_r1_decoder: 762 successes, max 2,849,367 bytes in seeded corpus; other variants typed refusals |
| U013 | 02 11.2 | ordered files -> consecutive drawing cells | implemented | c24_vectorize; SDK two-image variant |
| U014 | 02 11.2 | occupied ask/replace/skip and nonvector refusal | implemented | c24_vectorize; c24_contracts; E2 wrong/default camera target declaration |
| U015 | 02 11.2 | no selectable target -> new drawing element | implemented | c24_vectorize explicit empty-scene control |
| U016 | 02 11.2; SI-080 | type/filter/threshold defaults; thresholdOutOfRange | implemented | command-row blackAndWhite/70, all kernels; settings disagreement explicitly recorded |
| U017 | 02 11.2 | grey -> filter -> threshold -> trace -> smoothness-3 fit | implemented | vectorize Work account and fit_bounded; independent trace BFS and c24_trace_fuzz |
| U018 | 02 11.2 | ink brush outlines in one layer, 1200 du width, centred | implemented | c24_vectorize and c24_trace; rendering known pixels |
| U019 | 02 11.2 versus 03 11.6 | black generated swatch versus current colour; palette lifetime/names | divergent | C24-R1-05: ADR-0018 chooses 02, but contradictory 03 requirements have no spec-issue record |
| U020 | 02 11.2; 03 16 | withTexture bitmap swatch and coordinate mapping | implemented | c24_vectorize textured two-colour bar; source content-addressed PNG |
| U021 | 02 11.2; SI-081 | preview renders deterministically without document/file effects | implemented | c24_contracts and c24_vectorize; E2 hashes; PNG schema exception recorded |
| U022 | 02 11.2; SI-083 | job progress and cancellation keep completed prefix and skipped suffix | implemented | SDK variant completed >0 and <64, skipped count and ordered indices verified |
| U023 | 02 11.2; ADR-0018 | one transaction and undo step per committed image | implemented | c24_contracts; c24_vectorize; run_staged common acceptance |
| U024 | 02 11.3 | new drawing element, matching exposure, source image unchanged | implemented | c24_vectorize package with real bitmap -> project.open conversion |
| U025 | 02 11.4 | vector-file import | deferred-with-owner | Owner M10 C10.2, SI-082; declared command remains notImplemented |
| U026 | 02 11.5 | capture/scanner vectorization | deferred-with-owner | Owner M10 C10.2 import/acquisition surface |
| U027 | 02 11.6 | generic bitmap scaling preferences | deferred-with-owner | Owner M10 C10.2, generic import surface |
| U028 | 02 13.1.1-2 | structured command path and no modal dialogs | implemented | registered preview/query, run/job and typed confirmations; C0 guard and E2 |
| U029 | 02 13.1.3 | dry run and exact inverse history | implemented | c24_contracts common contract plus retained executor oracles |
| U030 | 02 13.1.4 | target addressing and IDs | implemented | c24_vectorize targets, image elements, empty target, wrong type |
| U031 | 02 13.1.5-6 | drawing coordinate mapping and Bezier inputs | implemented | retained c23 transform/point controls; vectorize emits authoritative brush Beziers |
| U032 | 02 13.1.7; 13.3 | interactive gesture equivalence for new dialogs/tool UI | deferred-with-owner | Owner M4 C4.1/C4.2; M2 command and geometry surface retained |
| U033 | 02 13.1.8 | confirmation does not block and changes nothing | implemented | c24_vectorize ask path; staged effect/rollback common contracts |
| U034 | 02 13.1.9 | same inputs deterministic on both OSes | implemented | CI symmetry/text goldens plus byte-identical seven E2 PNGs Windows/macOS/local |
| U035 | 02 13.1.10; 13.4 | query/render full content, zones and side IDs | implemented | E2 paint-by-ID, independent map readback; c24_r1_lifecycle record persistence |
| U036 | 02 13.1.11 | structured errors and revision/ID contracts | implemented | c24_contracts stale/locked/bad input; SDK stale expectRev refuses before job |
| U037 | 02 13.1.12 | jobs return IDs, progress, cancellation, final report | implemented | E2 + SDK variants; jobs cancelled state retains result |
| U038 | 02 13.1.13 | batching and atomic refusal for jobs | implemented | c24_contracts refuses vectorize in atomic batch; per-image granularity ruled |
| U039 | 02 13.2 | remembered vectorize settings | deferred-with-owner | Owner M10 C10.2; SI-080 interim command defaults, import.vectorize.settings.* declared |
| U040 | 02 13.5 | centreLine/breakApart/vectorize/symmetry schemas and examples | implemented | candidate CI check; per-crate tests; generator unchanged at 598 declarations |
| U041 | 02 13.6 | drawing changes and job state/progress | implemented | c24_contracts and E2; event publication through run_staged |
| U042 | 02 3.12; readiness 2 | independent topology oracle covers every topology operation | divergent | C24-R1-01: independent command routes cover only transform, segment cut, disjoint Flatten, vectorize |
| U043 | 02 3.12; readiness 2 | claimed exact metamorphic and Cutter area assertions | divergent | C24-R1-02: new seeds shrink two assertions; errors are within grid-snapping bound |
| U044 | 02 3.12; C24-T1 | lowest stroke/segment owner, write-through mirroring, unsplit overlap | implemented | fill::owner/read/write; new-seed fill property; three-way deletion/split and preexisting records |
| U045 | 10 2.2; C24-T1 | unknown extension namespaces stay opaque | implemented | c24_r1_geometry and c24_r1_lifecycle keep org.review.opaque through edits/history/package |
| U046 | 02 3.12; SI-059 | brush outlines never split by planar noding | implemented | retained geometry/cut controls and c24_trace brush outlines |
| U047 | C22-T2; ruling a | 100k Flatten completion at strokeAdd p50 and watchdog | deferred-with-owner | Owner M11 C11.1, accepted re-scope into R1-12; refusal explicitly not completion |
| U048 | C22-T2; ruling a | atomic work-charged refusal, fixpoint, phases and every-run gate retained | implemented | c24_measure, c24_flatten_phases, CI headless; new-seed fixpoint 4096 cases |
| U049 | C22-T2; ruling a; engineering | gate analyzer rejects malformed observations | divergent | C24-R1-03: negative, boolean and infinity timing accepted with exit 0 |
| U050 | rulings a/b; merge condition 1 | synchronized owner/decision/spec-issue bookkeeping | divergent | C24-R1-04: traceability still M2/proposed; SI-067 lacks confirmed middle coordinate step |
| U051 | 02 8 Feather; SI-077 | Feather bands/colour/alpha policy | deferred-with-owner | Owner spec owner SI-077 and M2 C2.4 C23-T2; explicit merge-scope decision still required |
| U052 | C21-R1-11 | configured mirror/kaleidoscope and same-font controls | implemented | c24_symmetry/c24_text_layout both OSes; session-only scope and fresh-session test |
| U053 | ADR-0003 rule 7 | OFL font manifest/hash/licence and dev-only hook; built artifact scan | implemented | cargo deny, release-feature proof, bundle logs scans both OSes; local headless scan |
| U054 | R1-12; readiness 8 | time/allocation/retained/RSS/queue/maintenance baselines | implemented | recomputed-series and recomputed-budget-ranges; proposed targets remain M11 C11.1 |
| U055 | R1-12 | interactive per-edit budgets at 100k | deferred-with-owner | Owner M11 C11.1, proposals: <=0.25s/64MiB; current baselines materially exceed |
| U056 | readiness 9 | Break Apart precharge cap; 8192 square render and queued reads | implemented | c24_aggregate + raw both OSes; 20000 visible-character work charge, typed limit.exceeded |
| U057 | 11 14.1-2 | command bindings, schema validation and executable examples | implemented | candidate check jobs, C0 guards, frozen pnpm, generator/IPC tests |
| U058 | 11 14.3 | save/open equality | implemented | E2 export/render equality; independent record lifetime test |
| U059 | 11 14.4 | new drawing gestures | deferred-with-owner | Owner M4 C4.1/C4.2 |
| U060 | 11 14.5 | cross-platform deterministic artefacts | implemented | seven E2 PNG bytes identical; symmetry/font goldens asserted in both check logs |
| U061 | 11 14.6-7 | never-block and official SDK protocol | implemented | mcp==2.0.0 local; E2 release, retained timeout/audit/queue suites |
| U062 | 11 14.8 E2 | complete paint-by-ID chain and base replay | implemented | 26/26; six SDK variants incl. guarded edited-base replay fail rev.conflict |
| U063 | engineering; handoff 6a | budget timing precision | divergent | C24-R1-06: macOS max snapshot 0.225958ms, actual maintenance requests 86-203ms and queue-read sends 103-175ms |

## Readiness

| id | item | state | evidenceOrOwner |
| --- | --- | --- | --- |
| 1 | Vectorize trace + fit | divergent | C24-R1-05 unresolved cross-section colour policy; 02 pipeline and SDK controls pass |
| 2 | Topology properties | divergent | C24-R1-01 incomplete independent breadth; C24-R1-02 numerical assertions fail |
| 3 | E2 headless chain | implemented | local 26/26, six variants, both CI artifacts byte-identical |
| 4 | C22-T2 | deferred-with-owner | Owner M11 C11.1 accepted ruling (a); synchronization C24-R1-04 |
| 5 | C23-T1 medial axis | implemented | builder suites and independent thin/T/neck/font/lifecycle controls |
| 6 | C23-T2 Feather | deferred-with-owner | spec owner SI-077, M2 C2.4 blocked; no milestone-scope waiver inferred |
| 7 | C21-R1-11 symmetry/text | implemented | both OS goldens and session-only control |
| 8 | R1-12 baselines | implemented | proposals not final target compliance; C24-R1-06 corrects precision |
| 9 | Aggregate limits | implemented | precharge, typed refusal and separate following/queued reads |

## Commands

| id | state | evidenceOrOwner |
| --- | --- | --- |
| draw.vectorize.preview | implemented | c24_vectorize/c24_contracts, E2 preview; SI-081 |
| draw.vectorize.run | divergent | functional 02 contract passes; 03 11.6 policy conflict C24-R1-05 |
| draw.centreLine.apply | implemented | c24_centre_line, retained spine/fill controls, reviewer geometry/font cases |
| draw.breakApart.apply | implemented | c24_aggregate 20000 bound; c24_text_layout; reviewer letter conversion |
| tool.symmetry.set | implemented | declared ahead of M4 C4.2, session-only; registered MCP ui scope appropriate |
| import.vectorize.settings.get | deferred-with-owner | M10 C10.2, SI-080 |
| import.vectorize.settings.set | deferred-with-owner | M10 C10.2, SI-080 |
| draw.vectorFile.import | deferred-with-owner | M10 C10.2, SI-082 ruling |
| draw.feather.apply | deferred-with-owner | M2 C2.4 blocked on SI-077; spec owner, no merge waiver |
