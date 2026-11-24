"""Independent Timeline live review. Raw captures stay in system TEMP."""
import importlib.util, json, os, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EVID=ROOT/'reviews/evidence/M5-C5.1b-r1'
def load(name,rel):
 s=importlib.util.spec_from_file_location(name,ROOT/rel);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def save(name,obj):
 EVID.mkdir(parents=True,exist_ok=True)
 data=json.dumps(obj,ensure_ascii=True,indent=2)+'\n';assert len(data.encode())<200000
 (EVID/name).write_text(data,encoding='utf-8',newline='\n')
def build():
 b=load('builder','tests/adversarial/c44b-r1-live.py');b.EVID=Path(tempfile.mkdtemp(prefix='c51b-build-summary-'))
 result=b.build();save('build.json',json.loads((b.EVID/'step1.json').read_text()));return result
async def body(t,step):
 import asyncio, shutil
 d=P.d
 src=(ROOT/'scripts/c43-d39-bindings.py').read_text(encoding='utf-8')
 src=src[src.index('class Cdp:'):src.index('# ----- OS input')].replace('class Cdp:','class TimingCdp:').replace('/run_command','/timing_gesture')
 exec(src,d.__dict__)
 def capture(self,body):
  outer=json.loads(body);self.calls.append(json.loads(outer['json']) if isinstance(outer.get('json'),str) else outer)
 d.TimingCdp._ipc=capture
 t.cdp=d.TimingCdp(d.CDP_PORT);t.cdp.record()
 c=lambda n,p={}: asyncio.wait_for(t.cmd(n,p),30)
 commands=[]
 async def setup(n,p):
  commands.append({'command':n,'params':p});return await c(n,p)
 await setup('project.create',{'name':'Timeline live review','onUnsaved':'fail'})
 ids={}
 for typ,name in [('peg','Peg-A'),('peg','Peg-B'),('drawing','Bob'),('drawing','Alice')]:
  ids[name]=(await setup('element.create',{'type':typ,'name':name}))['elementIds'][0]
 for name in ('Bob','Alice'):
  for i in range(3):await setup('element.drawing.create',{'elementId':ids[name],'name':str(i+1),'exposeFrom':i*4+1,'exposeTo':i*4+4})
 await setup('element.parent.set',{'elementIds':[ids['Bob']],'parentId':ids['Peg-A']})
 for name in ('Peg-A','Peg-B'):await setup('element.timing.set',{'elementIds':[ids[name]],'start':1,'duration':48})
 for frame in (1,8):await setup('anim.key.add',{'nodeIds':[ids['Peg-A']],'frame':frame,'channels':['position']})
 await setup('workspace.select',{'name':'Default'})
 await setup('window.panel.set',{'panelId':'timeline','visible':True,'focus':True})
 layout=await c('workspace.get',{'name':'Default'});layout=layout.get('layout',layout);layout['docks']['bottom'][0]['size']=350
 await setup('workspace.layout.set',{'layout':layout})
 await setup('timeline.view.set',{'zoomLevel':6,'firstFrame':1})
 save('fixture.json',commands)
 def js(x):return t.cdp.js(x)
 def dom(ident):return js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.outerHTML ?? null"%json.dumps(ident))
 async def wait(fn):
  deadline=asyncio.get_running_loop().time()+30
  while True:
   v=fn()
   if v:return v
   if asyncio.get_running_loop().time()>deadline:raise RuntimeError('State wait exceeded 30 seconds')
   await asyncio.sleep(.025)
 await wait(lambda:dom('timeline.row.'+ids['Bob']))
 def shown():return js("[...document.querySelectorAll('.timeline-row')].map(e=>({id:e.dataset.ttId,name:e.dataset.ttLabel,cls:e.className,html:e.outerHTML}))")
 def guard():
  h=d.window_handle()
  if d.u32.GetForegroundWindow()!=h:raise RuntimeError('Application does not own foreground; stopped before input')
 def click(ident):guard();t.click(ident)
 async def rows():return await c('timeline.rows.list',{'from':1,'to':48})
 async def until_cmd(n,p,pred):
  deadline=asyncio.get_running_loop().time()+30
  while True:
   v=await c(n,p)
   if pred(v):return v
   if asyncio.get_running_loop().time()>deadline:raise RuntimeError('MCP state wait exceeded 30 seconds')
   await asyncio.sleep(.025)
 def point(surface,x,y):
  import ctypes
  from ctypes import wintypes
  rr=js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']').getBoundingClientRect().toJSON()"%json.dumps(surface))
  o=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o));scale=js('devicePixelRatio')
  return round(o.x+(rr['x']+x)*scale),round(o.y+(rr['y']+y)*scale)
 def rowpoint(name,x=120,frac=.5):
  rr=js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']').getBoundingClientRect().toJSON()"%json.dumps('timeline.row.'+ids[name]))
  ll=js("document.querySelector('[data-tt-id=\"timeline.layers\"]').getBoundingClientRect().toJSON()")
  return point('timeline.layers',x,rr['y']-ll['y']+22*frac)
 def barpoint(name,frame=4):
  rr=js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']').getBoundingClientRect().toJSON()"%json.dumps('timeline.row.'+ids[name]))
  tr=js("document.querySelector('[data-tt-id=\"timeline.tracks\"]').getBoundingClientRect().toJSON()")
  return point('timeline.tracks',(frame-.5)*16,rr['y']-tr['y']+16)
 async def move(pt):
  import ctypes
  from ctypes import wintypes
  cur=wintypes.POINT();d.u32.GetCursorPos(ctypes.byref(cur))
  if (cur.x,cur.y)==tuple(pt):return
  guard();d.u32.SetCursorPos(*pt)
  await wait(lambda:js('window.reviewPointer && window.reviewPointer.x===%d && window.reviewPointer.y===%d'%pt))
 async def down(pt):
  await move(pt);guard();d.e6._send_button(True)
  await wait(lambda:js('window.reviewDown===true'))
 async def up():
  guard();d.e6._send_button(False)
  await wait(lambda:js('window.reviewDown===false'))
 async def drag(a,b):
  await down(a);await move(b);await up()
 js("window.reviewDown=false;window.addEventListener('pointermove',e=>window.reviewPointer={x:e.screenX,y:e.screenY});window.addEventListener('pointerdown',e=>{window.reviewDown=true;window.reviewTarget=e.target.dataset.ttId});window.addEventListener('pointerup',()=>window.reviewDown=false)")
 async def bar(name):return next(r['bar'] for r in (await rows())['rows'] if r['elementId']==ids[name])
 d.front();guard()
 if step==1:
  r=await rows();ui=shown();save('step1-rows.json',{'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'build':'default release','foreground':True,'shown':ui,'mcp':r})
  assert [x['name'] for x in ui]==[x['name'] for x in r['rows']]
 elif step==2 and os.environ.get('C51B_DIRECT'):
  await c('element.timing.set',{'elementIds':[ids['Peg-A']],'startDelta':3})
  expected=json.loads((EVID/'step2-result.json').read_text())['after']
  assert await bar('Peg-A')==expected
  save('step2-direct.json',{'freshLaunch':True,'bar':await bar('Peg-A'),'equal':True})
 elif step==2:
  before=await bar('Peg-A');pos=await t.position();a=barpoint('Peg-A');b=(a[0]+round(48*js('devicePixelRatio')),a[1])
  await drag(a,b)
  save('step2-probe.json',{'target':js('window.reviewTarget'),'point':[a,b],'calls':t.cdp.calls,'rows':await rows(),'status':js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent")})
  await until_cmd('timeline.rows.list',{'from':1,'to':60},lambda r:next(x['bar']['start'] for x in r['rows'] if x['elementId']==ids['Peg-A'])==4)
  after=await bar('Peg-A');hist=await t.steps_since(pos);assert len(hist)==1
  assert after['end']-after['start']==before['end']-before['start']
  await c('edit.undo');assert await bar('Peg-A')==before
  # The undo restores the fresh fixture's content before the direct command.
  await c('element.timing.set',{'elementIds':[ids['Peg-A']],'startDelta':3})
  assert await bar('Peg-A')==after
  save('step2-result.json',{'before':before,'after':after,'history':hist,'directEqual':True})
 elif step==3:
  canvas=lambda:js("document.querySelector('[data-tt-id=\"timeline.tracks\"]').toDataURL()")
  before=await bar('Peg-A');pos=await t.position();a=barpoint('Peg-A');b=(a[0]+round(48*js('devicePixelRatio')),a[1])
  original=canvas();await down(a);await move(b);await wait(lambda:canvas()!=original)
  guard();d.key(0x1b);await up();await wait(lambda:canvas()==original)
  assert await bar('Peg-A')==before;assert not await t.steps_since(pos)
  await drag(a,b)
  await until_cmd('timeline.rows.list',{'from':1,'to':60},lambda r:next(x['bar']['start'] for x in r['rows'] if x['elementId']==ids['Peg-A'])==4)
  inside=await bar('Peg-A');await c('edit.undo')
  await wait(lambda:canvas()==original)
  await down(a);await move(b);await move((b[0],0));await up()
  outside=await bar('Peg-A');assert outside in (before,inside)
  save('step3-result.json',{'escapeUnchanged':True,'escapeNoHistory':True,'previewGone':True,'inside':inside,'outside':outside,'outsidePoint':[b[0],0],'outsideOutcome':'cancelled' if outside==before else 'same as inside'})
 elif step==4:
  a=barpoint('Peg-A');b=(a[0]+round(48*js('devicePixelRatio')),a[1]);await down(a);await move(b)
  await c('element.timing.set',{'elementIds':[ids['Peg-A']],'start':6});pos=await t.position();await up()
  assert (await bar('Peg-A'))['start']==6;assert not await t.steps_since(pos)
  reason=await wait(lambda:js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent"))
  await c('edit.undo');await wait(lambda:True)
  # Wait for the restored bar's row read before pressing again.
  await c('timeline.view.set',{'zoomLevel':6,'firstFrame':1})
  a=barpoint('Peg-A');b=(a[0]+round(48*js('devicePixelRatio')),a[1]);await down(a);await move(b)
  await c('element.rename',{'elementId':ids['Alice'],'name':'Alice-renamed'});await up()
  await until_cmd('timeline.rows.list',{'from':1,'to':60},lambda r:next(x['bar']['start'] for x in r['rows'] if x['elementId']==ids['Peg-A'])==4)
  save('step4-result.json',{'cancelReason':reason,'unrelatedRenameDragStart':4})
 elif step==5:
  before=await rows();pos=await t.position()
  await drag(rowpoint('Alice'),rowpoint('Peg-A',frac=.05))
  after=await rows();assert [r['elementId'] for r in after['rows']]!=[r['elementId'] for r in before['rows']]
  assert len(await t.steps_since(pos))==1
  await c('edit.undo');assert (await rows())['rows']==before['rows']
  await drag(rowpoint('Alice'),rowpoint('Peg-A'))
  assert next(r for r in (await rows())['rows'] if r['elementId']==ids['Alice'])['parentId']==ids['Peg-A']
  await c('edit.undo');assert (await rows())['rows']==before['rows']
 elif step==6:
  eid=(await c('element.create',{'type':'drawing','name':'Agent-created'}))['elementIds'][0];row='timeline.row.'+eid
  await wait(lambda:dom(row))
  await c('element.rename',{'elementId':eid,'name':'Agent-renamed'});await wait(lambda:'Agent-renamed' in (dom(row) or ''))
  await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False});await wait(lambda:' hidden' in (dom(row) or ''))
  await c('element.lock.set',{'elementIds':[eid],'locked':True});await wait(lambda:dom(row+'.locked'))
  await c('element.colour.set',{'elementIds':[eid],'marker':'#FF0000FF'});await wait(lambda:'rgb(255, 0, 0)' in (dom(row) or ''))
  await c('element.delete',{'elementIds':[eid]});await wait(lambda:dom(row) is None)
 elif step==7:
  click('timeline.row.'+ids['Bob']+'.label')
  await until_cmd('edit.selection.get',{'context':'timeline'},lambda r:ids['Bob'] in r.get('ids',[]))
  state=await c('app.state.get');save('step7-current.json',state)
  assert (await c('cell.selection.get'))['currentElementId']==ids['Bob']
  guard();d.u32.keybd_event(0x10,0,0,0)
  try:click('timeline.row.'+ids['Alice']+'.label')
  finally:guard();d.u32.keybd_event(0x10,0,2,0)
  sel=await c('edit.selection.get',{'context':'timeline'})
  order=[r['elementId'] for r in (await rows())['rows']];lo,hi=sorted([order.index(ids['Bob']),order.index(ids['Alice'])]);assert set(sel['ids'])==set(order[lo:hi+1])
  await c('element.select',{'elementIds':[ids['Peg-B']]})
  await wait(lambda:' selected' in (dom('timeline.row.'+ids['Peg-B']) or ''))
 elif step==8:
  await c('playback.loop.set',{'enabled':False});await c('timeline.currentFrame.set',{'frame':1})
  click('mainToolbar.playStop')
  started=await until_cmd('playback.state.get',{},lambda r:r['playing'])
  await wait(lambda:'playback.stop' in (dom('mainToolbar.playStop') or ''))
  stopped=await until_cmd('playback.state.get',{},lambda r:not r['playing'])
  assert stopped['frame']==stopped['range']['end']
  await wait(lambda:'playback.start' in (dom('mainToolbar.playStop') or ''))
  await c('timeline.currentFrame.set',{'frame':1});click('mainToolbar.playStop')
  await until_cmd('playback.state.get',{},lambda r:r['playing'])
  await wait(lambda:'playback.stop' in (dom('mainToolbar.playStop') or ''))
  xy=point('timeline.ruler',(12-.5)*16,12);guard();d.click_screen(*xy)
  clicked=await until_cmd('playback.state.get',{},lambda r:not r['playing'])
  assert clicked['frame']==12
  await wait(lambda:'playback.start' in (dom('mainToolbar.playStop') or ''))
  save('step8-result.json',{'started':started,'end':stopped,'rulerClick':clicked,'toolbarAgreed':True})
 elif step==9:
  import time
  result=await asyncio.wait_for(d.e2e.call(t.s,'batch',{'label':'Large scene','steps':[{'id':'element.create','params':{'type':'drawing','name':'Large-%03d'%i}} for i in range(300)]}),30)
  assert result.get('status')=='ok',str(result)[:200]
  await wait(lambda:js("Number(document.querySelector('[data-tt-id=\"timeline.tracks\"]').dataset.ttRows)>=300"))
  js("window.reviewWheels=0;document.querySelector('[data-tt-id=\"timeline.tracks\"]').addEventListener('wheel',()=>window.reviewWheels++)")
  durations=[]
  async def queries():
   for _ in range(20):
    start=time.monotonic();await c('doc.state.get');durations.append(time.monotonic()-start)
  async def wheels():
   xy=point('timeline.tracks',100,70)
   for i in range(40):
    await move(xy);guard();d._send_mouse(0x0800,-120)
    await wait(lambda:js('window.reviewWheels')>=i+1)
  await asyncio.gather(queries(),wheels())
  v=await c('timeline.view.set');r=await c('timeline.rows.list',{'from':int(v['firstFrame']),'to':int(v['lastFrame'])})
  def visible():return js("(()=>{const l=document.querySelector('[data-tt-id=\"timeline.layers\"]'),b=l.getBoundingClientRect();return {scroll:l.scrollTop,height:l.clientHeight,ids:[...l.querySelectorAll('.timeline-row')].filter(e=>{const r=e.getBoundingClientRect();return r.bottom>b.top&&r.top<b.bottom}).map(e=>e.dataset.ttId)}})()")
  ui=visible();expected=['timeline.row.'+x['elementId'] for x in r['rows']][int(ui['scroll']//22):__import__('math').ceil((ui['scroll']+ui['height'])/22)]
  await wait(lambda:visible()['ids']==expected)
  save('step9-result.json',{'created':300,'batchCount':1,'wheels':js('window.reviewWheels'),'callSeconds':durations,'view':v,'visible':visible(),'expected':expected})
 elif step>=10:
  def key(k,mod=None):
   guard();d.key(k,mod) if mod else d.key(k)
  async def menu(name,item):
   guard();d.right_click_screen(*rowpoint(name))
   await wait(lambda:dom('timeline.rowMenu'));click('timeline.rowMenu.'+item)
  async def names():
   es=(await c('element.list'))['elements'];return [await c('element.get',{'elementId':e['id']}) for e in es]
  async def rename(name,value,escape=False):
   await down(rowpoint(name));await up();await down(rowpoint(name));await up()
   await wait(lambda:dom('timeline.row.'+ids[name]+'.name'))
   key(0x41,0x11);guard();d.type_text(value);key(0x1b if escape else 0x0d)
   await wait(lambda:not dom('timeline.row.'+ids[name]+'.name'))
  if step==10:
   action=os.environ.get('C51B_ACTION','rename');before=await names();pos=await t.position()
   await menu('Bob','timeline.row.'+action)
   if action=='rename':
    await wait(lambda:dom('timeline.row.'+ids['Bob']+'.name'));key(0x41,0x11);guard();d.type_text('Robert');key(0x0d)
   await wait(lambda:not dom('timeline.rowMenu'))
   after=await names();save('step10-'+action+'.json',{'before':before,'after':after,'history':await t.steps_since(pos),'dialogs':js("[...document.querySelectorAll('[role=dialog]')].map(e=>e.textContent)")})
   assert before!=after
   await c('edit.undo');assert await names()==before
  elif step==11:
   for item,dialog,value,field in [('changeStart','timelineStart','5','start'),('changeDuration','timelineDuration','30','duration'),('changeLoops','timelineLoops','2','loops')]:
    for ending in ('submit','escape','cancel'):
     before=await rows();pos=await t.position();await menu('Peg-A','timeline.row.'+item);await wait(lambda:dom(dialog+'.value'))
     click(dialog+'.value');key(0x41,0x11);guard();d.type_text(value)
     if ending=='escape':key(0x1b)
     else:click(dialog+'.'+ending)
     await wait(lambda:not dom(dialog+'.value'))
     if ending=='submit':
      b=await bar('Peg-A');assert (b['end']-b['start']+1 if field=='duration' else b[field])==int(value);await c('edit.undo');assert (await rows())['rows']==before['rows']
     else:assert (await rows())['rows']==before['rows'] and not await t.steps_since(pos)
   guard();d.choose_menu(['Element','New Elements']);await wait(lambda:dom('timelineNewElements.name'))
   for field,value in [('name','Leaf'),('count','3')]:
    click('timelineNewElements.'+field);key(0x41,0x11);guard();d.type_text(value)
   click('timelineNewElements.submit');es=await names();assert sorted(e['name'] for e in es if e['name'].startswith('Leaf'))==['Leaf','Leaf_1','Leaf_2']
  elif step==12:
   await rename('Alice','Alicia');await rename('Alice','Discard',True)
   assert (await c('element.get',{'elementId':ids['Alice']}))['name']=='Alicia'
   await down(rowpoint('Alice'));await up();await down(rowpoint('Alice'));await up();await wait(lambda:dom('timeline.row.'+ids['Alice']+'.name'))
   before=[await c('cell.selection.get'),await c('edit.selection.get',{'context':'timeline'}),await names(),await c('playback.state.get')]
   for k in (0xbf,0x25,0x27,0x26,0x28,0x2e,0x0d):key(k)
   after=[await c('cell.selection.get'),await c('edit.selection.get',{'context':'timeline'}),await names(),await c('playback.state.get')]
   save('step12-keys.json',{'before':before,'after':after});assert before==after
   for value in ('','Bob'):
    await rename('Alice',value);assert (await c('element.get',{'elementId':ids['Alice']}))['name']=='Alicia'
    await wait(lambda:js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent"))
  elif step==13:
   click('timeline.row.'+ids['Bob']+'.label');await c('xsheet.currentCell.set',{'elementId':ids['Bob'],'frame':3})
   order=[r['elementId'] for r in (await rows())['rows']];idx=order.index(ids['Bob']);frame=3
   for k,di,df in [(0x27,0,1),(0x25,0,-1),(0x28,1,0),(0x26,-1,0)]:
    key(k);idx+=di;frame+=df
    await until_cmd('app.state.get',{},lambda r:r['frame']==frame and r['element']['id']==order[idx])
   await c('xsheet.currentCell.set',{'elementId':order[-1],'frame':1});await wait(lambda:' current' in (dom('timeline.row.'+order[-1]) or ''));key(0x25);key(0x28)
   state=await c('app.state.get');save('step13-boundary.json',{'state':state,'expectedElement':order[-1]});assert state['frame']==1 and state['element']['id']==order[-1]
   await menu('Peg-A','timeline.row.changeStart');await wait(lambda:dom('timelineStart.value'));before=await c('cell.selection.get')
   for k in (0x25,0x27,0x26,0x28):key(k)
   assert await c('cell.selection.get')==before;key(0x1b)
  elif step==15:
   await c('element.lock.set',{'elementIds':[ids['Bob']],'locked':True});await wait(lambda:dom('timeline.row.'+ids['Bob']+'.locked'))
   a=barpoint('Bob');await drag(a,(a[0]+round(48*js('devicePixelRatio')),a[1]));assert (await bar('Bob'))['start']==4
   await rename('Bob','LockedBob');assert (await c('element.get',{'elementId':ids['Bob']}))['name']=='LockedBob'
   click('timeline.row.'+ids['Alice']+'.visible');await wait(lambda:' hidden' in (dom('timeline.row.'+ids['Alice']) or ''))
   click('timeline.row.'+ids['Bob']+'.label');await menu('Bob','timeline.row.hideOthers');save('step15-hidden.json',await names())
   await menu('Bob','timeline.row.showAll');save('step15-shown.json',await names())
  elif step==16:
   path=str(t.work/'lifecycle.animpkg');await c('project.saveAs',{'path':path});original=[r['name'] for r in (await rows())['rows']]
   await c('project.close',{'onUnsaved':'discard'});await wait(lambda:shown()==[])
   controls=js("[...document.querySelectorAll('.timeline-toolbar button,.timeline-toolbar input,.timeline-toolbar select')].map(e=>({id:e.dataset.ttId,disabled:e.disabled}))")
   save('step16-toolbar.json',controls);assert controls and all(e['disabled'] for e in controls)
   await c('project.create',{'name':'New','onUnsaved':'fail'});await c('element.create',{'type':'drawing','name':'Only'})
   await wait(lambda:[r['name'] for r in shown()]==['Only']);assert (await c('app.state.get'))['frame']==1
   await c('project.open',{'path':path,'onUnsaved':'discard'});await wait(lambda:[r['name'] for r in shown()]==original)
  elif step==14:
   trigger=Path(os.environ['TT_TEST_DEGRADE_TRIGGER']);trigger.write_text('stop',encoding='utf-8')
   result=await asyncio.wait_for(d.e2e.run_cmd(t.s,'element.create',{'type':'drawing','name':'Dies'}),30)
   await wait(lambda:dom('engineNotResponding'))
   a=barpoint('Peg-A');await drag(a,(a[0]+round(48*js('devicePixelRatio')),a[1]))
   status=lambda:dom('status.line')
   dragStatus=await wait(status)
   await menu('Bob','timeline.row.lock');itemStatus=await wait(status)
   save('step14-own.json',{'stopError':result.get('error'),'dragStatus':dragStatus,'itemStatus':itemStatus,'banner':bool(dom('engineNotResponding'))})
   raise RuntimeError('Stopped-engine attempts exercised; own Restart/Recover and post-recovery drag not exercised')
 t.run.check(True,'Step '+str(step)+' exercised',None)
def live(step):
 global P
 P=load('prior','tests/adversarial/c44a-r1-live.py');P.EVID=EVID
 if step==14:P.APP=str(ROOT/'target/debug/toontown.exe')
 P.live_body=body
 return P.live(step)
if __name__=='__main__':
 case,step=sys.argv[1],int(sys.argv[2])
 assert (case=='l1' and 1<=step<=5) or (case=='l2' and 6<=step<=9) or (case=='l3' and 10<=step<=13) or (case=='l4' and 14<=step<=16)
 if step==1 and not os.environ.get('C51B_SKIP_BUILD'):
  rc=build()
  if rc:sys.exit(rc)
 sys.exit(live(step))
