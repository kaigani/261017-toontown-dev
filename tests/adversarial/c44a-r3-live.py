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
   while not self.stopping.is_set():
    try:
     req=urllib.request.Request(self.endpoint,headers={'Authorization':'Bearer '+self.token,'Mcp-Session-Id':self.sid,'Accept':'text/event-stream'})
     with urllib.request.urlopen(req,timeout=2) as response:
      for line in response:
       if self.stopping.is_set():break
       if line.startswith(b'data:'):self.received.append(json.loads(line[5:]))
    except Exception as ex:
     self.stream_error=type(ex).__name__;self.stopping.wait(.1)
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
   for client in clients:await asyncio.to_thread(client.close)
   q=await fresh();b=await fresh()
   async def hold(client):
    path=t.work/('held-'+str(len(tasks))+'.animpkg');path.write_bytes(b'sentinel');task=asyncio.create_task(call(client,'project.saveAs',{'path':str(path),'onExists':'overwrite'}));tasks.append(task);await asyncio.sleep(.2);return task
   async def decide(held):
    for i in range(4):
     t.click('confirmSheet.accept' if i<2 else 'confirmSheet.decline');await asyncio.sleep(.25)
    answers=await asyncio.gather(*held);t.run.check(sum(a.get('status')=='ok' for a in answers)==2 and sum('confirmation.declined' in json.dumps(a) for a in answers)==2,'D-58 two accept two decline')
   if not __import__('os').environ.get('C44_SHARED_ONLY'):
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
   job=(await call(q,'render.range.run',{'from':1,'to':100000,'width':32,'height':32,'directory':str(t.work/'long-render'),'createDirs':True})).get('result',{})
   jid=job.get('jobId');t.run.check(bool(jid),'long job created')
   waits=[asyncio.create_task(asyncio.to_thread(q.tool,'job_wait',{'jobId':jid,'timeoutMs':60000})) for _ in range(4)];tasks.extend(waits);await asyncio.sleep(.25)
   fifth=await hold(q);await asyncio.sleep(.25)
   t.run.save('shared-slots-observation.json',{'fourWaitsPending':all(not w.done() for w in waits),'waitAnswers':[w.result() if w.done() else 'pending' for w in waits],'confirmable':fifth.result() if fifth.done() else 'held'})
   await call(q,'job.cancel',{'jobId':jid});await asyncio.gather(*waits)
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
  elif step==8:
   totals={}
   def fastclick(ident):
    r=t.rect(ident);o=wintypes.POINT();d.u32.ClientToScreen(d.front(),ctypes.byref(o));scale=t.cdp.js('devicePixelRatio');d.u32.SetCursorPos(int(o.x+r['x']*scale),int(o.y+r['y']*scale));d._send_mouse(0x0002);d._send_mouse(0x0004)
   for trigger in ('category','ok','cancel','escape'):
    counts.update(masked=0,refused=0,revealed=0,other=0)
    await prefs();safeRect=r2.dom(t,'agentAccess.token','e.getBoundingClientRect().toJSON()')
    if r2.dom(t,'agentAccess.token')==q.token:t.click('agentAccess.token.show');await r2.pause()
    if trigger=='category':t.click('preferences.category.general')
    elif trigger=='ok':t.click('preferences.submit')
    elif trigger=='cancel':t.click('preferences.cancel')
    else:d.key(0x1B)
    await r2.pause();safeAnswer=await call(q,'inspect.screenshot',{'target':'window'});safePixels=crop(Image.open(io.BytesIO(base64.b64decode(png(safeAnswer)))),safeRect).tobytes()
    if trigger=='category':t.click('preferences.category.agent');await r2.pause()
    for i in range(40):
     await show();rect=r2.dom(t,'agentAccess.token','e.getBoundingClientRect().toJSON()')
     if trigger=='category':fastclick('preferences.category.general')
     elif trigger=='ok':fastclick('preferences.submit')
     elif trigger=='cancel':fastclick('preferences.cancel')
     else:d.u32.keybd_event(0x1B,0,0,0);d.u32.keybd_event(0x1B,0,2,0)
     actionAt=time.perf_counter()
     while time.perf_counter()-actionAt<(i%16)/1000:pass
     # Even when the DOM no longer contains the field, audit its just-measured old pixels.
     requestedDelayMs=(time.perf_counter()-actionAt)*1000
     a=await call(q,'inspect.screenshot',{'target':'window'});data=png(a)
     if data:
      field=crop(Image.open(io.BytesIO(base64.b64decode(data))),rect).tobytes()
      kind='revealed' if field==revealed else 'masked' if field in (masked,safePixels) else 'other'
     else:kind='refused' if a.get('error',{}).get('code')=='render.failed' and a.get('error',{}).get('details',{}).get('reason')=='tokenShown' else 'other'
     counts[kind]+=1;t.run.check(kind in ('masked','refused'),'D-65 leave '+trigger+' '+str(i),{'kind':kind,'requestedDelayMs':round(requestedDelayMs,2)})
     if trigger=='category':t.click('preferences.category.agent');await r2.pause()
    totals[trigger]=dict(counts)
   await show()
   reloads=[]
   for vk,mods in ((0x74,()),(0x52,(0x11,))):
    await show();t.click('agentAccess.token')
    before=t.cdp.js('performance.timeOrigin');d.key(vk,*mods);await r2.pause();after=t.cdp.js('performance.timeOrigin');reloads.append({'key':vk,'reloaded':before!=after})
   if any(x['reloaded'] for x in reloads):
    counts.update(masked=0,refused=0,revealed=0,other=0)
    for i in range(40):
     await show();t.click('agentAccess.token');rect=r2.dom(t,'agentAccess.token','e.getBoundingClientRect().toJSON()')
     d.u32.keybd_event(0x74,0,0,0);d.u32.keybd_event(0x74,0,2,0)
     a=await call(q,'inspect.screenshot',{'target':'window'});data=png(a)
     if data:
      field=crop(Image.open(io.BytesIO(base64.b64decode(data))),rect).tobytes();kind='revealed' if field==revealed else 'masked' if field in (masked,safePixels) else 'other'
     else:kind='refused' if a.get('status')=='error' and a.get('error',{}).get('code') in ('render.failed','ui.viewNotFound','command.timeout','command.cancelled') else 'other'
     counts[kind]+=1;t.run.check(kind in ('masked','refused'),'D-65 reload '+str(i),{'kind':kind});await r2.pause()
    totals['reload']=dict(counts)
   if r2.dom(t,'agentAccess.token')==q.token:t.click('agentAccess.token.show');await r2.pause()
   t.run.save('leave-token-summary.json' ,{'schedules':totals,'reloadKeys':reloads})
  if step==6:t.run.check(not any(leaks(client.received) for client in clients),'D-62 received events prefix-free',{'objects':sum(len(client.received) for client in clients)})
  t.run.save('schedule-summary.json',{'counts':counts,'personAfter':person})
 finally:
  for _ in range(8):
   if r2.dom(t,'confirmSheet','true'):t.click('confirmSheet.decline');await asyncio.sleep(.1)
  for client in clients:await asyncio.to_thread(client.close)
  for task in tasks:
   if not task.done():task.cancel()
