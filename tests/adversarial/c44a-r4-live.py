"""Scoped closure probes. Raw images and secrets never leave TEMP or memory."""
import asyncio, base64, ctypes, importlib.util, io, json, math, sys, time
from pathlib import Path
from ctypes import wintypes
s=importlib.util.spec_from_file_location('prior',Path(__file__).with_name('c44a-r3-live.py'))
prior=importlib.util.module_from_spec(s);s.loader.exec_module(prior)
r1=prior.r1;r2=prior.r2

def menu_items():
 from pywinauto import Desktop
 found=[]
 def one(h,_):
  if ctypes.windll.user32.IsWindowVisible(h):
   name=ctypes.create_unicode_buffer(256);ctypes.windll.user32.GetClassNameW(h,name,256)
   if name.value=='Chrome_WidgetWin_1':
    try:
     w=Desktop(backend='uia').window(handle=h).wrapper_object()
     if w.descendants(control_type='Menu'):
      found.extend(e.window_text() for e in w.descendants(control_type='MenuItem'))
    except Exception:pass
  return True
 ctypes.windll.user32.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(one),0)
 return found

def native_windows():
 u=ctypes.windll.user32;found={}
 def one(h,_):
  if u.IsWindowVisible(h):
   name=ctypes.create_unicode_buffer(256);title=ctypes.create_unicode_buffer(512);u.GetClassNameW(h,name,256);u.GetWindowTextW(h,title,512)
   found[int(h)]={'class':name.value,'title':title.value}
  return True
 u.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(one),0)
 return found

