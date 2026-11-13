from pathlib import Path
import subprocess,time,json,os
E=Path('reviews/evidence/M3-C3.4-r4')
while not (E/'anim-final.done').exists():time.sleep(15)
jobs=[('r4-anim-final',['cargo','test','--locked','-p','tt-anim','-j','4','--features','test-hooks','--test','c34_r4_passage','--','--nocapture'])]
for pkg in ['tt-executor','tt-mcp','tt-vector','tt-anim']:jobs.append(('fmt-final-'+pkg,['cargo','fmt','-p',pkg,'--check']))
for name,args in jobs:
 print('START',name,flush=True);(E/(name+'.command.json')).write_text(json.dumps(args))
 with (E/(name+'.log')).open('w',encoding='utf-8') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
 (E/(name+'.exit')).write_text(str(r.returncode));print('DONE',name,r.returncode,flush=True)
(E/'repaired-gates.done').write_text('complete')
