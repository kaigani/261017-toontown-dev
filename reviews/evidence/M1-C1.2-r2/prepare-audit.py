"""Deterministic independent sampling and schema-leaf inventory, no writes to spec/product."""
import collections
import json
from pathlib import Path
import random
import re

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
SEED=0xc12220261019
rows=json.loads((HERE/'registry.json').read_text(encoding='utf-8'))
def holes(s,at=''):
    out=[]
    if not isinstance(s,dict): return out
    if not set(s).intersection({'type','$ref','enum','const','oneOf','anyOf','allOf','not','required'}):
        out.append({'pointer':at or '/', 'schema':s})
    if 'x-specType' in s: out.append({'pointer':at or '/', 'schema':s})
    for k in ('properties','$defs'):
        for n,v in s.get(k,{}).items(): out+=holes(v,at+'/'+k+'/'+n)
    if isinstance(s.get('items'),dict): out+=holes(s['items'],at+'/items')
    for k in ('oneOf','anyOf','allOf'):
        for i,v in enumerate(s.get(k,[])):out+=holes(v,at+'/'+k+'/'+str(i))
    if isinstance(s.get('x-jobResult'),dict): out+=holes(s['x-jobResult'],at+'/x-jobResult')
    return out
selected=sorted(random.Random(SEED).sample(sorted(r['id'] for r in rows),60))
prior=json.loads((HERE.parent/'M1-C1.2-r1'/'command-register.json').read_text(encoding='utf-8'))
spot=sorted(random.Random(SEED+1).sample(sorted(r['id'] for r in prior if r['status']=='implemented'),40))
for r in rows:
    r['parameterHoles']=holes(r['paramsSchema']);r['resultHoles']=holes(r['resultSchema'])
(HERE/'schema-holes.json').write_text(json.dumps([{'id':r['id'],'params':r['parameterHoles'],'result':r['resultHoles']} for r in rows],indent=2)+'\n',encoding='utf-8')
(HERE/'samples.json').write_text(json.dumps({'seed':hex(SEED),'population':796,'random60':selected,'previousImplemented40Seed':hex(SEED+1),'previousImplemented40':spot},indent=2)+'\n',encoding='utf-8')
for r in rows:
    if r['id'] in selected:
        print(r['id'], '| A:', r['spec']['params'], '| result:',r['spec']['result'])
        print(' META',r['needsProject'],r['scopes'],r['undoLabel'],r['enablement'])
        print(' HOLES',json.dumps({'p':r['parameterHoles'],'r':r['resultHoles']},ensure_ascii=False))
print('HOLE COUNTS',sum(bool(r['parameterHoles']) for r in rows),sum(bool(r['resultHoles']) for r in rows))
