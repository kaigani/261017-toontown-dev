import json,hashlib,sys,math,importlib.util
from pathlib import Path
from PIL import Image
root=Path.cwd(); e=root/'reviews/evidence/M3-C3.4-r1'
spec=importlib.util.spec_from_file_location('oracle',root/'tests/adversarial/c34-r1-numeric.py');o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)
results={}
def files(d): return {str(p.relative_to(d)).replace('\\','/'):p for p in d.rglob('*') if p.is_file() and p.name!='report.json' and 'projects' not in p.relative_to(d).parts and not p.name.endswith('.manifest.json')}
dirs=[p for p in [e/'ci-Windows',e/'ci-macOS',e/'local-a',e/'local-b'] if p.exists()]
base=files(dirs[0]); results['comparedFiles']=sorted(base)
assert len(base)==36
for d in dirs:
 f=files(d); assert f.keys()==base.keys(); assert all(p.read_bytes()==f[n].read_bytes() for n,p in base.items()); results[d.name]={'bytesEqual':True,'numericError':o.audit(d)}
 m=json.loads((d/'manifest.normalized.json').read_text()); assert m['frameCount']==24 and m['fps']==24 and m['durationSec']==1 and m['range']=={'from':1,'to':24,'step':1}
 worst=0
 for i,item in enumerate(m['files'],1):
  p=d/'sequence'/f'Scene-1_{i:04d}.png';b=p.read_bytes();assert item['sha256']==hashlib.sha256(b).hexdigest() and item['bytes']==len(b) and item['frame']==i
  im=Image.open(p).convert('RGB');assert im.size==(1920,1080)
  # Red ball over white background: green-channel coverage; independent decoder and integer moment accumulation.
  sx=sy=total=0
  for y in range(im.height):
   for x in range(im.width):
    w=255-im.getpixel((x,y))[1]
    total+=w;sx+=w*(2*x+1);sy+=w*(2*y+1)
  cx,cy=sx/(2*total),sy/(2*total)
  worst=max(worst,abs(cx-960),abs(cy-(540-160*o.position(i))))
 assert worst<0.001
 results[d.name]['centroidErrorPx']=worst
 sheet=Image.open(d/'contact-sheet.png'); meta=json.loads((d/'contact-sheet.json').read_text());assert sheet.size==(1224,769) and len(meta['cells'])==24
 labels=[]
 for i,c in enumerate(meta['cells']):
  assert c=={'frame':i+1,'x':4+(i%5)*244,'y':4+(i//5)*153,'w':240,'h':135}
  labels.append(sheet.crop((c['x'],c['y']+135,c['x']+240,c['y']+149)).tobytes())
 assert len(set(labels))==24
 results[d.name]['labelsDistinct']=24
(e/'independent-artifacts.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
