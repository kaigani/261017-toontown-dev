"""Normalize reviewer shell logs to UTF-8, retain downloaded observations and summarize actual results."""
import hashlib
import gzip
import json
import pathlib
import re
import shutil
import subprocess
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[2]
for p in HERE.iterdir():
    if p.suffix not in ('.json','.log','.py'):continue
    data=p.read_bytes()
    if data.startswith((b'\xff\xfe',b'\xfe\xff')):
        p.write_text(data.decode('utf-16'),encoding='utf-8',newline='\n')
    elif data.startswith(b'\xef\xbb\xbf'):
        p.write_bytes(data[3:])
for name,source in [('local','reviews/evidence/M1-C1.4/measurements'),
                    ('windows-ci','target/review-ci-r3/headless-Windows/c14-measurements'),
                    ('macos-ci','target/review-ci-r3/headless-macOS/c14-measurements')]:
    dest=HERE/'observations'/name
    dest.mkdir(parents=True,exist_ok=True)
    for p in (ROOT/source).glob('*.json'):shutil.copy2(p,dest/p.name)
sdk={}
for platform in ('Windows','macOS'):
    p=ROOT/f'target/review-ci-r3/headless-{platform}/sdk-conformance.json'
    if p.exists():
        shutil.copy2(p,HERE/f'sdk-ci-{platform}.json')
        sdk[platform]=json.loads(p.read_bytes())
def totals(name):
    text=(HERE/name).read_text(encoding='utf-8')
    a=re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
    return dict(zip(('passed','failed','ignored'),map(sum,zip(*(map(int,x) for x in a))))) if a else {}
binary=ROOT/'target/review-ci-r3/headless-Windows/toontown-headless.exe'
report={'product':'39ea1b2','ciRun':41056649947,
        'ciWindowsBinarySha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
        'workspace':totals('workspace.log'),'newReplay':totals('replay-final.log'),
        'historyOracles':[json.loads((HERE/f'mcp-history-{s}.json').read_text()) for s in ('r1','r3')],
        'sdkCI':{p:{k:v.get(k) for k in ('passed','failed','client')} for p,v in sdk.items()},
        'notes':['PowerShell shell wrappers can report exit 1 for native stderr, including successful test output; use the complete retained test-result records.',
                 'Initial replay-only and rust-r3 logs precede reviewer fixture refinements; replay-final supersedes their replay section.',
                 'New expected failures are deliberate review regressions, not product fixes.']}
(HERE/'summary.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
for p in HERE.glob('mcp-history-*-commands.json'):
    p.with_suffix('.json.gz').write_bytes(gzip.compress(p.read_bytes(),mtime=0))
    p.unlink()
for script in ('verify.py','readiness.py','register.py'):
    subprocess.run(['python',str(HERE/script)],cwd=ROOT,check=True)
manifest={p.relative_to(HERE).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(HERE.rglob('*')) if p.is_file() and p.name!='sha256.json'}
(HERE/'sha256.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:report[k] for k in ('workspace','newReplay','sdkCI')},indent=2))
