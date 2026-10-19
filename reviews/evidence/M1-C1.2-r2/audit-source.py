"""Retain the reviewed diff, policy-sensitive additions and reviewer-test edit evidence."""
import gzip,hashlib,json,re,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
diff=git('diff','f32d455..237c8f3')
(HERE/'builder-response.diff.gz').write_bytes(gzip.compress(diff,mtime=0))
paths=git('diff','--name-only','f32d455..237c8f3').decode().splitlines()
product=[p for p in paths if '/src/' in p and p.endswith('.rs') and not p.endswith('/declared.rs')]
risk=[];public=[]
for p in product:
    d=git('diff','--unified=0','f32d455..237c8f3','--',p).decode('utf-8')
    for l in d.splitlines():
        if l.startswith('+') and not l.startswith('+++') and re.search(r'#\[allow|unsafe\b|\.unwrap\(|\.expect\(|panic!|&mut',l):risk.append({'path':p,'added':l[1:]})
    for n,l in enumerate((ROOT/p).read_text(encoding='utf-8').splitlines(),1):
        if re.search(r'\bpub\s+(?:\([^)]*\)\s+)?fn |pub\s+.*&mut',l):public.append({'source':f'{p}:{n}','line':l})
review_paths=['crates/executor/tests/c12_r1_*.rs','crates/executor/tests/c12-r1-ui','crates/executor/tests/common',
              'reviews/evidence/M1-C1.2-r1','tests/adversarial','crates/model/tests/c11_*','crates/engine/tests/c11_*']
testdiff=git('diff','f32d455..237c8f3','--',*review_paths)
(HERE/'reviewer-test-edit.diff').write_bytes(testdiff)
assert git('diff','--name-only','f32d455..237c8f3','--',*review_paths).decode().splitlines()==['crates/executor/tests/c12_r1_contracts.rs']
assert b'request_id_collision_returns_original_result_per_literal_spec' in testdiff
locks=[p for p in paths if p.endswith(('Cargo.lock','pnpm-lock.yaml','package.json','Cargo.toml')) or Path(p).name.upper().startswith(('LICENSE','LICENCE'))]
assert not locks
data={'head':'237c8f34286921d4db5cb8d06c2aeb6392368418','diffSha256':hashlib.sha256(diff).hexdigest(),
      'productFiles':product,'riskSensitiveAdditions':risk,'publicSurface':public,
      'licenseLockManifestChanges':locks,'reviewerEdit':'Only collision-test assertions and ruling comment; 5 additions / 3 deletions; exactly declared R1-22.',
      'conclusions':[
          'No new product unsafe / unwrap() / expect() / panic! allowance or call; product clippy -D warnings passes.',
          'New Cx mutators are gated by unforgeable handler context; offline Document methods return copies. New at_earlier_revision can yield invalid revs and guarded load trusts it (C12-R2-01).',
          'Core, Request and handle sender remain private. LoadGrant private field; 12 compile-fail cases and read-only snapshot pass fixture retained.',
          'Engine facade and three-layer IPC routes unchanged. Headless --run exits 3 (C1.4 stub); no alternate live-document mutation route exposed there.',
          'Generated frontend schemas preserve IPC validation machinery, but declaration holes weaken command payload contracts (C12-R2-02).',
          'Stateless inversion not reintroduced. Staged commits still revalidate model integrity; JSON-04 breaks raw inverse exactness (C12-R2-11).',
          'Job event fix bfa828f sends through unbounded subscriber channel under job lock before making final state visible. No test weakening.'
      ]}
(HERE/'static-audit.json').write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8')
print('Reviewed',len(product),'non-generated Rust product files;',len(risk),'policy-sensitive added lines;',len(public),'public/context signatures.')
print('No lockfile/license/manifest changes. Exactly one declared reviewer edit.')
