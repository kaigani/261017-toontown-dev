from pathlib import Path
import subprocess,time,json,shutil
E=Path('reviews/evidence/M3-C3.4-r4')
while not (E/'late-probes.done').exists():time.sleep(15)
if shutil.disk_usage(Path.cwd()).free<120*1024**3:raise RuntimeError('disk floor')
args=['cargo','test','--locked','-p','tt-anim','-j','4','--features','test-hooks','--no-fail-fast']
(E/'anim-final.command.json').write_text(json.dumps(args));print('START anim-final',flush=True)
with (E/'anim-final.log').open('w',encoding='utf-8') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
(E/'anim-final.exit').write_text(str(r.returncode));print('DONE anim-final',r.returncode,flush=True)
(E/'anim-final.done').write_text('complete')
