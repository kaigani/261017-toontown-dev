"""Round 2 live review; import the unchanged round 1 fixture and harness."""
import importlib.util, sys, os, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
s=importlib.util.spec_from_file_location('round1',ROOT/'tests/adversarial/c51b-r1-live.py')
r=importlib.util.module_from_spec(s);s.loader.exec_module(r)
r.EVID=ROOT/'reviews/evidence/M5-C5.1b-r2/review'
def install():
 import inspect
 prefix=inspect.getsource(r.body).split(' if step==1:')[0]
 extra='''
 def key(k,mod=None):
  guard();d.key(k,mod) if mod else d.key(k)
 async def menu(name,item):
  guard();d.right_click_screen(*rowpoint(name))
  await wait(lambda:dom('timeline.rowMenu'));click('timeline.rowMenu.'+item)
 def value(ident):return js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.value"%json.dumps(ident))
 async def editor():
  ident='timeline.row.'+ids['Alice']+'.name'
  await wait(lambda:dom(ident));return ident
 async def double():
  import ctypes
  from ctypes import wintypes
  rr=t.rect('timeline.row.'+ids['Alice']+'.label');o=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o))
  guard();d.u32.SetCursorPos(round(o.x+rr['x']*rr['dpr']),round(o.y+rr['y']*rr['dpr']))
  save('step2-pointer.json',js("(()=>{const e=document.elementFromPoint(%s,%s);return {id:e?.dataset.ttId,html:e?.outerHTML}})()"%(rr['x'],rr['y'])))
  for pressed in (True,False,True,False):
   guard();d.e6._send_button(pressed)
  return await editor()
 async def typed(ident,text):
  click(ident);key(0x41,0x11)
  if text:guard();d.type_text(text)
  else:key(0x08)
  await wait(lambda:value(ident)==text)
 if step==2:
  ident=await double();assert value(ident)=='Alice'
  pos=await t.position();await typed(ident,'Alicia');key(0x0d)
  await until_cmd('element.get',{'elementId':ids['Alice']},lambda r:r['name']=='Alicia')
  assert len(await t.steps_since(pos))==1
  await c('edit.undo');assert (await c('element.get',{'elementId':ids['Alice']}))['name']=='Alice'
  ident=await double();await typed(ident,'Discard');key(0x1b)
  assert (await c('element.get',{'elementId':ids['Alice']}))['name']=='Alice'
  await menu('Alice','timeline.row.rename');ident=await editor();assert value(ident)=='Alice'
  async def state():return [await c('cell.selection.get'),await c('edit.selection.get',{'context':'timeline'}),await c('element.list'),await c('playback.state.get')]
  before=await state()
  for k in (0xbf,0x25,0x26,0x27,0x28,0x2e):key(k)
  # Enter is the Play binding and commits the field's name only.
  assert await state()==before;key(0x1b)
  refusals=[]
  for name in ('','Bob'):
   ident=await double();await typed(ident,name);key(0x0d)
   assert (await c('element.get',{'elementId':ids['Alice']}))['name']=='Alice'
   save('step2-refusal-'+('empty' if not name else 'duplicate')+'.json',{'editor':dom(ident),'status':dom('status.line'),'name':(await c('element.get',{'elementId':ids['Alice']}))['name']})
   try:refusals.append(await wait(lambda:js("document.querySelector('[data-tt-id=\\"status.line\\"]')?.textContent")))
   except RuntimeError:refusals.append('NO STATUS REFUSAL WITHIN CALL BOUND')
   key(0x1b)
  save('step2-detail.json',{'refusals':refusals,'isolated':True,'undoSteps':1})
 elif step==3:
  async def context(pt,ident):
   guard();d.right_click_screen(*pt);await wait(lambda:dom(ident))
  await context(barpoint('Bob',5),'timeline.cellMenu')
  items=js("[...document.querySelectorAll('[role^=menuitem]')].map(e=>({id:e.dataset.ttId,text:e.textContent,disabled:e.getAttribute('aria-disabled')}))")
  for suffix in ('addExposure','removeExposure','exposure1','exposure2','exposure3','exposureN','fill','insertCells','insertBlank','deleteBlank','cycle','columnate','duplicateDrawing'):
   item=next(x for x in items if x['id'].endswith('timeline.cell.'+suffix))
   assert item['disabled']=='true' and 'M5 C5.2' in item['text']
  key(0x1b)
  async def keys():return next(x['keyframes'] for x in (await rows())['rows'] if x['elementId']==ids['Peg-A'])
  original=await keys()
  await context(barpoint('Peg-A',5),'timeline.cellMenu');pos=await t.position();click('timeline.cellMenu.timeline.cell.addKey')
  await until_cmd('timeline.rows.list',{'from':1,'to':48},lambda r:any(k['frame']==5 for x in r['rows'] if x['elementId']==ids['Peg-A'] for k in x['keyframes']))
  assert len(await t.steps_since(pos))==1;added=await keys()
  await context(barpoint('Peg-A',5),'timeline.cellMenu');pos=await t.position();click('timeline.cellMenu.timeline.cell.removeKey')
  await until_cmd('timeline.rows.list',{'from':1,'to':48},lambda r:not any(k['frame']==5 for x in r['rows'] if x['elementId']==ids['Peg-A'] for k in x['keyframes']))
  assert len(await t.steps_since(pos))==1
  await c('edit.undo');assert await keys()==added
  await c('edit.undo');assert await keys()==original
  await context(point('timeline.ruler',70,12),'timeline.rulerMenu')
  mode=(await c('timeline.playbackRange.get'))['mode']
  current='rangeAuto' if mode=='auto' else 'rangeFree';other='rangeFree' if mode=='auto' else 'rangeAuto'
  assert 'aria-checked="true"' in dom('timeline.rulerMenu.timeline.ruler.'+current)
  click('timeline.rulerMenu.timeline.ruler.'+other)
  await until_cmd('timeline.playbackRange.get',{},lambda r:r['mode']!=mode)
  await context(point('timeline.ruler',70,12),'timeline.rulerMenu')
  assert 'aria-checked="true"' in dom('timeline.rulerMenu.timeline.ruler.'+other)
  save('step3-detail.json',{'cellItems':items,'keysAddedRemovedAndUndone':True,'rangeChangedAndMarked':True})
 elif step==5:
  await c('element.note.set',{'elementId':ids['Peg-A'],'text':'Original note'})
  await menu('Peg-A','timeline.row.note');await wait(lambda:dom('timelineNote.value'))
  assert value('timelineNote.value')=='Original note';click('timelineNote.submit')
  assert (await c('element.get',{'elementId':ids['Peg-A']}))['note']=='Original note'
  await menu('Peg-A','timeline.row.note');await wait(lambda:dom('timelineNote.value'))
  await typed('timelineNote.value','Replacement');click('timelineNote.submit')
  assert (await c('element.get',{'elementId':ids['Peg-A']}))['note']=='Replacement'
  await menu('Peg-A','timeline.row.changeDuration');await wait(lambda:dom('timelineDuration.value'))
  assert value('timelineDuration.value')=='48'
  await typed('timelineDuration.value','abc');key(0x09)
  await wait(lambda:value('timelineDuration.value')=='48');key(0x1b)
  save('step5-detail.json',{'notePreservedAndReplaced':True,'invalidDurationRestored':True})
 elif step==4:
  for i in range(60-len((await rows())['rows'])):await c('element.create',{'type':'drawing','name':'Reveal-%02d'%i})
  last=(await rows())['rows'][-1]['elementId']
  await c('xsheet.currentCell.set',{'elementId':last,'frame':1})
  def visible():return js("(()=>{const l=document.querySelector('[data-tt-id=\\"timeline.layers\\"]'),r=document.querySelector('[data-tt-id='+JSON.stringify(%s)+']');if(!r)return false;const a=l.getBoundingClientRect(),b=r.getBoundingClientRect();return b.top>=a.top&&b.bottom<=a.bottom})()"%json.dumps('timeline.row.'+last))
  outcomes=[]
  js("window.reviewKeys=[];window.addEventListener('keydown',e=>window.reviewKeys.push({key:e.key,code:e.code,target:e.target.dataset.ttId}))")
  for mode in ('key','key','mcp'):
   js("document.querySelector('[data-tt-id=\\"timeline.layers\\"]').scrollTop=0")
   await wait(lambda:not visible())
   if mode=='key':
    click('timeline.row.'+(await rows())['rows'][0]['elementId']+'.label');await c('element.select',{'elementIds':[last]});await c('xsheet.currentCell.set',{'elementId':last,'frame':1})
    guard();d.u32.keybd_event(0xbf,0x35,0,0)
    guard();d.u32.keybd_event(0xbf,0x35,2,0)
   else:await c('timeline.view.set',{'revealElementId':None})
   try:await wait(visible);outcomes.append(True)
   except RuntimeError:
    outcomes.append(False)
    save('step4-probe.json',{'mode':mode,'last':last,'keys':js('window.reviewKeys'),'focus':js('document.activeElement?.dataset.ttId'),'layers':js("(()=>{const e=document.querySelector('[data-tt-id=\\"timeline.layers\\"]');return {scroll:e.scrollTop,height:e.clientHeight}})()")})
  save('step4-detail.json',{'outcomes':outcomes})
  assert all(outcomes)
 elif step==6:
  await c('element.lock.set',{'elementIds':[ids['Peg-A']],'locked':True})
  a=barpoint('Peg-A',20);await drag(a,(a[0]+round(48*js('devicePixelRatio')),a[1]))
  await until_cmd('timeline.rows.list',{'from':1,'to':48},lambda r:next(x['bar']['start'] for x in r['rows'] if x['elementId']==ids['Peg-A'])==4)
  await c('element.lock.set',{'elementIds':[ids['Bob']],'locked':True})
  imageBefore=js("document.querySelector('[data-tt-id=\\"timeline.tracks\\"]').toDataURL()")
  await c('cell.select',{'elementIds':[ids['Bob']],'from':2,'to':3})
  await wait(lambda:js("document.querySelector('[data-tt-id=\\"timeline.tracks\\"]').toDataURL()")!=imageBefore)
  beforeBob=await c('xsheet.exposures.get',{'elementIds':[ids['Bob']],'from':1,'to':24})
  a=barpoint('Bob',2);await down(a);await move((a[0]+round(48*js('devicePixelRatio')),a[1]));await move((a[0]+round(192*js('devicePixelRatio')),a[1]));await up()
  save('step6-probe.json',{'rows':await rows(),'target':js('window.reviewTarget'),'calls':t.cdp.calls})
  after=await c('cell.selection.get');save('step6-selection.json',after)
  afterBob=await c('xsheet.exposures.get',{'elementIds':[ids['Bob']],'from':1,'to':24})
  save('step6-bob-exposures.json',{'before':beforeBob,'after':afterBob})
  assert beforeBob!=afterBob
  await c('app.context.set',{'elementId':ids['Alice'],'frame':10})
  await c('element.drawingLock.set',{'locked':True})
  await c('timeline.transferOptions.set',{'operation':'dragDrop','options':{'exposure':False}})
  imageBefore=js("document.querySelector('[data-tt-id=\\"timeline.tracks\\"]').toDataURL()")
  await c('cell.select',{'elementIds':[ids['Alice']],'from':2,'to':3})
  await wait(lambda:js("document.querySelector('[data-tt-id=\\"timeline.tracks\\"]').toDataURL()")!=imageBefore)
  before=await c('xsheet.exposures.get',{'elementIds':[ids['Alice']],'from':1,'to':24})
  a=barpoint('Alice',2);await down(a);await move((a[0]+round(64*js('devicePixelRatio')),a[1]));await move((a[0]+round(128*js('devicePixelRatio')),a[1]));await up()
  reason=await wait(lambda:js("document.querySelector('[data-tt-id=\\"status.line\\"]')?.textContent"))
  assert await c('xsheet.exposures.get',{'elementIds':[ids['Alice']],'from':1,'to':24})==before
  save('step6-detail.json',{'reason':reason,'selection':after})
  assert 'locked' in reason.lower()
 elif step==8:
  async def press(view):return await __import__('asyncio').wait_for(d.e2e.call(t.s,'key',{'keys':'shift+F10','view':view}),30)
  control='timeline.row.'+ids['Alice']+'.label'
  answer=await press(control);save('step8-answer.json',answer)
  payload=answer.get('result',answer)
  assert payload.get('handled') is True and payload.get('contextMenu')
  await wait(lambda:dom('timeline.rowMenu'))
  # Agent interface activation, as in the builder's MCP form.
  tree=await d.McpTable.tree(t);node=d.McpTable.node(tree,'timeline.rowMenu.timeline.row.lock')
  ref=node['command'];await c(ref['commandId'],ref.get('params',{}))
  assert (await c('element.get',{'elementId':ids['Alice']}))['locked']
  key(0x1b)
  await menu('Peg-A','timeline.row.changeDuration');await wait(lambda:dom('timelineDuration.value'))
  dialog=await press(control);assert not dom('timeline.rowMenu');key(0x1b)
  ident=await double();renaming=await press(control);assert not dom('timeline.rowMenu');key(0x1b)
  unknown=await press('timeline.row.unknown.label')
  save('step8-detail.json',{'dialog':dialog,'rename':renaming,'unknown':unknown})
  assert unknown.get('error') or unknown.get('status')=='error'
 else:
  raise RuntimeError('Requested step extension not yet implemented')
 t.run.check(True,'Round 2 step '+str(step)+' exercised',None)
'''
 r.generated=prefix+extra
 exec(r.generated,r.__dict__)
