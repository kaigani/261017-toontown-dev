"""Read-only change/evidence audit; output raw determinations for the review."""
import importlib.util
import json
import math
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M2-C2.2-r2'
# Receipts: a temporary directory on routine runs; durable evidence only with an explicit `--out PATH` (C22-R3-04).
if '--out' in sys.argv:
    _i = sys.argv.index('--out')
    RECEIPTS = pathlib.Path(sys.argv[_i + 1])
    del sys.argv[_i:_i + 2]
else:
    RECEIPTS = pathlib.Path(tempfile.mkdtemp(prefix='c22-r2-audit-'))

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8')

def mask(v):
    if isinstance(v, dict):
        return {k: mask(x) for k, x in v.items() if k not in ['sha256', 'bytes']}
    if isinstance(v, list):
        return [mask(x) for x in v]
    if isinstance(v, str):
        return re.sub(r'zn_[0-9A-Z]{26}', 'ZONE', v)
    return v

def si074(old, new):
    """Orchestrator ruling SI-074 (M2 C2.3): a texture drag mapping's y axis is the x axis turned clockwise, so a
    recorded [a, b, -b, a, e, f] becomes [a, b, b, -a, e, f]. Only that exact transformation is carried over."""
    if isinstance(old, dict) and isinstance(new, dict):
        return {k: si074(v, new.get(k)) for k, v in old.items()}
    if isinstance(old, list) and isinstance(new, list):
        nums = lambda xs: len(xs) == 6 and all(isinstance(x, (int, float)) and not isinstance(x, bool) for x in xs)
        if nums(old) and nums(new):
            a, b, c, d, e, f = old
            if c == -b and d == a and new == [a, b, b, -a, e, f]:
                return list(new)
            return old
        return [si074(o, n) for o, n in zip(old, new)] + old[len(new):]
    return old

def planar_receipt(d):
    """Reject malformed raw facts and recompute every maximum, rather than accepting the stored worst field."""
    def number(x):
        return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x) and x >= 0
    if not isinstance(d, dict) or not number(d.get('budgetUnits')) or not isinstance(d.get('cases'), list) or not d['cases']:
        raise ValueError('invalid root')
    cases = []
    for c in d['cases']:
        if not isinstance(c, dict) or not isinstance(c.get('name'), str) or not isinstance(c.get('runs'), list) or len(c['runs']) != 2:
            raise ValueError('invalid case')
        for r in c['runs']:
            if not isinstance(r, dict) or not number(r.get('wallMs')) or not number(r.get('units')):
                raise ValueError('invalid observation')
            if r.get('outcome') != 'ok' and not str(r.get('outcome')).startswith('limit.exceeded:'):
                raise ValueError('invalid outcome')
            # spend() records the attempted charge before refusing that step. Refused observations may exceed budget.
            if r['outcome'] == 'ok' and r['units'] > d['budgetUnits']:
                raise ValueError('successful operation exceeded budget')
        cases.append({'name': c['name'], 'maxWallMs': max(r['wallMs'] for r in c['runs']),
                      'maxUnits': max(r['units'] for r in c['runs'])})
    worst = max(cases, key=lambda c: c['maxWallMs'])
    if d.get('worst') != {'name': worst['name'], 'wallMs': worst['maxWallMs']}:
        raise ValueError('stored worst differs from observations')
    return {'cases': cases, 'maxWallMs': worst['maxWallMs'], 'underTenSeconds': worst['maxWallMs'] < 10000}

class Audit(unittest.TestCase):
    def test_planar_raw_observations_and_malformed_rejection(self):
        files = ['planar-worst-case-windows.json', 'ci-planar-worst-case-windows.json', 'ci-planar-worst-case-macos.json']
        receipts = {p: planar_receipt(json.loads((OUT/p).read_bytes())) for p in files}
        self.assertTrue(all(r['underTenSeconds'] for r in receipts.values()))
        (RECEIPTS/'planar-analysis.json').write_text(json.dumps(receipts, indent=2)+'\n', encoding='utf-8')
        raw = json.loads((OUT/files[0]).read_bytes())
        for bad in [True, float('inf'), -1, '1']:
            d = json.loads(json.dumps(raw));d['cases'][0]['runs'][0]['wallMs'] = bad
            with self.assertRaises(ValueError):planar_receipt(d)
        for d in [{}, {'budgetUnits': 1, 'cases': []}, {'budgetUnits': 1, 'cases': [None]}]:
            with self.assertRaises(ValueError):planar_receipt(d)
        raw['worst']['wallMs'] += 1
        with self.assertRaises(ValueError):planar_receipt(raw)

    def test_c22_golden_only_zone_ids_and_number_representation(self):
        p='crates/executor/tests/golden/c22-determinism.json'
        old=json.loads(git('show','648e22d:'+p))
        new=json.loads((ROOT/p).read_bytes())
        self.assertEqual(mask(si074(old,new)),mask(new))

    def test_reviewer_tests_unchanged_after_merge(self):
        paths=['tests/adversarial/','crates/*/tests/c0*_r*','crates/*/tests/c1*_r*',
               'crates/*/tests/c21_r*','crates/*/tests/c22_r1_*','desktop/src/test/']
        delta=git('diff','--stat','c902cb9..7ccea58','--',*paths)
        (RECEIPTS/'reviewer-tests-diff.txt').write_text(delta or 'Empty: no changes after review merge.\n')
        self.assertEqual(delta,'')

    def test_no_forbidden_new_product_constructs(self):
        paths=['crates/*/src/**','bins/*/src/**','desktop/src/**']
        diff=git('diff','--unified=0','648e22d..7ccea58','--',*paths)
        # Classify all matches manually against cfg(test); this receipt deliberately does not hide unit-test lines.
        file='';matches=[]
        for line in diff.splitlines():
            if line.startswith('+++ b/'):file=line[6:]
            elif line.startswith('+') and re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!|\bunsafe\b|#\[allow',line):
                matches.append({'file':file,'line':line})
        (RECEIPTS/'added-constructs.json').write_text(json.dumps(matches,indent=2)+'\n')
        self.assertFalse(any('#[allow' in x['line'] or 'unsafe ' in x['line'] for x in matches))

    def test_analyzer_rejects_malformed_and_recomputes(self):
        sp=importlib.util.spec_from_file_location('analyzer',ROOT/'scripts/analyse-measurements.py')
        a=importlib.util.module_from_spec(sp);sp.loader.exec_module(a)
        raw=json.loads((ROOT/'reviews/evidence/M2-C2.2/round-1/strokes-100k-windows.json').read_bytes())
        with tempfile.TemporaryDirectory() as tmp:
            p=pathlib.Path(tmp)/'strokes-100k-windows.json'
            for change in ['quantile','bool','nonfinite','shape','order']:
                d=json.loads(json.dumps(raw));s=next(iter(d['series'].values()))
                if change=='quantile':s['summary']['p50']+=1
                elif change=='bool':s['summary']['n']=True
                elif change=='nonfinite':s['samples'][0]['value']=float('inf')
                elif change=='shape':s['samples']=[None]
                else:s['samples'][1]['tMs']=-1
                p.write_text(json.dumps(d));self.assertFalse(a.check_file(p)['ok'],change)

if __name__=='__main__':
    unittest.main(verbosity=2)
