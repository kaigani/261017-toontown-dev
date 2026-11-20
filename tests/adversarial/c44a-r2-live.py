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
if __name__=='__main__':
 assert (sys.argv[1],int(sys.argv[2])) in [('m1a',2),('m1a',3),('m1a',4),('m1b',5),('m1b',6),('m1b',7)]
 r1.live_body=body
 raise SystemExit(r1.live(int(sys.argv[2])))
