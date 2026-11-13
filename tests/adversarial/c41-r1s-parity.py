"""Static command surface inventory; run from the repository root, no build."""
import json, pathlib, re
root = pathlib.Path('.')
index = (root/'desktop/src/ipc/generated/commands.ts').read_text(encoding='utf-8')
ids = set(re.findall(r'^  "([\w.]+)": \{\n    kind:', index, re.M))
manifest=json.loads((root/'desktop/src/ui-bindings/menu.json').read_text())
def walk(x):
    if isinstance(x,dict):
        if x.get('kind')=='command': yield x
        for v in x.values(): yield from walk(v)
    elif isinstance(x,list):
        for v in x: yield from walk(v)
items=list(walk(manifest))
just={}
for p in (root/'crates').rglob('*.rs'):
    s=p.read_text(encoding='utf-8')
    for m in re.finditer(r'#\[(?:tt_registry::)?command\((.*?)\)\]',s,re.S):
        b=m.group(1); i=re.search(r'id\s*=\s*"([\w.]+)"',b); j=re.search(r'agent_only\s*=\s*"([^"]+)"',b)
        if i and j: just[i.group(1)]=j.group(1)
written_gui={}
for p in (root/'crates').rglob('*.rs'):
    text=p.read_text(encoding='utf-8')
    for b in re.split(r'(?=id: "[\w.]+")', text):
        i=re.match(r'id: "([\w.]+)"',b)
        if not i: continue
        a=re.search(r'agent_only: Some\(\s*"([^\"]+)"\s*,?\s*\)',b)
        g=re.search(r'gui: Some\(\s*"([^\"]+)"\s*,?\s*\)',b)
        if a: just[i.group(1)]=a.group(1)
        if g: written_gui[i.group(1)]=g.group(1)
    for m in re.finditer(r'#\[(?:tt_registry::)?command\((.*?)\)\]', text, re.S):
        b=m.group(1); i=re.search(r'id\s*=\s*"([\w.]+)"',b); g=re.search(r'gui\s*=\s*"([^\"]+)"',b)
        if i and g: written_gui[i.group(1)]=g.group(1)
surfaces={c['command'] for c in items}
for p in (root/'desktop/src').rglob('*.tsx'):
    if '.test.' not in p.name:
        surfaces.update(re.findall(r'[\'\"]([a-z][\w]*(?:\.[\w]+)+)[\'\"]',p.read_text(encoding='utf-8')))
keys=json.loads((root/'crates/executor/src/keymap-default.json').read_text())
print(json.dumps({'registered':len(ids),'manifest_entries':len(items),'uncovered':sorted(ids-surfaces-just.keys()),'written_gui_locations':written_gui,'without_location_or_justification':sorted(ids-surfaces-just.keys()-written_gui.keys()),'written_agent_only':just,'unknown_manifest':sorted({c['command'] for c in items}-ids),'keymap':keys},indent=2))
assert all(c['command'] in ids for c in items)

