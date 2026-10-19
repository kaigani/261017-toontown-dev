"""Compare raw suite completions, not only Actions' green job badges."""
import collections,gzip,json,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def clean(s):return re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m','',s)
def counts(text):
    current='unlabelled';out=[];pending=[]
    for line in clean(text).splitlines():
        m=re.search(r'Running\s+(?:unittests\s+|tests[\\/])(\S+) ',line)
        if m:pending.append(m[1])
        m=re.search(r'Doc-tests (\S+)',line)
        if m:pending.append('Doc-tests '+m[1])
        if re.search(r'\brunning \d+ tests?\b',line) and pending:current=pending.pop(0)
        m=re.search(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',line)
        if m:out.append({'suite':current,'status':m[1],'passed':int(m[2]),'failed':int(m[3]),'ignored':int(m[4])})
    return out
result={}
for run in [39215102172,39217799686]:
    raw=gzip.decompress((HERE/f'ci-{run}.log.gz').read_bytes()).decode('utf-8')
    grouped=collections.defaultdict(list)
    for line in raw.splitlines():grouped[line.split('\t',1)[0]].append(line)
    jobs={}
    for name,lines in grouped.items():
        text='\n'.join(lines)
        jobs[name]={'rustSuites':counts(text),'markers':[s for s in [
            'pnpm lint','pnpm typecheck','pnpm test','pnpm format:check','pnpm licenses:check',
            '11/11 reviewer','cargo fmt --all --check','cargo clippy --workspace --all-targets --locked',
            'cargo deny --locked check','build:diagnostics','pnpm --dir desktop build',
            'selftest-2.json','headless dependency check passed','check-diagnostics-gating.py',
            'hashes.json','jobs_run_off_the_document_thread'
        ] if s in text]}
        jobs[name]['workspaceSuites']=counts('\n'.join(l for l in lines if '\tRun cargo test --workspace --locked\t' in l))
    result[str(run)]=jobs
good=result['39217799686']
expected_c11=[p.stem for p in (ROOT/'crates/model/tests').glob('c11_*.rs')]+[p.stem for p in (ROOT/'crates/engine/tests').glob('c11_*.rs')]
expected_c12=['c12_r1_contracts','c12_r1_stress','c12_r1_macro','c12_r1_scale','c12_never_block','c12_edit_ops','compile_fail']
for os in ['windows-latest','macos-latest']:
    job=good[f'check ({os})'];suites=job['rustSuites']
    for suite in expected_c11+expected_c12:
        matches=[r for r in suites if suite+'.rs' in r['suite']]
        assert matches and all(m['status']=='ok' for m in matches), (os,suite,matches)
    job['totalPassed']=sum(s['passed'] for s in suites)
    job['totalIgnored']=sum(s['ignored'] for s in suites)
    print(os,job['totalPassed'],'Rust passes',job['totalIgnored'],'ignored;',len(expected_c11),'C1.1 suites;',len(expected_c12),'selected C1.2 suites')
for key in ['baseline','final','final-complete']:
    path=HERE/key/'gates/test.log'
    if path.exists():result[key]={'rustSuites':counts(path.read_text(encoding='utf-8'))}
(HERE/'ci-audit.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
failed=clean(gzip.decompress((HERE/'ci-39215102172.log.gz').read_bytes()).decode())
lines=failed.splitlines()
idx=next(i for i,l in enumerate(lines) if "thread 'jobs_run_off_the_document_thread'" in l)
(HERE/'ci-ordering-failure.txt').write_text('\n'.join(lines[idx:idx+13])+'\n',encoding='utf-8')
print('bfa828f: notifier send now precedes table unlock/condition notification; original assertion preserved.')
