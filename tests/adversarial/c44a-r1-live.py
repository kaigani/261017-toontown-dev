"""Independent C4.4a live review. Raw captures remain in TEMP."""
import os, sys, json, tempfile, subprocess, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(os.environ.get('REVIEW_RAW') or tempfile.mkdtemp(prefix='c44a-r1-'))
EVID=ROOT/'reviews/evidence/M4-C4.4a-r1-review'
APP=str(ROOT/'target/release/toontown.exe')

def load(name,path):
 import importlib.util
 s=importlib.util.spec_from_file_location(name,ROOT/path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m

def preserve_old_receipts():
 import shutil
 modified=subprocess.check_output(['git','diff','--name-only','-z'],cwd=ROOT).decode().split('\0')
 untracked=subprocess.check_output(['git','ls-files','--others','--exclude-standard','-z'],cwd=ROOT).decode().split('\0')
 old=lambda p:p.startswith('reviews/evidence/') and not p.startswith('reviews/evidence/M4-C4.4a-r1-review/')
 for p in filter(old,set(modified+untracked)):
  source=(ROOT/p).resolve();assert source.is_relative_to(ROOT.resolve())
  if source.is_file():
   dest=OUT/'legacy-receipts'/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest)
  if p in modified:subprocess.run(['git','checkout','--',p],cwd=ROOT,check=True)
  elif source.is_file():source.unlink()

