import pathlib,json,collections,subprocess,re
R=pathlib.Path.cwd(); E=R/'reviews/evidence/M1-C1.4-r4'
d=json.loads((R/'reviews/evidence/M1-C1.4-r3/conformance.json').read_text())
for r in d['requirements']:
 if r['line'] in [26,505]: r.update(note='C14-R3-01 commit accounting closes, and atomic revisions/history replay passes; C14-R3-02 remains divergent: group expectRev is rejected as an unknown field. Owner builder C1.4 before M1 merge.',test='crates/mcp/tests/c14_r4_closure.rs')
 if r['line'] in [219,573]:
  r.update(status='implemented',note='Scoped closure on 5c46aa6: C14-R3-01/02/03 closed by unchanged r3 suites, r4 closure probes and cancellation phase tests.',test='crates/mcp/tests/c14_r3_replay.rs; crates/mcp/tests/c14_r4_closure.rs; bins/headless/tests/cancel_phases.rs; tests/adversarial/c14-r3-mcp.py')
for r in d['declarations']:
 if r['id']=='agent.activity.export': r.update(note='Atomic group export and history pass, but ruled group expectRev unsupported; C14-R3-02 remains open. Owner builder C1.4 before M1 merge.',test='crates/mcp/tests/c14_r4_closure.rs')
d['counts']=dict(collections.Counter(r['status'] for r in d['requirements']));d['declarationCounts']=dict(collections.Counter(r['status'] for r in d['declarations']))
assert [r['id'] for r in d['requirements'] if r['status']=='divergent']==['S11-L26', 'S11-L96', 'S11-L505']
(E/'conformance.json').write_text(json.dumps(d,indent=2)+'\n')
md='# C1.4 scoped closure conformance register\n\nProduct: `5c46aa6`. Original r3 unit boundaries, accepted closures and owned deferrals retained. Two cancellation requirement rows promoted. S11-L26 and S11-L505 plus agent.activity.export retain divergence because C14-R3-02 group expectRev is unsupported; S11-L96 (SI-049, spec owner) is unchanged.\n\nRequirements: **151 / 3 / 0 / 72**. Declarations: **20 / 1 / 0 / 11** (implemented / divergent / missing / deferred-with-owner).\n\n'
for key in ['requirements','declarations']:
 md+='## '+key+'\n\n| Unit | Status | Evidence and owner disposition |\n|---|---|---|\n'
 for r in d[key]: md+='| '+r['id']+' | '+r['status']+' | '+r['note'].replace('|','\\|').replace('\n',' ')+' |\n'
 md+='\n'
(E/'conformance.md').write_text(md,encoding='utf-8')
cases=[('5983f29','crates/mcp/tests/c14_tickets.rs','every_result_lists_its_effective_params'),('5983f29','crates/mcp/tests/c14_tickets.rs','namespace_docs_carry_a_worked_example_with_executed_results'),('04cc687','crates/persist/tests/zip64.rs','an_entry_over_4_gib_round_trips_with_zip64_sizes'),('4e34520','crates/executor/tests/c14_lock_races.rs','child_lock_holder'),('f0f55d8','crates/render/tests/c03_r1_stress.rs','c03_r1_04_simd_dispatch_must_not_change_final_pixels')]
def body(s,name):
 start=s.index('fn '+name+'('); start=s.index('{',start); depth=0
 for i in range(start,len(s)):
  if s[i]=='{':depth+=1
  elif s[i]=='}':
   depth-=1
   if depth==0:return s[start:i+1].replace('\r\n','\n')
for rev,p,name in cases:
 oldpath=p
 paths=subprocess.check_output(['git','ls-tree','-r','--name-only',rev],text=True).splitlines()
 if p not in paths:
  oldpath=next(x for x in paths if x.endswith('.rs') and ('fn '+name+'(') in subprocess.check_output(['git','show',rev+':'+x],text=True))
 old=subprocess.check_output(['git','show',rev+':'+oldpath],text=True)
 assert body(old,name)==body((R/p).read_text(),name),name
 print(name,': unchanged since',rev,'(',oldpath,')')
print(d['counts'],d['declarationCounts'])
