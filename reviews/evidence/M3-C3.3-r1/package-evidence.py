import datetime,gzip,hashlib,json,os,re,shutil,subprocess,tempfile
from pathlib import Path
ROOT=Path.cwd(); SRC=Path(tempfile.gettempdir())/'toontown-c33-r1'; OUT=ROOT/'reviews/evidence/M3-C3.3-r1'
def sha(data): return hashlib.sha256(data).hexdigest()
def read(path):
 data=path.read_bytes()
 return data.decode('utf-16' if data.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig',errors='replace')
def write(path,text):
 path.parent.mkdir(parents=True,exist_ok=True)
 normalized='\n'.join(line.rstrip() for line in text.splitlines()).rstrip()
 path.write_text(normalized+('\n' if normalized else ''),encoding='utf-8',newline='\n')
def git(*args): return subprocess.check_output(['git',*args],cwd=ROOT)
def groups(log):
 out=[]; current=None
 for line in log.splitlines():
  m=re.match(r'^\s*(Running|Doc-tests)\s+(.+)',line)
  if m:
   current=dict(target=m.group(2),summaries=[],failed=[]); out.append(current)
  m=re.search(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out; finished in ([0-9.]+)s',line)
  if m and current is not None:
   current['summaries'].append(dict(zip(['passed','failed','ignored','filtered','seconds'],[int(m[i]) for i in range(1,5)]+[float(m[5])])))
  m=re.match(r'^test (\S+) \.\.\. FAILED',line)
  if m and current is not None: current['failed'].append(m[1])
 for item in out:
  item['final']=item['summaries'][-1] if item['summaries'] else None
 return out
required=['pipeline.exit','scenarios.exit','release-goldens.exit','workspace.exit','fmt.exit','review-clippy.exit','final-checks.exit','review-release-pixels.exit','python-final-source.json']
assert all((SRC/f).exists() for f in required),[f for f in required if not (SRC/f).exists()]
OUT.mkdir(parents=True,exist_ok=True); raw=OUT/'raw';raw.mkdir(exist_ok=True)
receipts={}
for path in sorted(SRC.iterdir()):
 if path.is_file() and path.suffix in ['.log','.exit','.txt','.json','.ps1','.rs','.py']:
  data=path.read_bytes(); packed=gzip.compress(data,mtime=0)
  (raw/(path.name+'.gz')).write_bytes(packed)
  write(OUT/path.name,read(path))
  receipts[path.name]=dict(originalSha256=sha(data),originalBytes=len(data),gzipSha256=sha(packed),humanSha256=sha((OUT/path.name).read_bytes()),fileCreatedAt=datetime.datetime.fromtimestamp(path.stat().st_ctime,datetime.timezone.utc).isoformat(),fileLastWrite=datetime.datetime.fromtimestamp(path.stat().st_mtime,datetime.timezone.utc).isoformat())
for f in ['transparent-32x18.png','solid-32x18.png','rectangle-0-0.05.png','rectangle-37-0.png']:
 (OUT/'goldens').mkdir(exist_ok=True); shutil.copyfile(SRC/f,OUT/'goldens'/f)
review_goldens=[('transparent-32x18.png',32,18,'43b6c3ea64e44fb4b2195b086533047442b635f2cebeb6ecf52e21c1d826f238'),('solid-32x18.png',32,18,'187b989d25ae7ae51211c215bf156816e96854313178964ff70460245d432442'),('rectangle-0-0.05.png',120,68,'e6130113f9b2a264487cdcdd31a1bd2fcbd4d148653bba7e6078beabec42c1a0'),('rectangle-37-0.png',120,68,'58317fd335465043c65931a91360a0dc3a0e7af40a3755e29f54d17239db6dbb')]
for name,w,h,digest in review_goldens: assert sha((OUT/'goldens'/name).read_bytes())==digest
write(OUT/'goldens/index.json',json.dumps(dict(reviewedHead='e61c2651feb83c7e03174bedd10fa2cf563c4ad8',source='crates/executor/tests/c33_r1_pixels.rs',images=[dict(name=n,width=w,height=h,sha256=d) for n,w,h,d in review_goldens]),indent=2)+'\n')
for platform in ['Windows','macOS']:
 source=SRC/'artifacts'/('c33-'+platform)
 dest=OUT/'ci-artifacts'/('c33-'+platform)
 if source.exists(): shutil.copytree(source,dest,dirs_exist_ok=True)
local_comparisons=[]
for family,local_dir,ci_dir in [('c33','gate','c33-Windows'),('e2','e2','e2-Windows')]:
 for f in sorted((SRC/local_dir).rglob('*')):
  if f.is_file() and (f.suffix=='.png' or f.name in ['manifest.json','gate-path.json']):
   other=SRC/'artifacts'/ci_dir/f.relative_to(SRC/local_dir)
   equal=other.is_file() and f.read_bytes()==other.read_bytes()
   local_comparisons.append(dict(family=family,file=f.relative_to(SRC/local_dir).as_posix(),equal=equal,sha256=sha(f.read_bytes())))
for local,relative in [('e4','evaluate.json'),('e4-variant','variant/evaluate.json')]:
 f=SRC/local/'evaluate.json'; other=SRC/'artifacts/e4-Windows'/relative
 local_comparisons.append(dict(family=local,file='evaluate.json',equal=f.read_bytes()==other.read_bytes(),sha256=sha(f.read_bytes())))
assert all(item['equal'] for item in local_comparisons),local_comparisons
write(OUT/'local-ci-comparisons.json',json.dumps(local_comparisons,indent=2)+'\n')
ci_oracles=[]
for platform in ['Windows','macOS']:
 source=SRC/'artifacts'/('history-oracles-'+platform)
 if source.exists():
  shutil.copytree(source,OUT/'ci-artifacts'/('history-oracles-'+platform),dirs_exist_ok=True)
  for log in source.glob('*.log'):
   text=read(log)
   results=re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;',text)
   if results:
    assert all(state=='ok' and failed=='0' for state,passed,failed in results),str(log)
    ci_oracles.append(dict(platform=platform,file=log.name,summaries=results,sha256=sha(log.read_bytes())))
for log in (OUT/'ci-artifacts').rglob('*.log'):
 data=log.read_bytes(); target=raw/'ci-artifacts'/(log.relative_to(OUT/'ci-artifacts').as_posix()+'.gz'); target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes(gzip.compress(data,mtime=0)); write(log,read(log))
expected={'failed_sequence_publication_leaves_no_frames','sub_microfield_depth_difference_uses_timeline_order','wait_true_returns_completed_manifest','oversized_contact_sheet_is_refused_before_any_slow_work','oversized_inline_frame_is_refused_before_any_slow_work','killed_sequence_leaves_no_staged_files','render_frame_accepts_exclude_override','render_frame_reports_render_ms','empty_frame_observes_requested_cancellation'}
workspace=groups(read(SRC/'workspace.log'))
failed={name for item in workspace for name in item['failed']}
assert failed==expected,dict(missing=sorted(expected-failed),unexpected=sorted(failed-expected))
write(OUT/'workspace-results.json',json.dumps(workspace,indent=2)+'\n')
exits={p.name:read(p).strip() for p in SRC.glob('*.exit')}
for name in ['render.exit','clippy.exit','release.exit','gate.exit','deny.exit','e2.exit','e4.exit','e4-variant.exit','sdk.exit','release-goldens.exit','review-release-pixels.exit','fmt.exit','review-clippy.exit','python-final.exit','allows-final.exit','declarations-final.exit','post-test-tracked-diff.exit','artifact-download.exit','default-width.exit']:
 assert exits.get(name)=='0',(name,exits.get(name))
for name in ['executor.exit','mcp.exit','workspace.exit']: assert exits.get(name)=='101',(name,exits.get(name))
metadata=json.loads(read(SRC/'workspace-metadata.json'))
source_hashes={str(p.relative_to(ROOT)).replace('\\','/'):sha(p.read_bytes()) for p in list(ROOT.glob('crates/*/tests/c33_r1_*.rs'))+list(ROOT.glob('tests/adversarial/c33-r1-*.py'))}
changed_non_cargo=[]
for path,digest in metadata['sourceHashes'].items():
 if sha(Path(path).read_bytes())!=digest:
  assert path.endswith('.py'),path
  changed_non_cargo.append(path)
python_witness=json.loads(read(SRC/'python-final-source.json'))
assert python_witness['sha256']==sha((ROOT/'tests/adversarial/c33-r1-audit.py').read_bytes())
assert read(SRC/'python-final.exit').strip()=='0'
assert not metadata['receiptEnvironmentSet']
ci=json.loads(read(SRC/'ci.json'))
ci_summary=[]
for job in ci['jobs']:
 seconds=int((datetime.datetime.fromisoformat(job['completedAt'].replace('Z','+00:00'))-datetime.datetime.fromisoformat(job['startedAt'].replace('Z','+00:00'))).total_seconds())
 ci_summary.append(dict(name=job['name'],conclusion=job['conclusion'],seconds=seconds))
root_ci='D:/a/261017-toontown-dev/261017-toontown-dev/'
new_targets=list(ROOT.glob('crates/*/tests/c33_r1_*.rs'))
new_chars=sum(len(root_ci+p.relative_to(ROOT).as_posix())+3 for p in new_targets)
longest=max(len(root_ci+p.relative_to(ROOT).as_posix())+3 for p in ROOT.glob('crates/*/tests/*.rs'))
headroom=dict(baselineCharacters=26400,windowsLimit=32767,assumedCheckout=root_ci,newRoots=len(new_targets),newCharacters=new_chars,longestCurrentTestRoot=longest,estimatedFurtherLongestRoots=(32767-26400-new_chars)//longest)
write(OUT/'formatter-headroom.json',json.dumps(headroom,indent=2)+'\n')
summary=dict(reviewedHead='e61c2651feb83c7e03174bedd10fa2cf563c4ad8',nativeExits=exits,ciReleaseOracleSummaries=ci_oracles,localCiComparisons=local_comparisons,expectedFailingTests=sorted(expected),observedFailingTests=sorted(failed),workspaceTargets=len(workspace),workspaceTotals={k:sum(item['final'][k] for item in workspace if item['final']) for k in ['passed','failed','ignored','filtered']},sourceHashes=source_hashes,workspaceRustSourceHashesMatch=True,pythonSourcesChangedAfterWorkspaceStart=changed_non_cargo,finalPythonSourceWitness=python_witness,ci=ci_summary,formatterHeadroom=headroom,receipts=receipts)
write(OUT/'execution-summary.json',json.dumps(summary,indent=2)+'\n')
fixed={}
for rev in ['2184ae8','e61c265']:
 for name in ['reviews/M3-C3.3-handoff.md','reviews/M3-C3.3-plan.md','crates/render/src/lib.rs','crates/executor/src/commands/render_list.rs','crates/executor/src/commands/render.rs','crates/executor/src/commands/inspect.rs','crates/executor/src/commands/draw_render.rs','crates/io/src/image.rs','crates/mcp/src/tools.rs','crates/engine/src/selftest.rs','.github/workflows/ci.yml','crates/executor/tests/goldens/c23_render.json','crates/render/examples/c03_raster/hashes.json']:
  result=subprocess.run(['git','show',rev+':'+name],cwd=ROOT,capture_output=True)
  if result.returncode==0: fixed[rev+':'+name]=sha(result.stdout)
write(OUT/'ownership.txt',git('diff','--stat','2184ae8..e61c265','--','crates/*/tests/c*_r[0-9]_*','tests/adversarial','desktop/src/test','reviews/evidence/M3-C3.2-r*').decode())
write(OUT/'declared-test-edits.diff',git('diff','2184ae8..e61c265','--','crates/executor/tests/c12_registry.rs','crates/executor/tests/c12_kernel_state.rs','crates/executor/tests/common/examples.rs').decode())
for path in [OUT/'register.json',OUT/'register.md']:
 write(path,read(path))
files={p.relative_to(OUT).as_posix():sha(p.read_bytes()) for p in OUT.rglob('*') if p.is_file() and p.name!='provenance.json'}
write(OUT/'provenance.json',json.dumps(dict(reviewedHead=summary['reviewedHead'],sourceHashes=source_hashes,fixedInputs=fixed,rawReceiptPolicy='raw gzip preserves original bytes; human copies are UTF-8/LF with trailing whitespace removed',files=files),indent=2)+'\n')
print(json.dumps(dict(workspaceTotals=summary['workspaceTotals'],headroom=headroom,files=len(files)),indent=2))
