"""Check the public source and freshly generated rustdoc for stateless inversion."""
from pathlib import Path
import hashlib
import json
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
source_hits = []
for file in (ROOT / 'crates/model/src').rglob('*.rs'):
    for line, text in enumerate(file.read_text(encoding='utf-8').splitlines(), 1):
        if re.search(r'\bfn\s+invert\s*\(|\bpub\s+use\b[^;]*\binvert\b', text):
            source_hits.append(f'{file.relative_to(ROOT)}:{line}: {text}')
docs = ROOT / 'target/doc/tt_model'
assert docs.exists(), 'Generate with cargo doc -p tt-model --no-deps --locked first'
old_page = docs / 'change/fn.invert.html'
assert not old_page.exists()
doc_hits = [str(p.relative_to(docs)) for p in docs.rglob('*.html') if 'fn.invert.html' in p.read_text(encoding='utf-8')]
assert not source_hits and not doc_hits
pages = ['change/index.html', 'change/fn.invert_against.html', 'change/fn.apply_with_inverse.html']
result = dict(source_stateless_declaration_hits=source_hits, rustdoc_stateless_link_hits=doc_hits,
              stateless_page_exists=old_page.exists(), pages=[dict(path='target/doc/tt_model/'+p,
              sha256=hashlib.sha256((docs/p).read_bytes()).hexdigest()) for p in pages])
(HERE/'docs-audit.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
print(json.dumps(result, indent=2))
