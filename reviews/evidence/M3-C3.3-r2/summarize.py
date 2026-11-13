from pathlib import Path
import json,re,hashlib,collections
p=Path('reviews/evidence/M3-C3.3-r2')
def read(f):
 b=f.read_bytes();return b.decode('utf-16') if b.startswith((b'\xff\xfe',b'\xfe\xff')) else b.decode('utf-8-sig',errors='replace')
r={'nativeExits':{f.name:read(f).strip() for f in sorted(p.glob('*.exit'))},'workspace':[]}
f=p/'workspace.log'
if f.exists():
 current='preamble';rows={}
 for line in read(f).splitlines():
  if re.match(r'\s*(Running |Doc-tests )',line):current=line.strip()
  if line.startswith('test result: '):rows[current]=line
 r['workspace']=[{'target':k,'summary':v} for k,v in rows.items()]
 r['workspaceFailedTests']=re.findall(r'^test ([^\r\n]+) \.\.\. FAILED',read(f),re.M)
 r['workspaceCounts']=dict(collections.Counter())
 for row in r['workspace']:
  for n,kind in re.findall(r'(\d+) (passed|failed|ignored|filtered out)',row['summary']):r['workspaceCounts'][kind]=r['workspaceCounts'].get(kind,0)+int(n)
r['finalProbeSummaries']={}
for name in ['final-probes.log','cancel.log']:
 f=p/name
 if f.exists():r['finalProbeSummaries'][name]=re.findall(r'^test result: .+$',read(f),re.M)
r['finalProbeCounts']={}
for summaries in r['finalProbeSummaries'].values():
 for summary in summaries:
  for n,kind in re.findall(r'(\d+) (passed|failed|ignored|filtered out)',summary):r['finalProbeCounts'][kind]=r['finalProbeCounts'].get(kind,0)+int(n)
r['finalSources']={str(f):hashlib.sha256(f.read_bytes()).hexdigest() for folder in ['executor','render'] for f in sorted(Path(f'crates/{folder}/tests').glob('c33_r2_*.rs'))}
r['interpretation']='Initial executor/depth logs include reviewer harness exploration; final-probes.log is the final-source Cargo adjudication. Direct fingerprint runs are rustc-linked against Cargo fingerprint-selected workspace libraries, not additional Cargo builds. Unfiltered workspace includes the accepted crash-test failure unchanged.'
(p/'execution-summary.json').write_text(json.dumps(r,indent=2)+'\n')
print(r.get('workspaceCounts'),r.get('workspaceFailedTests'))