async def body(t,step):
 from PIL import Image
 d=r1.d;t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await r1.r3.modal_fixture(t,'Closure live fixture')
 await c('project.saveAs',{'path':str(t.work/'base.animpkg')})
 clients=[];tokens=[];answers=[];action_time=None;reveal_reference=None;cached_rect=None;cached_scale=1
 async def fresh():
  q=prior.Client(json.loads((t.work/'discovery.json').read_text()),'closure-independent')
  clients.append(q);tokens.append(q.token);await asyncio.to_thread(q.initialize);q.start_events();return q
 async def call(q,n,p=None):
  a=await asyncio.to_thread(q.command,n,p);answers.append(a);return a
 def dom(i,expr='e.value'):return r2.dom(t,i,expr)
 async def prefs():
  if dom('agentAccess.token') is None:
   d.key(0x1b);d.choose_menu(['File','Preferences'])
   for _ in range(1000):
    if dom('preferences.category.agent','e.textContent') is not None:break
    await asyncio.sleep(.01)
   quick_click('preferences.category.agent')
   for _ in range(1000):
    if dom('agentAccess.token') is not None:break
    await asyncio.sleep(.01)
   await asyncio.sleep(.03)
 async def show():
  await prefs()
  if dom('agentAccess.token')!=q.token:quick_click('agentAccess.token.show');await asyncio.sleep(.05)
 def leaks(x):
  txt=json.dumps(x,default=str)
  return any(tok[i:i+5] in txt for tok in tokens for i in range(len(tok)-4))
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
 def bounds(i):return dom(i,'e.getBoundingClientRect().toJSON()')
 def resize(w,h):d.resize_client(w,h,t.cdp.js('devicePixelRatio'))
 def quick_click(ident):
  rr=t.rect(ident);assert rr and rr['reachable'],ident
  o=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o));d.u32.SetCursorPos(round(o.x+rr['x']*rr['dpr']),round(o.y+rr['y']*rr['dpr']))
  d.e6._send_button(True);d.e6._send_button(False)
 def quick_key(vk):d.u32.keybd_event(vk,0,0,0);d.u32.keybd_event(vk,0,2,0)
 async def capture(label,target='window',client=None,origin=(0,0)):
  # Tree is read for each capture; it contains no field value.
  if step!=4:
   tree=await asyncio.to_thread((client or q).tool,'inspect_ui',{'target':'window','depth':64})
   t.run.check(not leaks(tree),'token-free tree '+label)
  rect=cached_rect if step==4 else bounds('agentAccess.token');scale=cached_scale if step==4 else t.cdp.js('devicePixelRatio')
  dispatch_ms=(time.perf_counter()-action_time)*1000 if action_time else None
  a=await call(client or q,'inspect.screenshot',{'target':target});raw=png(a);after_rect=bounds('agentAccess.token')
  result={'target':target,'kind':'refused','nonGrey':0,'dispatchMs':dispatch_ms}
  im=None
  if raw:
   im=Image.open(io.BytesIO(base64.b64decode(raw))).convert('RGB')
   result['size']=im.size;result['kind']='outside'
   if step==4:rect=after_rect
   if rect:
    box=[round((rect[k]-origin[j%2])*scale) for j,k in enumerate(('x','y','right','bottom'))]
    margin=math.ceil(4*scale);grown=[box[0]-margin,box[1]-margin,box[2]+margin,box[3]+margin]
    clipped=[max(0,grown[0]),max(0,grown[1]),min(im.width,grown[2]),min(im.height,grown[3])]
    result.update(rect=box,margin=margin)
    if clipped[2]>clipped[0] and clipped[3]>clipped[1]:
     pixels=list(im.crop(tuple(clipped)).getdata());bad=sum(p!=(128,128,128) for p in pixels)
     result.update(kind='overpainted' if not bad else 'unpainted',nonGrey=bad,colours=sorted(set(pixels))[:8])
   if reveal_reference is not None:
    import numpy as np,cv2
    score=float(cv2.minMaxLoc(cv2.matchTemplate(np.array(im),reveal_reference,cv2.TM_SQDIFF_NORMED))[0]);result['revealedMatchError']=score
    if score<.001:result['kind']='revealed'
  else:
   result['safe']=(a.get('error',{}).get('code')=='render.failed' and a.get('error',{}).get('details',{}).get('reason')=='tokenShown') or (label.startswith('regenerate') and a.get('http') in (401,403))
   result['code']=a.get('error',{}).get('code',a.get('http'))
   result['responseKeys']=sorted(a.keys());result['tokenFree']=not leaks(a)
   if label.startswith('regenerate') and not leaks(a) and (a.get('http')==200 or 'transportError' in a):
    result['safe']=True;result['sessionEnded']=True
  t.run.check(result['kind'] in ('overpainted','outside') or result.get('safe',False),'D-69 capture '+label,result)
  return result,im
 try:
  q=await fresh();b=await fresh()
  if step==2:
   for w,h in ((1280,800),(800,500)):
    resize(w,h);await r2.pause();await prefs();t.click('agentAccess.token');await r2.pause();rows=[];images=[]
    for state in ('masked','revealed','copy'):
     if state!='masked':await show()
     if state=='copy':t.click('agentAccess.token.copy')
     row,im=await capture(str((w,h,state)));rows.append(row);images.append(im)
    if all(images) and rows[0].get('rect')==rows[1].get('rect'):
     box=rows[0]['rect'];m=rows[0]['margin'];box=(box[0]-m,box[1]-m,box[2]+m,box[3]+m)
     t.run.check(images[0].crop(box).tobytes()==images[1].crop(box).tobytes(),'D-69 margin equality '+str((w,h)))
    t.run.save('pixels-'+str(w)+'.json',rows)
   t.run.save('preferences-movement.json',{'draggable':t.cdp.js("!!document.querySelector('[data-tt-id=preferences] [draggable=true]')")})
  elif step==3:
   await c('window.panel.set',{'panelId':'cameraView','visible':True});await prefs()
   for target in ('window','drawingView','cameraView'):
    origin=(0,0)
    if target!='window':
     rect=bounds(target+'.canvas');assert rect, target+' canvas absent';origin=(rect['x'],rect['y'])
    for i in range(20):await show();await capture(target+str(i),target,origin=origin)
   t.click('preferences.cancel');await r2.pause();await c('window.panel.set',{'panelId':'pen','visible':True,'floating':True});await prefs()
   for i in range(20):
    await show();a=await call(q,'inspect.screenshot',{'target':'pen'})
    t.run.check(bool(png(a)) and not leaks({k:v for k,v in a.items() if k!='result'}),'D-69 float outside '+str(i),{'fieldInMain':True,'capture':bool(png(a))})
  elif step==4:
   totals={}
   for action in ('quickShow','showHide','leavePage','ok','cancel','escape','regenerate','resize','simultaneous'):
    if len(sys.argv)>3 and action!=sys.argv[3]:continue
    if action=='simultaneous':b=await fresh()
    counts={'overpainted':0,'outside':0,'refused':0,'revealed':0,'unpainted':0}
    for i in range(40):
     await show()
     d.front()
     cached_rect=bounds('agentAccess.token');cached_scale=t.cdp.js('devicePixelRatio')
     if True:
      from PIL import ImageGrab
      import numpy as np
      rect=bounds('agentAccess.token');scale=t.cdp.js('devicePixelRatio');o=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o))
      reveal_reference=np.array(ImageGrab.grab().convert('RGB').crop((o.x+round(rect['x']*scale)+5,o.y+round(rect['y']*scale)+5,o.x+round(rect['right']*scale)-5,o.y+round(rect['bottom']*scale)-5)))
     if action=='quickShow':quick_click('agentAccess.token.show');quick_click('agentAccess.token.show')
     elif action=='showHide':quick_click('agentAccess.token.show')
     elif action=='leavePage':quick_click('preferences.category.general')
     elif action in ('ok','cancel'):quick_click('preferences.submit' if action=='ok' else 'preferences.cancel')
     elif action=='escape':quick_key(0x1b)
     elif action=='regenerate':
      t.click('agentAccess.regenerate');await r2.pause()
      ids=t.cdp.js("[...document.querySelectorAll('button')].filter(e=>e.textContent==='Regenerate').map(e=>e.dataset.ttId)");quick_click(ids[0]);action_time=time.perf_counter()
     if action!='regenerate':action_time=time.perf_counter()
     until=time.perf_counter()+(i%7)*.002
     while time.perf_counter()<until:pass
     if action=='resize':
      h=d.front();rr=wintypes.RECT();d.u32.GetWindowRect(h,ctypes.byref(rr));d.u32.SetCursorPos(rr.right-2,rr.bottom-2);d.u32.mouse_event(2,0,0,0,0)
      action_time=time.perf_counter();pending=asyncio.create_task(capture(action+str(i)));await asyncio.sleep(.005);d.u32.SetCursorPos(rr.right+(-60 if i%2==0 else 60),rr.bottom+(-30 if i%2==0 else 30));await asyncio.sleep(.01);d.u32.mouse_event(4,0,0,0,0);rows=[await pending]
     elif action=='simultaneous':rows=await asyncio.gather(capture(action+str(i),client=q),capture(action+str(i),client=b))
     else:rows=[await capture(action+str(i))]
     for row,_ in rows:counts[row['kind']]+=1
     if action=='regenerate':
      old=q
      for _ in range(1000):
       if json.loads((t.work/'discovery.json').read_text())['token']!=old.token:break
       await asyncio.sleep(.001)
      q=await fresh();await asyncio.to_thread(old.close)
    totals[action]=counts;t.run.save('schedules.json',totals)
  elif step==5:
   await show();tree=await asyncio.to_thread(q.tool,'inspect_ui',{'target':'window','depth':64});activity=await call(q,'agent.activity.list',{'limit':500})
   export=t.work/'export.jsonl'
   t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command',{json:JSON.stringify({command:'agent.activity.export',params:"+json.dumps({'path':str(export),'onExists':'overwrite'})+"})}).then(()=>true)");await r2.pause()
   for i in range(20):await show();await capture('error-audit'+str(i))
   await show()
   import threading
   blocker=threading.Thread(target=lambda:t.cdp.js('(()=>{const end=performance.now()+3000;while(performance.now()<end){};return true})()'),daemon=True)
   blocker.start();await asyncio.sleep(.05)
   refusal=await call(q,'inspect.screenshot',{'target':'window'})
   await asyncio.to_thread(blocker.join)
   t.run.check(refusal.get('error',{}).get('code')=='render.failed' and refusal.get('error',{}).get('details',{}).get('reason')=='tokenShown','D-69 unresponsive page refuses safely',{'code':refusal.get('error',{}).get('code')})
   files=[f for f in (t.work/'user').rglob('*') if f.is_file() and f.suffix in ('.log','.jsonl')]
   for label,value in [('tree',tree),('activity',activity),('export',export.read_text()),('events',[cl.received for cl in clients]),('errors',[a for a in answers if a.get('status')=='error']),('logs',[f.read_text(errors='replace') for f in files])]:
    t.run.check(not leaks(value),'D-69 prefix-free '+label,{'files':len(files)} if label=='logs' else None)
  elif step==6:
   await c('window.panel.set',{'panelId':'pen','visible':True});await r2.pause()
   t.click('pen.advanced');await r2.pause()
   def windows():
    found=set()
    def one(h,_):
     if d.u32.IsWindowVisible(h):found.add(int(h))
     return True
    d.u32.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(one),0);return found
   async def secondary(ident,text=False,empty=False):
    rr=t.rect(ident);assert rr and rr.get('reachable'),ident
    if empty:
     box=bounds(ident);rr['x']=box['right']-5;rr['y']=box['bottom']-5
    before=windows();o=wintypes.POINT();d.u32.ClientToScreen(d.front(),ctypes.byref(o));d.right_click_screen(o.x+rr['x']*rr['dpr'],o.y+rr['y']*rr['dpr']);await r2.pause()
    items=menu_items();after=windows();extra=[]
    for h in after-before:
     name=ctypes.create_unicode_buffer(256);d.u32.GetClassNameW(h,name,256)
     if name.value=='Chrome_WidgetWin_1' or name.value=='#32768':extra.append(name.value)
    good=bool(items) and all(any(x.lower().startswith(a) for a in ('emoji','undo','redo','cut','copy','paste','select all','writing direction')) for x in items) if text else not extra
    t.run.check(good,'D-70 secondary '+ident,{'items':items,'nativePopups':extra,'addedWindows':len(after-before)});d.key(0x1b);await asyncio.sleep(.1)
   ids=t.cdp.js("[...document.querySelectorAll('[data-tt-id]')].filter(e=>e.dataset.ttId.startsWith('pen.row.')).map(e=>e.dataset.ttId)")
   selectors=['.main-toolbar button','.main-toolbar select','input[type=range]','input[type=checkbox]','.pen-panel','.tool-rail','.status-bar','.pen-panel','.dock-tab','[data-tt-id="drawingView.canvas"]']
   # Discover each requested type from its actual DOM element; stable IDs are recorded.
   for idx,sel in enumerate(selectors):
    ident=t.cdp.js("(()=>{let e=document.querySelector("+json.dumps(sel)+");while(e&&!e.dataset.ttId)e=e.parentElement;return e?.dataset.ttId})()")
    if idx==4:ident=ids[0]
    if not ident:
     inventory=t.cdp.js("[...document.querySelectorAll('[data-tt-id]')].map(e=>({id:e.dataset.ttId,tag:e.tagName,type:e.type,cls:e.className}))")
     t.run.save('menu-inventory.json',inventory);raise RuntimeError('missing surface '+sel)
    await secondary(ident,empty=idx==7)
   await secondary('pen.maxSize.field',True)
   await c('window.panel.set',{'panelId':'text','visible':True});await r2.pause();await secondary('text.font',True)
   await c('window.panel.set',{'panelId':'pen','visible':True});await r2.pause();t.click(ids[0]);d.key(0x71);await r2.pause();await secondary('pen.rename',True)
   d.choose_menu(['File','Preferences']);await r2.pause();await secondary('preferences.search',True);d.key(0x1b)
   await r1.r3.preserve_fixture(t);d.choose_menu(['File','New Project']);await r2.pause();await secondary('newProject.name',True)
  elif step==7:
   keys=[('F5',0x74,()),('Shift+F5',0x74,(0x10,)),('Ctrl+F5',0x74,(0x11,)),('Ctrl+R',0x52,(0x11,)),('Ctrl+Shift+R',0x52,(0x11,0x10)),('Alt+Left',0x25,(0x12,)),('Alt+Right',0x27,(0x12,)),('Backspace',8,()),('MouseBack',None,(1,)),('MouseForward',None,(2,))]
   observations=[('F12',0x7b,()),('Ctrl+Shift+I',0x49,(0x11,0x10)),('Ctrl+U',0x55,(0x11,)),('Ctrl+P',0x50,(0x11,)),('Ctrl+F',0x46,(0x11,)),('Ctrl+G',0x47,(0x11,)),('Ctrl+J',0x4a,(0x11,)),('F7',0x76,()),('Ctrl+plus',0xbb,(0x11,)),('Ctrl+minus',0xbd,(0x11,)),('Ctrl+0',0x30,(0x11,)),('Ctrl+wheel',None,(0,))]
   for focus in ('canvas','text','token','dialog'):
    for name,key,mods in keys+observations:
     d.key(0x1b);await asyncio.sleep(.1)
     if focus=='canvas':t.click('drawingView.canvas')
     elif focus=='text':
      await c('window.panel.set',{'panelId':'pen','visible':True});await r2.pause();t.click('pen.maxSize.field')
     else:
      await prefs();t.click('agentAccess.token' if focus=='token' else 'preferences.search')
     marker='closure-'+focus+'-'+name;t.cdp.js('window.__closureMarker='+json.dumps(marker));t.clear();os_before=native_windows()
     if key is not None:d.key(key,*mods)
     elif name.startswith('Mouse'):
      d.u32.mouse_event(0x80,0,0,mods[0],0);d.u32.mouse_event(0x100,0,0,mods[0],0)
     else:
      d.u32.keybd_event(0x11,0,0,0);d.u32.mouse_event(0x800,0,0,120,0);d.u32.keybd_event(0x11,0,2,0)
     await asyncio.sleep(.15)
     survived=t.cdp.js('window.__closureMarker')==marker
     devtools=[]
     def dev(h,_):
      if d.u32.IsWindowVisible(h):
       text=ctypes.create_unicode_buffer(512);d.u32.GetWindowTextW(h,text,512)
       if 'devtools' in text.value.lower() or 'developer tools' in text.value.lower():devtools.append(text.value)
      return True
     d.u32.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(dev),0)
     t.run.check(not devtools,'Plan 3.2 diagnostics development-only '+focus+' '+name,{'developerWindows':len(devtools)})
     os_after=native_windows()
     t.run.check(survived,'D-70 marker '+focus+' '+name,{'commands':r2.sent(t),'scale':t.cdp.js('devicePixelRatio'),'dialog':t.cdp.js("document.querySelector('[role=dialog]')?.getAttribute('data-tt-id')"),'newWindows':[v for k,v in os_after.items() if k not in os_before],'observation':(name,key,mods) in observations})
  elif step==8:
   import contextlib,httpx
   from mcp import ClientSession,types
   from mcp.client.streamable_http import streamable_http_client
   discovery=json.loads((t.work/'discovery.json').read_text());figures=[]
   async def trial(delay):
    got=asyncio.Event()
    async def roots(_):
     got.set()
     if delay is None:await asyncio.Event().wait()
     else:await asyncio.sleep(delay)
     return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='fixture')])
    async with contextlib.AsyncExitStack() as stack:
     http=await stack.enter_async_context(httpx.AsyncClient(headers={'Authorization':'Bearer '+discovery['token']},timeout=20))
     streams=await stack.enter_async_context(streamable_http_client(discovery['endpoint'],http_client=http))
     session=await stack.enter_async_context(ClientSession(streams[0],streams[1],list_roots_callback=roots));await session.initialize()
     start=time.perf_counter();pending=asyncio.create_task(session.call_tool('run_command',{'id':'project.open','params':{'path':str(t.work/'base.animpkg'),'onUnsaved':'discard'}}))
     await asyncio.sleep(.05);readstart=time.perf_counter();read=await session.call_tool('run_command',{'id':'app.state.get','params':{}});readms=(time.perf_counter()-readstart)*1000;read_while_pending=not pending.done()
     response=await pending;ms=(time.perf_counter()-start)*1000
     content=response.structuredContent or json.loads(response.content[0].text)
     row={'delayMs':None if delay is None else delay*1000,'elapsedMs':round(ms,2),'readMs':round(readms,2),'status':content.get('status'),'code':content.get('error',{}).get('code'),'rootsRequested':got.is_set()};figures.append(row)
     t.run.check(row['code']=='fs.outsideRoots' if delay is None else row['status']=='ok','D-68 roots '+str(len(figures)),row)
     if delay is None:t.run.check(read_while_pending and not read.isError,'D-68 other read answered while roots pending',{'readMs':readms})
   for i in range(30):await trial(.3)
   await trial(None)
   for i in range(5):await trial(0)
   t.run.save('roots-figures.json',figures)
  elif step==9:
   from PIL import ImageGrab
   resize(800,500);await r2.pause()
   dimensions=t.cdp.js('({innerWidth,innerHeight,html:{h:document.documentElement.clientHeight,sh:document.documentElement.scrollHeight,w:document.documentElement.clientWidth},body:{h:document.body.clientHeight,sh:document.body.scrollHeight,w:document.body.clientWidth}})')
   t.run.check(all(v['h']==v['sh']==dimensions['innerHeight'] and v['w']==dimensions['innerWidth'] for v in (dimensions['html'],dimensions['body'])),'D-68 page does not scroll',dimensions)
   ImageGrab.grab().save(t.out/'page-800.png')
   rail=bounds('toolPalettes');o=wintypes.POINT();d.u32.ClientToScreen(d.front(),ctypes.byref(o));scale=t.cdp.js('devicePixelRatio');d.u32.SetCursorPos(round(o.x+(rail['x']+rail['width']/2)*scale),round(o.y+(rail['y']+rail['height']/2)*scale))
   for _ in range(20):d.u32.mouse_event(0x800,0,0,ctypes.c_ulong(-120).value,0);await asyncio.sleep(.02)
   last=t.cdp.js("[...document.querySelectorAll('.tool-rail button')].at(-1)?.dataset.ttId");rr=bounds(last);t.run.check(rr['bottom']<=rail['bottom'],'D-68 last tool wheel-visible',{'last':last,'rect':rr,'rail':rail});t.clear();t.click(last);await r2.pause();sent=r2.sent(t);active=await c('tool.active.get');t.run.check(bool(sent) and active.get('toolId')==last.split('.')[-1],'D-68 last tool chosen',{'commands':sent,'active':active})
   await c('window.panel.set',{'panelId':'pen','visible':True});await r2.pause()
   d.choose_menu(['File','Preferences']);await r2.pause();t.click('preferences.category.keyboard');await r2.pause();t.click('keymap.filter');d.type_text('edit.selectAll');await r2.pause();t.click('keymap.command.edit.selectAll');await r2.pause();t.click('keymap.capture');d.key(0x75);await r2.pause()
   if dom('keymap.conflict.override','e.textContent') is not None:t.click('keymap.conflict.override')
   t.click('preferences.submit');await r2.pause()
   t.click('pen.maxSize.field');t.clear();d.key(0x75);await r2.pause();t.run.check(any(x['command']=='edit.selectAll' for x in r2.sent(t)),'D-69 F6 reaches keymap',r2.sent(t))
   t.click('pen.maxSize.field');t.clear();d.key(0x79,0x10);await r2.pause();items=menu_items();t.run.check(bool(items) and not r2.sent(t),'D-69 Shift+F10 kept',{'items':items,'commands':r2.sent(t)})
  else:raise ValueError('step not implemented')
 finally:
  for cl in clients:await asyncio.to_thread(cl.close)

