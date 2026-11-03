from pathlib import Path
import json,gzip,re
HERE=Path(__file__).resolve().parent
out=[]
for run in [41196211322,41199280966]:
 d=json.loads((HERE/f'ci-{run}.json').read_text(encoding='utf8'))
 assert d['conclusion']=='success' and all(j['conclusion']=='success' for j in d['jobs'])
 log=HERE/f'ci-{run}.log'
 raw=log.read_bytes() if log.exists() else gzip.decompress((HERE/f'ci-{run}.log.gz').read_bytes())
 (HERE/f'ci-{run}.log.gz').write_bytes(gzip.compress(raw,mtime=0))
 lines=raw.decode('utf8').splitlines()
 selected=set()
 for i,line in enumerate(lines):
  if any(s in line for s in ['c21_determinism','SDK conformance','canonical_drawing','golden digest','test result:','closed_step_pressure_command']):
   selected.update(range(max(0,i-1),min(len(lines),i+3)))
 (HERE/f'ci-{run}-excerpts.txt').write_text('\n'.join(lines[i] for i in sorted(selected))+'\n',encoding='utf8')
 out.append(dict(run=run,head=d['headSha'],jobs=[dict(name=j['name'],conclusion=j['conclusion']) for j in d['jobs']]))
 if log.exists(): log.unlink()
(HERE/'ci-audit.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out,indent=2))
