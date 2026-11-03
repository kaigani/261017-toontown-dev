"""C2.4 R1 scoped conformance register; explicit --out only. No product or historical receipt writes."""
import collections
import json
from pathlib import Path
import sys
import unittest

HEAD='0ded6fa454ecf4a191fceeb3638d116f102c8873'
I='implemented';D='divergent';F='deferred-with-owner'
ROWS=[
('02 8.5','selected brush outline becomes pencil lines; source removed',I,'c24_centre_line; c24_r1_lifecycle font Y/T/O/X'),
('02 8.5','triangulation and medial axis of spineless shapes',I,'ops/centre; c24_centre bars/plus/frame/ring'),
('02 8.5; decision 12','short-branch pruning and junction merging',I,'c24_centre plus and frame; independent thin-neck/T probes'),
('02 8.5','holes and branches; no-spine refusals stated',I,'c24_centre and c24_r1_geometry; refusal conditions in ADR-0016'),
('02 8.5','source colour and texture matrix retained; colour splits by source',I,'draw_strokeops; c24_centre_line; c23_strokeops'),
('02 8.5','default thickness equals independent area / axis length',I,'c24_r1_geometry: 0.25/0.75 du bars, T and thin neck, 0.05 relative ceiling'),
('02 8.5','explicit thickness override',I,'c24_centre_line::an_explicit_thickness_overrides_the_average_width'),
('02 8.5','fill-only centreLineOnFill warning',I,'retained c23_strokeops explicit/default painted-zone tests'),
('02 8.5','one labelled undo; redo and save/open',I,'c24_centre_line vectorized frame/ring/triangle lifecycle'),
('02 11.1','menu/cell entry points',F,'Owner M4 C4.1; registered GUI locations, full drawing UI not M2'),
('02 11.2','PNG/JPEG/BMP/GIF/TIFF/TGA/flattened PSD; CMYK to RGB',I,'c24_image; c24_decode_fuzz; 20,000 independent cases, 212 fixtures'),
('02 11.2; ADR-0018','decoder typed errors, dimensions and allocation limits',I,'c24_r1_decoder: 762 successes, max 2,849,367 bytes in seeded corpus; other variants typed refusals'),
('02 11.2','ordered files -> consecutive drawing cells',I,'c24_vectorize; SDK two-image variant'),
('02 11.2','occupied ask/replace/skip and nonvector refusal',I,'c24_vectorize; c24_contracts; E2 wrong/default camera target declaration'),
('02 11.2','no selectable target -> new drawing element',I,'c24_vectorize explicit empty-scene control'),
('02 11.2; SI-080','type/filter/threshold defaults; thresholdOutOfRange',I,'command-row blackAndWhite/70, all kernels; settings disagreement explicitly recorded'),
('02 11.2','grey -> filter -> threshold -> trace -> smoothness-3 fit',I,'vectorize Work account and fit_bounded; independent trace BFS and c24_trace_fuzz'),
('02 11.2','ink brush outlines in one layer, 1200 du width, centred',I,'c24_vectorize and c24_trace; rendering known pixels'),
('02 11.2 versus 03 11.6','black generated swatch versus current colour; palette lifetime/names',D,'C24-R1-05: ADR-0018 chooses 02, but contradictory 03 requirements have no spec-issue record'),
('02 11.2; 03 16','withTexture bitmap swatch and coordinate mapping',I,'c24_vectorize textured two-colour bar; source content-addressed PNG'),
('02 11.2; SI-081','preview renders deterministically without document/file effects',I,'c24_contracts and c24_vectorize; E2 hashes; PNG schema exception recorded'),
('02 11.2; SI-083','job progress and cancellation keep completed prefix and skipped suffix',I,'SDK variant completed >0 and <64, skipped count and ordered indices verified'),
('02 11.2; ADR-0018','one transaction and undo step per committed image',I,'c24_contracts; c24_vectorize; run_staged common acceptance'),
('02 11.3','new drawing element, matching exposure, source image unchanged',I,'c24_vectorize package with real bitmap -> project.open conversion'),
('02 11.4','vector-file import',F,'Owner M10 C10.2, SI-082; declared command remains notImplemented'),
('02 11.5','capture/scanner vectorization',F,'Owner M10 C10.2 import/acquisition surface'),
('02 11.6','generic bitmap scaling preferences',F,'Owner M10 C10.2, generic import surface'),
('02 13.1.1-2','structured command path and no modal dialogs',I,'registered preview/query, run/job and typed confirmations; C0 guard and E2'),
('02 13.1.3','dry run and exact inverse history',I,'c24_contracts common contract plus retained executor oracles'),
('02 13.1.4','target addressing and IDs',I,'c24_vectorize targets, image elements, empty target, wrong type'),
('02 13.1.5-6','drawing coordinate mapping and Bezier inputs',I,'retained c23 transform/point controls; vectorize emits authoritative brush Beziers'),
('02 13.1.7; 13.3','interactive gesture equivalence for new dialogs/tool UI',F,'Owner M4 C4.1/C4.2; M2 command and geometry surface retained'),
('02 13.1.8','confirmation does not block and changes nothing',I,'c24_vectorize ask path; staged effect/rollback common contracts'),
('02 13.1.9','same inputs deterministic on both OSes',I,'CI symmetry/text goldens plus byte-identical seven E2 PNGs Windows/macOS/local'),
('02 13.1.10; 13.4','query/render full content, zones and side IDs',I,'E2 paint-by-ID, independent map readback; c24_r1_lifecycle record persistence'),
('02 13.1.11','structured errors and revision/ID contracts',I,'c24_contracts stale/locked/bad input; SDK stale expectRev refuses before job'),
('02 13.1.12','jobs return IDs, progress, cancellation, final report',I,'E2 + SDK variants; jobs cancelled state retains result'),
('02 13.1.13','batching and atomic refusal for jobs',I,'c24_contracts refuses vectorize in atomic batch; per-image granularity ruled'),
('02 13.2','remembered vectorize settings',F,'Owner M10 C10.2; SI-080 interim command defaults, import.vectorize.settings.* declared'),
('02 13.5','centreLine/breakApart/vectorize/symmetry schemas and examples',I,'candidate CI check; per-crate tests; generator unchanged at 598 declarations'),
('02 13.6','drawing changes and job state/progress',I,'c24_contracts and E2; event publication through run_staged'),
('02 3.12; readiness 2','independent topology oracle covers every topology operation',D,'C24-R1-01: independent command routes cover only transform, segment cut, disjoint Flatten, vectorize'),
('02 3.12; readiness 2','claimed exact metamorphic and Cutter area assertions',D,'C24-R1-02: new seeds shrink two assertions; errors are within grid-snapping bound'),
('02 3.12; C24-T1','lowest stroke/segment owner, write-through mirroring, unsplit overlap',I,'fill::owner/read/write; new-seed fill property; three-way deletion/split and preexisting records'),
('10 2.2; C24-T1','unknown extension namespaces stay opaque',I,'c24_r1_geometry and c24_r1_lifecycle keep org.review.opaque through edits/history/package'),
('02 3.12; SI-059','brush outlines never split by planar noding',I,'retained geometry/cut controls and c24_trace brush outlines'),
('C22-T2; ruling a','100k Flatten completion at strokeAdd p50 and watchdog',F,'Owner M11 C11.1, accepted re-scope into R1-12; refusal explicitly not completion'),
('C22-T2; ruling a','atomic work-charged refusal, fixpoint, phases and every-run gate retained',I,'c24_measure, c24_flatten_phases, CI headless; new-seed fixpoint 4096 cases'),
('C22-T2; ruling a; engineering','gate analyzer rejects malformed observations',D,'C24-R1-03: negative, boolean and infinity timing accepted with exit 0'),
('rulings a/b; merge condition 1','synchronized owner/decision/spec-issue bookkeeping',D,'C24-R1-04: traceability still M2/proposed; SI-067 lacks confirmed middle coordinate step'),
('02 8 Feather; SI-077','Feather bands/colour/alpha policy',F,'Owner spec owner SI-077 and M2 C2.4 C23-T2; explicit merge-scope decision still required'),
('C21-R1-11','configured mirror/kaleidoscope and same-font controls',I,'c24_symmetry/c24_text_layout both OSes; session-only scope and fresh-session test'),
('ADR-0003 rule 7','OFL font manifest/hash/licence and dev-only hook; built artifact scan',I,'cargo deny, release-feature proof, bundle logs scans both OSes; local headless scan'),
('R1-12; readiness 8','time/allocation/retained/RSS/queue/maintenance baselines',I,'recomputed-series and recomputed-budget-ranges; proposed targets remain M11 C11.1'),
('R1-12','interactive per-edit budgets at 100k',F,'Owner M11 C11.1, proposals: <=0.25s/64MiB; current baselines materially exceed'),
('readiness 9','Break Apart precharge cap; 8192 square render and queued reads',I,'c24_aggregate + raw both OSes; 20000 visible-character work charge, typed limit.exceeded'),
('11 14.1-2','command bindings, schema validation and executable examples',I,'candidate check jobs, C0 guards, frozen pnpm, generator/IPC tests'),
('11 14.3','save/open equality',I,'E2 export/render equality; independent record lifetime test'),
('11 14.4','new drawing gestures',F,'Owner M4 C4.1/C4.2'),
('11 14.5','cross-platform deterministic artefacts',I,'seven E2 PNG bytes identical; symmetry/font goldens asserted in both check logs'),
('11 14.6-7','never-block and official SDK protocol',I,'mcp==2.0.0 local; E2 release, retained timeout/audit/queue suites'),
('11 14.8 E2','complete paint-by-ID chain and base replay',I,'26/26; six SDK variants incl. guarded edited-base replay fail rev.conflict'),
('engineering; handoff 6a','budget timing precision',D,'C24-R1-06: macOS max snapshot 0.225958ms, actual maintenance requests 86-203ms and queue-read sends 103-175ms'),
]

