import json,subprocess,hashlib,collections
from pathlib import Path
root=Path.cwd(); out=root/'reviews/evidence/M3-C3.3-r2'
def git(*a):return subprocess.check_output(['git',*a])
def digest(b):return hashlib.sha256(b).hexdigest()
r={}
r['ownership']=git('diff','--stat','825b1e1..669ddc3','--','crates/*/tests/c*_r[0-9]_*','tests/adversarial','desktop/src/test','reviews/evidence/M3-C3.3-r1','reviews/evidence/M3-C3.2-r*').decode()
r['goldenDiff']=git('diff','--stat','e61c265..669ddc3','--','crates/executor/tests/golden','crates/render/tests/golden').decode()
r['declaredDiff']=git('diff','--stat','825b1e1..669ddc3').decode()
r['newForbidden']=[s for s in git('diff','825b1e1..669ddc3','--','crates/*/src').decode().splitlines() if s.startswith('+') and not s.startswith('+++') and any(x in s for x in ['.unwrap(','.expect(','panic!','unsafe ','#[allow'])]
comparisons=[]
for family in ['c33','e2','e4']:
 a=out/'artifacts'/f'{family}-Windows'; b=out/'artifacts'/f'{family}-macOS'
 for p in sorted(a.rglob('*')):
  if p.is_file() and (p.suffix=='.png' or p.name in ['evaluate.json','manifest.json','gate-path.json','hashes.json']):
   q=b/p.relative_to(a); data=p.read_bytes(); other=q.read_bytes() if q.exists() else b''
   comparisons.append(dict(family=family,path=p.relative_to(a).as_posix(),sha256=digest(data),otherSha256=digest(other),byteEqual=data==other,parsedEqual=json.loads(data)==json.loads(other) if p.suffix=='.json' and other else None))
r['comparisons']=comparisons
ci=json.loads((out/'ci.json').read_text(encoding='utf-8-sig')); r['ciHead']=ci['headSha']; r['jobs']=[dict(name=j['name'],conclusion=j['conclusion'],start=j['startedAt'],end=j['completedAt']) for j in ci['jobs']]
(out/'audit.json').write_text(json.dumps(r,indent=2)+'\n')
print('comparisons',len(comparisons),'byte differences',sum(not x['byteEqual'] for x in comparisons));print('ownership',r['ownership'],'goldens',r['goldenDiff'],'forbidden',r['newForbidden'])
