"""Independent change/surface/provenance audit; never changes product or prior evidence."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8')
files = git('diff','--name-only','d7a8205..2e0e5dd').splitlines()
prior_tests = [p for p in files if re.search(r'(c11_r\d|c12_r[12]|C01|c01-r)', p) and not p.startswith('reviews/')]
assert not prior_tests, prior_tests
dependency_files = [p for p in files if Path(p).name in {'Cargo.lock','Cargo.toml','package.json','pnpm-lock.yaml','deny.toml','LICENSE','LICENSE.md'}]
assert not dependency_files, dependency_files
patch = git('diff','--unified=0','d7a8205..2e0e5dd','--','crates','bins','desktop')
added=[]
file=''
line_number=0
for line in patch.splitlines():
    if line.startswith('+++ b/'): file=line[6:]
    elif line.startswith('@@'):
        line_number=int(re.search(r'\+(\d+)',line).group(1))
    elif line.startswith('+'):
        if re.search(r'#\[allow|\bunsafe\b|\.unwrap\(|\.expect\(|\bpanic!|&mut',line):
            content=(ROOT/file).read_text(encoding='utf-8').splitlines()
            cfg = next((n for n,l in enumerate(content,1) if l=='#[cfg(test)]'), len(content)+1)
            added.append({'file':file,'lineNumber':line_number,'testOnly': '/tests/' in file or line_number > cfg,'line':line[1:]})
        line_number+=1
    elif line.startswith(' '): line_number+=1
public=[]
for area in ('crates/executor/src','crates/model/src','crates/engine/src','bins','desktop/src-tauri/src'):
    for p in sorted((ROOT/area).rglob('*.rs')):
        for n,l in enumerate(p.read_text(encoding='utf-8').splitlines(),1):
            if re.search(r'pub (?:async )?fn |#\[tauri::command', l):
                public.append({'source':f'{p.relative_to(ROOT).as_posix()}:{n}','line':l.strip()})
result={'base':'d7a8205','reviewed':'2e0e5dd','changedFiles':files,'priorReviewerTestEdits':prior_tests,
        'manifestLicenseLockfileChanges':dependency_files,'addedSensitivePatterns':added,'publicSurface':public,
        'interpretation':'New dead_code allowances are declaration structs; new &mut uses are private executor/context operations or owned candidate/staged values, not live-handle borrows. No newly added unsafe/unwrap()/expect()/panic! in product. Public candidate constructors cannot bypass load integrity/dirty guards.',
        'reviewerOnlyCurrentChanges':git('status','--porcelain').splitlines()}
for r in added:
    if not r['testOnly']:
        assert not re.search(r'\bunsafe\s*\{|\.unwrap\(|\.expect\(|\bpanic!',r['line']), r
(HERE/'static-audit.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
# Exact current registry export is compared with the builder's copy only after independent validation.
current=(HERE/'registry.json').read_bytes()
builder=(HERE.parent/'M1-C1.2-r2-response/registry.json').read_bytes()
print(json.dumps({'priorReviewerTestEdits':len(prior_tests),'dependencyChanges':len(dependency_files),
    'sensitiveLinesForInspection':len(added),'publicSurfaceLines':len(public),
    'registryByteIdenticalToBuilder':current==builder,'currentRegistrySha256':hashlib.sha256(current).hexdigest()},indent=2))
