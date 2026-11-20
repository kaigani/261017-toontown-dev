"""Round 2 live 1. Real OS input; read-only DOM inspection; fresh fixtures."""
import asyncio, ctypes, json, sys, subprocess, os, time
from pathlib import Path
import importlib.util
s=importlib.util.spec_from_file_location('r1',Path(__file__).with_name('c44a-r1-live.py'))
r1=importlib.util.module_from_spec(s);s.loader.exec_module(r1)
original_body=r1.live_body
r1.EVID=r1.ROOT/'reviews/evidence/M4-C4.4a-r2-review'

def dom(t,ident,expr='e.value'):
 return t.cdp.js('(()=>{const e=document.querySelector("[data-tt-id="+JSON.stringify(%s)+"]");return e?%s:null})()'%(json.dumps(ident),expr))
def sent(t):return [x for x in t.recorded() if t.kinds.get(x['command'])!='query']
async def pause():await asyncio.sleep(.7)
async def typein(t,ident,text,key=0x0D):
 d=r1.d; t.click(ident);d.key(0x24);d.key(0x23,0x10);d.key(0x08)
 if text=="12 px":
  subprocess.run(["powershell","-NoProfile","-Command","Set-Clipboard -Value '12 px'"],check=True);d.key(0x56,0x11)
 else:d.type_text(text)
 t.lastTyped=dom(t,ident)
 if key:d.key(key)
 await pause()
async def current(t):
 st=await t.cmd('tool.pen.list');return next(p for p in st['pens'] if p['penId']==st['currentBrushPenId'])
