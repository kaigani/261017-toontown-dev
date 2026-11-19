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
  t.click('pen.row.'+pid);d.key(0x71);await pause()
  selected=t.cdp.js('({value:document.activeElement.value,start:document.activeElement.selectionStart,end:document.activeElement.selectionEnd})')
  t.run.check(selected.get('start')==0 and selected.get('end')==len(selected.get('value','')),'rename opens selected',selected)
  t.clear();d.type_text('Live2 pen');d.key(0x0D);await pause()
  t.run.check((await cur())['name']=='Live2 pen','typing replaces rename',{'pen':await cur(),'sent':sent()})
  t.click('pen.row.'+pid);d.key(0x71);await pause();d.type_text('Cancel me');d.key(0x1B);await pause()
  t.run.check((await cur())['name']=='Live2 pen','Escape cancels rename',await cur())
  t.clear();t.click('pen.delete');await pause();t.run.check(pid not in [x['penId'] for x in (await penstate())['pens']],'delete removes pen',sent())
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
   t.clear();d.front();dom('pen.'+field,'(e.focus(),true)');d.key(0x27);await pause()
   t.run.check(len([x for x in sent() if x['command']=='tool.pen.set'])>=1,'slider keyboard release '+field,sent())
  for shape in ('oval','square','flat','round'):
   t.clear();await t.do(('select','pen.tipShape',shape));await pause();t.run.check((await cur())['tipShape']==shape,'tip shape '+shape,sent())
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
    elif control['type']=='number':await typed(ident,4)
    else:continue
    await pause();t.run.check(bool(sent()),'tool option command '+tool+' '+ident,{'sent':sent(),'state':await c('tool.options.get',{'toolId':tool})})
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
  for flag in ('bold','italic','autoKern'):
   before=(await c('tool.options.get',{'toolId':'text'}))['options'][flag];t.clear();t.click('text.'+flag);await pause();t.run.check((await c('tool.options.get',{'toolId':'text'}))['options'][flag]!=before,'text flag '+flag,sent())
  for field,value in [('baseline','superscript'),('align','justify')]:
   t.clear();await t.do(('select','text.'+field,value));await pause();t.run.check((await c('tool.options.get',{'toolId':'text'}))['options'][field]==value,'text choice '+field,sent())
  t.clear();await typed('text.font','Live2 Missing Font 673');t.run.check('not bundled' in dom('text.fontNotBundled','e.textContent'),'unbundled font marked',{'sent':sent(),'options':await c('tool.options.get',{'toolId':'text'})})
 elif step==7:
  await c('window.panel.set',{'panelId':'properties','visible':True});await pause();t.click('properties.page.drawing');await pause()
  t.clear();await typed('inspector.name','Live2 drawing');listing=await c('draw.drawing.get');t.run.check('Live2 drawing' in json.dumps(listing),'inspector rename',{'sent':sent(),'drawing':listing})
  st=await c('app.state.get');t.run.save('inspector-state.json',st)
  await c('app.context.set',{'frame':100});await pause();controls=t.cdp.js("[...document.querySelectorAll('[data-tt-id^=\"inspector.\"]')].map(e=>({id:e.dataset.ttId,disabled:e.disabled,title:e.title}))")
  t.run.save('inspector-empty.json',controls)
  t.run.check(bool(controls) and all(x.get('disabled') and x.get('title') for x in controls),'empty drawing controls disabled with reason (design note 1)',controls)
 t.run.save('live2-commands.json',t.recorded())


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
  suffix='-context-repro' if os.environ.get('REVIEW_CONTEXT_REPRO') else '-dock' if os.environ.get('REVIEW_DOCK_REPRO') else ''
  (EVID/((sys.argv[1]+'-' if sys.argv[1] in ('l2a','l2b') else '')+'step'+str(step)+suffix+'.json')).write_text(json.dumps(summary,indent=2,default=str)[:190000]+'\n',encoding='utf-8',newline='\n')
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

if __name__=='__main__':
 if sys.argv[1] in ('l2a','l2b'):
  live_body=live2_body
  sys.exit(live(int(sys.argv[2])))
 if sys.argv[1]=='l1a':
  step=int(sys.argv[2])
  script,cases={2:('c43b-r1-live.py',['case3','case4','case5','case6','case7','case8']),3:('c43b-r2-live.py',['a2','a3','b1','b2','b3']),4:('c43b-r3-live.py',['a2','a3','b1','b2'])}[step]
  regression(step,script,cases)
 elif sys.argv[1]=='l1b':
  sys.exit(live(int(sys.argv[2])))