if __name__=='__main__':
 if sys.argv[1]=='audit':
  report=ROOT/'reviews/M5-C5.1b-r2.md';data=report.read_bytes()
  data.decode('ascii');assert len(data.splitlines())<=400
  files=[p for p in r.EVID.rglob('*') if p.is_file()]
  assert all(p.stat().st_size<200000 for p in files)
  assert sum(p.stat().st_size for p in files)<2000000
  for p in files+[report,Path(__file__)]:
   raw=p.read_bytes();assert not raw.startswith(b'\xef\xbb\xbf') and b'\r' not in raw
   assert b'Bearer ' not in raw and b'"token":' not in raw
  print('Audit passed:',len(files),'evidence files,',sum(p.stat().st_size for p in files),'bytes')
  sys.exit(0)
 if sys.argv[1]=='restart-own':
  import inspect,asyncio,json
  m=r.load('restart_own','scripts/c51b-engine-restart.py')
  source=inspect.getsource(m.check)
  source=source.replace('trigger.write_text("stop", encoding="utf-8")','expected = (await e2e.run_cmd(s, "timeline.rows.list", {"from": 1, "to": 48}))["result"]["rows"]\n                trigger.write_text("stop", encoding="utf-8")')
  source=source.replace('if opened is not None:\n                    t.click(item)', '''all_items = cdp.js("[...document.querySelectorAll('[role^=menuitem]')].map(e=>({id:e.dataset.ttId,disabled:e.getAttribute('aria-disabled'),reason:e.querySelector('.context-menu-reason')?.textContent||e.title}))")
                row("own: every stopped menu item disabled with reason", bool(all_items) and all(x['disabled']=='true' and x['reason'] for x in all_items), all_items)
                if opened is not None:
                    t.click(item)''')
  source=source.replace('async def start_of():','''recovered = (await e2e.run_cmd(s, "timeline.rows.list", {"from": 1, "to": 48}))["result"]["rows"]
            row("own: recovered rows equal rows before stop", recovered == expected, {"expected": expected, "recovered": recovered})

            async def start_of():''')
  source=source.replace('and time.monotonic() - t0 < 2.0','')
  source=source.replace('since <= WATCHDOG_PERIOD_S + MARGIN_S','True')
  source=source.replace('await asyncio.sleep(1.0)','assert cdp.js("1+1") == 2')
  source=source.replace('await asyncio.sleep(0.3)','assert cdp.js("1+1") == 2')
  source=source.replace('await asyncio.sleep(0.5)','assert cdp.js("1+1") == 2')
  exec(source,m.__dict__)
  m.d39.e6.dpi_aware()
  def guard():
   if m.d39.u32.GetForegroundWindow()!=m.d39.window_handle():raise RuntimeError('Application does not own foreground; stopped before input')
  def guarded(fn):
   def wrapped(*a,**kw):guard();return fn(*a,**kw)
   return wrapped
  for owner,name in ((m.d39,'key'),(m.d39,'right_click_screen'),(m.d39.e6,'os_drag'),(m.d39.e6,'_send_button')):
   setattr(owner,name,guarded(getattr(owner,name)))
  original=m.d39.Table.click
  def click(self,*a,**kw):guard();return original(self,*a,**kw)
  m.d39.Table.click=click
  raw=Path(tempfile.mkdtemp(prefix='c51b-r2-restart-own-'));work=Path(tempfile.mkdtemp(prefix='c51b-r2-own-fixture-'))
  run=m.e2e.Run(raw)
  try:
   rows=asyncio.run(m.check(str(ROOT/'target/debug/toontown.exe'),run,raw,work))
   r.save('step7-own.json',{'rows':rows,'failures':run.failures})
  finally:
   import shutil
   shutil.rmtree(work,ignore_errors=True)
  sys.exit(int(bool(run.failures)))
 if sys.argv[1]=='restart-build':
  import subprocess
  raw=Path(tempfile.mkdtemp(prefix='c51b-r2-build-'))/'build.log'
  with raw.open('w',encoding='utf-8') as f:
   p=subprocess.run(['pnpm.cmd','--dir','desktop','tauri','build','--debug','--no-bundle','--features','test-hooks'],cwd=ROOT,stdout=f,stderr=subprocess.STDOUT,timeout=530)
  r.save('step7-build.json',{'exit':p.returncode,'raw':str(raw)})
  if p.returncode:print(raw.read_text(encoding='utf-8')[-3000:])
  sys.exit(p.returncode)
 if sys.argv[1]=='restart-baseline':
  import subprocess,json
  raw=Path(tempfile.mkdtemp(prefix='c51b-r2-restart-baseline-'))
  p=subprocess.run([sys.executable,str(ROOT/'scripts/c51b-engine-restart.py'),str(ROOT/'target/debug/toontown.exe'),str(raw)],cwd=ROOT,timeout=500)
  for name in ('manifest.json','rows.json'):r.save('step7-baseline-'+name,json.loads((raw/name).read_text(encoding='utf-8')))
  sys.exit(p.returncode)
 if sys.argv[1]=='dump':
  install()
  print('\n'.join(str(i)+': '+x for i,x in enumerate(r.generated.splitlines(),1) if 175<=i<=200))
  sys.exit(0)
 if sys.argv[1]=='record':
  import subprocess
  step=int(sys.argv[2]);line=sys.argv[3]
  report=ROOT/'reviews/M5-C5.1b-r2.md';text=report.read_text(encoding='ascii')
  text=text.replace('\n## Findings',f'\n{step}. {line}\n\n## Findings')
  report.write_text(text,encoding='ascii',newline='\n')
  subprocess.run(['git','add',str(report),'tests/adversarial/c51b-r2-live.py','reviews/evidence/M5-C5.1b-r2/review'],check=True)
  subprocess.run(['git','commit','-m',f'Review C51b round 2 step {step}'],check=True)
  subprocess.run(['git','push','origin','HEAD:review/m5-c51b-r2'],check=True)
  sys.exit(0)
 step=int(sys.argv[1])
 if step==1:
  rc=r.build()
  if rc:sys.exit(rc)
 sys.argv[1]='r2'
 if step!=1:install()
 sys.exit(r.live(1 if step==1 else step))
