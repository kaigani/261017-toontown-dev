import pathlib,json,re,subprocess
old=subprocess.check_output(['git','show','812554b:crates/executor/tests/golden/c21-determinism.json']).decode()
new=pathlib.Path('crates/executor/tests/golden/c21-determinism.json').read_text(encoding="utf-8")
a=json.loads(old);node=a['drawing1']['nodes'][7]['children'][0];nid=node['id']
def mask(s):
 start=s.index('"strokes":',s.index(nid));start=s.index('[',start)
 _,n=json.JSONDecoder().raw_decode(s[start:]);s=s[:start]+'<noded strokes>'+s[start+n:]
 s=re.sub(r'"sha256": "[^"]+"','"sha256": "<digest>"',s)
 return re.sub(r'"bytes": \d+','"bytes": <length>',s)
assert mask(old)==mask(new),'undeclared byte change'
pathlib.Path('reviews/evidence/M2-C2.2-r1/golden-byte-check.txt').write_text('PASS: every byte identical after masking only sha256, bytes, and drawing1/nodes/7/children/0/strokes (3 -> 48). Layer '+nid+'\n')