async def body(t,step):
 d=r1.d;c=t.cmd
 if step==2:
  rect=t.rect
  def mounted(ident):
   until=time.monotonic()+15
   while True:
    r=rect(ident)
    if r is not None or time.monotonic()>=until:return r
    time.sleep(.1)
  t.rect=mounted
  await original_body(t,7)
  return
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await r1.r3.modal_fixture(t,'Round 2 live '+str(step))
 await r1.r3.wait(t,'drawingView.canvas')
 if step==3:
  # Ranges: spec 02 sections 6.1/7.6/7.7; spec 01 sections 1.2/9; agent.server.set declared range.
  controls=[('pen.maxSize.field',0,100,False),('pen.tipAngle.field',0,360,False),('text.kerning',-60,60,True),('text.indent',-99,1000,False),('toolOptions.thickness',0,100,False),('pref.general.recentProjects.maxCount',0,30,True),('agentAccess.port',1024,65535,True),('newProject.frameRate',1,120,False)]
  async def state(ident):
   if ident.startswith('pen.'):return (await current(t))[ident.split('.')[1]]
   if ident.startswith('text.'):return (await c('tool.options.get',{'toolId':'text'}))['options'][ident.split('.')[1]]
   if ident.startswith('toolOptions.'):return (await c('tool.options.get',{'toolId':'pencil'}))['options']['thickness']
   if ident.startswith('pref.'):return (await c('prefs.get',{'keys':['general.recentProjects.maxCount']}))['values']['general.recentProjects.maxCount']
   if ident.startswith('agentAccess.'):return (await c('prefs.get',{'keys':['agent.server.port']}))['values']['agent.server.port']
   return await c('project.properties.get')
  for ident,lo,hi,integer in controls:
   if os.environ.get('C44_FROM') and [x[0] for x in controls].index(ident)<int(os.environ['C44_FROM']):continue
   if ident.startswith('pen.'):
    await c('window.panel.set',{'panelId':'pen','visible':True});await c('tool.active.set',{'toolId':'brush'})
   elif ident.startswith('text.'):await c('window.panel.set',{'panelId':'text','visible':True})
   elif ident.startswith('toolOptions.'):
    await c('tool.active.set',{'toolId':'pencil'});await c('tool.pencil.options.set',{'thicknessSource':'fixed','thickness':3});await c('window.panel.set',{'panelId':'properties','visible':True});await pause();
    if dom(t,'properties.page.toolOptions') is None and t.rect('tab.properties'):t.click('tab.properties')
    await r1.r3.wait(t,'properties.page.toolOptions');t.click('properties.page.toolOptions')
   elif ident.startswith(('pref.','agentAccess.')):
    d.choose_menu(['File','Preferences']);await pause();t.click('preferences.category.'+('agent' if ident.startswith('agentAccess.') else 'general'));await pause()
    if ident.startswith('agentAccess.') and dom(t,ident,'e.disabled'):t.click('agentAccess.server')
   else:
    await r1.r3.preserve_fixture(t);d.choose_menu(['File','New Project']);await pause()
   await r1.r3.wait(t,ident)
   for text in ['', '   ','abc','+5','1e3','5.','.5','1,5','--1',str(lo-1),str(hi+1),str(lo),str(hi),'9'*40,'12 px']:
    before=await state(ident);old=dom(t,ident);t.clear();await typein(t,ident,text);after=await state(ident);commands=sent(t);closed=dom(t,ident) is None
    if dom(t,ident) is None and ident.startswith('pref.'):
     d.choose_menu(['File','Preferences']);await pause()
    if dom(t,ident) is None and ident.startswith('newProject.'):
     await r1.r3.preserve_fixture(t);d.choose_menu(['File','New Project']);await pause()
    shown=dom(t,ident)
    rec={'control':ident,'input':text,'typed':t.lastTyped,'formClosed':closed,'before':before,'kernel':after,'oldField':old,'shown':shown,'commands':commands,'problem':dom(t,ident,'e.title')}
    # Decimal/exponent notation is observed, not assigned an unstated lexical policy.
    invalid=text in ('','   ','abc','1,5','--1','12 px','9'*40,str(lo-1),str(hi+1)) or (integer and text=='.5')
    t.run.save('numeric-'+ident+'-'+str(len(t.run.transcript))+'.json',rec)
    t.run.check((not commands and after==before and shown==old) if invalid else (closed or (shown is not None and (float(shown)==float(text) if lo<=float(text)<=hi and (not integer or float(text).is_integer()) else shown==old))),'D-61 numeric '+ident+' '+repr(text),rec)
   if ident.startswith(('pref.','agentAccess.','newProject.')):d.key(0x1B);await pause()
 elif step==4:
  await c('window.panel.set',{'panelId':'pen','visible':True});await pause();ident='pen.maxSize.field';pid=(await current(t))['penId']
  await c('tool.pen.set',{'penId':pid,'maxSize':40});await pause()
  t.click(ident);t.clear();d.key(0x41,0x11);await pause()
  t.run.check(not sent(t) and dom(t,ident,'e.selectionStart')==0 and dom(t,ident,'e.selectionEnd')==2,'spec 01 section 8.4 Ctrl A edits numeric field',{'sent':sent(t),'start':dom(t,ident,'e.selectionStart'),'end':dom(t,ident,'e.selectionEnd')})
  t.clear();await typein(t,ident,'45',0x1B)
  t.run.check(not sent(t) and dom(t,ident)=='40','D-61 Escape restores',{'sent':sent(t),'shown':dom(t,ident)})
  t.clear();await typein(t,ident,'46',None);t.click('pen.preview');await pause()
  t.run.check(len([x for x in sent(t) if x['command']=='tool.pen.set'])==1 and (await current(t))['maxSize']==46,'D-61 blur commits once',sent(t))
  t.clear();await typein(t,ident,'47');t.click('pen.preview');await pause()
  t.run.check(len([x for x in sent(t) if x['command']=='tool.pen.set'])==1,'D-61 Enter plus blur once',sent(t))
  await c('edit.undo');await pause();t.run.check(dom(t,ident)=='46','spec 02 tool.pen.set undo restores field',dom(t,ident))
  await typein(t,ident,'48',None);await c('tool.pen.set',{'penId':pid,'maxSize':49});await pause()
  t.run.save('external-edit.json',{'typed':'48','agentValue':49,'shown':dom(t,ident),'sent':sent(t)})
  d.key(0x0D);await pause();t.run.check((await current(t))['maxSize']==49 and dom(t,ident)=='49','D-61 external kernel value retained',dom(t,ident))
  for menu,field,ok in [(['File','Preferences'],'pref.general.recentProjects.maxCount','preferences.submit'),(['File','Project Properties'],'projectProperties.frameRate','projectProperties.submit')]:
   d.choose_menu(menu);await pause();old=dom(t,field);t.clear();await typein(t,field,'abc',None);t.click(ok);await pause()
   actual=(await c('prefs.get',{'keys':['general.recentProjects.maxCount']}))['values']['general.recentProjects.maxCount'] if field.startswith('pref.') else (await c('project.properties.get'))['frameRate']
   t.run.check(actual==float(old),'D-61 staged invalid cannot be applied '+field,{'retained':old,'kernel':actual,'sent':sent(t)})
 else:await menus(t,step)

