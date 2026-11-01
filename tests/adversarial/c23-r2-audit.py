"""Independent C2.3 round-two declared-edit, generated coverage and CI artifact audit.

No historical receipt writes. --out explicitly opts into a new evidence directory.
"""
import argparse
import collections
import datetime
import gzip
import hashlib
import importlib.util
import json
import pathlib
import re
import shutil
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE, HEAD = '382a607', '797beff'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8')

def read(p):
    b = p.read_bytes()
    return b.decode('utf-16' if b.startswith((b'\xff\xfe', b'\xfe\xff')) else 'utf-8-sig')

def old(p):
    return json.loads(git('show', BASE + ':' + p))

def protected():
    files = git('ls-tree', '-r', '--name-only', BASE).splitlines()
    return [p for p in files if p.startswith(('spec/', 'tests/adversarial/', 'reviews/evidence/M2-C2.3-r1/'))
            or re.search(r'crates/[^/]+/tests/c\d+_r\d+_', p)
            or p in ['reviews/M2-C2.3-r1.md', 'scripts/product-allows.json', 'Cargo.lock']]

def golden_delta():
    p = 'crates/executor/tests/goldens/c23_render.json'
    a, b = old(p), json.loads((ROOT / p).read_bytes())
    return {'added': sorted(b.keys() - a.keys()), 'removed': sorted(a.keys() - b.keys()),
            'changed': [k for k in a.keys() & b.keys() if a[k] != b[k]]}

