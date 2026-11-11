"""Collect native results; count only final top-level summaries for each test process."""
from pathlib import Path
import json,re,subprocess,hashlib
E=Path(__file__).resolve().parent

def read(p):
 b=p.read_bytes();return b.decode('utf-16' if b.startswith(b'\xff\xfe') else 'utf-8-sig',errors='replace')
results={}
for p in E.glob('*.log'):
 if p.name.startswith('ci-'): continue
 roots={};current=None;failed=[]
 for line in read(p).splitlines():
  if re.match(r'\s*(Running |Doc-tests )',line): current=line.strip()
  m=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out',line)
  if m and current: roots[current]=list(map(int,m.groups()))
  if re.match(r'test .* \.\.\. FAILED',line): failed.append(line)
 x=p.with_suffix('.exit')
 results[p.stem]=dict(exit=read(x).strip() if x.exists() else 'not-recorded',targets=roots,totals=dict(zip(['passed','failed','ignored','filtered'],map(sum,zip(*roots.values())))) if roots else {},failedLines=failed)
(E/'execution-summary.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps({k:{q:v[q] for q in ['exit','totals','failedLines']} for k,v in results.items()},indent=2))
