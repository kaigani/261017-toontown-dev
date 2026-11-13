"""Final receipt checks; run after the report and mandatory gates are complete."""
from pathlib import Path
import json,subprocess,hashlib
E=Path(__file__).resolve().parent
assert (E/'workspace.exit').exists()
report=Path('reviews/M3-C3.3-r4-closure.md')
text=report.read_text(encoding='utf-8')
assert '**Verdict: pass.**' in text and 'WORKSPACE_' not in text
old=Path('reviews/M3-C3.3-r3.md').read_text(encoding='utf-8').split('## C3.4 readiness list\n\n')[1].split('\n## Closure checklist')[0]
new=text.split('## C3.4 readiness list\n\n')[1].strip()
a=old.strip().splitlines(); b=new.splitlines()
assert len(a)==len(b)
for x,y in zip(a,b):
 if not x.startswith('7. '): assert x==y,(x,y)
assert '**Closed this round:**' in next(x for x in b if x.startswith('7. '))
tracked=subprocess.check_output(['git','diff','--name-only'],text=True)
assert not tracked,tracked
status=subprocess.check_output(['git','status','--short','--untracked-files=all'],text=True)
allowed=['crates/executor/tests/c33_r4_','crates/mcp/tests/c33_r4_','reviews/evidence/M3-C3.3-r4/','reviews/M3-C3.3-r4-closure.md']
for line in status.splitlines():
 assert line.startswith('?? '),line
 assert any(line[3:].startswith(prefix) for prefix in allowed),line
(E/'final-status.txt').write_text('Tracked changes: none. Only new round-four reviewer files below.\n'+status,encoding='utf-8')
prov=json.loads((E/'provenance.json').read_text(encoding='utf-8'))
prov['reviewProbeSourcesLfNormalizedSha256']={p.as_posix():hashlib.sha256(p.read_bytes().replace(b'\r\n',b'\n')).hexdigest() for base in ['crates/executor/tests','crates/mcp/tests'] for p in Path(base).glob('c33_r4_*.rs')}
(E/'provenance.json').write_text(json.dumps(prov,indent=2)+'\n',encoding='utf-8')
manifest=[]
for p in sorted(E.rglob('*')):
 if p.is_file() and p.name!='evidence-manifest.json':
  data=p.read_bytes(); manifest.append(dict(file=p.relative_to(E).as_posix(),bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
manifest.append(dict(file='../../M3-C3.3-r4-closure.md',bytes=len(report.read_bytes()),sha256=hashlib.sha256(report.read_bytes()).hexdigest()))
(E/'evidence-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print('Final receipts verified; only new reviewer files; readiness unchanged except closed item 7 condition')
