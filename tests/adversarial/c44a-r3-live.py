"""Round 3 independent real-window probes. Secrets and raw pixels stay in TEMP."""
import asyncio,base64,ctypes,importlib.util,io,json,sys,time
from pathlib import Path
s=importlib.util.spec_from_file_location('r2',Path(__file__).with_name('c44a-r2-live.py'));r2=importlib.util.module_from_spec(s);s.loader.exec_module(r2)
r1=r2.r1
# Observe every SSE object consumed by the independent transport; never persist payloads.
import inspect,textwrap,threading,urllib.request
scope={};exec(textwrap.dedent(inspect.getsource(r1.IndependentHttp.rpc)).replace("obj.get('id')==self.seq","obj.get('id')==body['id']").replace("obj=json.loads(line[5:])","obj=json.loads(line[5:]);self.received.append(obj)"),r1.__dict__,scope)
class Client(r1.Live4Http):
 rpc=scope['rpc']
 def __init__(self,*a):
  super().__init__(*a);self.received=[];self.stopping=threading.Event();self.reader=None
 def start_events(self):
  def read():
   try:
    req=urllib.request.Request(self.endpoint,headers={'Authorization':'Bearer '+self.token,'Mcp-Session-Id':self.sid,'Accept':'text/event-stream'})
    with urllib.request.urlopen(req,timeout=2) as response:
     for line in response:
      if self.stopping.is_set():break
      if line.startswith(b'data:'):self.received.append(json.loads(line[5:]))
   except Exception as ex:self.stream_error=type(ex).__name__
  self.reader=threading.Thread(target=read,daemon=True);self.reader.start()
 def close(self):
  self.stopping.set();super().close()
  if self.reader:self.reader.join(3)
