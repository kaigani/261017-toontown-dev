"""Independent C2.4 R1 evidence collector. Writes only the explicit receipt directory."""
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BASE = '1df274b'
HEAD = '0ded6fa454ecf4a191fceeb3638d116f102c8873'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)

def load_script(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / (name + '.py'))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

def differences(a, b, path=''):
    if type(a) is not type(b):
        return [(path, a, b)]
    if isinstance(a, dict):
        return sum((differences(a.get(k), b.get(k), path+'/'+k) for k in sorted(a.keys() | b.keys())), [])
    if isinstance(a, list):
        if len(a) != len(b):
            return [(path+'/length', len(a), len(b))]
        return sum((differences(x, y, path+'/'+str(i)) for i,(x,y) in enumerate(zip(a,b))), [])
    return [] if a == b else [(path, a, b)]

def collect(out):
    out.mkdir(parents=True, exist_ok=True)
    changed = git('diff','--name-status',BASE+'..'+HEAD).decode().splitlines()
    (out/'changed-files.txt').write_text('\n'.join(changed)+'\n')
    protected = []
    for f in git('ls-tree','-r','--name-only',BASE).decode().splitlines():
        import re
        if f.startswith(('tests/adversarial/', 'desktop/src/test/')) or re.match(r'crates/[^/]+/tests/c(?:0\d|1\d|21|22|23)_r(?!ound)', f):
            if git('diff',BASE,HEAD,'--',f):
                protected.append(f)
    (out/'protected-diff.json').write_text(json.dumps(protected,indent=2))
    (out/'protected.diff').write_bytes(git('diff',BASE,HEAD,'--',*protected))
    for name in ['c21-determinism','c22-determinism','c24-symmetry']:
        path=f'crates/executor/tests/golden/{name}.json'
        old = git('show',f'{BASE}:{path}') if name != 'c24-symmetry' else git('show',f'80714a2:{path}')
        new = git('show',f'{HEAD}:{path}')
        (out/(name+'-structural.json')).write_text(json.dumps(differences(json.loads(old),json.loads(new)),indent=2))
    run=json.loads((out/'ci-run.json').read_text(encoding='utf-8-sig'))
    from datetime import datetime
    jobs=[{'name':j['name'],'conclusion':j['conclusion'],'minutes':(datetime.fromisoformat(j['completedAt'].replace('Z','+00:00'))-datetime.fromisoformat(j['startedAt'].replace('Z','+00:00'))).total_seconds()/60} for j in run['jobs']]
    (out/'ci-duration-summary.json').write_text(json.dumps(jobs,indent=2))
    analyser=load_script('analyse-measurements')
    reports=[]
    for path in (ROOT/'reviews/evidence/M2-C2.4').rglob('*.json'):
        try:
            doc=json.loads(path.read_text(encoding='utf-8-sig'))
        except (ValueError,UnicodeError):
            continue
        if 'series' in doc and isinstance(doc.get('environment'),dict):
            reports.append({'path':str(path.relative_to(ROOT)),**analyser.check_file(path)})
    (out/'recomputed-series.json').write_text(json.dumps(reports,indent=2))
    gate=load_script('c22-t2-gate')
    gates=[]
    for path in (ROOT/'reviews/evidence/M2-C2.4/c22-t2').rglob('flatten-100k-*.json'):
        doc=json.loads(path.read_text())
        other=path.with_name(path.name.replace('flatten-','strokes-'))
        if not other.exists():
            continue
        add=json.loads(other.read_text())
        gates.append({'path':str(path.relative_to(ROOT)), 'strokeAddP50Seconds':gate.p50(gate.values(add['series']['strokeAdd.wallMs']))/1000,'flattenP50Seconds':gate.p50([x['seconds'] for x in doc['unbudgeted']]),'budgeted':doc['budgeted']})
    (out/'recomputed-gates.json').write_text(json.dumps(gates,indent=2))
    print('protected changes:', protected)
    print('series',len(reports),'rejected',sum(not r['ok'] for r in reports))
    print('jobs',len(jobs),'max minutes',max(j['minutes'] for j in jobs))

class GateValidation(unittest.TestCase):
    """These assertions intentionally remain failing if a malformed observation can pass."""
    def run_bad(self, sample):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp)
            (d/'strokes.json').write_text(json.dumps({'series':{'strokeAdd.wallMs':[1000]*5}}))
            (d/'flatten.json').write_text(json.dumps({'os':'windows','unbudgeted':[{'seconds':sample}]*5,'budgeted':[{'seconds':1,'error':'limit.exceeded'}]*5,'unbudgetedAllCompleted':True}))
            r=subprocess.run([sys.executable,str(ROOT/'scripts/c22-t2-gate.py'),str(d/'strokes.json'),str(d/'flatten.json'),str(d/'out.json')],capture_output=True,text=True)
            print('malformed',repr(sample),'exit',r.returncode,r.stdout)
            self.assertEqual(r.returncode,2,'C22-T2 analyzer must reject malformed timing evidence')
    def test_negative_timing_is_rejected(self): self.run_bad(-1)
    def test_boolean_timing_is_rejected(self): self.run_bad(False)
    def test_infinite_timing_is_rejected(self): self.run_bad(float('inf'))

if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--collect':
        collect(Path(sys.argv[2]))
    else:
        unittest.main()
