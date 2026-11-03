"""Collect final gate, local render and protected-history receipts without overwriting prior evidence."""
import argparse,hashlib,json,pathlib,re,subprocess,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2];HEAD='989233ec83b78d37f41052b57bf12ee17d132940'
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT)
def text(p):
 b=p.read_bytes();return b.decode('utf-16' if b.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
def collect(out):
 out.mkdir(parents=True,exist_ok=True)
 def write(name,v):
  p=out/name
  if p.exists():raise FileExistsError('final receipt already exists: '+str(p))
  p.write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
 evidence=ROOT/'reviews/evidence/M2-C2.3-r3';counts={}
 for p in evidence.glob('*.log'):
  m=[tuple(map(int,m)) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored',text(p))]
  if m:counts[p.name]={'summaries':len(m),**dict(zip(['passed','failed','ignored'],[sum(x[i] for x in m) for i in range(3)]))}
 write('gate-counts.json',counts)
 base=set(git('ls-tree','-r','--name-only',HEAD).decode().splitlines());changed=set(git('diff','--name-only',HEAD).decode().splitlines());protected=sorted(base&changed);assert not protected,protected
 paths=['reviews/evidence/M1-C1.3-r3/open-latency.json','reviews/evidence/M1-C1.3-r2/inspection-memory.json']
 receipts=[]
 for p in paths:
  raw=(ROOT/p).read_bytes();original=git('show',HEAD+':'+p);receipts.append({'path':p,'rawSha256':hashlib.sha256(raw).hexdigest(),'originalBlobSha256':hashlib.sha256(original).hexdigest(),'byteIdentical':raw==original});assert raw==original
 write('hygiene.json',{'reviewedHead':HEAD,'changedPreexistingFiles':protected,'m1Receipts':receipts,'status':git('status','--short').decode(),'specDiff':git('diff',HEAD,'--','spec/').decode()})
 renders=[]
 for os in ['Windows','macOS']:
  for p in (evidence/'ci-renders'/os).glob('*.png'):
   local=ROOT/'target/evidence/c23-render'/p.name;a=p.read_bytes();b=local.read_bytes();assert a==b
   renders.append({'os':os,'name':p.name,'byteIdenticalToLocal':True,'sha256':hashlib.sha256(b).hexdigest()})
 write('local-render-comparisons.json',renders)
 v=json.loads((evidence/'sdk-contracts.json').read_bytes());register=json.loads((ROOT/'reviews/evidence/M2-C2.3-r2/register.json').read_bytes());called={c['id'] for o in v['observations'] for c in o['calls']};called.add('draw.selection.changed');missing=[c['id'] for c in register['commands'] if c['id'] not in called];assert not missing
 write('sdk-summary.json',{'sdkScenarios':len(v['observations']),'registerCommandsCovered':len(register['commands']),'missing':missing,'replaysPassed':sum(o.get('replayExit')==0 for o in v['observations']),'savedOpenIdentical':sum(o.get('savedOpenIdentical',False) for o in v['observations']),'oneRevisionEventChecks':sum(bool(o.get('revisionEvents')) for o in v['observations']),'eventOnlyCommandRefusal':v['observations'][0]['selectionEventInvocation']})
class Audit(unittest.TestCase):
 def test_preexisting_files_unchanged(self):
  base=set(git('ls-tree','-r','--name-only',HEAD).decode().splitlines());changed=set(git('diff','--name-only',HEAD).decode().splitlines());self.assertEqual(base&changed,set())
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--out',type=pathlib.Path);a,rest=p.parse_known_args()
 if a.out:collect(a.out.resolve())
 unittest.main(argv=[__file__,*rest],verbosity=2)
