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
 else:raise RuntimeError('Remaining gesture sentences not yet implemented')
 t.run.check(True,'Step '+str(step)+' exercised',None)
def live(step):
 global P
 P=load('prior','tests/adversarial/c44a-r1-live.py');P.EVID=EVID
 P.live_body=body
 return P.live(step)
if __name__=='__main__':
 case,step=sys.argv[1],int(sys.argv[2])
 assert (case=='l1' and 1<=step<=5) or (case=='l2' and 6<=step<=9)
 if step==1 and not os.environ.get('C51B_SKIP_BUILD'):
  rc=build()
  if rc:sys.exit(rc)
 sys.exit(live(step))
