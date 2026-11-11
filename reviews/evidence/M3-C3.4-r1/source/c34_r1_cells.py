import base64,io,json,subprocess,tempfile,re
from pathlib import Path
from PIL import Image
r=Path.cwd();e=r/'reviews/evidence/M3-C3.4-r1';work=Path(tempfile.mkdtemp(prefix='c34-r1-cells-'))
steps=json.loads((e/'local-a/e1-commands.json').read_text())['steps'];steps[0]['params']['path']=str(work/'fixture.animpkg')
assert not re.search(r'\b[a-z]{2,4}_[0-9A-HJKMNP-TV-Z]{26}\b',json.dumps(steps))
n=len(steps)
steps += [{'id':'render.frame.run','params':{'frame':f,'width':240}} for f in range(1,25)]
script=work/'batch.json';script.write_text(json.dumps({'label':'Independent cell parity','atomic':False,'steps':steps}))
log=work/'out.jsonl';p=subprocess.run([str(r/'target/release/toontown-headless.exe'),'--headless','--user-dir',str(work/'user'),'--allow-path',str(work),'--run',str(script),'--out',str(log)],capture_output=True,text=True)
assert p.returncode==0,p.stderr
sheet=Image.open(e/'local-a/contact-sheet.png').convert('RGBA');count=0
for line in log.read_text().splitlines():
 row=json.loads(line)
 if row.get('record')=='step' and row['index']>=n:
  i=row['index']-n;result=row['envelope']['result'];im=Image.open(io.BytesIO(base64.b64decode(result.get('data') or result.get('png')))).convert('RGBA')
  x=4+(i%5)*244;y=4+(i//5)*153
  assert im.size==(240,135) and im.tobytes()==sheet.crop((x,y,x+240,y+135)).tobytes();count+=1
assert count==24
(e/'independent-cell-parity.json').write_text(json.dumps({'cellsEqual':count,'rawIdsInRecordedScript':0,'work':str(work)},indent=2))
print('24 cells pixel-identical to independent CLI single-frame renders; no raw object IDs.')
