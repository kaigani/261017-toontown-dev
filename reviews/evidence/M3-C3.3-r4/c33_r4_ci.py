from pathlib import Path
import json,re,subprocess,datetime,collections
E=Path('reviews/evidence/M3-C3.3-r4')
def read(p):
 b=p.read_bytes(); return b.decode('utf-16' if b.startswith(b'\xff\xfe') else 'utf-8-sig',errors='replace')
allruns={}
for tag in ['old','new']:
 data=json.loads(read(E/f'ci-{tag}.json')); log=read(E/f'ci-{tag}.log')
 bins=collections.defaultdict(list); current={}; compilation=[]; probes=[]; events=collections.Counter(); nested=[]
 for line in log.splitlines():
  parts=line.split('\t',2)
  if len(parts)<3: continue
  job,step,msg=parts
  if step=='UNKNOWN STEP':
   try:
    when=datetime.datetime.fromisoformat(msg.split(' ',1)[0].replace('Z','+00:00'))
    meta=next(j for j in data['jobs'] if j['name']==job)
    step=next(s['name'] for s in meta['steps'] if datetime.datetime.fromisoformat(s['startedAt'].replace('Z','+00:00'))<=when<=datetime.datetime.fromisoformat(s['completedAt'].replace('Z','+00:00')))
   except (ValueError,StopIteration): pass
  msg=re.sub(r'\x1b\[[0-9;]*m|\^\[\[[0-9;]*m','',msg)
  if not job.startswith('check ('): continue
  if 'waiting_caller_does_not_block_document_reads ... ok' in msg or 'stale_stage_is_reported_and_orphan_backup_restored ... ok' in msg: probes.append(dict(job=job,step=step,line=msg))
  m=re.search(r'Running (.+?) \(',msg)
  if m:
   current[(job,step)]=m[1]; events[(job,step)]+=1
  doc=re.search(r'Doc-tests (.+)',msg)
  if doc:
   current[(job,step)]='Doc-tests '+doc[1]; events[(job,step)]+=1
  m=re.search(r'test result: .*finished in ([0-9.]+)s',msg)
  if m:
   row=dict(step=step,binary=current.get((job,step),'nested/doc'),seconds=float(m[1]),event=events[(job,step)])
   if bins[job] and bins[job][-1]['step']==step and bins[job][-1]['event']==row['event']:
    nested.append(dict(job=job,**bins[job][-1])); bins[job][-1]=row
   else: bins[job].append(row)
  if 'Finished `test`' in msg or 'Finished `dev`' in msg: compilation.append(dict(job=job,step=step,line=msg))
 def seconds(a,b): return (datetime.datetime.fromisoformat(b.replace('Z','+00:00'))-datetime.datetime.fromisoformat(a.replace('Z','+00:00'))).total_seconds()
 jobs=[]
 for j in data['jobs']:
  jobs.append(dict(name=j['name'],conclusion=j['conclusion'],seconds=seconds(j['startedAt'],j['completedAt']),steps=[dict(name=s['name'],seconds=seconds(s['startedAt'],s['completedAt']),conclusion=s['conclusion']) for s in j['steps']]))
 allruns[tag]=dict(head=data['headSha'],jobs=jobs,binaries=bins,compilation=compilation,restoredProbes=probes,nestedSummaries=nested)
(E/'ci-attribution.json').write_text(json.dumps(allruns,indent=2)+'\n')
for tag,r in allruns.items():
 print(tag,r['head'])
 for j in r['jobs']:
  if j['name'].startswith(('check (','c33 render release','review properties release','history oracles release')): print(j['name'],j['seconds'])
 print('restored',r['restoredProbes'])
 print('slow windows',[(v['binary'],v['seconds']) for k,vs in r['binaries'].items() if 'windows' in k for v in vs if v['seconds']>30])
