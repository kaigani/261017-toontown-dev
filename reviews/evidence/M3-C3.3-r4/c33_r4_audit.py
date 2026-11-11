from pathlib import Path
import subprocess,json,re,hashlib
E=Path('reviews/evidence/M3-C3.3-r4')
def git(*a): return subprocess.check_output(['git',*a]).decode('utf-8')
p='crates/executor/tests/c33_r2_wait.rs'
a=git('show','3fbf777:'+p); b=git('show','4984c25:'+p)
def assertions(s): return [' '.join(v.split()) for v in re.findall(r'assert(?:_eq|_ne)?!\(.*?\);',s,re.S)]
prior=json.loads(Path('reviews/evidence/M3-C3.3-r3/assertion-comparison.json').read_text())['c33_r2_wait.rs']['after']
assert assertions(a)==assertions(b)==prior
patch=git('diff','3fbf777..c3ce737','--',p)
added=[l[1:] for l in patch.splitlines() if l.startswith('+') and not l.startswith('+++')]
removed=[l[1:] for l in patch.splitlines() if l.startswith('-') and not l.startswith('---')]
assert sorted(x.strip() for x in added)==sorted(x.strip() for x in removed) and len(added)==2
paths=['crates/*/tests/c*_r[0-9]_*','tests/adversarial','desktop/src/test','reviews/evidence/M3-C3.3-r[123]','reviews/evidence/M3-C3.2-r*']
product=git('diff','3fbf777..4984c25','--','crates/*/src/**','bins/*/src/**','desktop/src/**','desktop/src-tauri/src/**')
bad=[l for l in product.splitlines() if l.startswith('+') and not l.startswith('+++') and re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!|\bunsafe\b|#\!?\[allow',l)]
assert not bad
prov=json.loads(Path('reviews/evidence/M3-C3.3-r3/provenance.json').read_text())
pins={}
for f in prov['files']:
 if '/golden/' in f or 'register' in f:
  pins[f]=dict(expected=prov['files'][f],actual=hashlib.sha256(Path(f).read_bytes()).hexdigest())
  assert pins[f]['expected']==pins[f]['actual']
regpaths=git('ls-files','reviews/*C3.1*register*','reviews/evidence/M3-C3.2-r*/register*').splitlines()
fixed={p:dict(base=hashlib.sha256(subprocess.check_output(['git','show','3fbf777:'+p])).hexdigest(),current=hashlib.sha256(Path(p).read_bytes()).hexdigest()) for p in regpaths}
assert all(v['base']==v['current'] for v in fixed.values())
out=dict(head=git('rev-parse','HEAD').strip(),ownership=git('diff','--stat','3fbf777..4984c25','--',*paths),sequencingPatch=patch,assertions=assertions(b),assertionsEqual=True,productForbiddenAdditions=bad,numstat=git('diff','--numstat','3fbf777..4984c25'),cargoChanges=git('diff','3fbf777..4984c25','--','**/Cargo.toml','Cargo.lock'),goldenChanges=git('diff','--stat','e61c265..4984c25','--','crates/executor/tests/golden'),siChanges=git('diff','3fbf777..4984c25','--','plan/spec-issues.md'),pins=pins,fixedRegisters=fixed,documentDiff=git('diff','3fbf777..4984c25','--','docs/decisions/ADR-0020-compositor-render-list.md','reviews/M3-C3.3-handoff.md','plan/traceability.csv'),receiptFlag=None)
(E/'audit.json').write_text(json.dumps(out,indent=2)+'\n')
print(out['ownership']);print('Assertions, product additions, pins and fixed registers pass')
