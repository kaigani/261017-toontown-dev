import json,hashlib,subprocess,gzip,re
from pathlib import Path
R=Path.cwd();E=R/'reviews/evidence/M3-C3.2-r1'
def clean(v):
 if isinstance(v,dict):return {k:clean(x) for k,x in v.items() if k not in ('createdAt','modifiedAt')}
 if isinstance(v,list):return [clean(x) for x in v]
 return v
pairs=[]
for name in ['live-export.json','replayed-export.json']:
 a=json.loads((E/'inheritance-ci/e2-Windows'/name).read_text());b=json.loads((E/'inheritance-ci/e2-macOS'/name).read_text());pairs.append(dict(file=name,equal_without_timestamps=clean(a)==clean(b)))
(E/'e2-export-comparison.json').write_text(json.dumps(pairs,indent=2))
diff=subprocess.run(['git','diff','ac47998..142226d','--','crates/*/src/**'],capture_output=True,text=True,encoding='utf-8').stdout
bad=[l for l in diff.splitlines() if l.startswith('+') and not l.startswith('+++') and re.search(r'\.unwrap\(|\.expect\(|\bpanic!|\bunsafe\b|#\[allow',l)]
(E/'product-additions-scan.json').write_text(json.dumps({'matches':bad,'count':len(bad),'base':'ac47998','head':'142226d'},indent=2))
ci=[]
for path in (E/'inheritance-ci').rglob('*.log'):
 text=path.read_text(errors='replace');results=[l for l in text.splitlines() if 'test result:' in l];ci.append(dict(file=str(path.relative_to(E)),results=results,failed=any('test result: FAILED' in l for l in results)))
(E/'inheritance-results.json').write_text(json.dumps(ci,indent=2))
print(json.dumps({'e2':pairs,'forbidden_product_additions':len(bad),'inherited_binaries':sum(len(r['results']) for r in ci),'inherited_failures':sum(r['failed'] for r in ci)}))