async def menu_extra(t,step):
 d=r1.d;c=t.cmd;t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 from ctypes import wintypes
 await r1.r3.modal_fixture(t,'Round 3 menus')
 def resize(w,h):
  handle=d.front();a=wintypes.RECT();b=wintypes.RECT();d.u32.GetWindowRect(handle,ctypes.byref(a));d.u32.GetClientRect(handle,ctypes.byref(b));scale=t.cdp.js('devicePixelRatio');d.u32.MoveWindow(handle,30,30,int(w*scale)+a.right-a.left-b.right,int(h*scale)+a.bottom-a.top-b.bottom,True)
 def screen(x,y):
  o=wintypes.POINT();d.u32.ClientToScreen(d.front(),ctypes.byref(o));scale=t.cdp.js('devicePixelRatio');return o.x+x*scale,o.y+y*scale
 def geo():return t.cdp.js("""(()=>{const m=document.querySelector('.context-menu');if(!m)return null;const box=e=>e.getBoundingClientRect().toJSON();return {menu:box(m),list:box(m.querySelector('.context-menu-items')),arrows:[...m.querySelectorAll('.context-menu-arrow')].map(e=>({box:box(e)})),items:[...m.querySelectorAll('[role^=menuitem]')].map(e=>({id:e.dataset.ttId,box:box(e),command:e.dataset.ttCommand,params:JSON.parse(e.dataset.ttParams||'{}')})),area:{w:document.documentElement.clientWidth,h:document.documentElement.clientHeight}}})()""")
 async def openmenu(ident):
  d.key(0x1B);r=t.rect(ident);d.right_click_screen(*screen(r['x'],r['y']));await r2.pause();return geo()
 def inside(g):
  return g and g.get('menu') and all(r['x']>=0 and r['y']>=0 and r['right']<=g['area']['w'] and r['bottom']<=g['area']['h'] for r in [g['menu']]+[x['box'] for x in g['arrows']])
 resize(800,500);await r2.pause()
 for view in ('drawingView','cameraView') if step==5 else ():
  await c('window.panel.set',{'panelId':view,'visible':True});await c('view.active.set',{'viewId':view});await r2.pause()
  for method in ('click','hover','wheel','down','end'):
   await c('tool.active.set',{'toolId':'brush'});await c('draw.text.add',{'position':{'x':0,'y':0},'text':'Review'});await c('edit.selectAll');await r2.pause()
   g=await openmenu(view+'.canvas');t.run.check(bool(inside(g)),'D-66 outer/arrows '+view+' '+method,g)
   if not g:continue
   top=g
   if g['arrows']:
    arrow=g['arrows'][-1]['box'];x,y=screen(arrow['x']+arrow['width']/2,arrow['y']+arrow['height']/2)
    if method=='click':
     for _ in range(12):d.click_screen(x,y)
    elif method=='hover':d.u32.SetCursorPos(int(x),int(y));await asyncio.sleep(3)
    elif method=='wheel':
     b=g['list'];d.wheel_screen(*screen(b['x']+b['width']/2,b['y']+b['height']/2),20)
   if method=='down':
    for _ in range(len(g['items'])+1):
     if t.cdp.js('document.activeElement?.dataset.ttId')==g['items'][-1]['id']:break
     d.key(0x28)
   elif method=='end':d.key(0x23)
   await r2.pause();g=geo();last=g['items'][-1];b=last['box'];l=g['list'];visible=b['y']>=l['y'] and b['bottom']<=l['bottom']
   t.run.save(view+'-'+method+'.json',{'top':top,'bottom':g})
   t.run.check(visible,'D-66 last visible '+view+' '+method)
   t.clear()
   if method in ('down','end'):d.key(0x0D)
   else:d.click_screen(*screen(b['x']+b['width']/2,b['y']+b['height']/2))
   await r2.pause();commands=r2.sent(t)
   t.run.check(sum(x['command']==last['command'] and x.get('params',{})==last['params'] for x in commands)==1,'D-66 declared command once '+view+' '+method,{'declared':last,'commands':commands})
   await t.settle_dialogs()
  for vk,mods in ((0x79,(0x10,)),(0x5D,())):
   t.click(view+'.canvas')
   for _ in range(100):
    if t.cdp.js('document.activeElement?.dataset.ttId')==view+'.canvas':break
    d.key(0x09)
   d.key(vk,*mods);await r2.pause();t.run.check(bool(geo()),'D-66 keyboard menu '+view+' '+str(vk));d.key(0x1B)
 if step==6:
  g=await openmenu('tab.drawingView');t.run.check(bool(inside(g)),'D-66 dock checklist',g)
  if g:
   b=g['list'];d.wheel_screen(*screen(b['x']+b['width']/2,b['y']+b['height']/2),20);d.key(0x23);await r2.pause();bottom=geo();last=bottom['items'][-1]['box'];t.run.check(last['bottom']<=bottom['list']['bottom'],'D-66 checklist scroll',bottom)
  d.key(0x1B)
  await c('window.panel.set',{'panelId':'cameraView','visible':True});await c('view.active.set',{'viewId':'cameraView'});await c('anim.tool.set',{'tool':'transform'});await c('anim.tool.set',{'tool':'select'});await r2.pause()
  g=await openmenu('cameraView.canvas');t.run.check(bool(inside(g)),'D-66 Camera scene menu',g);d.key(0x1B)
  await c('view.active.set',{'viewId':'drawingView'});await c('tool.active.set',{'toolId':'brush'});await r2.pause()

  resize(1280,800);await openmenu('drawingView.canvas');resize(800,500);await r2.pause();g=geo();t.run.check(bool(inside(g)),'D-66 resize open',g)
  d.key(0x1B);main=d.window_handle();original=d.window_handle;maincdp=t.cdp
  await c('window.panel.set',{'panelId':'drawingView','visible':True,'floating':True});await r2.pause()
  handles=[]
  callback=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
  def visit(h,_):
   pid=wintypes.DWORD();d.u32.GetWindowThreadProcessId(h,ctypes.byref(pid))
   if pid.value==d.e2e.APP_PID['pid'] and h!=main and d.u32.IsWindowVisible(h):handles.append(h)
   return True
  d.u32.EnumWindows(callback(visit),0);assert handles,'no floating window'
  init=textwrap.dedent(inspect.getsource(d.Cdp.__init__)).replace('"float" in (p.get("url") or "")','"float" not in (p.get("url") or "")')
  lines=init.splitlines()
  for i,line in enumerate(lines):
   if 'if self.js(' in line:lines[i]="        if self.js('!!document.querySelector(\"canvas\")'):"
  namespace={};exec('\n'.join(lines),d.__dict__,namespace)
  FloatCdp=type('FloatCdp',(d.Cdp,),{'__init__':namespace['__init__']})
  try:
   d.window_handle=lambda:handles[0];d.front();t.cdp=FloatCdp(d.CDP_PORT);t.cdp.record()
   for size in ((640,480),(160,120)):
    resize(*size);await r2.pause();g=await openmenu('drawingView.canvas');t.run.check(bool(inside(g)),'D-66 float '+str(size),g)
    if g:
     top=g;b=g['list'];d.wheel_screen(*screen(b['x']+b['width']/2,b['y']+b['height']/2),30);await r2.pause();bottom=geo();last=bottom['items'][-1]['box'];t.run.check(last['bottom']<=bottom['list']['bottom'],'D-66 float wheel '+str(size),bottom)
     d.key(0x24);await r2.pause();g=geo()
     if g['arrows']:
      b=g['arrows'][-1]['box'];x,y=screen(b['x']+b['width']/2,b['y']+b['height']/2);d.u32.SetCursorPos(int(x),int(y));await asyncio.sleep(5)
     bottom=geo();last=bottom['items'][-1]['box'];t.run.check(last['bottom']<=bottom['list']['bottom'],'D-66 float hover '+str(size),bottom)
     t.run.save('float-'+str(size[0])+'.json',{'top':top,'bottom':bottom});d.key(0x1B)
  finally:
   t.cdp=maincdp;d.window_handle=original;d.front()