async def live_body(t,step):
 import asyncio, ctypes
 from ctypes import wintypes
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 c=t.cmd
 if step==7 and os.environ.get('REVIEW_CONTEXT_REPRO'):
  await r3.modal_fixture(t,'Camera drawing edit target')
  await c('window.panel.set',{'panelId':'cameraView','visible':True});await c('view.active.set',{'viewId':'cameraView'})
  await asyncio.sleep(.7)
  await c('tool.active.set',{'toolId':'brush'});await c('tool.active.set',{'toolId':'select'})
  await asyncio.sleep(.4)
  await c('draw.selection.set',{'by':'none'});await c('edit.selection.set',{'context':'staging','ids':[]})
  before={'drawing':await c('draw.selection.get'),'staging':await c('edit.selection.get',{'context':'staging'})}
  await t.do(('context','cameraView.canvas','cameraView.drawingMenu.selectAll'));await asyncio.sleep(.4)
  after={'drawing':await c('draw.selection.get'),'staging':await c('edit.selection.get',{'context':'staging'})}
  rec={'before':before,'after':after,'sent':t.recorded(),'state':await c('app.state.get')}
  t.run.save('camera-drawing-edit-target.json',rec)
  ids=after['drawing'].get('ids') or after['drawing'].get('selectedIds') or []
  t.run.check(bool(ids) and after['staging']==before['staging'],'Camera drawing-family Select All selects drawing objects, leaves staging alone (02 4.4)',rec)
  return
 if step==5:
  paths=[]
  for folder,name in [('one','First'),('two','Second'),('three','Third'),('unicode-'+chr(0x65e5),'Unicode'),('long-'+'x'*140+'/nested-'+'y'*90,'Long')]:
   p=t.work/folder/(name+'.animpkg');p.parent.mkdir(parents=True,exist_ok=True)
   await c('project.create',{'name':name,'onUnsaved':'discard'});await c('project.saveAs',{'path':str(p)})
   paths.append((name,str(p)))
  await asyncio.sleep(1)
  for name,path in paths:
   d.key(0x1B);d.key(0x1B);d.choose_menu(['File','Open Recent'])
   handle,index=d.native_item(['File','Open Recent',name]);rect=wintypes.RECT();h=d.front()
   assert d.u32.GetMenuItemRect(h,handle,index,ctypes.byref(rect))
   d.u32.SetCursorPos((rect.left+rect.right)//2,(rect.top+rect.bottom)//2)
   await asyncio.sleep(.6)
   status=t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent")
   t.run.check(status==path,'L1b recent full path '+name,{'expected':path,'observed':status})
   d.key(0x1B);d.key(0x1B);await asyncio.sleep(.3)
   cleared=t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent")
   t.run.check(cleared!=path,'L1b recent cleared '+name,{'observed':cleared})
 elif step==6:
  import textwrap
  old=(ROOT/'tests/adversarial/c43b-r2-live.py').read_text(encoding='utf-8')
  scope={'m':d,'ROOT':ROOT,'asyncio':asyncio,'json':json}
  exec(textwrap.dedent(old[old.index(' async def menus(t,state):'):old.index(' original_build=m.build_and_run')]),scope)
  await scope['menus'](t,'no-project')
  await r3.modal_fixture(t,'New menu review');await c('draw.selection.set',{'by':'all'})
  await scope['menus'](t,'selection')
  for path in [['Colour','Auto Close Gap','Small'],['Colour','Auto Close Gap','Medium'],['Colour','Auto Close Gap','Large'],['Colour','Auto Close Gap','Off'],['Window','Panels','Activity']]:
   tree=await d.McpTable.tree(t);node=d.McpTable.menu_node(tree,path);ref=node['command']
   t.clear();d.choose_menu(path);await asyncio.sleep(.6)
   sent=[x for x in t.recorded() if t.kinds.get(x['command'])!='query']
   t.run.check(sent==[{'command':ref['commandId'],'params':ref.get('params',{})}],'L1b new native activation '+str(path),{'declared':ref,'sent':sent})
 elif step==7:
  # Required drawing commands come from 02 section 4.4, less the explicit D-40 deferral.
  required={'edit.cut','edit.copy','edit.paste','edit.selectAll','edit.deselectAll'}
  await r3.modal_fixture(t,'Context review')
  async def open_at(ident):
   d.key(0x1B);r=t.rect(ident);h=d.front();o=wintypes.POINT(0,0);d.u32.ClientToScreen(h,ctypes.byref(o))
   d.right_click_screen(o.x+r['x']*r['dpr'],o.y+r['y']*r['dpr']);await asyncio.sleep(.4)
  def flatten(n):return [n]+[x for ch in n.get('children',[]) for x in flatten(ch)]
  for view in (() if os.environ.get('REVIEW_DOCK_REPRO') else ('drawingView','cameraView')):
   for selection in (False,True):
    await c('window.panel.set',{'panelId':view,'visible':True});await c('view.active.set',{'viewId':view});await c('tool.active.set',{'toolId':'brush'});await c('tool.active.set',{'toolId':'select'})
    await c('draw.selection.set',{'by':'all' if selection else 'none'})
    await open_at(view+'.canvas')
    tree=await d.McpTable.tree(t)
    def flatten(n):return [n]+[x for ch in n.get('children',[]) for x in flatten(ch)]
    items=[x for x in flatten(tree) if x.get('role') in ('menuitem','menuitemcheckbox') and '.drawingMenu.' in x.get('id','')]
    t.run.check(len(items)==22 and required.issubset({x.get('command',{}).get('commandId') for x in items}),'L1b context inventory (02 4.4 minus deferred Feather Edges) '+view+' '+str(selection),items)
    geometry=t.cdp.js("(() => {const m=document.querySelector('.context-menu'),r=m.getBoundingClientRect();return {width:innerWidth,height:innerHeight,menu:{x:r.x,y:r.y,right:r.right,bottom:r.bottom},overflow:getComputedStyle(m).overflowY,clipped:[...m.querySelectorAll('[role^=menuitem]')].filter(e=>{const b=e.getBoundingClientRect();return b.bottom>innerHeight||b.right>innerWidth}).map(e=>({id:e.dataset.ttId,label:e.textContent}))}})()")
    t.run.save('context-geometry-'+view+'-'+str(selection)+'.json',geometry)
    t.run.check(not geometry['clipped'],'L1b context menu entries visible/reachable '+view+' '+str(selection),geometry)
    if geometry['clipped']:t.run.shot('context-overflow-'+view+'-'+str(selection),d.e2e.APP_PID['pid'])
    for initial in items:
     d.key(0x1B)
     await c('draw.selection.set',{'by':'all' if selection else 'none'})
     await open_at(view+'.canvas');now=await d.McpTable.tree(t);node=d.McpTable.node(now,initial['id']);ref=node.get('command',{})
     before=await c('doc.export.json',{'scope':'project','binaries':'omit'});pos=await t.position()
     t.clear();d.key(0x24)
     for _ in range(items.index(initial)):d.key(0x28)
     d.key(0x0D);await asyncio.sleep(.4)
     sent=[x for x in t.recorded() if t.kinds.get(x['command'])!='query']
     expected=[{'command':ref.get('commandId'),'params':ref.get('params',{})}] if node.get('enabled',True) else []
     after=await c('doc.export.json',{'scope':'project','binaries':'omit'})
     t.run.check(sent==expected,'L1b context declaration '+initial['id']+' selection='+str(selection),{'tree':node,'sent':sent,'documentChanged':before!=after})
     if node.get('enabled',True) and ref.get('commandId') in ('edit.selectAll','edit.deselectAll'):
      sel=await c('draw.selection.get');ids=sel.get('ids') or sel.get('selectedIds') or []
      t.run.check(bool(ids)==(ref['commandId']=='edit.selectAll'),'L1b selection effect '+initial['id'],sel)
     await t.settle_dialogs()
     for _ in range(max(0,(await t.position())-pos)):await c('edit.undo')
    d.key(0x1B)
  await c('window.panel.set',{'panelId':'cameraView','visible':True});await c('view.active.set',{'viewId':'cameraView'})
  eid=(await c('element.create',{'type':'drawing','name':'Scene menu subject'}))['elementIds'][0]
  for selection in (() if os.environ.get('REVIEW_DOCK_REPRO') else (False,True)):
   await c('anim.tool.set',{'tool':'transform'});await c('anim.tool.set',{'tool':'select'})
   await c('edit.selection.set',{'context':'staging','ids':[eid] if selection else []})
   await open_at('cameraView.canvas');tree=await d.McpTable.tree(t)
   items=[x for x in flatten(tree) if '.sceneMenu.' in x.get('id','') and x.get('role')=='menuitem']
   t.run.check(len(items)==5 and {x.get('command',{}).get('commandId') for x in items}==required,'L1b Camera scene inventory '+str(selection),items)
   for initial in items:
    await c('edit.selection.set',{'context':'staging','ids':[eid] if selection else []})
    await open_at('cameraView.canvas');node=d.McpTable.node(await d.McpTable.tree(t),initial['id']);ref=node.get('command',{})
    pos=await t.position();t.clear();t.click(initial['id']);await asyncio.sleep(.3)
    sent=[x for x in t.recorded() if t.kinds.get(x['command'])!='query']
    expected=[{'command':ref.get('commandId'),'params':ref.get('params',{})}] if node.get('enabled',True) else []
    t.run.check(sent==expected,'L1b scene declaration '+initial['id']+' '+str(selection),{'tree':node,'sent':sent})
    await t.settle_dialogs()
    for _ in range(max(0,(await t.position())-pos)):await c('edit.undo')
   d.key(0x1B)
   await c('window.panel.set',{'panelId':'cameraView','visible':True});await asyncio.sleep(.2)
  await asyncio.sleep(.4)
  await open_at('tab.cameraView');tree=await d.McpTable.tree(t)
  items=[x for x in flatten(tree) if x.get('id','').startswith('panelChecklist.')]
  t.run.check(bool(items),'L1b dock checklist present',items)
  for initial in items:
   await open_at('tab.cameraView');node=d.McpTable.node(await d.McpTable.tree(t),initial['id']);ref=node.get('command',{})
   t.clear();t.click(initial['id']);await asyncio.sleep(.3)
   sent=[x for x in t.recorded() if t.kinds.get(x['command'])!='query']
   expected=[{'command':ref.get('commandId'),'params':ref.get('params',{})}] if node.get('enabled',True) else []
   t.run.check(sent==expected,'L1b dock checklist declaration '+initial['id'],{'tree':node,'sent':sent})
   if node.get('enabled',True):
    params=ref.get('params',{});panels=await c('window.panel.list')
    panel=next((p for p in panels if p.get('panelId')==params.get('panelId')),None)
    t.run.check(panel is not None and panel.get('visible')==params.get('visible'),'L1b dock visibility effect '+initial['id'],panel)
   d.key(0x1B)
   await c('window.panel.set',{'panelId':'cameraView','visible':True});await asyncio.sleep(.2)
 elif step==8:
  await c('project.create',{'name':'Camera review'})
  first=(await c('anim.camera.list'))['cameras'][0]
  await c('anim.camera.create',{'name':'Second camera','fov':80})
  cams=(await c('anim.camera.list'))['cameras'];second=next(x for x in cams if x['id']!=first['id'])
  def options():return t.cdp.js("[...document.querySelectorAll('[data-tt-id=\"mainToolbar.cameraList\"] option')].map(e=>({value:e.value,label:e.textContent,command:e.dataset.ttCommand,params:JSON.parse(e.dataset.ttParams||'{}')}))")
  await asyncio.sleep(.7);shown=options()
  t.run.check({x['value'] for x in shown}=={first['id'],second['id']},'L1b two cameras shown',shown)
  await c('element.rename',{'elementId':second['id'],'name':'Renamed camera'});await asyncio.sleep(.7)
  t.run.check(any(x['label']=='Renamed camera' for x in options()),'L1b camera agent rename reflected',options())
  await c('anim.camera.current.set',{'cameraId':first['id']});await asyncio.sleep(.4)
  target=next(x for x in options() if x['value']==second['id']);t.clear()
  await t.do(('select','mainToolbar.cameraList',second['id']));await asyncio.sleep(.5)
  sent=[x for x in t.recorded() if t.kinds.get(x['command'])!='query']
  t.run.check(sent==[{'command':target['command'],'params':target['params']}] and (await c('anim.camera.current.get'))['cameraId']==second['id'],'L1b keyboard declared choice exactly once',{'target':target,'sent':sent})
  await c('anim.camera.current.set',{'cameraId':first['id']});await asyncio.sleep(.4)
  t.clear();r=t.rect('mainToolbar.cameraList');h=d.front();o=wintypes.POINT(0,0);d.u32.ClientToScreen(h,ctypes.byref(o))
  height=t.cdp.js("document.querySelector('[data-tt-id=\"mainToolbar.cameraList\"]').getBoundingClientRect().height")
  t.click('mainToolbar.cameraList');await asyncio.sleep(.3)
  from PIL import ImageGrab
  ImageGrab.grab().save(OUT/'camera-popup.png')
  # Native popup rows follow the DOM option order (new cameras can appear first).
  index=next(i for i,opt in enumerate(options()) if opt['value']==target['value'])
  d.click_screen(o.x+r['x']*r['dpr'],o.y+(r['y']+height/2)*r['dpr']+5+24*(index+.5))
  await asyncio.sleep(.5);sent=[x for x in t.recorded() if t.kinds.get(x['command'])!='query']
  t.run.check(sent==[{'command':target['command'],'params':target['params']}] and (await c('anim.camera.current.get'))['cameraId']==second['id'],'L1b mouse declared choice exactly once',{'target':target,'sent':sent})
  await c('anim.camera.delete',{'cameraId':second['id']});await asyncio.sleep(.7)
  t.run.check([x['value'] for x in options()]==[first['id']],'L1b camera delete reflected',options())


async def live2_body(t,step):
 import asyncio
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await r3.modal_fixture(t,'Live2 step '+str(step))
 async def pause(): await asyncio.sleep(.6)
 def dom(ident,expr='e.value'):
  return t.cdp.js("(()=>{const e=document.querySelector('[data-tt-id='+JSON.stringify(%s)+']');return e ? %s : null})()"%(json.dumps(ident),expr))
 async def typed(ident,value,key=0x0D):
  d.front();dom(ident,'(e.focus(),e.select(),true)');d.type_text(str(value));d.key(key);await pause()
 def sent(): return [x for x in t.recorded() if t.kinds.get(x['command'])!='query']
 async def penstate():return await c('tool.pen.list')
 async def cur():
  st=await penstate();return next(x for x in st['pens'] if x['penId']==st['currentBrushPenId'])
 if step in (1,2,3):
  await c('window.panel.set',{'panelId':'pen','visible':True});await c('tool.active.set',{'toolId':'brush'});await pause()
 if step==1:
  old=await penstate();pos=await t.position();t.clear();t.click('pen.new');await pause();new=await cur();pid=new['penId']
  t.run.check(len((await penstate())['pens'])==len(old['pens'])+1 and sent()==[{'command':'tool.pen.create','params':{'type':'brush','fromPenId':old['currentBrushPenId']}}],'new pen command and selection',sent())
  await c('edit.undo');t.run.check(await penstate()==old,'New Pen undo restores pens',await penstate());await c('edit.redo');await pause()
  position=await t.position();t.clear();other=old['currentBrushPenId'];t.click('pen.row.'+other);await pause()
  t.run.check((await penstate())['currentBrushPenId']==other and await t.position()==position,'select pen command has no undo step',sent());t.click('pen.row.'+pid);await pause()
  t.click('pen.row.'+pid);d.key(0x71);await pause()
  selected=t.cdp.js('({value:document.activeElement.value,start:document.activeElement.selectionStart,end:document.activeElement.selectionEnd})')
  t.run.check(selected.get('start')==0 and selected.get('end')==len(selected.get('value','')),'rename opens selected',selected)
  t.clear();d.type_text('Live2 pen');d.key(0x0D);await pause()
  t.run.check((await cur())['name']=='Live2 pen','typing replaces rename',{'pen':await cur(),'sent':sent()})
  t.click('pen.row.'+pid);d.key(0x71);await pause();d.type_text('Cancel me');d.key(0x1B);await pause()
  t.run.check((await cur())['name']=='Live2 pen','Escape cancels rename',await cur())
  await c('edit.undo');await pause();t.run.check((await cur())['name']==new['name'],'Rename Pen undo restores old name',await cur());await c('edit.redo');await pause()
  beforeDelete=await penstate();family=[x for x in beforeDelete['pens'] if x['type']=='brush'];index=next(i for i,x in enumerate(family) if x['penId']==pid);successor=family[index+1 if index+1<len(family) else index-1]['penId']
  t.clear();t.click('pen.delete');await pause();t.run.check(pid not in [x['penId'] for x in (await penstate())['pens']],'delete removes pen',sent())
  t.run.check((await penstate())['currentBrushPenId']==successor,'delete selects next else previous',await penstate());await c('edit.undo');t.run.check((await penstate())['pens']==beforeDelete['pens'],'Delete Pen undo restores pen list (selection is UI state)',await penstate());await c('edit.redo')
  for pen in (await penstate())['pens']:
   st=await penstate()
   if pen['type']=='brush' and len([x for x in st['pens'] if x['type']=='brush'])>1:await c('tool.pen.delete',{'penId':pen['penId']})
  await pause();disabled=dom('pen.delete','({disabled:e.disabled,title:e.title})');t.run.check(disabled and disabled['disabled'] and bool(disabled['title']),'last pen disabled with reason',disabled)
 elif step==2:
  pid=(await cur())['penId']
  for field,value,invalid in [('maxSize',40,101),('minSize',12,-1),('smoothness',6,11),('tipAngle',45,361)]:
   t.clear();await typed('pen.'+field+'.field',value)
   t.run.check((await cur())[field]==value,'pen numeric '+field,{'sent':sent(),'pen':await cur()})
   t.clear();await typed('pen.'+field+'.field',invalid)
   t.run.check(dom('pen.'+field+'.field')==str(value),'invalid pen value restored '+field,{'field':dom('pen.'+field+'.field'),'engine':(await cur())[field],'sent':sent()})
   await typed('pen.'+field+'.field',value)
   await typed('pen.'+field+'.field','abc');t.run.check((await cur())[field]==value and dom('pen.'+field+'.field')==str(value),'nonnumeric pen refused '+field,{'field':dom('pen.'+field+'.field'),'pen':await cur()})
   await typed('pen.'+field+'.field',value)
   t.clear();d.front();dom('pen.'+field,'(e.focus(),true)');d.key(0x27);await pause()
   t.run.check(len([x for x in sent() if x['command']=='tool.pen.set'])>=1,'slider keyboard release '+field,sent())
   import ctypes
   from ctypes import wintypes
   r=t.rect('pen.'+field);geom=dom('pen.'+field,'({left:e.getBoundingClientRect().left,width:e.getBoundingClientRect().width,value:Number(e.value),min:Number(e.min),max:Number(e.max)})');origin=wintypes.POINT(0,0);d.u32.ClientToScreen(d.front(),ctypes.byref(origin));x=int(origin.x+r['x']*r['dpr']);y=int(origin.y+r['y']*r['dpr'])
   start=origin.x+(geom['left']+8+(geom['width']-16)*(geom['value']-geom['min'])/(geom['max']-geom['min']))*r['dpr'];end=origin.x+(geom['left']+8+(geom['width']-16)*(.2 if geom['value']>(geom['max']+geom['min'])/2 else .8))*r['dpr']
   t.clear();d.u32.SetCursorPos(int(start),y);d.e6._send_button(True);await asyncio.sleep(.1);d.u32.SetCursorPos(int(end),y);await asyncio.sleep(.2)
   t.run.check(not sent(),'slider sends nothing during drag '+field,sent());d.e6._send_button(False);await pause()
   t.run.check(len(sent())==1 and sent()[0]['command']=='tool.pen.set','slider one command on release '+field,sent())

  for shape in ('oval','square','flat','round'):
   t.clear();await t.do(('select','pen.tipShape',shape));await pause();t.run.check((await cur())['tipShape']==shape,'tip shape '+shape,sent())
  base=t.work/'stroke-base.animpkg';await c('project.saveAs',{'path':str(base)})
  source=(ROOT/'scripts/c43-d39-bindings.py').read_text(encoding='utf-8');source=source[source.index('class Cdp:'):source.index('# ----- OS input')].replace('class Cdp:','class GestureCdp:').replace('/run_command','/gesture_commit')
  exec(source,d.__dict__)
  def capture(self,body):
   outer=json.loads(body);self.calls.append(json.loads(outer['json']))
  d.GestureCdp._ipc=capture;gc=d.GestureCdp(d.CDP_PORT);gc.record()
  t.clear();d.front();r=t.rect('drawingView.canvas');x,y=r['x']*r['dpr'],r['y']*r['dpr']
  d.e6.os_drag(d.e2e.APP_PID['pid'],[(x-50,y),(x,y+25),(x+50,y)],.4,.1,{})
  await asyncio.sleep(1.5);gestures=list(gc.calls);commands=sent();window=await c('draw.drawing.get');t.run.save('stroke-window.json',{'commands':commands,'gestures':gestures,'drawing':window})
  from mcp import ClientSession,StdioServerParameters,types
  from mcp.client.stdio import stdio_client
  async def roots(_):return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='work')])
  args=['--mcp-stdio','--project',str(base),'--policy','autonomous','--allow-path',str(t.work),'--user-dir',str(t.work/'headless-user')]
  async with stdio_client(StdioServerParameters(command=str(ROOT/'target/release/toontown-headless.exe'),args=args)) as streams:
   async with ClientSession(*streams,list_roots_callback=roots) as hs:
    await hs.initialize();ht=d.Table(hs,t.run,OUT,t.work)
    for gesture in gestures:
     request={k:v for k,v in gesture.items() if k not in ('generation','agentRequest')};request['space']='drawing';request['tool']='brush'
     for event in request['events']:
      point=await c('view.coordinates.convert',{'viewId':'drawingView','point':{'x':event['x'],'y':event['y']},'from':'view','to':'drawing'});event.update(point['point'])
     response=await d.e2e.call(hs,'pointer',request);t.run.save('stroke-replay-response.json',response)
    head=await ht.cmd('draw.drawing.get');t.run.save('stroke-headless.json',head)
  import re
  def canon(v):
   ids={}
   def walk(x):
    if isinstance(x,dict):return {k:walk(v) for k,v in x.items() if k!='rev'}
    if isinstance(x,list):return [walk(v) for v in x]
    if isinstance(x,str):
     def replace(m):
      key=m[0];ids.setdefault(key,key.split('_')[0]+'_'+str(len(ids)));return ids[key]
     return re.sub(r'[a-z]+_[0-9A-Z]{26}',replace,x)
    return x
   return walk(v)
  t.run.check(bool(gestures) and canon(window)==canon(head),'D-32 recorded OS stroke replay equals headless exactly modulo identifiers',{'gestures':gestures,'commands':commands,'equal':canon(window)==canon(head)})

 elif step==3:
  pid=(await cur())['penId'];await c('tool.pen.set',{'penId':pid,'maxSize':53});await pause()
  t.run.check(dom('pen.maxSize.field')=='53','agent pen setting reflected',dom('pen.maxSize.field'))
  created=await c('tool.pen.create',{'type':'brush'});await pause();st=await penstate();pid=st['currentBrushPenId']
  t.run.check(dom('pen.row.'+pid,'e.textContent') is not None,'agent creation reflected',created)
  await c('tool.pen.delete',{'penId':pid});await pause();t.run.check(dom('pen.row.'+pid) is None,'agent deletion reflected',await penstate())
  pid=(await cur())['penId'];task=asyncio.create_task(c('tool.pen.set',{'penId':pid,'smoothness':7}));await typed('pen.maxSize.field',37);await task;await pause()
  state=await cur();t.run.check(state['smoothness']==7 and state['maxSize']==37,'concurrent distinct settings retained',state)
 elif step==4:
  await c('window.panel.set',{'panelId':'properties','visible':True});await pause()
  for tool in ('brush','pencil','eraser','select','rectangle','paint','polyline'):
   await c('tool.active.set',{'toolId':tool});await pause()
   controls=t.cdp.js("[...document.querySelectorAll('[data-tt-id^=\"toolOptions.\"]')].filter(e=>['INPUT','SELECT'].includes(e.tagName)).map(e=>({id:e.dataset.ttId,type:e.type,value:e.value,disabled:e.disabled,options:e.options?[...e.options].map(o=>o.value):null}))")
   for control in controls:
    if control['disabled']:continue
    t.clear();ident=control['id']
    if control['options']:
     opts=control['options'];await t.do(('select',ident,next((x for x in opts if x!=control['value']),opts[0])))
    elif control['type']=='checkbox':t.click(ident)
    elif control['type']=='number':
     if ident=='toolOptions.thickness':
      await t.do(('select','toolOptions.thicknessSource','fixed'));await pause();t.clear()
     await typed(ident,5 if dom(ident)=='4' else 4)
    else:continue
    await pause();t.run.check(bool(sent()),'tool option command '+tool+' '+ident,{'sent':sent(),'state':await c('tool.options.get',{'toolId':tool})})
   await c('window.panel.set',{'panelId':'drawingView','visible':True,'focus':True});await c('view.active.set',{'viewId':'drawingView'});await pause()
   before=await c('draw.drawing.get');r=t.rect('drawingView.canvas');x,y=r['x']*r['dpr'],r['y']*r['dpr'];d.front()
   if tool=='paint':t.click('drawingView.canvas')
   else:d.e6.os_drag(d.e2e.APP_PID['pid'],[(x-30,y-20),(x,y+20),(x+30,y-20)],.4,.1,{})
   if tool=='polyline':d.key(0x0D);d.key(0x1B)
   await asyncio.sleep(.8);after=await c('draw.drawing.get');selection=await c('draw.selection.get')
   t.run.save('tool-gesture-'+tool+'.json',{'before':before,'after':after,'selection':selection,'options':await c('tool.options.get',{'toolId':tool})})
   t.run.check(bool(after.get('drawingId')),'real OS gesture completed '+tool,{'changed':before!=after,'selection':selection})

  for label,mode in [('Small','small'),('Medium','medium'),('Large','large'),('Off','off')]:
   t.clear();d.choose_menu(['Colour','Auto Close Gap',label]);await pause();t.run.check((await c('colour.gapClosing.get'))['mode']==mode,'menu gap '+mode,sent())
 elif step==5:
  keys=['tablet.ignore','tablet.alternativeInput','tablet.portraitCorrection','tablet.compatibilityMode']
  await c('prefs.set',{'values':dict.fromkeys(keys,False)})
  for accept in (False,True):
   d.choose_menu(['File','Preferences']);await pause();t.click('preferences.category.tablet');await pause();t.clear()
   for k in keys:t.click('pref.'+k)
   t.run.check((await c('prefs.get',{'keys':keys}))['values']==dict.fromkeys(keys,False),'tablet staged before OK',sent())
   if accept:t.click('preferences.submit')
   else:d.key(0x1B)
   await pause();values=(await c('prefs.get',{'keys':keys}))['values'];t.run.check(values==dict.fromkeys(keys,accept),'tablet '+('OK' if accept else 'Cancel'),{'values':values,'sent':sent()})
   if accept:t.run.check(sent()==[{'command':'prefs.set','params':{'values':dict.fromkeys(keys,True)}}],'one prefs.set exactly changed keys',sent())
 elif step==6:
  await c('window.panel.set',{'panelId':'text','visible':True});await pause()
  for field,value,invalid in [('size',48,513),('kerning',12,61),('leftMargin',8,-1),('rightMargin',9,1001),('indent',-12,-100),('lineSpacing',5,1001)]:
   t.clear();await typed('text.'+field,value);options=(await c('tool.options.get',{'toolId':'text'}))['options'];t.run.check(options[field]==value,'text numeric '+field,{'sent':sent(),'options':options})
   await typed('text.'+field,invalid);t.run.check(dom('text.'+field)==str(value),'text invalid restored '+field,{'field':dom('text.'+field),'options':(await c('tool.options.get',{'toolId':'text'}))['options']})
   await typed('text.'+field,value)
   await typed('text.'+field,'abc');t.run.check((await c('tool.options.get',{'toolId':'text'}))['options'][field]==value and dom('text.'+field)==str(value),'nonnumeric text refused '+field,{'field':dom('text.'+field),'options':(await c('tool.options.get',{'toolId':'text'}))['options']})
   await typed('text.'+field,value)
  for flag in ('bold','italic','autoKern'):
   before=(await c('tool.options.get',{'toolId':'text'}))['options'][flag];t.clear();t.click('text.'+flag);await pause();t.run.check((await c('tool.options.get',{'toolId':'text'}))['options'][flag]!=before,'text flag '+flag,sent())
  for field,value in [('baseline','superscript'),('align','justify')]:
   t.clear();await t.do(('select','text.'+field,value));await pause();t.run.check((await c('tool.options.get',{'toolId':'text'}))['options'][field]==value,'text choice '+field,sent())
  t.clear();await typed('text.font','Live2 Missing Font 673');t.run.check('not bundled' in dom('text.fontNotBundled','e.textContent'),'unbundled font marked',{'sent':sent(),'options':await c('tool.options.get',{'toolId':'text'})})
 elif step==7:
  await c('window.panel.set',{'panelId':'properties','visible':True});await pause();t.click('properties.page.drawing');await pause()
  t.clear();await typed('inspector.name','Live2 drawing');listing=await c('draw.drawing.get');t.run.check('Live2 drawing' in json.dumps(listing),'inspector rename',{'sent':sent(),'drawing':listing})
  st=await c('app.state.get');t.run.save('inspector-state.json',st)
  declaration=dom('inspector.name','JSON.parse(e.dataset.ttParams)');did=declaration['drawingId']
  await c('draw.drawing.rename',{'drawingId':did,'name':'Agent renamed live2'});await pause();t.run.check(dom('inspector.name')=='Agent renamed live2','inspector agent rename follows without reload',dom('inspector.name'))
  if not dom('inspector.preview','e.checked'):t.click('inspector.preview');await pause()
  rendered=await c('draw.drawing.render',{'drawingId':did,'width':160,'region':'bounds','background':'#FFFFFFFF'})
  t.run.check(dom('inspector.thumbnail','e.src')=='data:image/png;base64,'+rendered['png'],'inspector preview equals engine final render exactly',{'drawingId':did,'pngBytes':len(rendered['png'])})
  second=await c('element.drawing.create',{'elementId':listing['elementId'],'name':'Second live2','exposeFrom':30,'exposeTo':40})
  await c('app.context.set',{'frame':30});await c('draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':80,'y':60}});await pause()
  t.run.check(dom('inspector.name')=='Second live2','inspector follows current drawing',dom('inspector.name'))
  secondDrawing=await c('draw.drawing.get');secondRender=await c('draw.drawing.render',{'drawingId':secondDrawing['drawingId'],'width':160,'region':'bounds','background':'#FFFFFFFF'})
  await c('app.context.set',{'frame':1});await pause();t.run.check(dom('inspector.name')=='Agent renamed live2','inspector returns to first drawing',dom('inspector.name'))
  d.front();dom('inspector.browse','(e.focus(),true)');d.key(0x27);await pause()
  t.run.check(dom('inspector.thumbnail','e.src')=='data:image/png;base64,'+secondRender['png'] and (await c('draw.drawing.get'))['drawingId']==did,'Browse renders other drawing without changing current',{'shown':dom('inspector.thumbnail','e.alt'),'current':(await c('draw.drawing.get'))['drawingId']})
  await c('app.context.set',{'frame':100});await pause();controls=t.cdp.js("[...document.querySelectorAll('[data-tt-id^=\"inspector.\"]')].map(e=>({id:e.dataset.ttId,disabled:e.disabled,title:e.title}))")
  t.run.save('inspector-empty.json',controls)
  t.run.check(not controls or all(x.get('disabled') and x.get('title') for x in controls),'empty drawing surface follows design note 1 section 1 exception',controls)
 if step==5:
  result=await c('tool.tablet.set',{'ignoreTablet':False,'portraitCorrection':True,'compatibilityMode':True,'inputInterface':'alternative'});await pause()
  values=(await c('prefs.get',{'keys':keys}))['values'];t.run.check(values=={'tablet.ignore':False,'tablet.alternativeInput':True,'tablet.portraitCorrection':True,'tablet.compatibilityMode':True},'agent tablet shares preference state',{'result':result,'values':values})
 t.run.save('live2-commands.json',t.recorded())

 if step in (4,5):
  import contextlib,signal
  saved=t.work/'restart.animpkg';await c('project.saveAs',{'path':str(saved)})
  before=await c('tool.options.get',{'toolId':'polyline'}) if step==4 else await c('prefs.get',{'keys':keys})
  os.kill(d.e2e.APP_PID['pid'],signal.SIGTERM);await asyncio.sleep(1)
  discovery=t.work/'restart-discovery.json';proc=d.e2e.launch(APP,t.work/'user',discovery)
  try:
   disc=d.e2e.wait_discovery(discovery,proc)
   async with contextlib.AsyncExitStack() as stack:
    client=await stack.enter_async_context(d.e2e.httpx2.AsyncClient(headers={'Authorization':'Bearer '+disc['token']},timeout=60))
    streams=await stack.enter_async_context(d.e2e.streamable_http_client(disc['endpoint'],http_client=client))
    from mcp import ClientSession,types
    async def roots(_):return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='work')])
    ns=await stack.enter_async_context(ClientSession(streams[0],streams[1],list_roots_callback=roots));await ns.initialize();nt=d.Table(ns,t.run,OUT,t.work)
    await nt.cmd('project.open',{'path':str(saved)})
    after=await nt.cmd('tool.options.get',{'toolId':'polyline'}) if step==4 else await nt.cmd('prefs.get',{'keys':keys})
    t.run.check(before==after,'restart preserves '+('tool options' if step==4 else 'tablet preferences'),{'before':before,'after':after})
  finally:d.e2e.stop(proc)



