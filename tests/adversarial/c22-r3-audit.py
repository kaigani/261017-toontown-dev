"""Final-round audits. Tests write only temporary files; --collect writes this round's receipts."""
import copy
import importlib.util
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest
from collections import Counter
from datetime import datetime

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M2-C2.2-r3'

def read(path):
    data = path.read_bytes()
    return data.decode('utf-16' if data.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8')

def structural():
    p = 'crates/executor/tests/golden/c21-determinism.json'
    a = json.loads(git('show', '7ccea58:' + p))
    b = json.loads((ROOT / p).read_bytes())
    old = a['drawing1']['nodes'][7]['children'][0]['strokes']
    new = b['drawing1']['nodes'][7]['children'][0]['strokes']
    receipt = {'oldPieces': len(old), 'newPieces': len(new), 'masked': [
        '/drawing1/nodes/7/children/0/strokes', '/sha256', '/bytes']}
    for v in [a, b]:
        v['drawing1']['nodes'][7]['children'][0]['strokes'] = 'MASK'
        for k in ['sha256', 'bytes']:
            v[k] = 'MASK'
    receipt['everythingElseEqual'] = a == b
    return receipt

def reviewer_delta():
    return git('diff', '--stat', 'c2aca83..02feb53', '--', 'tests/adversarial',
               'crates/*/tests/c0*_r*', 'crates/*/tests/c1*_r*', 'crates/*/tests/c21_r*',
               'crates/*/tests/c22_r1_*', 'crates/*/tests/c22_r2_*', 'desktop/src/test')

def analyzer():
    spec = importlib.util.spec_from_file_location('measurement_analysis', ROOT / 'scripts/analyse-measurements.py')
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

def planar(raw):
    import math
    def number(v): return isinstance(v,(int,float)) and not isinstance(v,bool) and math.isfinite(v) and v>=0
    if not isinstance(raw,dict) or not number(raw.get('budgetUnits')) or not isinstance(raw.get('cases'),list) or not raw['cases']:
        raise ValueError('invalid planar root')
    cases=[]
    for case in raw['cases']:
        if not isinstance(case,dict) or not isinstance(case.get('name'),str) or not isinstance(case.get('runs'),list) or len(case['runs'])!=2:
            raise ValueError('invalid planar case')
        for r in case['runs']:
            if not isinstance(r,dict) or not number(r.get('wallMs')) or not number(r.get('units')): raise ValueError('invalid planar observation')
            if r.get('outcome')!='ok' and not str(r.get('outcome')).startswith('limit.exceeded:'): raise ValueError('invalid outcome')
            if r['outcome']=='ok' and r['units']>raw['budgetUnits']: raise ValueError('accepted over budget')
        cases.append({'name':case['name'],'maxWallMs':max(r['wallMs'] for r in case['runs']),'maxUnits':max(r['units'] for r in case['runs'])})
    worst=max(cases,key=lambda c:c['maxWallMs'])
    if raw.get('worst')!={'name':worst['name'],'wallMs':worst['maxWallMs']}: raise ValueError('bad stored worst')
    return {'cases':cases,'maxWallMs':worst['maxWallMs'],'underTenSeconds':worst['maxWallMs']<10000}

class Audit(unittest.TestCase):
    def test_planar_analyzer_recomputes_and_rejects_malformed(self):
        raw=json.loads((ROOT/'reviews/evidence/M2-C2.2/round-2/planar-worst-case-windows.json').read_bytes())
        self.assertTrue(planar(raw)['underTenSeconds'])
        for v in [None,True,'1',float('inf'),-1]:
            d=copy.deepcopy(raw);d['cases'][0]['runs'][0]['wallMs']=v
            with self.assertRaises(ValueError):planar(d)
        d=copy.deepcopy(raw);d['worst']['wallMs']+=1
        with self.assertRaises(ValueError):planar(d)
    def test_c21_masks_only_explained_piece_list_and_digest(self):
        r = structural()
        self.assertEqual((r['oldPieces'], r['newPieces']), (52, 46))
        self.assertTrue(r['everythingElseEqual'])

    def test_predecessor_tests_unchanged_at_response_head(self):
        self.assertEqual(reviewer_delta(), '')

    def test_measurement_analyzer_rejects_bad_shape_types_and_quantiles(self):
        raw = json.loads((ROOT / 'reviews/evidence/M2-C2.2/round-2/strokes-100k-windows.json').read_bytes())
        a = analyzer()
        with tempfile.TemporaryDirectory() as tmp:
            p = pathlib.Path(tmp) / 'strokes-100k-windows.json'
            p.write_text(json.dumps(raw), encoding='utf-8')
            self.assertTrue(a.check_file(p)['ok'])
            for change in ['quantile', 'bool', 'nonfinite', 'shape', 'order']:
                d = copy.deepcopy(raw)
                s = next(iter(d['series'].values()))
                if change == 'quantile': s['summary']['p50'] += 1
                elif change == 'bool': s['summary']['n'] = True
                elif change == 'nonfinite': s['samples'][0]['value'] = float('inf')
                elif change == 'shape': s['samples'] = [None]
                else: s['samples'][1]['tMs'] = -1
                p.write_text(json.dumps(d), encoding='utf-8')
                self.assertFalse(a.check_file(p)['ok'], change)

def collect():
    OUT.mkdir(parents=True, exist_ok=True)
    def write(name, value):
        (OUT / name).write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    write('c21-structural.json', structural())
    (OUT / 'reviewer-tests-diff.txt').write_text(reviewer_delta() or 'Empty: no reviewer-owned tests changed.\n', encoding='utf-8')
    (OUT / 'c21-golden.diff').write_text(git('diff', '7ccea58..02feb53', '--', 'crates/executor/tests/golden/c21-determinism.json'), encoding='utf-8')
    write('new-packages.json', git('diff', '7ccea58..02feb53', '--', 'Cargo.lock'))
    # Product constructs are classified by their line number relative to the module's cfg(test).
    diff = git('diff', '--unified=0', '7ccea58..02feb53', '--', 'crates/*/src/**', 'bins/*/src/**', 'desktop/src/**')
    found = []
    file = ''
    line_no = 0
    for line in diff.splitlines():
        if line.startswith('+++ b/'):
            file = line[6:]
        elif line.startswith('@@'):
            line_no = int(re.search(r'\+(\d+)', line)[1]) - 1
        elif line.startswith('+'):
            line_no += 1
            if re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!|\bunsafe\b|#\[allow', line):
                content = (ROOT / file).read_text(encoding='utf-8').splitlines()
                test_start = next((i+1 for i,l in enumerate(content) if l == '#[cfg(test)]'), len(content)+1)
                found.append({'file': file, 'line': line_no, 'text': line[1:], 'cfgTest': line_no > test_start})
        elif not line.startswith('-'):
            line_no += 1
    write('added-constructs.json', found)
    # Preserve every predecessor source unit, wording, owner and source reference; record explicit deltas.
    rows = []
    for line in (ROOT / 'reviews/evidence/M2-C2.2-r2/conformance.md').read_text(encoding='utf-8').splitlines():
        if not re.match(r'^\| U\d{3} \|', line): continue
        fields = [v.strip() for v in line.strip('|').split('|')]
        if len(fields) != 7: raise ValueError(f'malformed predecessor row: {line}')
        unit, req, text, r1, prior, note, ref = fields
        current = prior
        evidence = note
        if prior == 'divergent':
            current = 'implemented'
            evidence = 'R2 proving tests and R3 variants reproduced; see report closure table.'
        if unit == 'U095':
            current = 'divergent'
            evidence = 'C22-R3-02: native size uses sorted value-key order, not palette first style; C2.3 owns rendering axis verification.'
        if unit == 'U008':
            current = 'divergent'
            evidence = 'C22-R3-03: T-junction pass splits a containing collinear stroke; no-junction ruling not applied there.'
        if unit in ['U007','U071','U193','U329','U331','U345']:
            current = 'divergent'
            evidence = 'C22-R3-01: recover_with preserves full data, but import revision reset corrupts opaque nested type/rev values before recovery.'
        rows.append({'unit':unit,'requirement':req,'text':text,'round1':r1,'round2':prior,'round3':current,'evidenceOrOwner':evidence,'priorAudit':ref})
    if len(rows) != 345: raise ValueError(f'{len(rows)} units')
    counts=Counter(r['round3'] for r in rows)
    write('register.json', {'target':'02feb53','counts':{k:counts[k] for k in ['implemented','divergent','missing','deferred-with-owner']},
        'previous':{'implemented':176,'divergent':11,'missing':0,'deferred-with-owner':158},
        'deltas':[{'unit':r['unit'],'from':r['round2'],'to':r['round3']} for r in rows if r['round2']!=r['round3']], 'units':rows})
    header='# M2 C2.2 round 3: 345-unit conformance register\n\nTarget `02feb53`. Unchanged units inherit the predecessor source audit; this is not 345 independently executed tests.\n\n'
    header+='| Unit | Requirement | Text / field | Round 2 | Round 3 | Evidence / owner | Prior audit |\n|---|---|---|---|---|---|---|\n'
    (OUT/'conformance.md').write_text(header+'\n'.join('| '+' | '.join(r[k] for k in ['unit','requirement','text','round2','round3','evidenceOrOwner','priorAudit'])+' |' for r in rows)+'\n',encoding='utf-8')
    log = read(OUT / 'ci.log')
    jobs = json.loads(read(OUT / 'ci-jobs.json'))
    ci = {'headSha': jobs['headSha'], 'conclusion': jobs['conclusion'], 'jobs': []}
    for j in jobs['jobs']:
        secs = (datetime.fromisoformat(j['completedAt']) - datetime.fromisoformat(j['startedAt'])).total_seconds()
        # The workflow has multiple cargo invocations: retain every result, identify the workspace step separately.
        matches = []
        for l in log.splitlines():
            if l.startswith(j['name'] + '\t') and '\tRun cargo test --workspace --locked\t' in l:
                m = re.search(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored', l)
                if m: matches.append(tuple(map(int,m.groups())))
        ci['jobs'].append({'name':j['name'],'conclusion':j['conclusion'],'wallSeconds':secs,
            'workspaceResultSummaries':len(matches),'passed':sum(m[0] for m in matches),
            'failed':sum(m[1] for m in matches),'ignored':sum(m[2] for m in matches)})
    write('ci-recomputed.json',ci)
    files = list((OUT / 'ci-artifacts').glob('headless-*/c21-measurements/*.json'))
    files += list(OUT.glob('strokes-100k-*.json'))
    write('measurement-recomputed.json',[analyzer().check_file(p) | {'source':str(p.relative_to(ROOT))} for p in files])
    planar_files=list(OUT.glob('planar-worst-case-*.json'))+list((OUT/'ci-artifacts').glob('headless-*/c22-planar-worst-case/*.json'))
    planar_files+=list((ROOT/'reviews/evidence/M2-C2.2-r2').glob('planar-worst-case-windows.json'))
    write('planar-recomputed.json',{str(p.relative_to(ROOT)):planar(json.loads(p.read_bytes())) for p in planar_files})
    print('receipts:',len(rows),'units;',len(files),'measurement files')

if __name__ == '__main__':
    if sys.argv[1:] == ['--collect']: collect()
    else: unittest.main(verbosity=2)
