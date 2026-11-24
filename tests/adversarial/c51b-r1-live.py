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
 t.cdp=d.Cdp(d.CDP_PORT)
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
 d.front();guard()
 if step==1:
  r=await rows();ui=shown();save('step1-rows.json',{'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'build':'default release','foreground':True,'shown':ui,'mcp':r})
  assert [x['name'] for x in ui]==[x['name'] for x in r['rows']]
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
  guard();d.u32.keybd_event(0x10,0,0,0)
  try:click('timeline.row.'+ids['Alice']+'.label')
  finally:guard();d.u32.keybd_event(0x10,0,2,0)
  sel=await c('edit.selection.get',{'context':'timeline'});assert len(sel['ids'])>=2
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
