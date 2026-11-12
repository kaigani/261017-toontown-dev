from pathlib import Path
import subprocess,json,os
E=Path('reviews/evidence/M3-C3.4-r4')
for name,args in [
 ('frontend-install',['cmd','/d','/c','pnpm --dir desktop install --frozen-lockfile']),
 ('frontend-build',['cmd','/d','/c','pnpm --dir desktop build']),
 ('reviewer-python-final',['python','scripts/run-reviewer-python-suites.py'])]:
 print('START',name,flush=True);(E/(name+'.command.json')).write_text(json.dumps(args))
 with (E/(name+'.log')).open('w',encoding='utf-8') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT,env={**os.environ,'CI':'1','PYTHONIOENCODING':'utf-8'})
 (E/(name+'.exit')).write_text(str(r.returncode));print('DONE',name,r.returncode,flush=True)
(E/'frontend.done').write_text('complete')
