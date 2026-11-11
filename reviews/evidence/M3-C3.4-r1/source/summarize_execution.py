import json,re
from pathlib import Path
E=Path('reviews/evidence/M3-C3.4-r1')
assert (E/'workspace.exit').exists(), 'workspace still running'
runs=json.loads((E/'continuation-runs.json').read_text())
for i,r in enumerate(runs):
 suffix='-initial' if i<3 else '-prelint' if i in [3,5] else '-normalization-diagnostic' if i==4 else ''
 r['log']=r['name']+suffix+'.log'
 r['exitReceipt']=r['name']+suffix+'.exit'
if (E/'final-reviewer-runs.json').exists(): runs+=json.loads((E/'final-reviewer-runs.json').read_text())
for r in runs:
 r.setdefault('log',r['name']+'.log')
 r.setdefault('exitReceipt',r['name']+'.exit')
text=(E/'workspace.log').read_text(encoding='utf-8',errors='strict')
suites=[]
current=None
for line in text.splitlines():
 if line.lstrip().startswith('Running '): current=line.strip()
 if line.lstrip().startswith('Doc-tests '): current=line.strip()
 match=re.search(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in ([\d.]+)s',line)
 if match:
  suites.append(dict(name=current,state=match[1],passed=int(match[2]),failed=int(match[3]),ignored=int(match[4]),measured=int(match[5]),filtered=int(match[6]),durationSeconds=float(match[7])))
nested=[]
unique=[]
for suite in suites:
 if unique and unique[-1]['name']==suite['name']:
  nested.append(unique.pop())
 unique.append(suite)
suites=unique
workspace={'nestedHarnessSummaries':nested,'countingMethod':'Last harness summary per cargo Running/Doc-tests entry; earlier child-process summaries retained separately and excluded from totals.','exitCode':int((E/'workspace.exit').read_text()),'suiteCount':len(suites),'binaryCount':sum(s['name'] is not None and s['name'].startswith('Running ') for s in suites),'docTestSuiteCount':sum(s['name'] is not None and s['name'].startswith('Doc-tests ') for s in suites),'totals':{k:sum(s[k] for s in suites) for k in ['passed','failed','ignored','measured','filtered']},'suites':suites,'failedTests':re.findall(r'^test (.+) \.\.\. FAILED$',text,re.M)}
workspace['failureTriage']=[{'test':name,'classification':'pre-existing accepted CI skip' if name=='killed_sequence_leaves_no_staged_files' else 'requires review','reason':'Killed-process no-staging premise is excluded by the supplied ruling; next-job sidecar recovery remains the contract.' if name=='killed_sequence_leaves_no_staged_files' else 'Unclassified failure'} for name in workspace['failedTests']]
inherited=[]
for p in sorted(E.glob('*.exit')):
 inherited.append({'receipt':p.name,'exitCode':int(p.read_text(encoding='utf-8-sig').strip())})
pipeline=(E/'source/c34_r1_pipeline.ps1').read_text(encoding='utf-8-sig')
inheritedCommands=[{'name':m[1],'command':m[2],'durationSeconds':None,'note':'Prior-session command declaration; execution evidence is its retained log/sidecar, not this declaration.'} for m in re.finditer(r"^Run '([^']+)' '([^']+)'",pipeline,re.M)]
summary={'inheritedCommandDeclarations':inheritedCommands,'reviewedHead':'45b41dad19538970bd8878d34d864f82fa4bfb28','runs':runs,'workspace':workspace,'workspaceStorage':json.loads((E/'workspace-storage.json').read_text()),'toolVersions':json.loads((E/'tool-versions.json').read_text()),'allExitSidecars':inherited,'inheritedReceiptNote':'Prior-session logs and exit sidecars retained; durations unavailable unless recorded in their logs. Continuation command durations are measured wall time. Initial contract comparison and lint diagnostics are reviewer defects; bounded merge-note failure is intentional C34-R1-01.'}
(E/'execution-summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in workspace.items() if k!='suites'},indent=2))