async def fields(t,step):
 d=r1.d;c=t.cmd;t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await r1.r3.modal_fixture(t,'Round 3 text keys')
 await c('project.saveAs',{'path':str(t.work/'base.animpkg')})
 await c('draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':10,'y':10}})
 await c('edit.selectAll')
 async def openfield(kind):
  d.key(0x1B);await asyncio.sleep(.2)
  if kind in ('pen','rename','font'):
   await c('window.panel.set',{'panelId':'text' if kind=='font' else 'pen','visible':True});await r2.pause()
   if kind=='rename':
    row=t.cdp.js('[...document.querySelectorAll("[data-tt-id]")].find(e=>e.dataset.ttId.startsWith("pen.row."))?.dataset.ttId')
    assert row, 'pen row absent'
    t.click(row);d.key(0x71);await r2.pause();return 'pen.rename'
   if kind=='font':return t.cdp.js('[...document.querySelectorAll(".text-panel input")].find(e=>e.inputMode!=="decimal")?.dataset.ttId')
   return 'pen.maxSize.field'
  if kind in ('search','pref','keymap'):
   d.choose_menu(['File','Preferences']);await r2.pause()
   if kind=='keymap':t.click('preferences.category.keyboard');await r2.pause()
   return {'search':'preferences.search','pref':'pref.general.recentProjects.maxCount','keymap':'keymap.filter'}[kind]
  if kind=='toolbar':d.choose_menu(['Window','Customize Toolbar']);await r2.pause();return 'toolbarSheet.filter'
  if kind=='new':
   await r1.r3.preserve_fixture(t);d.choose_menu(['File','New Project']);await r2.pause();return 'newProject.name'
  d.choose_menu(['File','Project Properties']);await r2.pause();return 'projectProperties.frameRate'
 for kind in ('pen','rename','font','search','pref','keymap','new','props','toolbar'):
  await c('draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':10,'y':10}});await c('edit.selectAll')
  ident=await openfield(kind);await r1.r3.wait(t,ident)
  inventory=t.cdp.js('[...document.querySelectorAll("input,textarea,[contenteditable=true]")].map(e=>({id:e.dataset.ttId,type:e.type,disabled:e.disabled}))')
  t.run.save('inventory-'+kind+'.json',inventory)
  await c('edit.selectAll');await c('edit.copy');docclip=await c('edit.clipboard.get')
  t.click(ident);old=r2.dom(t,ident);pos=await t.position();tool=await c('tool.active.get');t.clear()
  if step==2:
   d.key(0x41,0x11);selection=r2.dom(t,ident,'({start:e.selectionStart,end:e.selectionEnd,value:e.value})')
   t.run.check(selection['start']==0 and selection['end']==len(old),'D-66 select field '+kind,selection)
   d.type_text('alpha beta');d.key(0x41,0x11);d.key(0x43,0x11)
   clip=__import__('subprocess').check_output(['powershell','-NoProfile','-Command','Get-Clipboard -Raw']).decode().strip()
   d.key(0x58,0x11);empty=r2.dom(t,ident);d.key(0x56,0x11);pasted=r2.dom(t,ident)
   d.key(0x5A,0x11);undo=r2.dom(t,ident);d.key(0x59,0x11);redo=r2.dom(t,ident)
   t.run.check(clip=='alpha beta' and empty=='' and pasted=='alpha beta','D-66 clipboard '+kind,{'clipboard':clip,'cut':empty,'paste':pasted})
   t.run.check(undo=='' and redo=='alpha beta','D-66 field undo redo '+kind,{'undo':undo,'redo':redo})
   for vk,mods in [(0x25,(0x11,)),(0x27,(0x11,)),(0x25,(0x10,)),(0x24,()),(0x23,()),(0x2E,()),(0x08,()),(0x08,(0x11,))]:d.key(vk,*mods)
   d.key(0x42);d.key(0x20)
   commands=t.recorded();t.run.check(not r2.sent(t) and await t.position()==pos and await c('tool.active.get')==tool and await c('edit.clipboard.get')==docclip,'D-66 text ownership '+kind,{'commands':commands,'undo':undo,'redo':redo,'value':r2.dom(t,ident)})
  elif step==3:
   d.key(0x53,0x11);await r2.pause();commands=t.recorded()
   modal=t.cdp.js('!!document.activeElement.closest("[role=dialog]")')
   t.run.check(any(x['command']=='project.save' for x in commands) or modal,'D-66 Save keymap/modal '+kind,commands)
   d.key(0x1B);await r2.pause()
   if t.cdp.js('!!document.querySelector("[role=dialog]")'):d.key(0x1B);await r2.pause()
   await c('tool.active.set',{'toolId':'pencil'});t.click('drawingView.canvas')
   clickFocus=t.cdp.js('document.activeElement?.dataset.ttId')
   for _ in range(100):
    if t.cdp.js('document.activeElement?.dataset.ttId')=='drawingView.canvas':break
    d.key(0x09)
   t.run.save('focus-exit-'+kind+'.json',{'afterClick':clickFocus,'afterTab':t.cdp.js('document.activeElement?.dataset.ttId')})
   canvaspos=await t.position();t.clear();d.key(0x41,0x11);d.key(0x5A,0x11);d.key(0x42);await r2.pause()
   cmds=[x['command'] for x in r2.sent(t)]
   t.run.check(all(x in cmds for x in ('edit.selectAll','edit.undo','tool.group.select')) and await t.position()==canvaspos-1 and (await c('tool.active.get')).get('toolId')!='pencil','D-66 document resumes '+kind,cmds)
  else:
   d.key(0x41,0x11);d.type_text('7' if kind in ('pen','pref','props') else 'Review name');d.key(0x1B);await r2.pause()
   t.run.save('escape-'+kind+'.json',{'old':old,'after':r2.dom(t,ident),'commands':t.recorded(),'dialog':t.cdp.js('document.querySelector("[role=dialog]")?.dataset.ttId')})
   # Numeric and rename fields have an explicit restoration contract; staged forms use their modal Cancel.
   if kind in ('pen','pref','props','rename'):t.run.check(not r2.sent(t) and (r2.dom(t,ident)==old or kind in ('rename','pref','props') and r2.dom(t,ident) is None),'D-61 Escape '+kind,{'after':r2.dom(t,ident),'commands':t.recorded()})
   ident=await openfield(kind);await r1.r3.wait(t,ident);t.click(ident);t.clear();d.key(0x41,0x11);d.type_text('8' if kind in ('pen','pref','props') else 'Review name');d.key(0x0D);await r2.pause()
   if r2.dom(t,ident) is not None:d.key(0x09);await r2.pause()
   t.run.save('enter-'+kind+'.json',{'after':r2.dom(t,ident),'commands':t.recorded()})
   cmds=r2.sent(t);t.run.check(len(cmds)<=1,'D-61 Enter once '+kind,cmds)
  d.key(0x1B);await r2.pause()
  if step==4 and kind=='new':await r1.r3.modal_fixture(t,'After staged form')
 await c('window.panel.set',{'panelId':'activity','visible':True});await r2.pause()
 t.run.save('activity-fields.json',t.cdp.js('[...document.querySelectorAll(".activity-panel input")].map(e=>({id:e.dataset.ttId,type:e.type}))'))