def coverage():
    spec = importlib.util.spec_from_file_location('r1_audit', ROOT/'tests/adversarial/c23-r1-audit.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.inventory()

def declared():
    names = git('diff', '--name-only', BASE+'..'+HEAD).splitlines()
    return {'base': BASE, 'head': HEAD, 'changedPaths': names, 'protectedPaths': protected(),
            'protectedDiff': git('diff', BASE+'..'+HEAD, '--', *protected()), 'goldenDelta': golden_delta(),
            'builderTestDiff': git('diff', BASE+'..'+HEAD, '--', 'crates/executor/tests/c23_*.rs'),
            'normalFeatureDiff': git('diff', BASE+'..'+HEAD, '--', 'crates/executor/Cargo.toml', 'crates/vector/Cargo.toml')}

def ci(out):
    run = json.loads(read(out/'ci-run.json'))
    log = read(ROOT/'target/c23-r2-ci/ci.log')
    results = []
    for job in run['jobs']:
        duration = (datetime.datetime.fromisoformat(job['completedAt'].replace('Z','+00:00'))-
                    datetime.datetime.fromisoformat(job['startedAt'].replace('Z','+00:00'))).total_seconds()
        lines = [l for l in log.splitlines() if l.startswith(job['name']+'\t') and '\tRun cargo test --workspace --locked\t' in l]
        counts = [tuple(map(int,m)) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored','\n'.join(lines))]
        results.append({'name':job['name'],'conclusion':job['conclusion'],'seconds':duration,
                        'above55Minutes':job['name']=='check (windows-latest)' and duration>3300,
                        'workspaceSummaries':len(counts), 'workspaceCounts':dict(zip(['passed','failed','ignored'],[sum(x[i] for x in counts) for i in range(3)]))})
    manifests = {}
    comparisons = []
    expected = json.loads((ROOT/'crates/executor/tests/goldens/c23_render.json').read_bytes())
    for os in ['Windows','macOS']:
        folder = ROOT/'target/c23-r2-ci'/('check-'+os)/'c23-render'
        manifest = json.loads((folder/'hashes.json').read_bytes())
        manifests[os] = {'entries':len(manifest),'equalsCommitted':manifest==expected}
        for name in expected:
            a, b = folder/(name+'.png'), ROOT/'reviews/evidence/M2-C2.3/goldens'/(name+'.png')
            comparisons.append({'os':os,'name':name,'bytes':a.stat().st_size,'sha256':hashlib.sha256(a.read_bytes()).hexdigest(),'equalsCommitted':a.read_bytes()==b.read_bytes()})
    assert all(x['equalsCommitted'] for x in comparisons)
    assert all(x['equalsCommitted'] for x in manifests.values())
    assert run['headSha'].startswith(HEAD)
    return {'run':41320596822,'head':run['headSha'],'successfulJobs':sum(x['conclusion']=='success' for x in results),'jobs':results,'manifests':manifests,'pngComparisons':comparisons}

def collect(out):
    out.mkdir(parents=True,exist_ok=True)
    def write(n,v): (out/n).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    write('declared-edits.json',declared())
    write('command-inventory.json',coverage())
    if (out/'ci-run.json').exists(): write('ci-recomputed.json',ci(out))
    diff=git('diff','--unified=0',BASE+'..'+HEAD,'--','crates/*/src/**','bins/*/src/**','desktop/src/**')
    found=[];file='';number=0
    for line in diff.splitlines():
        if line.startswith('+++ b/'):file=line[6:]
        elif line.startswith('@@'):number=int(re.search(r'\+(\d+)',line)[1])-1
        elif line.startswith('+'):
            number+=1
            if re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!|\bunsafe\b|#!?\[allow',line):
                content=(ROOT/file).read_text(encoding='utf-8').splitlines()
                test=next((i+1 for i,l in enumerate(content) if l=='#[cfg(test)]'),len(content)+1)
                found.append({'file':file,'line':number,'text':line[1:],'cfgTest':number>test})
        elif not line.startswith('-'):number+=1
    write('added-constructs.json',found)
    counts={}
    for p in out.glob('*.log'):
        m=[tuple(map(int,m)) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored',read(p))]
        if m:counts[p.name]={'summaries':len(m),**dict(zip(['passed','failed','ignored'],[sum(x[i] for x in m) for i in range(3)]))}
    write('gate-counts.json',counts)
    # Keep the independently downloaded outputs together with their receipts, not only under target/.
    for os in ['Windows','macOS']:
        source=ROOT/'target/c23-r2-ci'/('check-'+os)/'c23-render'
        if source.exists():shutil.copytree(source,out/'ci-renders'/os,dirs_exist_ok=True)
    ci_log=ROOT/'target/c23-r2-ci/ci.log'
    if ci_log.exists():(out/'ci-all.log.gz').write_bytes(gzip.compress(ci_log.read_bytes(),mtime=0))
    headless=ROOT/'target/c23-r2-ci/headless-Windows'
    if headless.exists():
        write('headless-artifact-hashes.json',[{'path':str(p.relative_to(headless)),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in headless.rglob('*') if p.is_file()])
    module_spec=importlib.util.spec_from_file_location('r1_analyzers',ROOT/'tests/adversarial/c23-r1-audit.py')
    analyzers=importlib.util.module_from_spec(module_spec);module_spec.loader.exec_module(analyzers)
    series_spec=importlib.util.spec_from_file_location('measurement_series',ROOT/'scripts/analyse-measurements.py')
    series=importlib.util.module_from_spec(series_spec);series_spec.loader.exec_module(series)
    measurements=out/'measurements';previous=ROOT/'reviews/evidence/M2-C2.3-r1/measurements'
    results={}
    for name in ['render-100k.json','flatten-100k.json','centre-line-large.json']:
        p=measurements/name
        if p.exists():
            a,b=json.loads((previous/name).read_bytes()),json.loads(p.read_bytes())
            results[name]={**analyzers.c23_measurement(b),'priorSeconds':a['seconds'],'ratio':b['seconds']/a['seconds']}
    name='c21-strokes-100k-windows.json'
    if (measurements/name).exists():
        a,b=series.check_file(previous/name),series.check_file(measurements/name)
        results['c21']={'priorValid':a['ok'],'currentValid':b['ok'],'series':[{'name':k,'prior':v,'current':b['series'][k],'p50Ratio':b['series'][k]['p50']/v['p50']} for k,v in a['series'].items() if k.endswith(('.wallMs','.allocatedBytes'))]}
    name='c22-planar-worst-case-windows.json'
    if (measurements/name).exists():
        a,b=analyzers.planar(json.loads((previous/name).read_bytes())),analyzers.planar(json.loads((measurements/name).read_bytes()))
        by={c['name']:c for c in b['cases']}
        results['c22']={'prior':a,'current':b,'comparisons':[{'name':c['name'],'maxWallRatio':by[c['name']]['maxWallMs']/c['maxWallMs'],'sameWorkAndOutcome':(c['units'],c['outcomes'])==(by[c['name']]['units'],by[c['name']]['outcomes'])} for c in a['cases']]}
    write('performance-recomputed.json',results)

class Audit(unittest.TestCase):
    def test_protected_files_are_unchanged(self):
        self.assertEqual(git('diff',BASE+'..'+HEAD,'--',*protected()),'')
    def test_only_new_inset_golden_was_added(self):
        self.assertEqual(golden_delta(),{'added':['texture-stretch-inset'],'removed':[],'changed':[]})
    def test_derived_contract_inventory_has_no_gaps(self):
        v=coverage();self.assertEqual(v['missingFromCommonContract'],[])
        self.assertTrue(v['generatedDeltaExactlyMatchesNewHandlers'])
        self.assertEqual(len(v['commands']),37)
    def test_symmetry_owner_agrees_with_declaration_and_handoff(self):
        lines=(ROOT/'plan/traceability.csv').read_text(encoding='utf-8').splitlines()
        row=next(x for x in lines if x.startswith('C21-R1-11 '))
        self.assertIn(',M2,C2.4,',row)
        self.assertIn('C21-R1-11',(ROOT/'reviews/M2-C2.3-handoff.md').read_text(encoding='utf-8'))
        declarations=(ROOT/'crates/registry/src/declared.rs').read_text(encoding='utf-8')
        feather=declarations.split('id: "draw.feather.apply"',1)[1].split('handler:',1)[0]
        self.assertIn('Status::NotImplemented { owner: "M2 C2.4" }',feather)

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--out',type=pathlib.Path);args,rest=parser.parse_known_args()
    if args.out:collect(args.out.resolve())
    unittest.main(argv=[__file__,*rest],verbosity=2)
