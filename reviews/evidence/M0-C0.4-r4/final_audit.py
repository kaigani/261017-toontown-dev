"""Audit fresh receipts, unchanged product/plan/spec, and the closure report's evidence.

This is an evidence-integrity audit, not a passing product verdict: the two named closure regressions must fail
on the reviewed head. Their expected value in the original command receipts remains zero.
"""
import csv
import gzip
import hashlib
import json
import pathlib
import re
import subprocess

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
HEAD='b93b2280cadeef164eceea0da4c10b7527a593e4'

def read(name):
    path=OUT/name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(path.suffix+'.gz').read_bytes())

receipts={p.name:json.loads(p.read_bytes()) for p in OUT.glob('*.receipt.json')}
failures={'closure-probes.receipt.json','page-probe.receipt.json'}
for name,r in receipts.items():
    assert r['exit']==(1 if name in failures else 0),(name,r)
required=['fmt','clippy','rust-tests','clippy-diagnostics','rust-tests-diagnostics','deny',
          'lint','typecheck','test','format-check','licenses-check','frontend-build-default',
          'frontend-build-diagnostics','tauri-build-default','tauri-build-diagnostics',
          'binary-gating-default','binary-gating-diagnostics','independent-gating-default',
          'independent-gating-diagnostics','headless-build','headless-selftest-1','headless-selftest-2',
          'headless-guard','raster','test_decision_rules','c04-r2-decisions','c04-r3-decisions',
          'c04-r3-page','closure-probes','page-probe','copy-probe']
assert all(n+'.receipt.json' in receipts for n in required)
checks=[[(c['id'],c['ok']) for c in json.loads(read('headless-selftest-'+str(i)+'.log'))['checks']] for i in (1,2)]
assert checks[0]==checks[1] and all(ok for _,ok in checks[0])
raster=json.loads(read('raster.json'))
assert raster['hashes']==json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_bytes())
assert not raster['checkMismatches'] and not raster['repeatFailures']
for mode in ('default','diagnostics'):
    p=json.loads(read('independent-gating-'+mode+'.json'))
    assert all(bool(v)==(mode=='diagnostics') for k,v in p['binary_counts'].items() if k.startswith(('spike_','bench_')))
assert json.loads(read('ipc-bench.json'))['frontend']['ok']
ci=json.loads(read('ci-audit.json'))
assert ci['head']==HEAD and ci['configuredFrontendFilesPerOS']==34 and len(ci['jobs'])==8
copy_probe=json.loads(read('copy-consistency.json'))
assert [r['exit'] for r in copy_probe['results']]==[0,1,1,0]
for path,sha in copy_probe['originals'].items():
    assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest()==sha
paths=['spec','plan','docs','desktop','crates','bins','scripts','Cargo.lock','pnpm-lock.yaml',
       'reviews/M0-C0.4-go-no-go.md','reviews/M0-C0.4-handoff.md']
assert subprocess.check_output(['git','diff','--name-only',HEAD,'--',*paths],cwd=ROOT)==b''
assert not list((ROOT/'desktop/src/test').glob('c04-r4-*.test.ts*'))
rows=list(csv.reader((ROOT/'plan/traceability.csv').open(encoding='utf-8')))
csv_observations=[{'line':i+1,'columns':len(row),'requirement':row[0]} for i,row in enumerate(rows) if len(row)!=6]
subprocess.run(['git','merge-base','--is-ancestor','b9b966b',HEAD],cwd=ROOT,check=True)
assert subprocess.check_output(['git','show','b9b966b:plan/IMPLEMENTATION_PLAN.md'],cwd=ROOT).replace(b'\r\n',b'\n')==(ROOT/'plan/IMPLEMENTATION_PLAN.md').read_bytes().replace(b'\r\n',b'\n')
rust={}
for name in ('rust-tests','rust-tests-diagnostics'):
    nums=[tuple(map(int,x)) for x in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',read(name+'.log').decode())]
    rust[name]={'passed':sum(x[0] for x in nums),'failed':sum(x[1] for x in nums),'ignored':sum(x[2] for x in nums)}
summary=dict(reviewed=HEAD,receipts=receipts,productPathsUnchanged=True,planSyncVerified=True,
             headlessChecks=checks[0],rasterEntries=sum(len(v) for v in raster['hashes']['renderers'].values()),rust=rust,
             ciJobs=8,frontendSuitesPerOS=34,traceabilityRows=len(rows)-1,
             incidentalCsvObservations=csv_observations,
             closureFailures={'C04-R4-01':'outside-span tail changes episode and platform decision',
                              'C04-R4-02':'malformed key sequences accepted with matching transition count'})
(OUT/'final-audit.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print('Audited',len(receipts),'receipts, all ordinary gates pass; two disclosed closure regressions remain.')
print('Rust:',rust,'; identical headless checks:',checks[0])

# Compress bulky evidence only inside this explicitly named directory; no recursive moves/deletes.
if __name__=='__main__':
    for path in OUT.iterdir():
        if path.is_file() and (path.suffix=='.log' or (path.suffix=='.json' and path.stat().st_size>100000)):
            data=path.read_bytes()
            path.with_suffix(path.suffix+'.gz').write_bytes(gzip.compress(data,mtime=0))
            path.unlink()
