"""Reproduce the reviewer-test and public API audit at the reviewed commits."""
from pathlib import Path
from collections import Counter
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8')
def save(name, value):
    (HERE/name).write_text(json.dumps(value, indent=2)+'\n', encoding='utf-8')

roots = ['crates/model/tests', 'crates/engine/tests', 'crates/render/tests',
         'desktop/src/ipc', 'desktop/src/test', 'tests']
paths = git('ls-tree', '-r', '--name-only', 'bd5ec34', '--', *roots).splitlines()
paths = [p for p in paths if p.endswith(('.rs', '.ts', '.tsx', '.py', '.mjs'))]
diff = git('diff', 'bd5ec34..fb31cfe', '--', *paths)
assert not diff
save('test-edit-audit.json', dict(base='bd5ec34', head='fb31cfe', paths=paths, diff_bytes=len(diff)))

pattern = re.compile(r'^\s*pub (?!\()(?:const |async |unsafe )?(?:fn|struct|enum|trait|type|mod|use|const|static)\b[^\n]*', re.M)
api = {}
for rev in ['9c3f586', 'fb31cfe']:
    declarations = []
    for file in git('ls-tree', '-r', '--name-only', rev, '--', 'crates/model/src').splitlines():
        if file.endswith('.rs'):
            declarations += [(file, s.strip()) for s in pattern.findall(git('show', f'{rev}:{file}'))]
    api[rev] = Counter(declarations)
fix = git('diff', 'a448459^', 'a448459', '--', 'crates/model/src')
added = [line for line in fix.splitlines() if line.startswith('+') and not line.startswith('+++')]
result = dict(base='9c3f586', head='fb31cfe',
    added=list((api['fb31cfe']-api['9c3f586']).elements()),
    removed=list((api['9c3f586']-api['fb31cfe']).elements()),
    added_allow_unsafe_mut_lines=[line for line in added if re.search(r'#\[allow|\bunsafe\b|&(?:\x27\w+\s+)?mut\b', line)],
    changed_model_sources=git('diff', '--name-only', '9c3f586..fb31cfe', '--', 'crates/model/src').splitlines(),
    signature='pub fn with_patched_entry(&self, key: &EntryKey, patch: &[crate::change::PatchOp]) -> Result<(Document, Vec<crate::change::PatchOp>), Vec<ModelError>>',
    inspection='Full diff inspected: no new product allows/unsafe/public mutable reference; private scanner mutates only a local traversal stack. Builder staged_commit.rs adds the declared test-helper unwrap_used/panic allowance. No stateless invert returns.')
assert result['added'] == [('crates/model/src/document/mod.rs', 'pub fn with_patched_entry(')]
assert not result['removed']
assert not any(re.search(r'#\[allow|\bunsafe\b', line) for line in added)
save('api-audit.json', result)
print(json.dumps(result, indent=2))
