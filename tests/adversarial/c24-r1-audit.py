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
    scalar=[]
    def ranges(doc,prefix=''):
        if isinstance(doc,dict):
            return {k:v for key,value in doc.items() for k,v in ranges(value,prefix+'/'+key).items()}
        if isinstance(doc,list) and doc and all(isinstance(x,dict) for x in doc):
            keys=set().union(*(x.keys() for x in doc))
            result={}
            for key in sorted(keys):
                values=[x.get(key) for x in doc]
                if all(isinstance(x,(int,float)) and not isinstance(x,bool) for x in values):
                    s=sorted(values)
                    result[prefix+'/*/'+key]={'n':len(s),'min':min(s),'max':max(s),'p50NearestRank':s[int(math.floor((len(s)-1)*.5+.5))]}
            return result
        if isinstance(doc,(int,float)) and not isinstance(doc,bool): return {prefix:doc}
        return {}
    for path in (ROOT/'reviews/evidence/M2-C2.4/budgets').rglob('*.json'):
        if path.name.startswith(('budgets-','aggregate-')):
            scalar.append({'path':str(path.relative_to(ROOT)),'recomputed':ranges(json.loads(path.read_text()))})
    (out/'recomputed-budget-ranges.json').write_text(json.dumps(scalar,indent=2))
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

class ReadinessCoverage(unittest.TestCase):
    def test_every_topology_operation_has_a_seeded_independent_exact_oracle_route(self):
        import re
        required={'draw.stroke.add','draw.stroke.set','draw.object.delete','draw.cut.apply','draw.erase.apply',
                  'draw.flatten.apply','draw.optimize.apply','draw.smooth.apply','draw.linesToBrush.apply',
                  'draw.centreLine.apply','draw.breakApart.apply','draw.transform.apply','draw.envelope.apply',
                  'draw.point.move','draw.vectorize.run'}
        found=set(); routes=[]
        for p in (ROOT/'crates').glob('*/tests/*.rs'):
            if p.name.startswith('c24_r1_'): continue
            s=p.read_text(encoding='utf-8')
            if ('vector/tests/oracle' in s or 'mod oracle;' in s) and 'TestRunner' in s:
                commands=set(re.findall(r'"(draw\.[A-Za-z.]+)"',s))
                found |= commands; routes.append(str(p.relative_to(ROOT)))
        print('seeded independent oracle routes',routes,'missing',sorted(required-found))
        self.assertEqual(sorted(required-found),[], 'C2.4 readiness item 2 and plan section 2 require every topology operation')

    def test_accepted_flatten_deferral_is_synchronized_to_m11(self):
        row=next(x for x in (ROOT/'plan/traceability.csv').read_text().splitlines() if x.startswith('C22-T2 '))
        self.assertIn(',M11,C11.1,',row,'Binding R1 ruling (a) accepts the handoff re-scope; traceability must name the new owner')

if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--collect':
        collect(Path(sys.argv[2]))
    else:
        unittest.main()
