from pathlib import Path
import json
HERE=Path(__file__).resolve().parent
out=[]
for p in sorted((HERE.parent/'M2-C2.1').glob('ci-41196211322-*worst-case-*.json'))+sorted(HERE.glob('brush-*.json')):
 d=json.loads(p.read_text(encoding='utf8'))
 runs=[(c['name'],r) for c in d['cases'] for r in c['runs']]
 name,r=max(runs,key=lambda x:x[1]['wallMs'])
 assert abs(r['wallMs']-d['worst']['wallMs'])<1e-6
 if p.name.startswith('brush') or 'brush-worst' in p.name:
  assert all(r['units']<=d['budgetUnits'] for _,r in runs if r['outcome']=='ok')
 assert all(r['wallMs']<30000 for _,r in runs)
 out.append(dict(file=p.name,observations=len(runs),worstCase=name,worstMs=r['wallMs']))
(HERE/'measurement-audit.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out,indent=2))
