"""Exact-SHA CI audit: keep complete log plus concise actual execution/count receipts."""
import gzip
import json
from pathlib import Path
import re

OUT=Path(__file__).resolve().parent
ROOT=OUT.parents[2]
def read(path):
    raw=path.read_bytes()
    return raw.decode('utf-16' if raw.startswith(b'\xff\xfe') else 'utf-8-sig')
meta=json.loads(read(OUT/'ci-jobs.json'))
log=read(ROOT/'target/review-r4-ci-log.txt')
(OUT/'ci-jobs.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
(OUT/'ci-full-log.txt.gz').write_bytes(gzip.compress(log.encode(),mtime=0))
plain=re.sub(r'\x1b\[[0-9;]*m','',log)
assert meta['headSha']=='35197875063b0c46aeb20bedfbcfda7dc6d53a65'
assert meta['conclusion']=='success' and len(meta['jobs'])==5
results=[]
for job in meta['jobs']:
    assert job['conclusion']=='success'
    lines=[s for s in plain.splitlines() if s.startswith(job['name']+'\t')]
    rows=[s for s in lines if re.search(r'test result:|Test Files|Tests |# tests|# pass|# fail|c02-r[123]-|PASS|"ok": true|forbidden|43 crates',s)]
    executed=[s['name'] for s in job['steps'] if s['conclusion']=='success']
    skipped=[s['name'] for s in job['steps'] if s['conclusion']=='skipped']
    rust=sum(int(re.search(r'test result: ok\. (\d+) passed',s)[1]) for s in lines if 'test result: ok.' in s)
    if job['name'].startswith('check '):
        assert rust==(65 if 'windows' in job['name'] else 59)
        assert any('232 passed' in s and '3 expected fail' in s for s in lines)
        assert any('# pass 9' in s for s in lines)
        for n in (1,2,3):
            assert any(f'c02-r{n}-tooling.test.ts' in s for s in lines)
        for needle in ('clippy','cargo test','cargo deny','cargo fmt','Headless binary','Front end'):
            assert any(needle in step for step in executed)
    if job['name'].startswith('bundle '):
        assert any('IPC benchmark' in s for s in executed)
        assert any('Tauri build' in s for s in executed)
        assert any('"ok": true' in s for s in lines)
        assert any('tablet spike' in s for s in (executed if 'windows' in job['name'] else skipped))
    results.append(dict(job=job['name'],conclusion=job['conclusion'],rustTests=rust,executed=executed,skipped=skipped,evidence=rows))
(OUT/'ci-audit.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
for row in results:
    print(row['job'],row['conclusion'],'Rust:',row['rustTests'],'skipped:',row['skipped'])
print('Exact-commit CI: 5/5; both OSes execute gates, reviewer suites and real IPC; macOS tablet step skipped.')
