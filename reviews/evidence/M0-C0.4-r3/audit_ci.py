import csv
import gzip
import hashlib
import json
import pathlib
import re

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]

def read(path):
    b=path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(path.suffix+'.gz').read_bytes())
    return b.decode('utf-16' if b.startswith(b'\xff\xfe') else 'utf-8-sig')

meta=json.loads(read(OUT/'ci-39140679107.json'))
logs=read(OUT/'ci-39140679107.log')
assert meta['headSha']=='47459da976a34963962c5dc7ca479273dac3273a'
assert meta['conclusion']=='success' and len(meta['jobs'])==8
jobs=[]
for job in meta['jobs']:
    assert job['conclusion']=='success'
    lines=[x for x in logs.splitlines() if x.startswith(job['name']+'\t')]
    executed=[s['name'] for s in job['steps'] if s['conclusion']=='success']
    skipped=[s['name'] for s in job['steps'] if s['conclusion']=='skipped']
    summaries=[re.sub(r'\x1b\[[0-9;]*m','',x) for x in lines if any(s in x for s in
        ('Test Files','Tests ','test result:','# tests','# pass','234','diagnostic command','self-test passed'))]
    jobs.append(dict(name=job['name'],success=executed,skipped=skipped,summaries=summaries))
    if job['name'].startswith('check '):
        for required in ('pnpm lint','pnpm typecheck','pnpm test','pnpm format:check','pnpm licenses:check',
                         'cargo fmt','cargo clippy','cargo test --workspace','cargo deny',
                         'build:diagnostics','check-headless-deps.py'):
            assert any(required in x for x in lines),(job['name'],required)
        for suite in ('actions.test.tsx','decisionRules.test.ts','decisionTables.test.ts'):
            assert any(suite in x for x in lines),(job['name'],suite)
    if job['name'].startswith('raster determinism'):
        for required in ('--check crates/render/examples/c03_raster/hashes.json','c03_fixed_configurations','--canvas-bench'):
            assert any(required in x for x in lines),(job['name'],required)
        assert any('for run in 1 2' in x for x in lines)
        assert sum('| stress_15 |' in x for x in lines)==2,job['name']
rows=list(csv.reader((ROOT/'plan/traceability.csv').open(encoding='utf-8')))
assert all(len(row)==6 for row in rows)
audit=dict(head=meta['headSha'],url=meta['url'],jobs=jobs,traceabilityRows=len(rows)-1,
           planSha256=hashlib.sha256((ROOT/'plan/IMPLEMENTATION_PLAN.md').read_bytes()).hexdigest())
(OUT/'ci-audit.json').write_text(json.dumps(audit,indent=2)+'\n',encoding='utf-8')
(OUT/'ci-39140679107.json').write_text(json.dumps(meta,indent=2)+'\n',encoding='utf-8')
(OUT/'ci-39140679107.log.gz').write_bytes(gzip.compress(logs.encode(),mtime=0))
print('Exact-head CI: 8/8 jobs; both platform check suites and three raster jobs verified; traceability',len(rows)-1,'rows')
