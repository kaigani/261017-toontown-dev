"""Real release executable probes, no input injection or security-dialog interaction."""
import gzip
import json
import os
from pathlib import Path
import subprocess
import time

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
exe=ROOT/'target/release/toontown.exe'
for name,args,timeout in [
    ('ipc-bench',['--ipc-bench',str(OUT/'ipc-bench.json'),'--ipc-bench-count','300'],180),
    ('canvas-auto',['--spike','canvas','--spike-log',str(OUT/'canvas-auto.json'),'--spike-auto'],240),
]:
    start=time.time()
    with (OUT/(name+'.log')).open('wb') as f:
        try:
            p=subprocess.run([str(exe),*args],cwd=ROOT,stdout=f,stderr=subprocess.STDOUT,timeout=timeout)
            code=p.returncode
        except subprocess.TimeoutExpired:
            code='timeout'
    (OUT/(name+'.receipt.json')).write_text(json.dumps(dict(command=[str(exe),*args],exitCode=code,seconds=time.time()-start),indent=2)+'\n')
    print(name,code,flush=True)
    if name=='canvas-auto' and (OUT/'canvas-auto.json').exists():
        with (OUT/'canvas-auto.analysis.md').open('wb') as f:
            p=subprocess.run(['python','scripts/analyse-canvas-log.py',str(OUT/'canvas-auto.json'),'--json',str(OUT/'canvas-auto.analysis.json')],cwd=ROOT,stdout=f,stderr=subprocess.STDOUT,env={**os.environ,'PYTHONIOENCODING':'utf-8'})
        (OUT/'canvas-auto.json.gz').write_bytes(gzip.compress((OUT/'canvas-auto.json').read_bytes(),mtime=0))
        print('analyzer',p.returncode,flush=True)
