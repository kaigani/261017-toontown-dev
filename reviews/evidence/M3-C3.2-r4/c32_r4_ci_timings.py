"""Rebuild timing receipts from the downloaded logs; run from the repository root."""
from pathlib import Path
import re,json,collections,gzip,subprocess
out=Path('reviews/evidence/M3-C3.2-r4')
def records(path):
 raw=subprocess.check_output(['git','show','fd14a8e:'+path.as_posix()]) if 'M3-C3.2-r3' in path.parts else (path.read_bytes() if path.exists() else gzip.decompress(Path(str(path)+'.gz').read_bytes())); cur={}; counts=collections.Counter(); result=[]
 def finish(job):
  if job in cur:
   group=cur.pop(job)
   if group['harness_summaries']:
    result.append({k:v for k,v in group.items() if k!='harness_summaries'} | group['harness_summaries'][-1] | {'nested_harness_summaries':group['harness_summaries'][:-1]})
 for l in raw.decode('utf-16' if raw[:2]==b'\xff\xfe' else 'utf-8-sig').splitlines():
  p=l.split('\t',2)
  if len(p)!=3:continue
  job,step,msg=p
  if not job.startswith('check (') or step!='Run cargo test --workspace --locked':continue
  msg=re.sub(r'\x1b\[[0-9;]*m|\^\[\[[0-9;]*m','',msg)
  m=re.search(r'Running\s+(.+?) \((.+)\)',msg)
  if m:
   finish(job); counts[job,m[1]]+=1;cur[job]={'job':job,'name':m[1],'occurrence':counts[job,m[1]],'executable':m[2],'harness_summaries':[]}
  if 'Doc-tests ' in msg:finish(job)
  m=re.search(r'test result: (.+); finished in ([0-9.]+)s',msg)
  if m and job in cur:cur[job]['harness_summaries'].append({'seconds':float(m[2]),'result':m[1]})
 for job in list(cur):finish(job)
 return result
current=records(out/'ci-42762495633.log');old=records(Path('reviews/evidence/M3-C3.2-r3/ci-42738948001.log'))
(out/'ci-binary-timings.json').write_text(json.dumps(current,indent=2))
s=json.loads((out/'ci-summary.json').read_text());s['debug_above_60']=[x for x in current if x['seconds']>60];s['prior_debug_above_60']=[x for x in old if x['seconds']>60];s['properties_18_test_binaries']=[dict(run=run,**x) for run,data in [('42738948001',old),('42762495633',current)] for x in data if x['name'].replace('\\','/')=='tests/properties.rs' and '18 passed' in x['result']]
(out/'ci-summary.json').write_text(json.dumps(s,indent=2));(out/'ci-comparison-timings.json').write_text(json.dumps({'42738948001':old,'42762495633':current},indent=2))
print('CI binaries:',len(current),'current >60:',[(x['name'],x['seconds']) for x in s['debug_above_60']])
