"""Final-round evidence audit. Writes only a new explicitly selected output directory."""
import argparse,collections,datetime,gzip,hashlib,importlib.util,json,pathlib,re,subprocess,tempfile,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
BASE='c8f73b5';HEAD='989233e'
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT).decode('utf-8')
def read(p):
 b=p.read_bytes();return b.decode('utf-16' if b.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
def audit(out):
 out.mkdir(parents=True,exist_ok=True)
 def write(n,v):(out/n).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
 protected=['tests/adversarial','crates/*/tests/c0*_r*','crates/*/tests/c1*_r*','crates/*/tests/c21_r*','crates/*/tests/c22_r*','crates/*/tests/c23_r*','desktop/src/test','spec/']
 diff=git('diff',BASE+'..'+HEAD,'--',*protected)
 existing=[p for p in git('ls-tree','-r','--name-only',BASE).splitlines() if p.startswith(('spec/','tests/adversarial/')) or re.search(r'crates/[^/]+/tests/c\d+_r\d+_',p)]
 assert git('diff',BASE+'..'+HEAD,'--',*existing)==''
 golden='crates/executor/tests/goldens/c23_render.json';expected=json.loads((ROOT/golden).read_bytes());assert expected==json.loads(git('show',BASE+':'+golden))
 write('declared-edits.json',{'base':BASE,'head':HEAD,'protectedPaths':protected,'protectedDiff':diff,'preexistingReviewerFilesUnchanged':True,'patternDiscrepancy':'declared new builder c23_round2.rs matches c23_r*','goldensStructurallyUnchanged':True,'allChanges':git('diff','--stat',BASE+'..'+HEAD),'testAndProfileDiff':git('diff',BASE+'..'+HEAD,'--','Cargo.toml','crates/executor/tests/c12_kernel_state.rs','crates/executor/tests/c14_obligations.rs','crates/executor/tests/c23_watchdog.rs')})
 run=json.loads(read(out/'ci-run.json'));assert run['headSha'].startswith(HEAD);logs=read(ROOT/'target/c23-r3-ci/ci.log');jobs=[]
 for j in run['jobs']:
  seconds=(datetime.datetime.fromisoformat(j['completedAt'].replace('Z','+00:00'))-datetime.datetime.fromisoformat(j['startedAt'].replace('Z','+00:00'))).total_seconds()
  lines=[l for l in logs.splitlines() if l.startswith(j['name']+'\t') and '\tRun cargo test --workspace --locked\t' in l]
  counts=[tuple(map(int,m)) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored','\n'.join(lines))]
  jobs.append({'name':j['name'],'conclusion':j['conclusion'],'seconds':seconds,'above55Minutes':j['name']=='check (windows-latest)' and seconds>3300,'workspaceCounts':dict(zip(['passed','failed','ignored'],[sum(x[i] for x in counts) for i in range(3)]))})
 comparisons=[]
 from PIL import Image
 for os in ['Windows','macOS']:
  source=ROOT/'target/c23-r3-ci'/('check-'+os)/'c23-render';manifest=json.loads((source/'hashes.json').read_bytes());assert manifest==expected
  dest=out/'ci-renders'/os;dest.mkdir(parents=True,exist_ok=True)
  for p in source.iterdir(): (dest/p.name).write_bytes(p.read_bytes())
  for name in expected:
   p=source/(name+'.png');b=p.read_bytes();im=Image.open(p).convert('RGBA');pixels=bytes(round(c*px[3]/255) if k<3 else px[3] for px in im.getdata() for k,c in enumerate(px));hashed=hashlib.sha256(pixels).hexdigest()
   equal=b==(ROOT/'reviews/evidence/M2-C2.3/goldens'/(name+'.png')).read_bytes();assert equal and hashed==expected[name]
   comparisons.append({'os':os,'name':name,'pngSha256':hashlib.sha256(b).hexdigest(),'equalsCommittedBytes':equal,'premultipliedPixelsSha256':hashed,'equalsLocalBytes':None if not (ROOT/'target/evidence/c23-render'/(name+'.png')).exists() else b==(ROOT/'target/evidence/c23-render'/(name+'.png')).read_bytes()})
 write('ci-recomputed.json',{'run':41343129891,'head':run['headSha'],'successfulJobs':sum(j['conclusion']=='success' for j in jobs),'jobs':jobs,'renderComparisons':comparisons})
 (out/'ci-all.log.gz').write_bytes(gzip.compress((ROOT/'target/c23-r3-ci/ci.log').read_bytes(),mtime=0))
 counts={}
 for p in out.glob('*.log'):
  m=[tuple(map(int,m)) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored',read(p))]
  if m:counts[p.name]={'summaries':len(m),**dict(zip(['passed','failed','ignored'],[sum(x[i] for x in m) for i in range(3)]))}
 write('gate-counts.json',counts)
 changed=git('diff','--name-only',HEAD).splitlines();write('hygiene.json',{'changedPreexistingFiles':changed,'status':git('status','--short'),'m1ReceiptDiff':git('diff',HEAD,'--','reviews/evidence/M1-C1.3*'),'specDiff':git('diff',HEAD,'--','spec/')})
 # Preserve register identity and deferred owners, closing only the four round-two divergent units.
 prior=json.loads((ROOT/'reviews/evidence/M2-C2.3-r2/register.json').read_bytes());v=json.loads(json.dumps(prior));deltas=[]
 for u in v['units']:
  if u['status']=='divergent':
   before=dict(u);u['status']='implemented';u['evidenceOrOwner']='Round-3 closure variants and preserved round-2 regressions; see M2-C2.3-r3.md';deltas.append({'before':before,'after':dict(u)})
 for c in v['commands']:
  if c['status']=='divergent':c['status']='implemented';c['finding']=None
 v['target']=run['headSha'];v['counts']=dict(collections.Counter(x['status'] for x in v['units']));v['commandCounts']=dict(collections.Counter(x['status'] for x in v['commands']));write('register.json',v);write('register-deltas.json',{'priorCounts':prior['counts'],'counts':v['counts'],'priorCommandCounts':prior['commandCounts'],'commandCounts':v['commandCounts'],'unitDeltas':deltas})
class Audit(unittest.TestCase):
 def test_protected_diff_and_golden(self):
  self.assertEqual(git('diff',BASE+'..'+HEAD,'--','tests/adversarial','crates/*/tests/c23_r[0-9]*','spec/'),'')
  p='crates/executor/tests/goldens/c23_render.json';self.assertEqual(json.loads(git('show',BASE+':'+p)),json.loads((ROOT/p).read_bytes()))
 def test_measurement_analyzer_rejects_malformed(self):
  with tempfile.TemporaryDirectory() as t:
   folder=pathlib.Path(t);(folder/'malformed.json').write_text('{bad')
   p=subprocess.run(['python',str(ROOT/'scripts/analyse-measurements.py'),str(folder)],capture_output=True)
   self.assertNotEqual(p.returncode,0)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--out',type=pathlib.Path);args,rest=p.parse_known_args()
 if args.out:audit(args.out.resolve())
 unittest.main(argv=[__file__,*rest],verbosity=2)
