"""Extract owning-section locations independently of the declaration generator."""
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
samples=json.loads((HERE/'samples.json').read_text())
rows={r['id']:r for r in json.loads((HERE/'registry.json').read_text(encoding='utf-8'))}
out=[]
for cid in samples['random60']:
    row=rows[cid]
    section=row['section']
    paths=sorted((ROOT/'spec').glob(f'{section[:2]}_*.md'))
    matches=[]
    for p in paths:
        heading=''
        for n,line in enumerate(p.read_text(encoding='utf-8').splitlines(),1):
            if line.startswith('#'):heading=line
            if f'`{cid}`' in line:
                matches.append({'source':f'{p.relative_to(ROOT).as_posix()}:{n}','heading':heading,'text':line})
    out.append({'id':cid,'matches':matches})
    print(cid, row['scopes'], row['needsProject'], row['undoLabel'])
    for m in matches:print(' ',m['source'],m['heading'],m['text'])
(HERE/'sample-owning-sections.json').write_text(json.dumps(out,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
