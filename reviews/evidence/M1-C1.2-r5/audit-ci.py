"""Audit the exact build's successful steps and actual test execution in downloaded GitHub logs."""
import gzip
import json
import re
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
run=json.loads((HERE/'logs/ci-metadata.log').read_text(encoding='utf-8'))
raw=(HERE/'logs/ci-full.log').read_bytes()
(HERE/'ci-full.log.gz').write_bytes(gzip.compress(raw,mtime=0))
text=re.sub(r'\^\[\[[0-9;]*m|\x1b\[[0-9;]*m','',raw.decode('utf-8','replace'))
required={'c12_r1_contracts','c12_r1_stress','c12_r1_macro','c12_r1_scale','c12_r2_contracts','c12_r2_registry',
          'c12_r2_scale','c12_r3_contracts','c12_r3_registry','c12_r3_stress','c12_checkpoint_property','c12_invalidation_property','c12_r4_closure','c12_never_block',
          'c11_r1_adversarial','c11_r2_adversarial','c11_r3_adversarial','c11_r3_random','c11_r3_register',
          'c11_r4_exposure_bounds','c11_r5_write_bounds','c11_r6_staged_inverse','c11_r1_wire','c11_r2_wire',
          'c11_r3_wire','adversarial_c01','frontend_schemas','compile_fail','declared'}
integrity=json.loads((HERE/'static-audit.json').read_text(encoding='utf-8'))
inherited={Path(r['path']).stem for r in integrity['unchangedReviewerTests'] if r['path'].endswith('.rs') and '/tests/' in r['path']}
required |= inherited
r3names=[]
for f in (ROOT/'crates/executor/tests').glob('c12_r3_*.rs'):
    r3names+=re.findall(r'#\[test\]\s*fn\s+(\w+)',f.read_text(encoding='utf-8'))
assert len(r3names)==32
r4names=re.findall(r'#[\[]test[\]]\s*fn\s+(\w+)', (ROOT/'crates/executor/tests/c12_r4_closure.rs').read_text(encoding='utf-8'))
assert len(r4names)==4
r3names += r4names
jobs=[]
for j in run['jobs']:
    body='\n'.join(l for l in text.splitlines() if l.startswith(j['name']+'\t'))
    targets=sorted(set(re.findall(r'[/\\]deps[/\\](\w+)-[a-f0-9]+',body)))
    totals=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored',body)
    steps=[{'name':s['name'],'conclusion':s['conclusion']} for s in j['steps']]
    assert all(s['conclusion'] == 'success' or
               (j['name'] == 'bundle (macos-latest)' and s['conclusion'] == 'skipped' and 'tablet spike' in s['name'])
               for s in steps), (j['name'], steps)
    record={'name':j['name'],'conclusion':j['conclusion'],'logLines':len(body.splitlines()),'targets':targets,
            'rustTotals':{'passed':sum(int(x[1]) for x in totals),'failed':sum(int(x[2]) for x in totals),
                          'ignored':sum(int(x[3]) for x in totals)},'steps':steps}
    if j['name'].startswith('check ('):
        record['missingRequiredTargets']=sorted(required-set(targets))
        record['r3AndR4TestsPassed']=[n for n in r3names if re.search(r'test '+n+r' \.\.\. ok',body)]
        record['frontend333']=bool(re.search(r'Tests\s+333 passed',body))
        record['node9']='# pass 9' in body
        record['python11']='11/11' in body or '11 of 11' in body or 'All 11' in body
        record['pythonSuiteLines']=[l.split('\t')[-1] for l in body.splitlines() if 'adversarial/' in l or 'adversarial\\' in l or '11/11' in l]
        assert not record['missingRequiredTargets'], record['missingRequiredTargets']
        assert len(record['r3AndR4TestsPassed'])==36, (j['name'],record['r3AndR4TestsPassed'])
        assert record['frontend333'] and record['node9'] and record['python11']
        assert all(s['conclusion']=='success' for s in steps)
    record['proofExcerpts']=[l.split('\t')[-1] for l in body.splitlines() if re.search(r'234|selftest|display|headless-deps|committed hashes|no forbidden|IPC.*ok|hash.*match',l,re.I)][-65:]
    jobs.append(record)
assert run['headSha']=='ec4329cb1abb47821ada701175730a3e691d41d7'
assert run['conclusion']=='success' and len(jobs)==8 and all(j['conclusion']=='success' and j['logLines'] for j in jobs)
out={'run':39258912422,'head':run['headSha'],'url':run['url'],'jobs':jobs,
     'inheritedReviewerTargets': sorted(inherited),
     'scope':'Both product OS workspace/frontend/diagnostics/reviewer suites and bundles; Linux headless and raster only.'}
(HERE/'ci-audit.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'head':out['head'],'jobs':[{k:j[k] for k in ['name','conclusion','rustTotals']} for j in jobs]},indent=2))
