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
  click('timeline.row.'+ids['Alice']+'.label');click('timeline.row.'+ids['Alice']+'.label')
  return await editor()
 async def typed(ident,text):
  click(ident);key(0x41,0x11);guard();d.type_text(text)
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
  key(0x0d)
  assert await state()==before;key(0x1b)
  refusals=[]
  for name in ('','Bob'):
   ident=await double();await typed(ident,name);key(0x0d)
   assert (await c('element.get',{'elementId':ids['Alice']}))['name']=='Alice'
   refusals.append(await wait(lambda:js("document.querySelector('[data-tt-id=\\"status.line\\"]')?.textContent")))
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
  for i in range(56):last=(await c('element.create',{'type':'drawing','name':'Reveal-%02d'%i}))['elementIds'][0]
  await c('xsheet.currentCell.set',{'elementId':last,'frame':1})
  def visible():return js("(()=>{const l=document.querySelector('[data-tt-id=\\"timeline.layers\\"]'),r=document.querySelector('[data-tt-id='+JSON.stringify(%s)+']');if(!r)return false;const a=l.getBoundingClientRect(),b=r.getBoundingClientRect();return b.top>=a.top&&b.bottom<=a.bottom})()"%json.dumps('timeline.row.'+last))
  for mode in ('key','key','mcp'):
   js("document.querySelector('[data-tt-id=\\"timeline.layers\\"]').scrollTop=0")
   await wait(lambda:not visible())
   if mode=='key':
    click('timeline.row.'+ids['Peg-A']+'.label');await c('xsheet.currentCell.set',{'elementId':last,'frame':1});key(0xbf)
   else:await c('timeline.view.set',{'revealElementId':None})
   await wait(visible)
  save('step4-detail.json',{'revealedThreeTimes':True})
 elif step==8:
  async def press(view):return await __import__('asyncio').wait_for(d.e2e.call(t.s,'key',{'keys':'shift+F10','view':view}),30)
  control='timeline.row.'+ids['Alice']+'.label'
  answer=await press(control);save('step8-answer.json',answer)
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
 exec(prefix+extra,r.__dict__)
if __name__=='__main__':
 step=int(sys.argv[1])
 if step==1:
  rc=r.build()
  if rc:sys.exit(rc)
 sys.argv[1]='r2'
 if step!=1:install()
 sys.exit(r.live(1 if step==1 else step))
