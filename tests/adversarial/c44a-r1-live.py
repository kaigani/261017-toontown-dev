"""Independent C4.4a live review. Raw captures remain in TEMP."""
import os, sys, json, tempfile, subprocess, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(os.environ.get('REVIEW_RAW') or tempfile.mkdtemp(prefix='c44a-r1-'))
EVID=ROOT/'reviews/evidence/M4-C4.4a-r1-review'
APP=str(ROOT/'target/release/toontown.exe')

def regression(step,script,cases):
 OUT.mkdir(parents=True,exist_ok=True); EVID.mkdir(parents=True,exist_ok=True)
 rows=[]
 for case in cases:
  dest=OUT/(script+'-'+case);dest.mkdir(exist_ok=True)
  log=dest/'stdout.txt'
  with log.open('w',encoding='utf-8') as f:
   p=subprocess.run([sys.executable,str(ROOT/'tests/adversarial'/script),APP,case],cwd=ROOT,env={**os.environ,'PYTHONIOENCODING':'utf-8','REVIEW_RAW':str(dest)},stdout=f,stderr=subprocess.STDOUT)
  lines=log.read_text(encoding='utf-8',errors='replace').splitlines()
  row={'case':case,'exit':p.returncode,'failLines':[x[:1800] for x in lines if 'FAIL' in x or 'Error' in x or 'Traceback' in x]}
  rows.append(row);print(json.dumps(row),flush=True)
  (EVID/('step'+str(step)+'.json')).write_text(json.dumps(rows,indent=2)[:190000]+'\n',encoding='utf-8')
 print('STEP',step,'COMPLETE',flush=True)

if __name__=='__main__':
 if sys.argv[1]=='l1a':
  step=int(sys.argv[2])
  script,cases={2:('c43b-r1-live.py',['case3','case4','case5','case6','case7','case8']),3:('c43b-r2-live.py',['a2','a3','b1','b2','b3']),4:('c43b-r3-live.py',['a2','a3','b1','b2'])}[step]
  regression(step,script,cases)
 elif sys.argv[1]=='l1b':
  raise SystemExit('L1b cases are added after the unchanged regression runs.')
