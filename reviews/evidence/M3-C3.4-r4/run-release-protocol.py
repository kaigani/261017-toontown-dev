from pathlib import Path
import subprocess,time,json,shutil
E=Path('reviews/evidence/M3-C3.4-r4')
while not (E/'final-checks.done').exists():time.sleep(15)
if shutil.disk_usage(Path.cwd()).free<120*1024**3:raise RuntimeError('disk floor')
args=['cargo','test','--release','--locked','-p','tt-mcp','-j','4','--features','tt-persist/test-hooks','--test','c34_r2_protocol','--','--nocapture']
(E/'protocol-release.command.json').write_text(json.dumps(args));print('START protocol-release',flush=True)
with (E/'protocol-release.log').open('w',encoding='utf-8') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
(E/'protocol-release.exit').write_text(str(r.returncode));print('DONE protocol-release',r.returncode,flush=True)
(E/'release-protocol.done').write_text('complete')
