import json,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
rows=json.loads((HERE/'registry.json').read_text(encoding='utf-8'))
holes={x['id']:x for x in json.loads((HERE/'schema-holes.json').read_text(encoding='utf-8'))}
generated=set(re.findall(r'id: "([^"]+)"',(ROOT/'crates/registry/src/declared.rs').read_text(encoding='utf-8')))
print('generated',len(generated),'with holes',sum(bool(holes[k]['params'] or holes[k]['result']) for k in generated))
for x in rows:
    if x['id'] in generated and not holes[x['id']]['params'] and not holes[x['id']]['result']:
        print(x['id'],'|',x['spec']['params'],'|',x['spec']['result'])
print('PARAMETER HOLES:')
for x in rows:
    if x['id'] in generated and holes[x['id']]['params']:
        print(x['id'],x['spec']['params'],json.dumps(holes[x['id']]['params']))
