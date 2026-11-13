from pathlib import Path
import subprocess,time,json,os,shutil
E=Path('reviews/evidence/M3-C3.4-r4')
while not (E/'gates.done').exists():time.sleep(15)
for name,args in [
 ('atomic-shape',['cargo','test','--locked','-p','tt-executor','-j','4','--test','c34_r4_cancellation_shape','--','--nocapture']),
 ('clippy-workspace-product',['cargo','clippy','--workspace','--lib','--bins','--examples','--locked','-j','4','--','-D','warnings']),
 ('clippy-workspace-targets',['cargo','clippy','--workspace','--all-targets','--locked','-j','4','--','-D','warnings','-A','clippy::empty_line_after_doc_comments'])]:
 if shutil.disk_usage(Path.cwd()).free<120*1024**3:raise RuntimeError('disk floor')
 print('START',name,flush=True);(E/(name+'.command.json')).write_text(json.dumps(args))
 with (E/(name+'.log')).open('w',encoding='utf-8') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
 (E/(name+'.exit')).write_text(str(r.returncode));print('DONE',name,r.returncode,flush=True)
(E/'final-checks.done').write_text('complete')
