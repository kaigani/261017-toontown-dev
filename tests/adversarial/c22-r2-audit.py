"""Read-only change/evidence audit; output raw determinations for the review."""
import importlib.util
import json
import pathlib
import re
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M2-C2.2-r2'

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

class Audit(unittest.TestCase):
    def test_c22_golden_only_zone_ids_and_number_representation(self):
        p='crates/executor/tests/golden/c22-determinism.json'
        old=json.loads(git('show','648e22d:'+p))
        new=json.loads((ROOT/p).read_bytes())
        self.assertEqual(mask(old),mask(new))

    def test_reviewer_tests_unchanged_after_merge(self):
        paths=['tests/adversarial/','crates/*/tests/c0*_r*','crates/*/tests/c1*_r*',
               'crates/*/tests/c21_r*','crates/*/tests/c22_r1_*','desktop/src/test/']
        delta=git('diff','--stat','c902cb9..7ccea58','--',*paths)
        (OUT/'reviewer-tests-diff.txt').write_text(delta or 'Empty: no changes after review merge.\n')
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
        (OUT/'added-constructs.json').write_text(json.dumps(matches,indent=2)+'\n')
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
