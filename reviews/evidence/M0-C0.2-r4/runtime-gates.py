"""Run real release headless/IPC paths; retain output and never interact with OS prompts."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

OUT=Path(__file__).resolve().parent
ROOT=OUT.parents[2]
receipts=json.loads((OUT/'runtime-gates.json').read_text()) if '--ipc-only' in sys.argv else []
def run(name,args,timeout=90):
    start=time.time()
    r=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=timeout,env=dict(os.environ,PYTHONIOENCODING='utf-8'))
    (OUT/f'{name}.txt').write_text(r.stdout+r.stderr,encoding='utf-8')
    receipts.append(dict(name=name,command=[str(x) for x in args],exit=r.returncode,seconds=round(time.time()-start,2)))
    (OUT/'runtime-gates.json').write_text(json.dumps(receipts,indent=2),encoding='utf-8')
    print(name,r.returncode,flush=True)
    assert r.returncode==0,r.stderr
    return r

if '--ipc-only' not in sys.argv:
    headless=ROOT/'target/release/toontown-headless.exe'
    checks=[]
    for n in (1,2):
        r=run(f'headless-selftest-{n}',[headless,'--selftest','--json'])
        data=json.loads(r.stdout)
        (OUT/f'headless-selftest-{n}.json').write_text(json.dumps(data,indent=2),encoding='utf-8')
        checks.append([(c['id'],c['ok']) for c in data['checks']])
    assert checks[0]==checks[1] and all(ok for _,ok in checks[0])
    print('Deterministic headless checks:',checks[0],flush=True)
    run('headless-guard',[sys.executable,'scripts/check-headless-deps.py',headless])
if '--ipc' in sys.argv or '--ipc-only' in sys.argv:
    app=ROOT/'target/release/toontown.exe'
    output=OUT/'ipc-bench.json'
    run('ipc-bench',[app,'--ipc-bench',output,'--ipc-bench-count','300'])
    data=json.loads(output.read_text())
    assert data['frontend']['ok'] is True
    print('Real IPC frontend.ok=true',flush=True)