READINESS=[
('1','Vectorize trace + fit',D,'C24-R1-05 unresolved cross-section colour policy; 02 pipeline and SDK controls pass'),
('2','Topology properties',D,'C24-R1-01 incomplete independent breadth; C24-R1-02 numerical assertions fail'),
('3','E2 headless chain',I,'local 26/26, six variants, both CI artifacts byte-identical'),
('4','C22-T2',F,'Owner M11 C11.1 accepted ruling (a); synchronization C24-R1-04'),
('5','C23-T1 medial axis',I,'builder suites and independent thin/T/neck/font/lifecycle controls'),
('6','C23-T2 Feather',F,'spec owner SI-077, M2 C2.4 blocked; no milestone-scope waiver inferred'),
('7','C21-R1-11 symmetry/text',I,'both OS goldens and session-only control'),
('8','R1-12 baselines',I,'proposals not final target compliance; C24-R1-06 corrects precision'),
('9','Aggregate limits',I,'precharge, typed refusal and separate following/queued reads'),
]
COMMANDS=[
('draw.vectorize.preview',I,'c24_vectorize/c24_contracts, E2 preview; SI-081'),
('draw.vectorize.run',D,'functional 02 contract passes; 03 11.6 policy conflict C24-R1-05'),
('draw.centreLine.apply',I,'c24_centre_line, retained spine/fill controls, reviewer geometry/font cases'),
('draw.breakApart.apply',I,'c24_aggregate 20000 bound; c24_text_layout; reviewer letter conversion'),
('tool.symmetry.set',I,'declared ahead of M4 C4.2, session-only; registered MCP ui scope appropriate'),
('import.vectorize.settings.get',F,'M10 C10.2, SI-080'),
('import.vectorize.settings.set',F,'M10 C10.2, SI-080'),
('draw.vectorFile.import',F,'M10 C10.2, SI-082 ruling'),
('draw.feather.apply',F,'M2 C2.4 blocked on SI-077; spec owner, no merge waiver'),
]