if __name__=='__main__':
 case,step=sys.argv[1],int(sys.argv[2]);assert case in ('p1a','p1b')
 import os
 setter=os._Environ.__setitem__
 def accessible_env(self,key,value):
  if key.endswith('ADDITIONAL_BROWSER_ARGUMENTS'):value+=' --force-renderer-accessibility'
  return setter(self,key,value)
 os._Environ.__setitem__=accessible_env
 if step==1:
  import runpy
  sys.argv=[str(Path(__file__).with_name('c44a-r3-live.py')),'n2b','6']
  runpy.run_path(sys.argv[0],run_name='__main__');raise SystemExit()
 r1.EVID=r1.ROOT/'reviews/evidence/M4-C4.4a-r4-review';r1.live_body=body
 result=r1.live(step)
 # Only compact summaries enter the repository.
 rows=json.loads((r1.OUT/'transcript.json').read_text())
 summary={'checks':len(rows),'failures':[x for x in rows if not x.get('ok',False)],'raw':str(r1.OUT)}
 if step in (6,7,8,9):summary['results']=rows
 captures=[x.get('detail') for x in rows if isinstance(x.get('detail'),dict) and 'kind' in x['detail']]
 summary['captures']={kind:sum(x['kind']==kind for x in captures) for kind in ('overpainted','outside','refused','revealed')}
 timings=[x['dispatchMs'] for x in captures if x.get('dispatchMs') is not None]
 if timings:summary['dispatchMs']={'min':min(timings),'max':max(timings),'above30':sum(x>30 for x in timings)}
 if step==4:summary['refusals']=[x for x in captures if x['kind']=='refused']
 for pattern in ('pixels-*.json','schedules.json','roots-figures.json','preferences-movement.json'):
  for f in r1.OUT.glob(pattern):summary[f.name]=json.loads(f.read_text())
 (r1.EVID/('step'+str(step)+'.json')).write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8',newline='\n')
 sys.exit(result)
