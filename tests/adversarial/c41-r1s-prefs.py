"""Preference catalogue comparison receipt, stdlib only."""
import pathlib,json,re
r=pathlib.Path('.')
prefs={p['key']:p for p in json.loads((r/'crates/executor/src/prefs.json').read_text())['prefs']}
s=(r/'spec/01_application_shell.md').read_text(encoding='utf-8').split('### 7.3 Preference catalogue')[1].split('## 8.')[0]
rows=[]
for line in s.splitlines():
    m=re.match(r'\| `([\w.]+)` \| (.*?) \| (.*?) \| (.*?) \|',line)
    if not m: continue
    key,ty,default,ran=m.groups(); p=prefs.get(key)
    rows.append({'key':key,'spec_type':ty,'spec_default':default,'spec_range':ran,'catalogue':p,'present':p is not None,'same_type':p is not None and p['type']==ty})
extras=[p for k,p in prefs.items() if k not in {x['key'] for x in rows}]
print(json.dumps({'rows':rows,'extras':extras},indent=2,ensure_ascii=True))
assert all(x['present'] and x['same_type'] for x in rows)
