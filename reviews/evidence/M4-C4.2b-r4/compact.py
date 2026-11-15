"""Keep closure receipts small; retain hashes of verbose source receipts."""
import hashlib,json,struct
from pathlib import Path
O=Path(__file__).resolve().parent
keys={'case','name','tool','verdict','input','sameCommands','sameContent','sameLabels','labels','statuses','expectedLabel','undoRedo','historyDelta','unchanged','eventsEqual','drawingEqual','selectionEqual','contentEqualModuloIds','comparison','results','ms','code','reason','response','error','afterRelease','changed'}
def slim(v):
    if isinstance(v,dict):
        return {k:slim(x) for k,x in v.items() if k in keys or k=='observed'}
    if isinstance(v,list):
        return [slim(x) for x in v] if len(v)<100 else {'count':len(v)}
    return v
receipts=[]
for p in O.rglob('*.json'):
    if p.stat().st_size>100000:
        raw=p.read_bytes(); data=json.loads(raw)
        if p.name=='c2-escape.json': data={'unchanged':data['unchanged']}
        else: data=slim(data)
        receipts.append({'path':str(p.relative_to(O)).replace('\\','/'),'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()})
        p.write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8',newline='\n')
if receipts:
    (O/'verbose-receipt-hashes.json').write_text(json.dumps(receipts,indent=2)+'\n',encoding='utf-8',newline='\n')
for p in O.rglob('*'):
    if p.is_file() and p.suffix in {'.txt','.json','.py','.exit','.log'}:
        try: text=p.read_text(encoding='utf-8')
        except UnicodeDecodeError: text=p.read_text(encoding='cp1252')
        p.write_text(text,encoding='utf-8',newline='\n')
    if p.suffix=='.png':
        raw=p.read_bytes(); out=raw[:8]; i=8
        while i<len(raw):
            n=struct.unpack('>I',raw[i:i+4])[0]; kind=raw[i+4:i+8]
            if kind in (b'IHDR',b'PLTE',b'IDAT',b'IEND'): out+=raw[i:i+n+12]
            i+=n+12
        assert len(out)<1000000
        p.write_bytes(out)