async def body(t,step):
 from PIL import Image,ImageGrab
 from ctypes import wintypes
 d=r1.d;c=t.cmd;t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await r1.r3.modal_fixture(t,'Round 3 independent schedules')
 await c('project.saveAs',{'path':str(t.work/'base.animpkg')})
 clients=[];tasks=[]
 async def fresh():
  q=Client(json.loads((t.work/'discovery.json').read_text()),'round3');clients.append(q);await asyncio.to_thread(q.initialize);q.start_events();return q
 async def call(q,n,p=None):return await asyncio.to_thread(q.command,n,p)
 async def prefs():
  if r2.dom(t,'agentAccess.token') is None:
   d.choose_menu(['File','Preferences']);await r2.pause();t.click('preferences.category.agent');await r2.pause()
 async def show(settle=True):
  await prefs()
  if r2.dom(t,'agentAccess.token')!=q.token:
   t.click('agentAccess.token.show')
   if settle:await asyncio.sleep(.12)
 def crop(im,rect,origin=(0,0)):
  scale=t.cdp.js('devicePixelRatio');return im.convert('RGB').crop((origin[0]+round(rect['x']*scale)+4,origin[1]+round(rect['y']*scale)+4,origin[0]+round(rect['right']*scale)-4,origin[1]+round(rect['bottom']*scale)-4))
 def png(x):
  if isinstance(x,dict):
   if isinstance(x.get('png'),str):return x['png']
   for v in x.values():
    z=png(v)
    if z:return z
  if isinstance(x,list):
   for v in x:
    z=png(v)
    if z:return z
 counts={'masked':0,'refused':0,'revealed':0,'other':0};person=[]
 async def capture(target='window',client=None,label='capture'):
  rect=r2.dom(t,'agentAccess.token','e.getBoundingClientRect().toJSON()')
  a=await call(client or q,'inspect.screenshot',{'target':target});data=png(a)
  if not data:
   safe=a.get('error',{}).get('code')=='render.failed' and a.get('error',{}).get('details',{}).get('reason')=='tokenShown'
   kind='refused' if safe else 'other';detail={k:v for k,v in a.items() if k!='result'}
  else:
   im=Image.open(io.BytesIO(base64.b64decode(data)));field=crop(im,rect) if rect and target=='window' else None
   kind=('masked' if field.tobytes()==masked else 'revealed' if field.tobytes()==revealed else 'other') if field else 'masked'
   detail={'target':target,'fieldPresent':field is not None,'size':im.size}
  counts[kind]+=1
  # Never record raw responses: errors are checked in memory, with equality only.
  t.run.check(kind in ('masked','refused') and not leaks(a) if not data else kind in ('masked','refused'),'D-62/D-65 '+label,{'kind':kind,'target':target,'fieldPresent':bool(rect),'errorTokenFree':not leaks(a)})
  person.append({'target':target,'shownAfter':r2.dom(t,'agentAccess.token')==q.token,'buttonAfter':r2.dom(t,'agentAccess.token.show','e.textContent')})
  return a
 def leaks(x):
  text=json.dumps(x,default=str);return any(q.token[i:i+5] in text for i in range(len(q.token)-4))
 try:
  q=await fresh();b=await fresh()
  if step==7:
   await prefs();await t.do(('select','agentAccess.policy','assist'));t.click('preferences.submit');await r2.pause()
   async def hold(client):
    path=t.work/('held-'+str(len(tasks))+'.animpkg');path.write_bytes(b'sentinel');task=asyncio.create_task(call(client,'project.saveAs',{'path':str(path),'onExists':'overwrite'}));tasks.append(task);await asyncio.sleep(.2);return task
   async def decide(held):
    for i in range(4):
     t.click('confirmSheet.accept' if i<2 else 'confirmSheet.decline');await asyncio.sleep(.25)
    answers=await asyncio.gather(*held);t.run.check(sum(a.get('status')=='ok' for a in answers)==2 and sum('confirmation.declined' in json.dumps(a) for a in answers)==2,'D-58 two accept two decline')
   for cycle in range(11):
    held=[await hold(q) for _ in range(4)]
    t.run.check(all(not a.done() for a in held) and '1 of 4' in (r2.dom(t,'confirmSheet.queue','e.textContent') or ''),'D-65 four slots cycle '+str(cycle))
    fifth=await hold(q);await asyncio.sleep(.25)
    read=await call(q,'app.state.get');mutation=await call(q,'tool.active.set',{'toolId':'brush'})
    answer=fifth.result() if fifth.done() else {};status=await call(b,'agent.server.status')
    t.run.check(fifth.done() and 'limit.exceeded' in json.dumps(answer) and len(status['result']['pendingConfirmations'])==4 and read.get('status')=='ok' and mutation.get('status')=='ok','D-65 fifth refused/read/mutation cycle '+str(cycle),{'fifth':answer,'pending':len(status['result']['pendingConfirmations'])})
    await decide(held)
   held=[await hold(q) for _ in range(4)];await asyncio.to_thread(q.close);await asyncio.gather(*held);q=await fresh();held=[await hold(q) for _ in range(4)]
   t.run.check(all(not a.done() for a in held) and '1 of 4' in (r2.dom(t,'confirmSheet.queue','e.textContent') or ''),'D-65 reconnect four holds');await decide(held)
   job=await c('render.range.run',{'from':1,'to':100000,'width':32,'height':32,'directory':str(t.work/'long-render'),'createDirs':True})
   jid=job.get('jobId');t.run.check(bool(jid),'long job created')
   waits=[asyncio.create_task(asyncio.to_thread(q.tool,'job_wait',{'jobId':jid,'timeoutMs':60000})) for _ in range(4)];tasks.extend(waits);await asyncio.sleep(.25)
   fifth=await hold(q);await asyncio.sleep(.25)
   t.run.save('shared-slots-observation.json',{'fourWaitsPending':all(not w.done() for w in waits),'confirmable':fifth.result() if fifth.done() else 'held'})
   await c('job.cancel',{'jobId':jid});await asyncio.gather(*waits)
   if not fifth.done():t.click('confirmSheet.decline')
   await fifth
   return
  await c('window.panel.set',{'panelId':'cameraView','visible':True})
  main_handle=d.window_handle();d.window_handle=lambda:main_handle
  await c('window.panel.set',{'panelId':'pen','visible':True,'floating':True});await r2.pause();await prefs()
  rect=r2.dom(t,'agentAccess.token','e.getBoundingClientRect().toJSON()');a=await call(q,'inspect.screenshot',{'target':'window'});masked=crop(Image.open(io.BytesIO(base64.b64decode(png(a)))),rect).tobytes()
  await show();origin=wintypes.POINT();d.u32.ClientToScreen(d.front(),ctypes.byref(origin));revealed=crop(ImageGrab.grab(),r2.dom(t,'agentAccess.token','e.getBoundingClientRect().toJSON()'),(origin.x,origin.y)).tobytes()
  t.run.check(masked!=revealed and r2.dom(t,'agentAccess.token')==q.token,'in-memory references distinguish token')
  if step==4:
   for target in ('window','drawingView','cameraView','pen'):
    for i in range(30):
     await show(False);await asyncio.sleep((i%11)*.005);await capture(target,label='shown '+str(i))
  elif step==5:
   for trigger in ('copy','regenerate','background','ok','cancel','escape','float','simultaneous'):
    for i in range(20):
     await show()
     if trigger=='copy':t.click('agentAccess.token.copy')
     elif trigger=='regenerate':
      t.click('agentAccess.regenerate');await r2.pause();buttons=t.cdp.js("[...document.querySelectorAll('button')].filter(e=>e.textContent==='Regenerate').map(e=>e.dataset.ttId)");t.click(buttons[0]);await r2.pause()
      for client in clients:await asyncio.to_thread(client.close)
      q=await fresh();b=await fresh()
     elif trigger=='background':d.key(0x09,0x12)
     elif trigger=='ok':t.click('preferences.submit')
     elif trigger=='cancel':t.click('preferences.cancel')
     elif trigger=='escape':d.key(0x1B)
     if trigger=='simultaneous':await asyncio.gather(capture(client=q,label=trigger),capture(client=b,label=trigger))
     else:await capture('pen' if trigger=='float' else 'window',label=trigger)
     if trigger=='background':d.key(0x09,0x12);await r2.pause()
  elif step==6:
   await show();tree=await asyncio.to_thread(q.tool,'inspect_ui',{'target':'window','depth':64});activity=await call(q,'agent.activity.list',{'limit':500});export=t.work/'export.jsonl'
   t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command',{json:JSON.stringify({command:'agent.activity.export',params:"+json.dumps({'path':str(export),'onExists':'overwrite'})+"})}).then(()=>true)")
   await r2.pause();files=[f for f in (t.work/'user').rglob('*') if f.is_file() and f.suffix in ('.log','.jsonl')]
   t.run.check(not leaks(tree),'D-62 complete inspect_ui prefix-free');t.run.check(not leaks(activity) and export.exists() and not leaks(export.read_text()),'D-62 activity/export prefix-free')
   t.run.check(not any(leaks(f.read_text(errors='replace')) for f in files),'D-62 logs prefix-free',{'files':len(files)})
   for i in range(20):await show(False);await capture(label='error prefix audit')
  if step==6:t.run.check(not any(leaks(client.received) for client in clients),'D-62 received events prefix-free',{'objects':sum(len(client.received) for client in clients)})
  t.run.save('schedule-summary.json',{'counts':counts,'personAfter':person})
 finally:
  for _ in range(8):
   if r2.dom(t,'confirmSheet','true'):t.click('confirmSheet.decline');await asyncio.sleep(.1)
  for client in clients:await asyncio.to_thread(client.close)
  for task in tasks:
   if not task.done():task.cancel()
