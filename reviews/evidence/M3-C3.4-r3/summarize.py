"""Summarize completed local gates and compare the same 36 E1 artifacts as round 2."""
from pathlib import Path
import json,re,hashlib
root=Path('reviews/evidence/M3-C3.4-r3')
def read(p):
 b=p.read_bytes();return b.decode('utf-16' if b[:2] in (b'\xff\xfe',b'\xfe\xff') else 'utf-8-sig',errors='replace')
summary={}
for p in root.glob('*.exit'): summary[p.stem]={'exit':int(read(p).strip())}
for p in root.glob('*.log'):
 s=read(p); key=p.stem
 summary.setdefault(key,{})['results']=re.findall(r'test result:.*',s)
 summary[key]['failures']=re.findall(r'^test (.+) \.\.\. FAILED',s,re.M)
 summary[key]['measurements']=[l for l in s.splitlines() if any(x in l for x in ['PERF ','STRESS requests=','blocked probe:'])]
 if key=='executor':
  binaries=[];name=''
  for line in s.splitlines():
   m=re.search(r'Running (.+?) \(target',line)
   if m:name=m[1]
   if 'test result:' in line:binaries.append({'binary':name,'result':line})
  summary[key]['review_and_watchdog']=[r for r in binaries if 'c34' in r['binary'] or 'c23_r' in r['binary']]
(root/'gate-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
if (root/'E1-final/report.json').exists():
 old=Path('reviews/evidence/M3-C3.4-r2/E1-final');new=root/'E1-final'
 files=json.loads(Path('reviews/evidence/M3-C3.4-r2/e1-final-comparison.json').read_text())
 rows=[]
 for r in files:
  f=r['file']; b=(new/f).read_bytes(); rows.append({'file':f,'equal':b==(old/f).read_bytes(),'sha256':hashlib.sha256(b).hexdigest()})
 (root/'e1-comparison.json').write_text(json.dumps(rows,indent=2)+'\n')
 print('E1 comparison',len(rows),'equal',sum(r['equal'] for r in rows))
print({k:v.get('exit') for k,v in summary.items() if 'exit' in v})