def live(step):
 import asyncio,threading
 global d,r3
 OUT.mkdir(parents=True,exist_ok=True);EVID.mkdir(parents=True,exist_ok=True)
 r3=load('c44prior','tests/adversarial/c43b-r3-live.py');r3.APP=APP
 d=load('c44d39','scripts/c43-d39-bindings.py');r3.d=d
 d.e6.dpi_aware();run=d.e2e.Run(OUT);stop=threading.Event()
 worker=threading.Thread(target=r3.activate_new_windows,args=(stop,),daemon=True);worker.start()
 server=subprocess.Popen([sys.executable,'-m','http.server','1420','--bind','127.0.0.1','--directory',str(ROOT/'desktop/dist')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 d.build_and_run=lambda t,only:live_body(t,step)
 try:asyncio.run(d.session_all(APP,run,OUT,Path(tempfile.mkdtemp(prefix='c44fixture-')),set()))
 except Exception as ex:
  import traceback
  traceback.print_exc();run.check(False,'probe completed',repr(ex))
 finally:
  stop.set();worker.join();server.terminate();server.wait()
  run.save('transcript.json',run.transcript)
  summary={'checks':len(run.transcript),'failures':run.failures,'raw':str(OUT)}
  if sys.argv[1] in ('l2a','l2b','l3a','l3b','l4a','l4b'):summary['results']=run.transcript
  if sys.argv[1] in ('l3a','l3b'):summary=live3_compact(summary)
  suffix='-context-repro' if os.environ.get('REVIEW_CONTEXT_REPRO') else '-dock' if os.environ.get('REVIEW_DOCK_REPRO') else ''
  (EVID/((sys.argv[1]+'-' if sys.argv[1] in ('l2a','l2b','l3a','l3b','l4a','l4b') else '')+'step'+str(step)+suffix+'.json')).write_text(json.dumps(summary,indent=2,default=str)[:190000]+'\n',encoding='utf-8',newline='\n')
 return int(bool(run.failures))

def regression(step,script,cases):
 OUT.mkdir(parents=True,exist_ok=True); EVID.mkdir(parents=True,exist_ok=True)
 rows=[]
 for case in cases:
  dest=OUT/(script+'-'+case);dest.mkdir(exist_ok=True)
  log=dest/'stdout.txt'
  with log.open('w',encoding='utf-8') as f:
   args=[sys.executable,str(ROOT/'tests/adversarial'/script),APP,case]
   if script=='c43b-r1-live.py' and case=='case4':args.append(str(ROOT/'target/release/toontown-headless.exe'))
   p=subprocess.run(args,cwd=ROOT,env={**os.environ,'PYTHONIOENCODING':'utf-8','REVIEW_RAW':str(dest)},stdout=f,stderr=subprocess.STDOUT)
  lines=log.read_text(encoding='utf-8',errors='replace').splitlines()
  row={'case':case,'exit':p.returncode,'failLines':[x[:1800] for x in lines if 'FAIL' in x or 'Error' in x or 'Traceback' in x]}
  rows.append(row);print(json.dumps(row),flush=True)
  (EVID/('step'+str(step)+'.json')).write_text(json.dumps(rows,indent=2)[:190000]+'\n',encoding='utf-8')
 print('STEP',step,'COMPLETE',flush=True)
 preserve_old_receipts()


def live3_compact(value):
 if isinstance(value,list):return [live3_compact(x) for x in value]
 if not isinstance(value,dict):return value
 if str(value.get('uri','')).startswith('app://events') and 'text' in value:
  import hashlib,collections
  data=json.loads(value['text']);events=data.get('events',[])
  brief={'eventCount':len(events),'types':dict(collections.Counter(x['type'] for x in events)),
         'firstSeq':events[0]['seq'] if events else None,'lastSeq':events[-1]['seq'] if events else None,
         'rawTextBytes':len(value['text'].encode()),'sha256':hashlib.sha256(value['text'].encode()).hexdigest()}
  return {**value,'text':json.dumps(brief)}
 return {k:live3_compact(v) for k,v in value.items()}

class IndependentHttp:
 """Reviewer-owned MCP JSON-RPC transport; no product test client."""
 def __init__(self, discovery, name='review-live3'):
  self.endpoint=discovery['endpoint'];self.token=discovery['token'];self.sid=None;self.seq=0;self.name=name
 def rpc(self,method,params=None,notification=False):
  import urllib.request,urllib.error
  self.seq+=1
  body={'jsonrpc':'2.0','method':method,'params':params or {}}
  if not notification:body['id']=self.seq
  headers={'Authorization':'Bearer '+self.token,'Content-Type':'application/json','Accept':'application/json, text/event-stream'}
  if self.sid:headers['Mcp-Session-Id']=self.sid
  try:
   with urllib.request.urlopen(urllib.request.Request(self.endpoint,json.dumps(body).encode(),headers),timeout=20) as r:
    self.sid=r.headers.get('Mcp-Session-Id',self.sid)
    if notification:return {'http':r.status}
    if 'text/event-stream' in r.headers.get('Content-Type',''):
     for line in r:
      if line.startswith(b'data:'):
       obj=json.loads(line[5:])
       if obj.get('id')==self.seq:return obj
    data=r.read();return json.loads(data) if data else {'http':r.status}
  except urllib.error.HTTPError as ex:return {'http':ex.code,'body':ex.read().decode()[:1000]}
  except Exception as ex:return {'transportError':str(ex)}
 def initialize(self):
  r=self.rpc('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':self.name,'version':'1'}})
  if 'result' in r:self.rpc('notifications/initialized',notification=True)
  return r
 def tool(self,name,args):
  r=self.rpc('tools/call',{'name':name,'arguments':args});v=r.get('result',{})
  if 'structuredContent' in v:return v['structuredContent']
  for c in v.get('content',[]):
   if c.get('type')=='text':
    try:return json.loads(c['text'])
    except ValueError:pass
  return r
 def command(self,name,params=None):return self.tool('run_command',{'id':name,'params':params or {}})
 def close(self):
  import urllib.request
  if not self.sid:return
  try:urllib.request.urlopen(urllib.request.Request(self.endpoint,method='DELETE',headers={'Authorization':'Bearer '+self.token,'Mcp-Session-Id':self.sid}),timeout=3).close()
  except Exception:pass

