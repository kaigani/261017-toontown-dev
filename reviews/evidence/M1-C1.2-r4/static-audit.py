"""Independent fix-diff and reviewer-test integrity audit."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def git(*a): return subprocess.check_output(['git',*a],cwd=ROOT).decode('utf-8')
files=git('diff','--name-only','be5ef32..f210057').splitlines()
test_changes=[f for f in files if '/tests/' in f or '/test/' in f or f.startswith('tests/')]
assert set(test_changes)=={'crates/executor/tests/c12_checkpoint_property.rs','crates/registry/tests/declared.rs'}
old_tests=[f for f in git('ls-tree','-r','--name-only','be5ef32').splitlines()
           if re.search(r'(?:c11_r\d|c12_r\d|adversarial_c01|tests/adversarial/|desktop/src/test/)',f) and not f.startswith('reviews/')]
integrity=[]
for f in old_tests:
    a=subprocess.check_output(['git','show','be5ef32:'+f],cwd=ROOT)
    b=(ROOT/f).read_bytes().replace(b'\r\n',b'\n')
    assert a==b, f
    integrity.append({'path':f,'sha256':hashlib.sha256(a).hexdigest(),'unchanged':True})
patch=git('diff','--unified=0','be5ef32..f210057','--','crates','bins','desktop')
sensitive=[]; path=''; number=0
for line in patch.splitlines():
    if line.startswith('+++ b/'): path=line[6:]
    elif line.startswith('@@'): number=int(re.search(r'\+(\d+)',line).group(1))
    elif line.startswith('+'):
        if re.search(r'#\!?\[allow|\bunsafe\b|&mut|\.unwrap\(|\.expect\(|panic!',line):
            sensitive.append({'path':path,'line':number,'testOnly':'/tests/' in path,'text':line[1:]})
        number+=1
    elif line.startswith(' '): number+=1
assert not any(re.search(r'#\!?\[allow|\bunsafe\s*\{|\.unwrap\(|\.expect\(|panic!',r['text']) for r in sensitive if not r['testOnly'])
deps=[f for f in files if Path(f).name in {'Cargo.toml','Cargo.lock','package.json','pnpm-lock.yaml','deny.toml','LICENSE'}]
assert not deps and not any(f.startswith('spec/') for f in files)
out={'base':'be5ef32','reviewed':'f210057','commits':git('log','--oneline','be5ef32..f210057').splitlines(),
     'changedFiles':files,'changedTestFiles':test_changes,'unchangedReviewerTests':integrity,
     'dependencyChanges':deps,'sensitiveAddedLines':sensitive,
     'inspection':'No added product allow/unsafe/unwrap/expect/panic. New mutable methods are private request-cache internals and pub(crate) stamped_document returning an owned Document; no live mutable reference return. schema_ts only flattens/deduplicates flat union members. Existing registry test changes are builder-owned, not reviewer-owned.'}
(HERE/'static-audit.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'unchangedReviewerTests':len(integrity),'testChanges':test_changes,'sensitiveLines':len(sensitive),'dependencyChanges':deps},indent=2))