async def menus(t,step):
 d=r1.d;c=t.cmd
 from ctypes import wintypes
 def point(x,y,right=False):
  o=wintypes.POINT(0,0);d.u32.ClientToScreen(d.front(),ctypes.byref(o));scale=t.cdp.js('devicePixelRatio');(d.right_click_screen if right else d.click_screen)(o.x+x*scale,o.y+y*scale)
 async def openat(view,fx=.5,fy=.5):
  d.key(0x1B);r=dom(t,view+'.canvas','(()=>{const r=e.getBoundingClientRect(),x=Math.max(0,r.x),y=Math.max(0,r.y);return {x,y,w:Math.min(innerWidth,r.right)-x,h:Math.min(innerHeight,r.bottom)-y}})()');point(r['x']+r['w']*fx,r['y']+r['h']*fy,True);await pause()
 def geometry():return t.cdp.js('(()=>{const m=document.querySelector(".context-menu");return {client:{w:innerWidth,h:innerHeight},menu:m?m.getBoundingClientRect().toJSON():null,arrows:[...document.querySelectorAll(".context-menu-arrow")].map(e=>({text:e.textContent,rect:e.getBoundingClientRect().toJSON(),class:e.className})),items:[...document.querySelectorAll(".context-menu [role^=menuitem]")].map(e=>({id:e.dataset.ttId,rect:e.getBoundingClientRect().toJSON(),label:e.textContent}))}})()')
 async def setup(view):
  await c('window.panel.set',{'panelId':view,'visible':True});await c('view.active.set',{'viewId':view});await c('tool.active.set',{'toolId':'brush'});await pause()
  if dom(t,view+'.canvas') is None and t.rect('tab.'+view):
   await asyncio.sleep(2)
   rec={'view':view,'canvasPresent':dom(t,view+'.canvas','true'),'state':await c('app.state.get'),'panels':await c('window.panel.list'),'body':t.cdp.js('document.body.innerText')}
   t.run.save('panel-focus-'+view+'.json',rec)
   t.run.check(dom(t,view+'.canvas','true') is True,'spec 01 panel/view focus mounts '+view,rec)
   t.click('tab.'+view)
  await r1.r3.wait(t,view+'.canvas')
 def resize(w,h):
  handle=d.front();outer=wintypes.RECT();client=wintypes.RECT();d.u32.GetWindowRect(handle,ctypes.byref(outer));d.u32.GetClientRect(handle,ctypes.byref(client));scale=t.cdp.js('devicePixelRatio')
  d.u32.MoveWindow(handle,30,30,int(w*scale)+outer.right-outer.left-client.right,int(h*scale)+outer.bottom-outer.top-client.bottom,True)
 if step==5:
  for size in [(1280,800),(800,500)]:
   h=d.front();resize(*size);await pause()
   for view in ['drawingView','cameraView']:
    if os.environ.get('C44_MIN_CAMERA') and (size!=(800,500) or view!='cameraView'):continue
    await setup(view)
    for fx,fy in [(.01,.01),(.5,.01),(.99,.01),(.01,.5),(.99,.5),(.01,.99),(.5,.99),(.99,.99)]:
     await openat(view,fx,fy);g=geometry();m=g['menu'];area=g['client'];inside=m and m['x']>=0 and m['y']>=0 and m['right']<=area['w'] and m['bottom']<=area['h']
     t.run.check(bool(inside),'D-60 placement '+str((size,view,fx,fy)),g)
     if not g['items']:
      t.run.check(False,'D-60 menu opened at '+str((size,view,fx,fy)),{'geometry':g,'canvas':dom(t,view+'.canvas','e.getBoundingClientRect().toJSON()')});continue
     # Exercise visible scroll affordance to reach the final item.
     for _ in range(20):
      g=geometry();last=g['items'][-1]['rect'];m=g['menu']
      if last['bottom']<=m['bottom']-20:break
      arrow=next((a for a in g['arrows'] if 'spent' not in a['class'] and a['rect']['y']>m['y']+m['height']/2),None)
      if not arrow:break
      a=arrow['rect'];oldY=last['y'];point(a['x']+a['width']/2,a['y']+a['height']/2);await pause()
      if geometry()['items'][-1]['rect']['y']==oldY:break
     last=g['items'][-1];r=last['rect'];hit=t.cdp.js('document.elementFromPoint(%s,%s)?.closest("[role^=menuitem]")?.dataset.ttId'%(r['x']+r['width']/2,r['y']+r['height']/2));t.clear();point(r['x']+r['width']/2,r['y']+r['height']/2);await pause()
     t.run.check(hit==last['id'],'D-60 last item reachable '+str((size,view,fx,fy)),{'geometry':g,'sent':sent(t)})
     await t.settle_dialogs()
   await setup('drawingView');await openat('drawingView');resize(900 if size[0]==1280 else 1280,600);await pause();t.run.save('resize-open-'+str(size[0])+'.json',geometry());d.key(0x1B)
 elif step==6:
  await setup('drawingView');resize(800,500);await pause();r=dom(t,'drawingView.canvas','(()=>{const r=e.getBoundingClientRect();return {x:Math.max(0,r.x),y:Math.max(0,r.y),right:Math.min(innerWidth,r.right),bottom:Math.min(innerHeight,r.bottom)}})()');point((r['x']+r['right'])/2,(r['y']+r['bottom'])/2);before=t.cdp.js('document.activeElement?.dataset.ttId')
  for _ in range(100):
   if before=='drawingView.canvas':break
   d.key(0x09);before=t.cdp.js('document.activeElement?.dataset.ttId')
  t.run.check(before=='drawingView.canvas','design note 1 section 1 canvas reachable by keyboard',before)
  t.run.save('keyboard-before.json',{'focus':before,'element':t.cdp.js('document.activeElement?.outerHTML')});d.key(0x79,0x10);await pause();g=geometry();t.run.check(bool(g['menu']),'design note 1 section 1 keyboard open Shift F10',g)
  if not g['menu']:
   d.key(0x5D);await pause();g=geometry();t.run.check(bool(g['menu']),'design note 1 section 9 context-menu key opens',{'focus':t.cdp.js('document.activeElement?.dataset.ttId'),'geometry':g})
  if not g['menu']:await openat('drawingView')
  path=[]
  for _ in range(30):
   d.key(0x28)
   path.append(t.cdp.js('(()=>{const e=document.activeElement,r=e.getBoundingClientRect();return {id:e.dataset.ttId,rect:r.toJSON(),w:innerWidth,h:innerHeight}})()'))
  t.run.check(all(x['rect']['y']>=0 and x['rect']['bottom']<=x['h'] for x in path),'design note 1 section 9 arrows cross scroll boundary visibly',path)
  d.key(0x24);d.key(0x28);d.key(0x28);d.key(0x28)
  await pause();t.run.save('keyboard-scroll.json',{'geometry':geometry(),'focus':t.cdp.js('document.activeElement?.dataset.ttId')});t.clear();d.key(0x0D);await pause();t.run.check(not t.cdp.js('!!document.querySelector(".context-menu")'),'keyboard Enter activates/closes',sent(t));await t.settle_dialogs()
  opener=t.cdp.js('document.activeElement?.dataset.ttId');await openat('drawingView');d.key(0x1B);await pause();t.run.check(not t.cdp.js('!!document.querySelector(".context-menu")') and t.cdp.js('document.activeElement?.dataset.ttId')==opener,'keyboard Escape closes and returns focus',{'before':before,'menuFocus':opener,'after':t.cdp.js('document.activeElement?.dataset.ttId')})
  await openat('drawingView');t.clear();point(5,t.cdp.js('innerHeight')-5);await pause();t.run.check(not t.cdp.js('!!document.querySelector(".context-menu")'),'design note 1 section 9 outside click closes',{'sent':sent(t),'geometry':geometry()})
 elif step==7:
  for view in ['drawingView','cameraView']:
   for family in ['drawing','scene']:
    await setup(view)
    if family=='scene':await c('anim.tool.set',{'tool':'transform'});await c('anim.tool.set',{'tool':'select'})
    else:await c('tool.active.set',{'toolId':'brush'});await c('tool.active.set',{'toolId':'select'})
    for action in ['selectAll','deselectAll','copy','cut','paste']:
     await r1.r3.modal_fixture(t,'Family '+view+' '+family+' '+action);await setup(view)
     if family=='scene':await c('anim.tool.set',{'tool':'transform'});await c('anim.tool.set',{'tool':'select'})
     else:await c('tool.active.set',{'toolId':'brush'});await c('tool.active.set',{'toolId':'select'})
     await c('draw.selection.set',{'by':'all'})
     expectedContext='staging' if family=='scene' and view=='cameraView' else 'drawing'
     drawingElement=(await c('app.state.get'))['element']['id']
     eid=(await c('element.create',{'type':'drawing','name':'Staging sentinel'}))['elementIds'][0]
     await c('edit.selection.set',{'context':'staging','ids':[eid]});await c('app.context.set',{'elementId':drawingElement,'frame':1})
     if action=='paste':await c('edit.copy',{'context':expectedContext})
     await c('window.panel.set',{'panelId':'pen','visible':True});await r1.r3.wait(t,'pen.maxSize.field');t.click('pen.maxSize.field');await openat(view)
     items=geometry()['items'];suffix=('.sceneMenu.' if family=='scene' and view=='cameraView' else '.drawingMenu.')+action
     item=next((x for x in items if x['id'].endswith(suffix)),None)
     before={'drawing':await c('draw.selection.get'),'staging':await c('edit.selection.get',{'context':'staging'})};t.clear()
     if item:t.click(item['id'])
     await pause();after={'drawing':await c('draw.selection.get'),'staging':await c('edit.selection.get',{'context':'staging'})};commands=sent(t);context='staging' if family=='scene' and view=='cameraView' else 'drawing'
     t.run.check(bool(item) and len([x for x in commands if x['command']=='edit.'+action and x['params'].get('context')==context])==1,'D-60 family '+view+' '+family+' '+action,{'before':before,'after':after,'sent':commands,'items':items})
     other='staging' if context=='drawing' else 'drawing'
     t.run.check(before[other]==after[other],'D-60 other selection preserved '+view+' '+family+' '+action,{'before':before,'after':after})
     if action in ('selectAll','deselectAll'):
      ids=after[context].get('selectedIds',after[context].get('ids',[]))
      t.run.check(bool(ids)==(action=='selectAll'),'D-60 selection effect '+view+' '+family+' '+action,after)
     await t.settle_dialogs();d.key(0x1B)

