"""One native auto attempt, without any UI automation or dismissing covering windows."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

OUT=Path(__file__).resolve().parent
ROOT=OUT.parents[2]
path=OUT/'real-tablet-log.json'
command=[str(ROOT/'target/release/toontown.exe'),'--spike','tablet','--spike-log',str(path),'--spike-auto']
start=time.time()
r=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=100)
(OUT/'real-tablet-attempt.txt').write_text(r.stdout+r.stderr,encoding='utf-8')
receipt=dict(command=command,exit=r.returncode,seconds=round(time.time()-start,2),uiAutomation='none; no windows or security prompts dismissed')
assert r.returncode==0,r.stderr
log=json.loads(path.read_text(encoding='utf-8'))
assert log['source']=='auto'
a=subprocess.run([sys.executable,'scripts/analyse-tablet-log.py',str(path),'--json',str(OUT/'real-tablet.analysis.json')],cwd=ROOT,capture_output=True,text=True,encoding='utf-8',env=dict(os.environ,PYTHONIOENCODING='utf-8'))
(OUT/'real-tablet-analysis.txt').write_text(a.stdout+a.stderr,encoding='utf-8')
receipt.update(source=log['source'],analyzerExit=a.returncode,outcome=[s for s in a.stdout.splitlines() if 'outcome:' in s],errors=[r.get('error') or (r.get('probe') or {}).get('error') for r in log['runs']])
(OUT/'real-tablet-receipt.json').write_text(json.dumps(receipt,indent=2),encoding='utf-8')
print(json.dumps(receipt,indent=2))
assert a.returncode==0,a.stderr
