"""Verify exact reviewed SHA and actual job logs, including every configured frontend suite."""
import gzip
import json
import pathlib
import re

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
meta=json.loads((OUT/'ci-39149491879.json').read_bytes())
logs=gzip.decompress((OUT/'ci-39149491879.log.gz').read_bytes()).decode('utf-8')
assert meta['headSha']=='929f91a99ab06402bc024108fbc82f9aae3f91d9'
assert meta['status']=='completed' and meta['conclusion']=='success'
expected={'check (windows-latest)','check (macos-latest)','bundle (windows-latest)','bundle (macos-latest)',
          'raster determinism (windows-latest)','raster determinism (macos-latest)',
          'raster determinism (ubuntu-latest)','headless without a display (ubuntu)'}
assert {j['name'] for j in meta['jobs']}==expected
files={p.relative_to(ROOT/'desktop').as_posix() for p in (ROOT/'desktop/src').rglob('*')
       if re.search(r'\.test\.tsx?$',p.name)}
rust_log = ((OUT/'rust-tests.log').read_bytes() if (OUT/'rust-tests.log').exists()
            else gzip.decompress((OUT/'rust-tests.log.gz').read_bytes()))
rust_files = set(re.findall(r'deps[/\\]([a-zA-Z0-9_]+)-[0-9a-f]{16}', rust_log.decode('utf-8')))
assert len(rust_files) == 24, rust_files
jobs=[]
for job in meta['jobs']:
    assert job['conclusion']=='success'
    lines=[x for x in logs.splitlines() if x.startswith(job['name']+'\t')]
    summaries=[re.sub(r'\x1b\[[0-9;]*m','',x) for x in lines if any(s in x for s in
        ('Test Files','Tests ','test result:','# tests','# pass','diagnostic command','self-test passed'))]
    steps=[s['name'] for s in job['steps'] if s['conclusion']=='success']
    skipped=[s['name'] for s in job['steps'] if s['conclusion']=='skipped']
    row=dict(name=job['name'],executed=steps,skipped=skipped,summaries=summaries)
    if job['name'].startswith('check '):
        for required in ('pnpm lint','pnpm typecheck','pnpm test','pnpm format:check','pnpm licenses:check',
                         'cargo fmt','cargo clippy','cargo test --workspace','cargo deny',
                         'build:diagnostics','check-headless-deps.py','selftest-1.json','selftest-2.json',
                         '--features m0-diagnostics'):
            assert any(required in x for x in lines),(job['name'],required)
        missing=files-{f for f in files if any(f in x for x in lines)}
        assert not missing,(job['name'],missing)
        row['frontendSuites']=sorted(files)
        actual_rust = set(re.findall(r'deps[/\\]([a-zA-Z0-9_]+)-[0-9a-f]{16}', '\n'.join(lines)))
        assert rust_files <= actual_rust, (job['name'], rust_files-actual_rust)
        row['rustTestTargets'] = sorted(rust_files)
        assert all(s['conclusion']=='success' for s in job['steps']), job['name']
    if job['name'].startswith('raster determinism'):
        for required in ('--check crates/render/examples/c03_raster/hashes.json','c03_fixed_configurations','--canvas-bench'):
            assert any(required in x for x in lines),(job['name'],required)
        assert any('for run in 1 2' in x for x in lines)
        assert sum('| stress_15 |' in x for x in lines)==2,job['name']
        row['twoFixtureTablesExecuted']=True
    if job['name'].startswith('bundle '):
        for required in ('--expect default','--expect diagnostics','--ipc-bench-count 300',
                         'is an M0 diagnostic and this build has none','analyse-canvas-log.py'):
            assert any(required in x for x in lines),(job['name'],required)
        assert all(s['conclusion']=='success' for s in job['steps'] if 'Tauri build' in s['name'])
    if job['name'].startswith('headless without'):
        for required in ('selftest-1.json', 'selftest-2.json', 'check-headless-deps.py', 'DISPLAY', 'WAYLAND_DISPLAY'):
            assert any(required in x for x in lines), (job['name'], required)
    jobs.append(row)
assert len(files)==34,len(files)
(OUT/'ci-audit.json').write_text(json.dumps(dict(head=meta['headSha'],url=meta['url'],jobs=jobs,
    configuredFrontendFilesPerOS=len(files),standaloneReviewerScriptsInCI=False),indent=2)+'\n',encoding='utf-8')
print('Exact-head CI: 8 successful jobs; all 34 configured frontend suites on both OSes; three raster jobs execute two hash-checked runs.')
for j in jobs:
    print(j['name'],*[x for x in j['summaries'] if any(s in x for s in ('Test Files','Tests ','# tests'))],sep='\n')
