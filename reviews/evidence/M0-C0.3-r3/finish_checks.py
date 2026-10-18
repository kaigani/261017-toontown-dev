"""Supplementary release suites, fresh CI canvas logs, and before/after encoder timing."""
import gzip
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
env={**os.environ,'PYTHONIOENCODING':'utf-8'}

def run(name,args):
    start=time.time()
    p=subprocess.run(args,cwd=ROOT,capture_output=True,env=env)
    (OUT/(name+'.log')).write_bytes(p.stdout+p.stderr)
    (OUT/(name+'.receipt.json')).write_text(json.dumps(dict(command=args,exitCode=p.returncode,seconds=time.time()-start),indent=2)+'\n')
    assert p.returncode==0,(name,p.stderr)
    return p.stdout

if __name__=='__main__':
    work=ROOT/'target/review-r3'
    work.mkdir(parents=True,exist_ok=True)
    (work/'previous-renderlist.rs').write_bytes(subprocess.check_output(['git','show','0dfeec1:desktop/src-tauri/src/spikes/canvas/renderlist.rs'],cwd=ROOT))
    run('encoder-compile',['rustc','--edition=2024','-O',str(OUT/'encoder_probe.rs'),'-o',str(work/'encoder-probe.exe')])
    run('release-reviewer-rust',['cargo','test','--release','--locked','-p','tt-render','--test','c03_r1_stress','--test','c03_fixed_configurations','--test','c03_r2_quality'])
    for osname in ['Windows','macOS']:
        p=ROOT/f'target/review-r3/desktop-artifacts/desktop-{osname}/canvas-spike-{osname}.json'
        (OUT/f'canvas-ci-{osname}-39129350790.json.gz').write_bytes(gzip.compress(p.read_bytes(),mtime=0))
        run('canvas-ci-'+osname,['python','scripts/analyse-canvas-log.py',str(p),'--json',str(OUT/f'canvas-ci-{osname}.analysis.json')])
    raw=run('encoder-probe',[str(ROOT/'target/review-r3/encoder-probe.exe')]).decode()
    rows=[list(map(int,s.split(',')[1:])) for s in raw.splitlines() if s.startswith('sample,')]
    a=statistics.median(r[1] for r in rows)/1e6
    b=statistics.median(r[2] for r in rows)/1e6
    result=dict(beforeMs=a,afterMs=b,changePercent=100*(b/a-1),samples=len(rows),warmupPairs=20,alternatingOrder=True,sceneBytes=10085716)
    (OUT/'encoder-bench-summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(result)
