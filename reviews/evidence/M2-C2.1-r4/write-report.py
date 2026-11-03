"""Assemble the final report from independently retained results."""
from pathlib import Path
import json,re
ROOT=Path(__file__).resolve().parents[3]; HERE=Path(__file__).resolve().parent
def totals(name):
 s=(HERE/(name+'.log')).read_text(encoding='utf8',errors='replace')
 m=re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',s)
 return list(map(sum,zip(*(map(int,x) for x in m))))
audit=json.loads((HERE/'audit.json').read_text())
gates=json.loads((HERE/'gates.json').read_text())
extra=json.loads((HERE/'extra-gates.json').read_text())
assert all(x['exit']==0 for x in gates+extra)
for n in ['closure-debug','closure-release']:
 assert totals(n)==[4,0,0]
assert totals('workspace')[1]==0
brush=[]
for mode in ['debug','release']:
 d=json.loads((HERE/f'brush-{mode}.json').read_text())
 case=next(c for c in d['cases'] if c['name']=='closed-decagon-99000-alternating-knots-round-100')
 assert all(r['wallMs']<30000 for c in d['cases'] for r in c['runs'])
 brush.append(f"{mode}: decagon {', '.join(format(r['wallMs']/1000,'.3f')+' s' for r in case['runs'])}; corpus maximum {d['worst']['wallMs']/1000:.3f} s")
