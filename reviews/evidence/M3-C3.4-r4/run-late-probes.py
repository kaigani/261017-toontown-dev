from pathlib import Path
import subprocess,time,json
E=Path('reviews/evidence/M3-C3.4-r4')
while not (E/'release-protocol.done').exists():time.sleep(15)
args=['cargo','test','--locked','-p','tt-executor','-j','4','--test','c34_r4_late_probe','--','--nocapture']
(E/'late-probes.command.json').write_text(json.dumps(args));print('START late-probes',flush=True)
with (E/'late-probes.log').open('w',encoding='utf-8') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
(E/'late-probes.exit').write_text(str(r.returncode));print('DONE late-probes',r.returncode,flush=True)
(E/'late-probes.done').write_text('complete')
