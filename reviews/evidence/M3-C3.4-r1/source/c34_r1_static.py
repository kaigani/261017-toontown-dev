import json,datetime,re,subprocess,hashlib
from pathlib import Path
r=Path.cwd();e=r/'reviews/evidence/M3-C3.4-r1';d=json.loads((e/'ci-final.json').read_text(encoding='utf-16'));out=[]
for j in d['jobs']:
 start=datetime.datetime.fromisoformat(j['startedAt'].replace('Z','+00:00'));end=datetime.datetime.fromisoformat(j['completedAt'].replace('Z','+00:00'))
 out.append({'name':j['name'],'conclusion':j['conclusion'],'seconds':(end-start).total_seconds(),'steps':[{'name':s['name'],'conclusion':s['conclusion'],'startedAt':s['startedAt'],'completedAt':s['completedAt']} for s in j['steps']]})
assert len(out)==28 and all(x['conclusion']=='success' for x in out)
(e/'ci-summary.json').write_text(json.dumps(out,indent=2))
print([(x['name'],x['seconds']) for x in out])
args=['git','diff','--stat','2daaf99..45b41da','--','crates/*/tests/c*_r[0-9]_*','tests/adversarial','desktop/src/test','reviews/evidence/M3-C3.[123]-r*','reviews/evidence/M3-C3.2-r*']
s=subprocess.check_output(args,text=True);assert not s;(e/'ownership.txt').write_text('Fixed-head gate empty: '+repr(args)+'\n')
paths=subprocess.check_output(['git','ls-tree','-r','--name-only','2daaf99','reviews/evidence'],text=True).splitlines();pins=[]
for p in paths:
 if re.search(r'M3-C3\.[123]-r',p) and Path(p).name.startswith('register'):
  a=subprocess.check_output(['git','rev-parse','2daaf99:'+p],text=True).strip();b=subprocess.check_output(['git','rev-parse','45b41da:'+p],text=True).strip();assert a==b;pins.append({'path':p,'baseline':'2daaf99','candidate':'45b41da','blob':a})
(e/'historical-registers.json').write_text(json.dumps(pins,indent=2));print('register pins',len(pins))
patch=subprocess.check_output(['git','diff','2daaf99..45b41da','--','crates/*/src','bins/*/src'],text=True)
adds=[x for x in patch.splitlines() if x.startswith('+') and not x.startswith('+++')]
forbidden=[x for x in adds if re.search(r'\b(unwrap|expect)\s*\(|\b(panic|unsafe)\b|#\[allow',x)]
(e/'product-diff-audit.json').write_text(json.dumps({'addedLines':len(adds),'forbidden':forbidden},indent=2));print('forbidden',forbidden)

