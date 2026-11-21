"""Independent C4.4b round-one live probes. Raw output stays in TEMP."""
import importlib.util, json, os, subprocess, sys, tempfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
EVID = ROOT / 'reviews/evidence/M4-C4.4b-r1-review'
APP = ROOT / 'target/release/toontown.exe'
HEAD = ROOT / 'target/release/toontown-headless.exe'
def load(name, rel):
    s = importlib.util.spec_from_file_location(name, ROOT / rel)
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
def save(name, value):
    EVID.mkdir(parents=True, exist_ok=True)
    txt = json.dumps(value, indent=2, ensure_ascii=True) + '\n'
    assert len(txt.encode()) <= 200000
    (EVID / name).write_text(txt, encoding='utf-8', newline='\n')
def regressions():
    rows=[]
    for script,args in [('c44a-r4-live.py',['p1a','2']),('c44a-r4-live.py',['p1b','7']),('c44a-r4-live.py',['p1b','9']),('c44a-r3-live.py',['n2a','2']),('c44a-r2-live.py',['m1a','3']),('c43b-r1-live.py',[str(APP),'case4',str(HEAD)])]:
        raw=Path(tempfile.mkdtemp(prefix='c44b-regression-'))
        path=ROOT/'tests/adversarial'/script
        if script=='c43b-r1-live.py':
            code="import runpy,sys;sys.argv="+repr([str(path),*args])+";g=runpy.run_path(sys.argv[0],run_name='probe');g['case4'].__globals__['OUT']=__import__('pathlib').Path("+repr(str(raw))+");g['case4']()"
            cmd=[sys.executable,'-c',code]
        else: cmd=[sys.executable,str(path),*args]
        with (raw/'console.txt').open('w',encoding='utf-8') as f:
            result=subprocess.run(cmd,cwd=ROOT,env={**os.environ,'REVIEW_RAW':str(raw)},stdout=f,stderr=subprocess.STDOUT,timeout=530)
        failures=[]
        for p in raw.rglob('transcript.json'):
            failures += [r for r in json.loads(p.read_text(encoding='utf-8')) if not r.get('ok',False)]
        row={'script':script,'args':args,'exit':result.returncode,'failures':failures,'raw':str(raw)}
        rows.append(row); print(json.dumps(row),flush=True)
        changed=subprocess.check_output(['git','diff','--name-only'],cwd=ROOT,text=True).splitlines()
        for p in changed:
            if p.startswith('reviews/evidence/') and not p.startswith('reviews/evidence/M4-C4.4b-r1-review/'):
                subprocess.run(['git','checkout','--',p],cwd=ROOT,check=True)
        save('step2.json',rows)
    return int(any(r['exit'] for r in rows))
if __name__=='__main__':
    case,step=sys.argv[1],int(sys.argv[2]); assert case in ('l1a','l1b')
    if case=='l1a' and step==2: sys.exit(regressions())
    raise ValueError('Step not implemented yet')