async def live2(t,step):
 import hashlib,io,base64
 from PIL import Image
 d=r1.d;c=t.cmd;t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await r1.r3.modal_fixture(t,'Round 2 live 2')
 await c('project.saveAs',{'path':str(t.work/'base.animpkg')})
 clients=[];tasks=[]
 async def fresh(name):
  q=r1.Live4Http(json.loads((t.work/'discovery.json').read_text()),name);clients.append(q);await asyncio.to_thread(q.initialize);return q
 async def call(q,n,p=None):return await asyncio.to_thread(q.command,n,p)
 async def prefs():
  d.choose_menu(['File','Preferences']);await pause();t.click('preferences.category.agent');await pause()
 def text(i):return dom(t,i,'e.textContent')
 async def drain():
  for _ in range(12):
   if dom(t,'confirmSheet','true'):t.click('confirmSheet.decline')
   await asyncio.sleep(.2)
 async def hold(q):
  path=t.work/('sentinel-'+str(len(tasks))+'.animpkg');path.write_bytes(b'sentinel')
  task=asyncio.create_task(call(q,'project.saveAs',{'path':str(path),'onExists':'overwrite'}));tasks.append(task);await asyncio.sleep(.4);return task,path
 try:
  if step>=5:
   await prefs();await t.do(('select','agentAccess.policy','assist'));t.click('preferences.submit');await pause()
  q=await fresh('round2-A');b=await fresh('round2-B')
  if step==5:
   for first in (False,True):
    task,path=await hold(q);status=await call(b,'agent.server.status');cid=status['result']['pendingConfirmations'][0]['confirmationId']
    t.click('confirmSheet.accept' if first else 'confirmSheet.decline')
    reply=t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command',{json:JSON.stringify({command:'agent.confirmation.respond',params:"+json.dumps({'confirmationId':cid,'accept':not first})+"})}).then(x=>JSON.parse(typeof x==='string'?x:new TextDecoder().decode(new Uint8Array(x))))")
    answer=await task
    t.run.check('confirmation.expired' in json.dumps(reply) and (path.read_bytes()!=b'sentinel')==first,'D-59 first decision '+str(first),{'second':reply,'answer':answer})
  elif step==6:
   a,p=await hold(q)
   read=await call(q,'app.state.get');mutation=await call(q,'tool.active.set',{'toolId':'brush'});a2,p2=await hold(q)
   t.run.check(read.get('status')=='ok' and mutation.get('status')=='ok' and '1 of 2' in (text('confirmSheet.queue') or ''),'D-63 one held read mutation second call',{'read':read.get('status'),'mutation':mutation,'queue':text('confirmSheet.queue')})
   await drain();await asyncio.gather(a,a2)
   held=[]
   for client in (q,b,q):held.append((await hold(client))[0])
   t.run.check('1 of 3' in (text('confirmSheet.queue') or ''),'D-63 A B A queue',text('confirmSheet.queue'));await drain();await asyncio.gather(*held)
   held=[]
   for _ in range(5):held.append((await hold(q))[0])
   read=asyncio.create_task(call(q,'app.state.get'));tasks.append(read);await asyncio.sleep(2)
   t.run.check(read.done() and read.result().get('status')=='ok','D-63 fifth held call leaves read answered',{'read':read.result() if read.done() else 'blocked','fifth':held[-1].result() if held[-1].done() else 'held','queue':text('confirmSheet.queue'),'status':await call(b,'agent.server.status')})
   await drain();answers=await asyncio.gather(*held,read);t.run.check('confirmation.declined' in json.dumps(answers[4]) and answers[-1].get('status')=='ok','fifth declines and blocked read completes after decisions',{'fifth':answers[4],'readStatus':answers[-1].get('status')})
  elif step==7:
   task,path=await hold(q);shown=text('confirmSheet.countdown');status=await call(b,'agent.server.status');clock=shown.rsplit(' ',1)[-1].split(':');seconds=int(clock[0])*60+int(clock[1]);remaining=status['result']['pendingConfirmations'][0]['expiresInMs']/1000
   t.run.check(abs(seconds-remaining)<=1,'D-63 countdown within one second',{'shown':seconds,'remaining':remaining})
   await asyncio.to_thread(q.close);await pause();status=await call(b,'agent.server.status')
   t.run.check(not status['result']['pendingConfirmations'] and path.read_bytes()==b'sentinel','closed session retires held request',status)
   q=await fresh('round2-replacement');held=[]
   for _ in range(4):held.append((await hold(q))[0])
   read=await call(q,'app.state.get');t.run.check(read.get('status')=='ok' and '1 of 4' in (text('confirmSheet.queue') or ''),'four slots available after disconnect',text('confirmSheet.queue'));await drain();await asyncio.gather(*held)
   task,path=await hold(q);request_id=q.seq
   await asyncio.to_thread(q.rpc,'notifications/cancelled',{'requestId':request_id,'reason':'review cancellation'},True);await pause()
   status=await call(b,'agent.server.status');t.run.check(True,'observation cancellation pending sheet',{'pending':status['result']['pendingConfirmations'],'sheet':text('confirmSheet')});t.click('confirmSheet.accept');await pause();t.run.check(path.read_bytes()==b'sentinel','cancelled request cannot act after accept');held=[]
   for _ in range(4):held.append((await hold(q))[0])
   read=await call(q,'app.state.get');t.run.check(read.get('status')=='ok','cancellation released slot for four further held calls',{'readStatus':read.get('status'),'queue':text('confirmSheet.queue')});await drain();await asyncio.gather(*held)
   t.run.check(dom(t,'engine.restart','true') is None,'restart control absent while engine healthy',{'restartOffered':dom(t,'engine.restart','true')})
  elif step==3:
   main_handle=d.window_handle();d.window_handle=lambda:main_handle
   await c('window.panel.set',{'panelId':'pen','visible':True,'floating':True});await pause()
   await prefs();token=q.token;sha=lambda x:hashlib.sha256(x.encode()).hexdigest()
   t.run.check(dom(t,'agentAccess.token')=='\u2022'*16,'token masked on opening')
   t.click('agentAccess.token.copy');await pause();clip=subprocess.check_output(['powershell','-NoProfile','-Command','Get-Clipboard -Raw']).decode().strip();t.run.check(sha(clip)==sha(token),'Copy matches discovery',{'length':len(clip),'sha256':sha(clip)})
   rect=dom(t,'agentAccess.token','e.getBoundingClientRect().toJSON()');scale=t.cdp.js('devicePixelRatio')
   async def shot(target):
    answer=await call(q,'inspect.screenshot',{'target':target})
    def find(x):
     if isinstance(x,dict):
      for k,v in x.items():
       if k=='png' and isinstance(v,str):return v
       found=find(v)
       if found:return found
     if isinstance(x,list):
      for v in x:
       found=find(v)
       if found:return found
    png=find(answer)
    if not png:return None,answer
    im=Image.open(io.BytesIO(base64.b64decode(png))).convert('RGB')
    if target=='window':im=im.crop((round(rect['x']*scale)+4,round(rect['y']*scale)+4,round(rect['right']*scale)-4,round(rect['bottom']*scale)-4))
    return im,{'size':im.size}
   baseline,info=await shot('window')
   t.click('agentAccess.token.show');await pause()
   t.run.check(sha(dom(t,'agentAccess.token'))==sha(token),'revealed reference matches discovery hash')
   from PIL import ImageGrab,ImageChops
   from ctypes import wintypes
   origin=wintypes.POINT(0,0);d.u32.ClientToScreen(d.front(),ctypes.byref(origin))
   revealed=ImageGrab.grab().convert('RGB').crop((origin.x+round(rect['x']*scale)+4,origin.y+round(rect['y']*scale)+4,origin.x+round(rect['right']*scale)-4,origin.y+round(rect['bottom']*scale)-4))
   t.click('agentAccess.token.show');await pause()
   for target in ('window','drawingView'):
    t.click('agentAccess.token.show');await pause();tree=await asyncio.to_thread(q.tool,'inspect_ui',{'target':'window','depth':32});im,info=await shot(target)
    t.run.check(token not in json.dumps(tree) and dom(t,'agentAccess.token')!=token and (target!='window' or (im is not None and im.tobytes()==baseline.tobytes())),'D-62 shown token protected '+target,info)
   from ctypes import wintypes
   button=t.rect('agentAccess.token.show');origin=wintypes.POINT(0,0);d.u32.ClientToScreen(d.front(),ctypes.byref(origin));d.u32.SetCursorPos(int(origin.x+button['x']*button['dpr']),int(origin.y+button['y']*button['dpr']))
   def fastclick():d.e6._send_button(True);d.e6._send_button(False)
   for i in range(30):
    if dom(t,'agentAccess.token')==token:t.click('agentAccess.token.show')
    fastclick();await asyncio.sleep((i%5)*.005);fastclick();await asyncio.sleep((i%7)*.004)
    if text('agentAccess.token.show')=='Hide':fastclick()
    im,info=await shot('window')
    if im is not None:
     info['maskedPixelMatch']=im.tobytes()==baseline.tobytes();info['revealedPixelMatch']=im.tobytes()==revealed.tobytes()
     info['maskDifferentPixels']=sum(a!=b for a,b in zip(im.getdata(),baseline.getdata()));info['revealDifferentPixels']=sum(a!=b for a,b in zip(im.getdata(),revealed.getdata()))
     if not info['maskedPixelMatch']:im.save(r1.OUT/('capture-mismatch-'+str(i)+'.png'))
    t.run.check((im is not None and info['maskedPixelMatch']) or (im is None and info.get('error',{}).get('code')=='render.failed' and info.get('error',{}).get('details',{}).get('reason')=='tokenShown'),'late reveal capture '+str(i),info)
   t.click('agentAccess.token.show');await pause();im,info=await shot('pen');t.run.check(im is not None and dom(t,'agentAccess.token')!=token,'float panel capture masks main token',info)
   if dom(t,'agentAccess.token')!=token:t.click('agentAccess.token.show');await pause()
   activity=await call(q,'agent.activity.list',{'limit':500});export=t.work/'token-export.jsonl';t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command',{json:JSON.stringify({command:'agent.activity.export',params:"+json.dumps({'path':str(export),'onExists':'overwrite'})+"})}).then(()=>true)")
   leaks=[str(f.relative_to(t.work)) for f in (t.work/'user').rglob('*') if f.is_file() and f.suffix in ('.log','.jsonl') and token.encode() in f.read_bytes()]
   t.run.check(token not in json.dumps(activity) and export.exists() and token not in export.read_text() and not leaks,'token absent activity export logs',{'leaks':leaks})
  elif step==2:
   async def c(name,params=None):
    reply=t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command',{json:JSON.stringify({command:"+json.dumps(name)+",params:"+json.dumps(params or {})+"})}).then(x=>typeof x==='string'?JSON.parse(x):JSON.parse(new TextDecoder().decode(new Uint8Array(x))))")
    return reply.get('result',reply)
   async def status(label):
    nonlocal q
    q=await fresh(label);answer=await call(q,'agent.server.status');discovery=json.loads((t.work/'discovery.json').read_text());await prefs();line=text('agentAccess.status');saved=dom(t,'agentAccess.server','e.checked')
    t.run.check(answer.get('result',{}).get('endpoint')==discovery['endpoint'] and discovery['endpoint'] in line and ('saved setting' in line),'D-62 endpoint '+label,{'status':answer,'discoveryEndpoint':discovery['endpoint'],'line':line,'saved':saved})
   await status('explicit flag preference off')
   t.click('agentAccess.server');t.click('preferences.submit');await pause();await status('enabled during run')
   t.click('agentAccess.server');t.click('preferences.submit');await pause();await prefs();off=await c('agent.server.status');t.run.check(not off.get('endpoint'),'off endpoint cleared',off)
   t.click('agentAccess.server');t.click('preferences.submit');await pause();await status('off then on')
   t.click('agentAccess.regenerate');await pause();buttons=t.cdp.js("[...document.querySelectorAll('button')].filter(e=>e.textContent==='Regenerate').map(e=>e.dataset.ttId)");t.click(buttons[0]);await pause();d.key(0x1B);await status('regenerated')
   d.key(0x1B)
   for client in clients:await asyncio.to_thread(client.close)
   os.kill(d.e2e.APP_PID['pid'],15);await asyncio.sleep(1)
   disc=t.work/'discovery.json';disc.unlink(missing_ok=True)
   proc=subprocess.Popen([r1.APP,'--user-dir',str(t.work/'user'),'--mcp-discovery',str(disc)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);d.e2e.APP_PID['pid']=proc.pid
   try:
    await asyncio.to_thread(d.e2e.wait_discovery,disc,proc);await asyncio.sleep(2);t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();await status('saved preference launch without HTTP flag')
   finally:d.e2e.stop(proc)
  elif step==4:
   import re
   source=(r1.ROOT/'desktop/src/ipc/generated/commands.ts').read_text()
   labels=dict(re.findall(r'"([^"\n]+)": \{\s*kind: "[^"\n]+",\s*label: "([^"\n]+)"',source))
   cases=[('query','app.state.get',{}),('mutation','tool.active.set',{'toolId':'brush'}),('batch','batch',{'label':'Review fifty steps','steps':[{'id':'draw.rectangle.add','params':{'from':{'x':0,'y':0},'to':{'x':5,'y':5}}}]*50}),('job','project.save',{}),('refused','agent.server.set',{'enabled':False})]
   for name,command,params in cases:
    t.cdp.js("window.reviewLabels=[];window.reviewObserver?.disconnect();window.reviewObserver=new MutationObserver(()=>{window.reviewLabels.push(document.querySelector('[data-tt-id=agent]')?.textContent)});window.reviewObserver.observe(document.querySelector('[data-tt-id=agent]'),{subtree:true,childList:true,characterData:true,attributes:true});true")
    samples=[];answers=[]
    for _ in range(8):
     task=asyncio.create_task(asyncio.to_thread(q.tool,'batch',params) if command=='batch' else call(q,command,params))
     while not task.done():
      samples.append(text('agent'));await asyncio.sleep(.01)
     answer=await task;answers.append({k:answer[k] for k in ('status','error','jobId') if k in answer})
     for _ in range(30):samples.append(text('agent'));await asyncio.sleep(.02)
    samples.extend(t.cdp.js('window.reviewLabels'));t.cdp.js('window.reviewObserver.disconnect();true')
    expected='Review fifty steps' if command=='batch' else labels.get(command)
    t.run.check(bool(expected) and any(expected in (x or '') for x in samples) and not any('('+command+')' in (x or '') for x in samples),'D-62 indicator '+name,{'expected':expected,'samples':list(dict.fromkeys(samples)),'answers':answers})
 finally:
  try:await drain()
  except Exception:pass
  for client in clients:await asyncio.to_thread(client.close)
  for task in tasks:
   if not task.done():task.cancel()

if __name__=='__main__':
 case,step=sys.argv[1],int(sys.argv[2])
 if case in ('m2a','m2b'):r1.live_body=live2
 else:
  assert (case,step) in [('m1a',2),('m1a',3),('m1a',4),('m1b',5),('m1b',6),('m1b',7)]
  r1.live_body=body
 old_receipt=r1.EVID/('step'+str(step)+'.json')
 saved_receipt=old_receipt.read_bytes() if old_receipt.exists() else None
 result=r1.live(step)
 if case in ('m2a','m2b'):
  if saved_receipt is not None:old_receipt.write_bytes(saved_receipt)
  else:old_receipt.unlink(missing_ok=True)
 if case in ('m2a','m2b'):
  transcript=json.loads((r1.OUT/'transcript.json').read_text())
  receipt={'checks':len(transcript),'results':transcript,'raw':str(r1.OUT)}
  (r1.EVID/(case+'-step'+str(step)+'.json')).write_text(json.dumps(receipt,indent=2,default=str)+'\n',encoding='utf-8',newline='\n')
 raise SystemExit(result)