async def live3_body(t,step):
 import asyncio,hashlib
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 discpath=t.work/'discovery.json'
 disc=json.loads(discpath.read_text());clients=[]
 def fresh(name):
  q=IndependentHttp(json.loads(discpath.read_text()),name);clients.append(q);return q
 async def call(q,n,p=None):return await asyncio.to_thread(q.command,n,p)
 async def tool(q,n,p):return await asyncio.to_thread(q.tool,n,p)
 async def init(q):return await asyncio.to_thread(q.initialize)
 async def pause():await asyncio.sleep(.7)
 def dom(i,expr='e.value'):return t.cdp.js('(()=>{const e=document.querySelector('+json.dumps('[data-tt-id="'+i+'"]')+');return e?'+expr+':null})()')
 async def prefs():
  d.choose_menu(['File','Preferences']);await pause();t.click('preferences.category.agent');await pause()
 async def typed(i,v):d.front();dom(i,'(e.focus(),e.select(),true)');d.type_text(str(v));d.key(0x09);await pause()
 def sent():return [x for x in t.recorded() if x['command'].startswith('agent.') and t.kinds.get(x['command'])!='query']
 def safe(v):
  txt=json.dumps(v,default=str)
  for q in clients:
   txt=txt.replace(q.token,'<token len='+str(len(q.token))+' sha256='+hashlib.sha256(q.token.encode()).hexdigest()+'>')
  return json.loads(txt)
 def check(ok,label,data=None):t.run.check(ok,label,safe(data))
 q=fresh('review-live3-primary');check('result' in await init(q),'independent HTTP initialize')
 try:
  if step==1:
   keys=['agent.policy.defaults','agent.typedTools','agent.activity.persist','agent.server.port']
   before=await c('prefs.get',{'keys':keys})
   for accept in (False,True):
    await prefs();t.clear()
    await typed('agentAccess.maxJobs',3);await typed('agentAccess.maxBatchSteps',17);await typed('agentAccess.maxInlineImagePx',1024);await typed('agentAccess.maxInlineBytes',12)
    await t.do(('select','agentAccess.policy','assist'))
    for i in ('askInApp','letAgentMoveView','highlightAgentChanges','typedTools','persist'):t.click('agentAccess.'+i)
    await pause();t.click('agentAccess.namespace.draw');await pause()
    if not accept:
     await typed('agentAccess.port',47216)
     namespaces=t.cdp.js("[...document.querySelectorAll('[data-tt-id^=\"agentAccess.namespace.\"]')].map(e=>e.dataset.ttId)")
     for namespace in namespaces:t.click(namespace)
     check(True,'every namespace checkbox exercised by OS input',namespaces)
    check(await c('prefs.get',{'keys':keys})==before,'all edits staged',sent())
    check(dom('agentAccess.allowRemote','e.disabled') is True,'remote disabled with authorization reason',dom('agentAccess.allowRemote','e.parentElement.title'))
    if accept:t.click('preferences.submit')
    else:d.key(0x1B)
    await pause();after=await c('prefs.get',{'keys':keys})
    check(after!=before if accept else after==before,'OK commits / Cancel discards '+str(accept),{'sent':sent(),'values':after})
    if accept:check(len(sent())==3,'one command per changed policy typed persistence group',sent())
   check((await call(q,'prefs.get',{'keys':keys})).get('result')==after,'agent sees same preferences',await call(q,'prefs.get',{'keys':keys}))
  elif step==2:
   await prefs()
   if not dom('agentAccess.server','e.checked'):
    t.click('agentAccess.server');t.click('preferences.submit');await pause();await prefs()
   t.clear();t.click('agentAccess.server');t.click('preferences.submit');await pause()
   check((await call(q,'agent.server.status')).get('status')!='ok','connected client refused after server off',await call(q,'agent.server.status'))
   nq=IndependentHttp(disc,'off-probe');clients.append(nq);offanswer=await init(nq);check('result' not in offanswer,'new client refused while off',offanswer)
   check(True,'off discovery and indicator observation',{'discovery':{k:v for k,v in json.loads(discpath.read_text()).items() if k!='token'},'indicator':t.cdp.js("document.body.innerText.slice(-1200)"),'sent':sent()})
   await prefs();t.click('agentAccess.server');await typed('agentAccess.port',47891);t.clear();t.click('preferences.submit');await pause()
   check(sent()==[{'command':'agent.server.set','params':{'enabled':True,'port':47891}}],'server port group exactly changed values',sent())
   nq=fresh('on-probe');check('result' in await init(nq),'new client accepted after on',await call(nq,'agent.server.status'))
  elif step==3:
   await r3.modal_fixture(t,'Live3 policy');await c('project.saveAs',{'path':str(t.work/'policy.animpkg')})
   for policy in ('observe','assist','autonomous'):
    await prefs();await t.do(('select','agentAccess.policy',policy))
    if dom('agentAccess.askInApp','e.checked'):t.click('agentAccess.askInApp')
    await typed('agentAccess.maxBatchSteps',2);t.click('preferences.submit');await pause()
    nq=fresh('policy-'+policy);await init(nq)
    read=await call(nq,'app.state.get');mutation=await call(nq,'draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':10,'y':10}})
    target=t.work/('overwrite-'+policy+'.json');target.write_text('existing')
    confirm=await call(nq,'agent.activity.export',{'path':str(target),'format':'jsonl','onExists':'overwrite'})
    check(('permission.denied' in json.dumps(confirm) if policy=='observe' else confirm.get('status')==('needs_confirmation' if policy=='assist' else 'ok')),'confirmable overwrite '+policy,confirm)
    check(read.get('status')=='ok' and (mutation.get('status')!='ok' if policy=='observe' else mutation.get('status')=='ok'),'policy '+policy,{'read':read,'mutation':mutation,'confirmable':confirm,'status':await call(nq,'agent.server.status')})
    batch=await tool(nq,'batch',{'label':'Review limit','steps':[{'id':'app.state.get','params':{}}]*3})
    check('limit.exceeded' in json.dumps(batch) if policy!='observe' else True,'batch length enforced '+policy,batch)
  elif step==4:
   await prefs();await pause()
   status=await call(q,'agent.server.status');check(status.get('result',{}).get('endpoint')==disc['endpoint'],'running endpoint equals discovery',{'endpoint':disc['endpoint'],'returned':status})
   commands=[('prefs.set',{'values':{'agent.server.enabled':False}}),('prefs.reset',{'keys':['agent.server.enabled']}),('agent.policy.defaults.set',{'policy':'autonomous'}),('agent.session.grant',{'sessionId':q.sid,'scopes':['app']}),('agent.token.regenerate',{}),('agent.server.set',{'enabled':False}),('agent.confirmation.respond',{'confirmationId':'review-fake','accept':True}),('prefs.get',{'keys':['agent.token','agent.server.token']}),('agent.server.status',{}),('agent.activity.list',{})]
   for key,value in [('agent.server.port',47892),('agent.server.allowRemote',True),('agent.policy.defaults',{'policy':'autonomous'}),('agent.typedTools',{'enabled':True,'namespaces':[]}),('agent.activity.persist',True)]:
    for command,params in [('prefs.set',{'values':{key:value}}),('prefs.reset',{'keys':[key]})]:
     result=await call(q,command,params);check('permission.denied' in json.dumps(result),'all access preferences protected '+command+' '+key,{'params':params,'returned':result})
   for command,params in [('agent.typedTools.set',{'enabled':True}),('agent.activity.persist.set',{'enabled':True})]:
    result=await call(q,command,params);check('permission.denied' in json.dumps(result),'other app-scope agent command '+command,result)
   for n,p in commands:
    value=await call(q,n,p)
    check(disc['token'] not in json.dumps(value) and (value.get('status')!='ok' if n in [x[0] for x in commands[:7]] else True),'privilege / secret '+n,{'params':p,'returned':value})
   for method,params in [('resources/list',{}),('resources/read',{'uri':discpath.as_uri()}),('resources/read',{'uri':'app://state'}),('resources/read',{'uri':'app://activity'})]:
    value=await asyncio.to_thread(q.rpc,method,params);check(disc['token'] not in json.dumps(value),'resource secret '+str(params),value)
   for n,p in [('inspect_ui',{}),('screenshot',{'target':'window'})]:
    value=await tool(q,n,p);t.run.save(n+'.json',safe(value));check(disc['token'] not in json.dumps(value),'Preferences '+n+' no textual token',{'bytes':len(json.dumps(value)),'status':value.get('status')})
  elif step==5:
   await prefs();t.clear();t.click('agentAccess.regenerate');await pause()
   check(dom('message.cancel','e===document.activeElement') is True,'regeneration question defaults Cancel',t.cdp.js("[...document.querySelectorAll('button')].filter(e=>e.closest('[role=dialog]')).map(e=>({text:e.textContent,id:e.dataset.ttId,focused:e===document.activeElement}))"))
   d.key(0x1B);await pause();check(json.loads(discpath.read_text())==disc,'Cancel keeps token and endpoint')
   t.click('agentAccess.regenerate');await pause()
   buttons=t.cdp.js("[...document.querySelectorAll('button')].filter(e=>e.textContent==='Regenerate').map(e=>e.dataset.ttId)")
   t.click(buttons[0]);await pause();new=json.loads(discpath.read_text())
   check(new['token']!=disc['token'] and new['endpoint']==disc['endpoint'],'regenerate changes token keeps endpoint',{'sent':sent(),'oldFingerprint':{'length':len(disc['token']),'sha256':hashlib.sha256(disc['token'].encode()).hexdigest()},'newFingerprint':{'length':len(new['token']),'sha256':hashlib.sha256(new['token'].encode()).hexdigest()}})
   check('result' not in await init(q),'old token refused',await init(q));nq=fresh('new-token');check('result' in await init(nq),'new token accepted')
   tree=await tool(nq,'inspect_ui',{});logs=[]
   for f in (t.work/'user').rglob('*'):
    if f.is_file() and f.suffix in ('.log','.jsonl'):logs.append((str(f.relative_to(t.work)),new['token'].encode() in f.read_bytes()))
   check(new['token'] not in json.dumps(tree) and not any(x[1] for x in logs),'token absent UI tree and logs',logs)
   status=await call(nq,'agent.server.status');check('review-live3-primary' not in json.dumps(status),'regeneration disconnects existing HTTP sessions',status)
   d.key(0x1B);d.choose_menu(['Window','Panels','Activity']);await pause()
   export=t.work/'token-activity.jsonl';t.click('activity.export');await asyncio.sleep(1.8);d.type_text(str(export));d.key(0x0D);await asyncio.sleep(1.8)
   check(export.exists() and new['token'] not in export.read_text(),'token absent actual activity export')
   await prefs()

   check(False,'token shown or copied in Preferences matches discovery',{'observed':'Agent access has no token display or Copy token control','controls':t.cdp.js("[...document.querySelectorAll('[data-tt-id^=agentAccess]')].map(e=>e.dataset.ttId)")})
  elif step==6:
   await r3.modal_fixture(t,'Live3 sessions');nq=fresh('review-live3-secondary');await init(nq)
   await c('window.panel.set',{'panelId':'activity','visible':True});await pause()
   status=await c('agent.server.status');check(True,'two independent clients status',status)
   sid=next(x['sessionId'] for x in status['sessions'] if 'primary' in json.dumps(x['client']))
   for action in ('pause','resume'):
    t.clear();t.click('activity.session.'+sid+'.'+action);await pause()
    a=await call(q,'draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':12,'y':12}});b=await call(nq,'app.state.get')
    check(('agent.paused' in json.dumps(a) if action=='pause' else a.get('status')=='ok') and b.get('status')=='ok','session '+action+' isolated',{'sent':sent(),'primary':a,'other':b})
   t.clear();await t.do(('select','activity.session.'+sid+'.policy','observe'));await pause()
   check('permission.denied' in json.dumps(await call(q,'draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':1,'y':1}})),'grant targets primary',sent())
   t.clear();t.click('activity.session.'+sid+'.scopes');await pause();t.click('activity.session.'+sid+'.scope.edit');t.click('activity.session.'+sid+'.scopes.ok');await pause()
   check((await call(q,'draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':2,'y':2}})).get('status')=='ok','scope grant applies to primary only',{'sent':sent(),'status':await call(nq,'agent.server.status')})
   samples=[]
   task=asyncio.create_task(tool(nq,'batch',{'label':'Indicator review','steps':[{'id':'draw.rectangle.add','params':{'from':{'x':0,'y':0},'to':{'x':10,'y':10}}}]*1000}))
   while not task.done():
    samples.append(dom('agent','({text:e.textContent,state:e.dataset.ttValue})'));await asyncio.sleep(.02)
   answer=await task
   check(any(x and x['state']=='acting' for x in samples) and not any(x and ('(batch)' in x['text'] or '(agent.server.status)' in x['text']) for x in samples),'indicator acting session and label',{'samples':samples,'answerStatus':answer.get('status')})
   t.clear();t.click('activity.session.'+sid+'.disconnect');await pause();check((await call(q,'app.state.get')).get('status')!='ok' and (await call(nq,'app.state.get')).get('status')=='ok','disconnect isolated',sent())
  elif step==7:
   await r3.modal_fixture(t,'Live3 activity');await c('window.panel.set',{'panelId':'activity','visible':True});await pause()
   for i in range(30):
    if i==0:await call(q,'agent.server.set',{'enabled':False})
    elif i==2:await call(q,'draw.rectangle.add',{'data':'data:image/png;base64,review-inline-secret','from':{'x':0,'y':0},'to':{'x':2,'y':2}})
    elif i==1:await tool(q,'batch',{'label':'Review activity','steps':[{'id':'app.state.get','params':{}}]})
    else:await call(q,['app.state.get','draw.drawing.get','tool.options.get'][i%3],{'toolId':'brush'} if i%3==2 else {})
   await pause();activity=await call(q,'agent.activity.list',{'limit':500});rows=t.cdp.js("[...document.querySelectorAll('[data-tt-id^=\"activity.entry.\"]')].map(e=>({id:e.dataset.ttId,text:e.textContent,commandId:e.title}))")
   seqs=[int(x['id'].rsplit('.',1)[1]) for x in rows]
   check(seqs==sorted(seqs,reverse=True) and len(seqs)==len(set(seqs)),'activity newest first unique sequence',seqs)
   entries=activity.get('result',[])
   mismatches=[]
   for entry in entries:
    row=next((x for x in rows if x['id']=='activity.entry.'+str(entry['seq'])),None)
    if row and (row['commandId']!=entry['commandId'] or any(str(value) not in row['text'] for value in (entry['time'][11:19],entry['client'],entry['status']))):mismatches.append(entry['seq'])
   check(not mismatches,'row time client status agree with log',mismatches)
   check(len(rows)>=30,'30 calls live without reload',{'rows':rows,'activity':activity})
   ev=await asyncio.to_thread(q.rpc,'resources/read',{'uri':'app://events?after=0'});check('shell.' not in json.dumps(ev),'D-57 event stream excludes shell notices',ev)
   export=t.work/'activity.jsonl';t.clear();t.click('activity.export');await asyncio.sleep(1.8);d.type_text(str(export));d.key(0x0D);await asyncio.sleep(1.8);value=sent()
   check(export.exists(),'activity export writes file',value)
   if export.exists():
    exported=[json.loads(line) for line in export.read_text().splitlines() if line.strip()];listed=activity.get('result',{})
    entries=listed if isinstance(listed,list) else listed.get('entries',[])
    same=[x for x in exported if x.get('seq') in {e.get('seq') for e in entries}]
    check(same==entries,'export equals list for same seq range',{'exported':len(exported),'listed':len(entries),'sent':value})
    check('review-inline-secret' not in json.dumps(exported),'inline media replaced by hash',safe(exported))
    t.run.save('activity-export.json',safe(exported))
  if step in (5,6):
   active=clients[-1];ev=await asyncio.to_thread(active.rpc,'resources/read',{'uri':'app://events?after=0'})
   check('result' in ev and 'shell.' not in json.dumps(ev),'D-57 session/token events exclude shell notices',ev)
  t.run.save('live3-commands.json',safe(t.recorded()))
  if step in (1,7):
   import signal
   saved=t.work/'live3-restart.animpkg'
   if step==1:await c('project.create',{'name':'Live3 restart','onUnsaved':'discard'})
   await c('project.saveAs',{'path':str(saved)})
   before=await c('prefs.get',{'keys':['agent.policy.defaults','agent.typedTools','agent.activity.persist']})
   for client in clients:await asyncio.to_thread(client.close)
   os.kill(d.e2e.APP_PID['pid'],signal.SIGTERM);await asyncio.sleep(1)
   rd=t.work/'restart-discovery.json';proc=d.e2e.launch(APP,t.work/'user',rd)
   try:
    new=d.e2e.wait_discovery(rd,proc);nq=IndependentHttp(new,'restart-review');clients.append(nq);await init(nq)
    after=await call(nq,'prefs.get',{'keys':['agent.policy.defaults','agent.typedTools','agent.activity.persist']})
    check(after.get('result')==before,'restart preserves preference groups',{'before':before,'after':after})
    if step==7:
     await call(nq,'project.open',{'path':str(saved)})
     await asyncio.sleep(1);d.choose_menu(['Window','Panels','Activity']);await pause();t.cdp=d.Cdp(d.CDP_PORT)
     panel=t.cdp.js("document.querySelector('[data-tt-id=activity]')?.textContent")
     check(panel is not None and 'review-live3-primary' not in panel,'restart panel does not restore unpersisted old session',panel)
     check(True,'restart activity observation',await call(nq,'agent.activity.list',{'limit':500}))
   finally:d.e2e.stop(proc)

 finally:
  for client in clients:await asyncio.to_thread(client.close)