readiness=(ROOT/'reviews/M2-C2.1-r3.md').read_text(encoding='utf8').split('## C2.2 readiness list\n',1)[1]
readiness=readiness.replace('These are testable acceptance obligations for the C2.2 brief; R3-01/02/03 must first be adjudicated and closed through scoped checks.','The round-3 list is unchanged. R3-01/02/03 are now closed as ruled; the following obligations remain for C2.2.')
text=f'''# M2 C2.1 round 4 — scoped closure check

**Verdict: pass. 0 P0 / 0 P1.** Reviewed builder `4e304cf0a0d616eda3cfd6cac0b5dd0cfc502556` against round-3 report `b81904c`. All three ruled items close as ruled, both CI follow-ups are sound, and the scoped regression sweep found no product regression. No product code or specification was changed. Accepted closures, deferrals and rulings remain in force.

| Item | Closure | Independent evidence |
|---|---|---|
| C21-R3-01 | closed as ruled | `clean` validates and returns every sample; position merging occurs in `collapse`. Original reviewer regressions pass debug/release. Builder first/interior/last stationary tests and new closed stationary fitted/polyline cases pass; the complete command produces a nonzero radius-max dab for stationary 0→1→0. |
| C21-R3-02 | closed as ruled | Closing collapse-group pressures appear at t=1, or immediately before the first position's own pressure when corner detection rotates the geometry. Original regression passes debug/release. New cases cover several varying tail pressures, smoothness 0–3, fitted/polyline, open return, and a near tail after rotation. Distinct retained position groups retain their respective arc positions, as ruled. |
| C21-R3-03 | closed as ruled | Output runs follow each mapped scalar's source run; composites retain the starter's run. Original SHIN sizes [10,30,20], separator deletion, and Bengali r2 tests pass. New cases cover three differently formatted reordered marks, cross-run composition, paragraph boundary blocking and newline deletion that permits reordering. |
| c1adff9 | sound | Four-shape fixed union chunks; overlap charge is vertices × chunk shapes × 16, spent before each overlay call. The shared 1.2-billion-unit account remains in place through final fitting. Debug/release original watchdog and complete closed command regressions pass; full brush corpus observations retained. Golden differences are limited to two outlines' handles (precise correction below). |
| d57e746 | sound | Only the builder-owned unit test changes: 5 ms sleeps while the probe does not report free, with a 2 s elapsed-time bound; original assertion and subsequent takeover/heartbeat assertions remain. Product lock protocol, single arbiter, read-back and fail-closed heartbeat are unchanged. C1.3/C1.4 lock suites pass. |

The [final conformance appendix](evidence/M2-C2.1-r4/conformance.md) has **134 implemented / 0 divergent / 0 missing / 231 deferred with owner**, 365 source rows total. The fourteen former divergent rows now cite closure evidence; all other dispositions and future owners are unchanged. These are source-row dispositions, not defect counts.

## Follow-up details and limits

The old CI failure [41191607130](https://github.com/kaigani/261017-toontown-dev/actions/runs/41191607130) is explained by its retained failed-job log: Windows closed-command watchdog exceeded 30 s; macOS released-flock probe returned Some(true) where Some(false) was asserted. The fixes address those exact failures without weakening the assertions.

Brush corpus wall observations (not quantiles or a universal time proof): {'; '.join(brush)}. The original `many_pressure_knots_brush_respects_watchdog` also passes in both modes, and complete executor acceptance/refusal plus subsequent-worker responsiveness remain covered by `c21_r3_contracts`. The round-3 qualifications about upstream uncharged work, calibration versus a formal time bound, bridge limits and future aggregate symmetry work remain unchanged.

The golden correction is **24 scalar handle-coordinate changes in two outlines**, with identical anchors and every other corpus field unchanged except derived byte count/digest. Maximum coordinate displacement is **{audit['goldenMaxCoordinateDu']:.12f} du**; maximum 2-D handle displacement is **{audit['goldenMaxHandleDu']:.12f} du**. Thus “at most 0.008 du” is numerically inaccurate, but the observed displacement remains below 1/64 du (0.015625 du), and does not establish a geometry-tolerance failure. The new digest `375e65ce0cd2d962eb0c3ff6a5b3cb23f4521666b86b1b9d90c5eece10c84835` reproduces locally and on both CI OSes.

ADR-0004 requires fixed operation and iteration order for the same input; it does not require invariance under changing algorithms or permuting an ordered stroke's samples. Chunk size four and vector iteration order are fixed, with no clock/hash/thread-dependent chunk boundaries. Grid snapping can make hierarchical union depend on grouping, so changing chunk size can change handles. This is compatible with determinism for fixed ordered input. ADR-0016 now states that resolution limit accurately. Two source comments still overstate associativity/grouping independence; this does not change execution or the accepted ADR interpretation.

## Gates and CI

Frozen pnpm install passed. Full `cargo test --workspace --locked --no-fail-fast` passed; raw summary totals **{'/'.join(map(str,totals('workspace')))}** (passed/failed/ignored, including subprocess summaries). The builder receipt independently recounts **844/0/15**, matching its logs; the earlier 826/0/12 was the pre-round-3 receipt. The four added closure tests pass separately in debug and release. Frontend `pnpm test` passes: **333 Vitest tests in 40 files, plus 9 Node tests**. Focused release reviewer geometry/contracts, golden reproduction, lock suites, format, allow inventory, generated declarations and retained reviewer Python suites pass. The vector release unit suite and clippy of the new reviewer tests also pass (`-D warnings`). Commands, exits and wall times are in [gates.json](evidence/M2-C2.1-r4/gates.json) and [extra-gates.json](evidence/M2-C2.1-r4/extra-gates.json); logs and observations are adjacent.

The scoped diff audit protects **{audit['reviewerPaths']} reviewer-owned paths**: c21 r1–r3, c1* reviewer suites, adversarial files, three reports and their evidence. No changes. Declared builder test edits match the response table (property test, three fit unit tests, M1 lock unit test, worst-case corpus and golden). No new product unwrap/expect/panic/unsafe/allow; allow gate remains 98 in 40 files. Cargo.lock and deny.toml unchanged; generated registry declarations remain fresh and IPC/schema checks pass in the workspace. Retained examples are unchanged and execute in the workspace gate. The only new Rust test is `c21_r4_closure.rs`.

Both [41196211322](https://github.com/kaigani/261017-toontown-dev/actions/runs/41196211322) at `c867879` and [41199280966](https://github.com/kaigani/261017-toontown-dev/actions/runs/41199280966) at `4e304cf` independently verify **10/10 successful jobs**: both check OSes, both headless release OSes, Linux display-less SDK, bundles and raster jobs. Downloaded receipts, compressed raw logs and excerpts are retained; golden tests succeed on macOS and Windows. These are external CI results, distinct from local Windows observations.

## C2.2 readiness list

'''+readiness
(ROOT/'reviews/M2-C2.1-r4-closure.md').write_text(text,encoding='utf8')
print('report written; workspace totals',totals('workspace'))