if __name__=='__main__':
 case,step=sys.argv[1],int(sys.argv[2]);assert case in ('n1a','n1b')
 if case=='n1a':
  import subprocess,os,tempfile
  if step==1:
   commands=[(['pnpm.cmd','install','--frozen-lockfile'],r1.ROOT/'desktop'),(['pnpm.cmd','build'],r1.ROOT/'desktop'),(['cargo','build','--release','--locked','-p','tt-desktop','-p','tt-headless'],r1.ROOT)]
  else:
   pairs=([('m2a',i) for i in (1,2)]+[('m2a',3)]*10+[('m2a',4)]+[('m2b',i) for i in (5,6,7)]) if step==2 else [('l3a',i) for i in range(1,5)]+[('l3b',i) for i in range(5,8)]+[('l4a',i) for i in range(1,6)]
   commands=[([sys.executable,str(Path(__file__).with_name('c44a-r2-live.py' if k.startswith('m') else 'c44a-r1-live.py')),k,str(i)],r1.ROOT) for k,i in pairs]
  failed=False
  for args,cwd in commands:
   result=__import__('subprocess').run(args,cwd=cwd,timeout=530);failed|=bool(result.returncode)
  raise SystemExit(int(failed))
 assert step in (4,5,6,7)
 r1.EVID=r1.ROOT/'reviews/evidence/M4-C4.4a-r3-review';r1.live_body=body;sys.exit(r1.live(step))