if __name__=='__main__' and sys.argv[1] in ('n2a','n2b'):
 case,step=sys.argv[1],int(sys.argv[2])
 r1.EVID=r1.ROOT/'reviews/evidence/M4-C4.4a-r3-review'
 if case=='n2a' and step==1:
  import subprocess,os,tempfile
  results=[]
  for i in (2,3,4):
   raw=Path(tempfile.mkdtemp(prefix='c44-r3-earlier-'))
   result=subprocess.run([sys.executable,str(Path(__file__).with_name('c44a-r2-live.py')),'m1a',str(i)],env={**os.environ,'REVIEW_RAW':str(raw)},timeout=530)
   results.append({'step':i,'exit':result.returncode,'raw':str(raw)})
   subprocess.run(['git','checkout','--','reviews/evidence/M4-C4.4a-r2-review'],check=True)
  (r1.EVID/'n2a-1.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8',newline='\n')
  sys.exit(int(any(x['exit'] for x in results)))
 if case=='n2b' and step==5 and not __import__('os').environ.get('C44_N2_EXTRA_ONLY'):
  import subprocess,os,tempfile
  earlier=[]
  for i in (5,6,7):
   raw=Path(tempfile.mkdtemp(prefix='c44-r3-earlier-'))
   result=subprocess.run([sys.executable,str(Path(__file__).with_name('c44a-r2-live.py')),'m1b',str(i)],env={**os.environ,'REVIEW_RAW':str(raw)},timeout=530)
   earlier.append({'step':i,'exit':result.returncode,'raw':str(raw)})
   subprocess.run(['git','checkout','--','reviews/evidence/M4-C4.4a-r2-review'],check=True)
  (r1.EVID/'n2b-5-earlier.json').write_text(json.dumps(earlier,indent=2)+'\n',encoding='utf-8',newline='\n')
 r1.live_body=fields if case=='n2a' else menu_extra if step in (5,6) else body
 logical=step
 if case=='n2b' and step==7:step=8
 old_receipt=r1.EVID/('step'+str(step)+'.json');saved=old_receipt.read_bytes() if old_receipt.exists() else None
 result=r1.live(step)
 if case=='n2b' and logical==5 and not __import__('os').environ.get('C44_N2_EXTRA_ONLY'):result|=int(any(x['exit'] for x in earlier))
 if saved is not None:old_receipt.write_bytes(saved)
 else:old_receipt.unlink(missing_ok=True)
 transcript=json.loads((r1.OUT/'transcript.json').read_text())
 receipt={'checks':len(transcript),'results':transcript,'raw':str(r1.OUT)}
 text=json.dumps(receipt,indent=2,default=str)+'\n'
 assert len(text.encode())<=200000,'summary too large'
 (r1.EVID/(case+'-'+str(logical)+'.json')).write_text(text,encoding='utf-8',newline='\n')
 patterns=('inventory-*.json','focus-exit-*.json','escape-*.json','enter-*.json','activity-fields.json','leave-token-summary.json','drawingView-*.json','cameraView-*.json','float-*.json')
 for pattern in patterns:
  for source in r1.OUT.glob(pattern):
   data=source.read_bytes();assert len(data)<=200000
   (r1.EVID/(case+'-'+str(logical)+'-'+source.name)).write_text(data.decode('utf-8').replace('\r\n','\n'),encoding='utf-8',newline='\n')
 sys.exit(result)

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
