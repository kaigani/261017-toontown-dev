import subprocess,tempfile,pathlib,json,re,concurrent.futures,zipfile
root=pathlib.Path(tempfile.gettempdir()); out=pathlib.Path('reviews/evidence/M3-C3.4-r5')
sets={}; proof=[]; summaries={}
def fetch(job):
 p=subprocess.run(['gh','api','--allow-escape-sequences',f"repos/kaigani/261017-toontown-dev/actions/jobs/{job['databaseId']}/logs"],capture_output=True,check=True)
 return job,p.stdout.decode('utf-8',errors='replace')
for run in (43279940120,43169948278):
 jobs=json.loads(subprocess.check_output(['gh','run','view',str(run),'--json','jobs']))['jobs']; summaries[str(run)]=jobs
 names=set()
 with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
  for j,s in pool.map(fetch,jobs):
   s=re.sub(r'\x1b\[[0-9;]*m','',s)
   names.update(re.findall(r'test ([^\r\n]+?) \.\.\. ok',s))
   if run==43279940120 and any(x in j['name'] for x in ['check properties','headless release','bundle','c33 render release']):
    lines=[l for l in s.splitlines() if ('test ' in l and ' ... ok' in l) or 'Desktop host with the M0' in l or 'test result:' in l]
    proof.extend([j['name'],*lines])
 sets[str(run)]=sorted(names)
(out/'ci-summary.json').write_text(json.dumps(summaries,indent=2))
(out/'ci-name-sets.json').write_text(json.dumps(sets,indent=2))
(out/'ci-no-drop.json').write_text(json.dumps({'old':len(sets['43169948278']),'new':len(sets['43279940120']),'missing':sorted(set(sets['43169948278'])-set(sets['43279940120']))},indent=2))
(out/'ci-routing-proof.txt').write_text('\n'.join(proof))
