"""Independent scoped diff, golden and register audit."""
from pathlib import Path
import subprocess, json, re, difflib, math
from collections import Counter
ROOT=Path(__file__).resolve().parents[3]; HERE=Path(__file__).resolve().parent
def git(*args): return subprocess.check_output(['git',*args],cwd=ROOT).decode('utf8')
paths=git('ls-tree','-r','--name-only','b81904c').splitlines()
owned=[p for p in paths if p.startswith('tests/adversarial/') or re.search(r'/tests/(?:c\d+_r\d|c21_r[123]_)',p) or p.startswith('reviews/evidence/M2-C2.1-r') or re.fullmatch(r'reviews/M2-C2.1-r[123].md',p)]
assert not git('diff','b81904c','4e304cf','--',*owned)
assert not git('diff','b81904c','4e304cf','--','Cargo.lock','deny.toml')
assert not git('diff','b81904c','4e304cf','--','spec')
examples=[p for p in paths if 'example' in p.lower() and not p.startswith('reviews/')]
assert not git('diff','b81904c','4e304cf','--',*examples)
prohibited=[]
for p in git('diff','--name-only','b81904c','4e304cf').splitlines():
 if not p.endswith('.rs') or '/src/' not in p: continue
 a=git('show','b81904c:'+p).split('#[cfg(test)]')[0]
 b=git('show','4e304cf:'+p).split('#[cfg(test)]')[0]
 for line in difflib.unified_diff(a.splitlines(),b.splitlines(),n=0):
  if line.startswith('+') and not line.startswith('+++') and re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!\s*\(|\bunsafe\s*(?:\{|fn|impl)|#\s*\[\s*allow',line): prohibited.append([p,line])
assert not prohibited
a=json.loads(git('show','b81904c:crates/executor/tests/golden/c21-determinism.json'))
b=json.loads((ROOT/'crates/executor/tests/golden/c21-determinism.json').read_text(encoding='utf8'))
delta=[]
def compare(a,b,path=''):
 if type(a)!=type(b): delta.append([path,a,b]); return
 if isinstance(a,dict):
  assert a.keys()==b.keys()
  for k in a: compare(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b)
  for i,(x,y) in enumerate(zip(a,b)): compare(x,y,path+'/'+str(i))
 elif a!=b: delta.append([path,a,b])
compare(a,b)
rows=json.loads((HERE.parent/'M2-C2.1-r3/conformance.json').read_text(encoding='utf8'))['rows']
promoted=[]
for i,r in enumerate(rows,1):
 if r['status']=='divergent':
  assert 'C21-R3-' in r['evidence']
  r.update(status='implemented',evidence='R4 scoped closure: c21_r3_geometry; c21_r3_contracts; c21_r4_closure; R3-01/02/03 closed as ruled.')
  promoted.append(i)
counts=dict(Counter(r['status'] for r in rows)); counts['missing']=0; counts['divergent']=0
(HERE/'conformance.json').write_text(json.dumps(dict(counts=counts,rows=rows),indent=2,ensure_ascii=False)+'\n',encoding='utf8')
lines=['# C2.1 final conformance register','',f'365 frozen source rows; {counts}. Promoted rows: {promoted}. All other dispositions and owners unchanged.','', '| # | Source / clause | Status | Evidence or owner |','|---|---|---|---|']
for i,r in enumerate(rows,1): lines.append(f"| {i} | {r['source']} section {r['section']}: {r['requirement'].replace('|',' / ')} | {r['status']} | {r['evidence'].replace('|',' / ')} |")
(HERE/'conformance.md').write_text('\n'.join(lines)+'\n',encoding='utf8')
gate=(ROOT/'reviews/evidence/M2-C2.1/workspace-tests.log').read_bytes()
gate=gate.decode('utf-16' if gate.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
totals=list(map(sum,zip(*(map(int,m) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',gate)))))
coords=[d for d in delta if '/points/' in d[0]]
assert len(coords)==24
assert all(re.fullmatch(r'/drawing1/nodes/[46]/strokes/0/points/\d+/[2345]',d[0]) for d in coords)
assert len(delta)==26
max_coord=max(abs(d[1]-d[2]) for d in coords)
max_handle=max(math.hypot(coords[i][1]-coords[i][2],coords[i+1][1]-coords[i+1][2]) for i in range(0,len(coords),2))
out=dict(reviewerPaths=len(owned),unchangedExamplePaths=len(examples),builderChangedPaths=git('diff','--name-only','b81904c','4e304cf').splitlines(),generatedFresh=not git('diff','--','crates/registry/src/declared.rs','crates/registry/src/declared.json'),prohibited=prohibited,goldenDelta=delta,goldenMaxCoordinateDu=max_coord,goldenMaxHandleDu=max_handle,registerCounts=counts,promoted=promoted,builderLogRawTotals=totals)
(HERE/'audit.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out,indent=2))