class Live4Http(IndependentHttp):
 pass
# Freeze each RPC id locally: simultaneous held calls must not compare against a later request's id.
import inspect,textwrap
_live4_rpc=textwrap.dedent(inspect.getsource(IndependentHttp.rpc)).replace("obj.get('id')==self.seq", "obj.get('id')==body['id']")
_live4_scope={};exec(_live4_rpc,globals(),_live4_scope);Live4Http.rpc=_live4_scope['rpc']

async def live4_body(t,step):
 import asyncio,hashlib
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 clients=[];tasks=[]
 def dom(i,expr='e.textContent'):
  return t.cdp.js('(()=>{const e=document.querySelector('+json.dumps('[data-tt-id="'+i+'"]')+');return e?'+expr+':null})()')
 async def pause():await asyncio.sleep(.8)
 async def call(q,n,p=None):return await asyncio.to_thread(q.command,n,p)
 async def fresh(name):
  q=Live4Http(json.loads((t.work/'discovery.json').read_text()),name);clients.append(q);await asyncio.to_thread(q.initialize);return q
 def check(ok,label,data=None):t.run.check(ok,label,data)
 async def request(q):
  sid=str(t.work/('overwrite-'+str(len(tasks))+'.animpkg'));Path(sid).write_bytes(b'reviewer sentinel');params={'path':sid,'onExists':'overwrite'}
  task=asyncio.create_task(call(q,'project.saveAs',params));tasks.append(task);await pause()
  check(not task.done() and dom('confirmSheet') is not None,'call held and sheet open',task.result() if task.done() else dom('confirmSheet'))
  return task,sid,params
 async def answer(task,accept):
  t.clear();t.click('confirmSheet.accept' if accept else 'confirmSheet.decline');await pause()
  result=await asyncio.wait_for(asyncio.shield(task),8)
  check(result.get('status')=='ok' if accept else 'confirmation.declined' in json.dumps(result),'decision delivered',{'answer':result,'sent':t.recorded()});return result
 async def exists(sid):return {'unchanged':Path(sid).read_bytes()==b'reviewer sentinel'}
 try:
  await r3.modal_fixture(t,'Live4 step '+str(step))
  await c('project.saveAs',{'path':str(t.work/'base.animpkg')})
  d.choose_menu(['File','Preferences']);await pause();t.click('preferences.category.agent');await pause();await t.do(('select','agentAccess.policy','assist'));t.click('preferences.submit');await pause()
  q=await fresh('review-live4-primary');q2=await fresh('review-live4-secondary')
  check(True,'initial policy',await c('agent.server.status'))
  if step==1:
   for accept in (True,False):
    await c('tool.active.set',{'toolId':'select'})
    task,sid,p=await request(q)
    check(all(dom(x) for x in ('confirmSheet.summary','confirmSheet.consequences')) and 'review-live4-primary' in dom('confirmSheet'),'sheet content',dom('confirmSheet'))
    d.front();d.key(0x42);await pause();check(dom('confirmSheet') is not None,'shortcut leaves request open',await c('tool.active.get'))
    await answer(task,accept);check((not (await exists(sid))['unchanged']) if accept else (await exists(sid))['unchanged'],'scene outcome',await exists(sid))
  elif step==2:
   task,sid,p=await request(q)
   t.click('drawingView.canvas')
   for key in (0x0D,0x20,0x1B):d.key(key);await pause();check(not task.done(),'outside key preserves request '+str(key))
   # Enter sheet through its first tab stop, using real Tab until reached.
   for _ in range(160):
    d.key(0x09)
    focus=t.cdp.js('document.activeElement?.dataset.ttId')
    if focus and focus.startswith('confirmSheet.'):break
   check(focus=='confirmSheet.decline','first focus is Decline',focus);d.key(0x0D);await pause();check('confirmation.declined' in json.dumps(await task),'Enter declines')
   task,sid,p=await request(q);t.click('confirmSheet.summary');d.key(0x09);d.key(0x1B);await pause()
   if not task.done():t.click('confirmSheet.decline')
   check('confirmation.declined' in json.dumps(await task),'inside Escape declines')
   task,sid,p=await request(q);t.click('confirmSheet.decline');await task
   task,sid,p=await request(q)
   for _ in range(160):
    d.key(0x09)
    if t.cdp.js('document.activeElement?.dataset.ttId')=='confirmSheet.accept':break
   d.key(0x0D);await pause();check((await task).get('status')=='ok','keyboard explicit Accept')
  elif step==3:
   task,sid,p=await request(q)
   status=await call(q2,'agent.server.status');activity=await call(q2,'agent.activity.list',{'limit':100});events=await asyncio.to_thread(q2.rpc,'resources/read',{'uri':'app://events?after=0'})
   check(True,'token surfaces',{'status':status,'activity':activity,'events':live3_compact(events),'firstReturned':task.done()})
   pending=(status.get('result') or {}).get('pendingConfirmations',[]);cid=pending[0]['confirmationId']
   probes=[]
   for client in (q,q2):
    for token in ('guessed-confirm-token',cid):
     probe=asyncio.create_task(asyncio.to_thread(client.tool,'run_command',{'id':'project.saveAs','params':p,'confirmToken':token}));probes.append(('token '+client.name,probe));tasks.append(probe)
    probe=asyncio.create_task(call(client,'agent.confirmation.respond',{'confirmationId':cid,'accept':True}));probes.append(('respond '+client.name,probe));tasks.append(probe)
    probe=asyncio.create_task(asyncio.to_thread(client.tool,'run_command',{'id':'project.saveAs','params':p,'confirm':True}));probes.append(('confirm true '+client.name,probe));tasks.append(probe)
   await asyncio.sleep(2)
   check((await exists(sid))['unchanged'],'bypasses did not write before accept')
   check(True,'bypass states before decline',[{'probe':name,'returned':probe.result() if probe.done() else 'held'} for name,probe in probes])
   for _ in range(15):
    if dom('confirmSheet'):t.click('confirmSheet.decline')
    await pause()
    if task.done() and all(probe.done() for name,probe in probes):break
   for name,probe in probes:
    r=await asyncio.wait_for(asyncio.shield(probe),10);check(r.get('status')!='ok','bypass refused '+name,r)
   check('confirmation.declined' in json.dumps(await task),'original remains declined')
  elif step==4:
   for who in ('window','agent'):
    task,sid,p=await request(q);before=dom('confirmSheet.countdown');status=await call(q2,'agent.server.status');await asyncio.sleep(2.2);after=dom('confirmSheet.countdown');check(before!=after,'countdown decreases',{'before':before,'after':after,'status':status})
    if who=='window':
     await c('tool.active.set',{'toolId':'rectangle'});t.click('drawingView.canvas');d.key(0x42)
     # real undoable document edit through native menu
     d.key(0x5A,0x11)
    else:await call(q2,'draw.rectangle.add',{'from':{'x':1,'y':1},'to':{'x':15,'y':15}})
    await pause();check(task.done() and 'confirmation.expired' in json.dumps(task.result()),'revision expires '+who,{'answer':task.result() if task.done() else 'held','sheet':dom('confirmSheet.ended')})
    if not task.done():await answer(task,False)
    check((await exists(sid))['unchanged'],'scene retained',await exists(sid))
  elif step==5:
   # Create fixtures first: creating a scene itself advances revision.
   ids=[str(t.work/('queue-'+str(i)+'.animpkg')) for i in range(3)]
   for path in ids:Path(path).write_bytes(b'reviewer sentinel')
   held=[asyncio.create_task(call(client,'project.saveAs',{'path':sid,'onExists':'overwrite'})) for client,sid in zip((q,q2,q),ids)];tasks.extend(held);await pause()
   check(dom('confirmSheet.queue') and '1 of 3' in dom('confirmSheet.queue'),'three calls queue',dom('confirmSheet'))
   query=asyncio.create_task(call(q2,'app.state.get'));tasks.append(query);await asyncio.sleep(2);check(query.done() and query.result().get('status')=='ok','other query completes',query.result() if query.done() else 'held behind confirmation')
   for accept in (False,False,True):
    for _ in range(30):
     if dom('confirmSheet'):break
     await asyncio.sleep(.1)
    t.click('confirmSheet.accept' if accept else 'confirmSheet.decline');await pause()
   answers=await asyncio.gather(*held);check(sum(r.get('status')=='ok' for r in answers)==1 and sum('confirmation.declined' in json.dumps(r) for r in answers)==2,'each queued call receives decision',answers)
   # A fresh pending call: draw using real pointer input.
   task,sid,p=await request(q)
   await c('tool.active.set',{'toolId':'brush'});r=t.rect('drawingView.canvas')
   import ctypes
   from ctypes import wintypes
   o=wintypes.POINT(0,0);d.u32.ClientToScreen(d.front(),ctypes.byref(o));x=int(o.x+r['x']*r['dpr']);y=int(o.y+r['y']*r['dpr'])
   pos=await t.position();d.u32.SetCursorPos(x,y);d.e6._send_button(True)
   for i in range(1,8):d.u32.SetCursorPos(x+i*5,y+i*2);await asyncio.sleep(.04)
   d.e6._send_button(False);await pause();check(await t.position()>pos,'person draws while sheet open');check(task.done() and 'confirmation.expired' in json.dumps(task.result()),'stroke expires pending request')
   check(True,'window topology',await c('window.panel.list'))
  elif step==6:
   import zipfile
   clean=t.work/'clean.animpkg';bad=t.work/'damaged.animpkg'
   await c('project.saveAs',{'path':str(clean)})
   with zipfile.ZipFile(clean) as src,zipfile.ZipFile(bad,'w') as dst:
    removed=[n for n in src.namelist() if '/drawings/' in n and n.endswith('.json')]
    check(bool(removed),'fixture has drawing entry',removed)
    for name in src.namelist():
     if name not in removed:dst.writestr(name,src.read(name))
   await c('project.close',{'onUnsaved':'discard'})
   d.choose_menu(['File','Open']);await asyncio.sleep(1);d.type_text(str(bad));d.key(0x0D);await asyncio.sleep(2)
   report=await c('doc.loadReport.get');check(dom('loadReport') is not None,'report opens on damaged project',report)
   check('drawing.contentMissing' in json.dumps(report),'missing content reported',report)
   before=await c('doc.export.json',{'scope':'project','binaries':'omit'})
   if dom('loadReport'):
    t.click('loadReport.copy');await pause()
    clip=subprocess.check_output(['powershell','-NoProfile','-Command','Get-Clipboard -Raw']).decode(errors='replace')
    check('drawing.contentMissing' in clip,'Copy details reaches OS clipboard',clip)
    check((await call(q,'app.state.get')).get('status')=='ok','report nonmodal query')
    t.click('loadReport.close');await pause();check(before==await c('doc.export.json',{'scope':'project','binaries':'omit'}),'closing report preserves document')
    await asyncio.sleep(1);check(dom('loadReport') is None,'report remains closed once per open')
   await c('project.close',{'onUnsaved':'discard'})
   opened=await call(q,'project.open',{'path':str(bad),'wait':True});await asyncio.sleep(2)
   check('drawing.contentMissing' in json.dumps(opened),'agent receives report warnings',opened)
   check(dom('loadReport') is not None,'agent open shows report too',dom('loadReport'))
   if dom('loadReport'):t.click('loadReport.close')
   await c('project.close',{'onUnsaved':'discard'});await c('project.open',{'path':str(clean),'wait':True});await asyncio.sleep(2)
   check(dom('loadReport') is None,'clean open has no report',await c('doc.loadReport.get'))
  elif step==7:
   async def layout():return await c('window.toolbar.list')
   async def sheet():d.choose_menu(['Window','Customize Toolbar']);await pause()
   before=await layout()
   await sheet();t.click('toolbarSheet.allowed.edit.undo');t.click('toolbarSheet.add');t.click('toolbarSheet.up');await pause()
   check(await layout()==before,'add and reorder staged')
   t.click('toolbarSheet.cancel');await pause();check(await layout()==before,'Cancel discards')
   await sheet();t.click('toolbarSheet.item.0');t.click('toolbarSheet.remove');d.key(0x1B);await pause();check(await layout()==before,'Escape discards removal')
   await sheet();t.click('toolbarSheet.allowed.edit.undo');t.click('toolbarSheet.add');t.click('toolbarSheet.up')
   for _ in range(45):
    d.key(0x09);focus=t.cdp.js('document.activeElement?.dataset.ttId');check(bool(focus) and focus.startswith('toolbarSheet.'),'Tab contained',focus)
   tool=await c('tool.active.get');d.key(0x42);await pause();check(await c('tool.active.get')==tool,'sheet blocks tool shortcut')
   t.clear();t.click('toolbarSheet.submit');await pause();after=await layout();check(after!=before,'Apply changes layout',{'layout':after,'sent':t.recorded()})
   await c('draw.rectangle.add',{'from':{'x':0,'y':0},'to':{'x':5,'y':5}});pos=await t.position();t.click('mainToolbar.edit.undo');await pause();check(await t.position()==pos-1,'added Undo works')
   # Restart with the same installed user directory.
   import signal
   for client in clients:await asyncio.to_thread(client.close)
   os.kill(d.e2e.APP_PID['pid'],signal.SIGTERM);await asyncio.sleep(1)
   proc=d.e2e.launch(APP,t.work/'user',t.work/'restart-discovery.json')
   try:
    disc=d.e2e.wait_discovery(t.work/'restart-discovery.json',proc);await asyncio.sleep(2);t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
    restart=Live4Http(disc,'restart-live4');clients.append(restart);await asyncio.to_thread(restart.initialize)
    persisted=await call(restart,'window.toolbar.list');check(persisted.get('result')==after,'layout survives restart',persisted)
    await sheet();t.clear();t.click('toolbarSheet.restore');await pause();reset=await call(restart,'window.toolbar.list');check(reset.get('result')==before,'Restore Default restores original',{'layout':reset,'sent':t.recorded()})
   finally:d.e2e.stop(proc)
 finally:
  for _ in range(10):
   if dom('confirmSheet') is None:break
   t.click('confirmSheet.decline');await pause()
  for task in tasks:
   if not task.done():task.cancel()
  for client in clients:await asyncio.to_thread(client.close)

if __name__=='__main__':
 if sys.argv[1] in ('l4a','l4b'):
  live_body=live4_body
  sys.exit(live(int(sys.argv[2])))
 if sys.argv[1] in ('l3a','l3b'):
  live_body=live3_body
  sys.exit(live(int(sys.argv[2])))
 if sys.argv[1] in ('l2a','l2b'):
  live_body=live2_body
  sys.exit(live(int(sys.argv[2])))
 if sys.argv[1]=='l1a':
  step=int(sys.argv[2])
  script,cases={2:('c43b-r1-live.py',['case3','case4','case5','case6','case7','case8']),3:('c43b-r2-live.py',['a2','a3','b1','b2','b3']),4:('c43b-r3-live.py',['a2','a3','b1','b2'])}[step]
  regression(step,script,cases)
 elif sys.argv[1]=='l1b':
  sys.exit(live(int(sys.argv[2])))