def build():
    units=[{'id':f'U{i:03}','clause':c,'requirement':r,'state':s,'evidenceOrOwner':e} for i,(c,r,s,e) in enumerate(ROWS,1)]
    readiness=[{'id':n,'item':r,'state':s,'evidenceOrOwner':e} for n,r,s,e in READINESS]
    commands=[{'id':n,'state':s,'evidenceOrOwner':e} for n,s,e in COMMANDS]
    def counts(rows):
        actual=collections.Counter(r['state'] for r in rows)
        return {'total':len(rows),**{s:actual[s] for s in [I,D,'missing',F]}}
    return {'reviewedHead':HEAD,'scope':'02 8.5/11/13 and 11 14 touched by C2.4, supporting rulings and tickets; not a replacement for earlier registers','counts':{'requirements':counts(units),'readiness':counts(readiness),'commands':counts(commands)},'requirements':units,'readiness':readiness,'commands':commands}

if __name__=='__main__':
    if '--out' in sys.argv:
        path=Path(sys.argv[sys.argv.index('--out')+1]);doc=build()
        path.write_text(json.dumps(doc,indent=2)+'\n',encoding='utf-8')
        lines=['# C2.4 R1 conformance register','',f'Reviewed `{HEAD}`. Counts: `{json.dumps(doc["counts"])}`.','']
        for title,key,cols in [('Requirements','requirements',['id','clause','requirement','state','evidenceOrOwner']),('Readiness','readiness',['id','item','state','evidenceOrOwner']),('Commands','commands',['id','state','evidenceOrOwner'])]:
            lines += ['## '+title,'','| '+' | '.join(cols)+' |','| '+' | '.join('---' for _ in cols)+' |']
            lines += ['| '+' | '.join(str(r[c]).replace('|','/') for c in cols)+' |' for r in doc[key]]
            lines += ['']
        path.with_suffix('.md').write_text('\n'.join(lines),encoding='utf-8');print(json.dumps(doc['counts']))
    else:
        class Register(unittest.TestCase):
            def test_unique_units_and_owner_for_deferrals(self):
                doc=build();self.assertEqual(len({r['id'] for r in doc['requirements']}),len(ROWS))
                for row in doc['requirements']:
                    if row['state']==F:self.assertIn('Owner',row['evidenceOrOwner'])
        unittest.main()
