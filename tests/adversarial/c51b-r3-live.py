"""Narrow live closure review. Imports both earlier rounds without editing them."""
import importlib.util, inspect, sys, json, os, subprocess, shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
s=importlib.util.spec_from_file_location('r2',ROOT/'tests/adversarial/c51b-r2-live.py')
r2=importlib.util.module_from_spec(s);s.loader.exec_module(r2)
r=r2.r
r.EVID=ROOT/'reviews/evidence/M5-C5.1b-r3/review'
assert shutil.disk_usage(ROOT).free>120_000_000_000
r2.install()
prefix=r.generated.split(' if step==2:')[0]
extra=r'''
 def text(ident):return js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.textContent"%json.dumps(ident))
 def focus():return js('document.activeElement?.dataset.ttId')
 def menus():return js("[...document.querySelectorAll('[data-tt-role=menu]')].map(e=>e.dataset.ttId)")
 async def press(view):return await asyncio.wait_for(d.e2e.call(t.s,'key',{'keys':'shift+F10','view':view}),30)
 def code(answer):return answer.get('error',{}).get('code')
 if step==1:
  rr=await rows();ui=shown();guard()
  assert [x['name'] for x in ui]==[x['name'] for x in rr['rows']]
  save('step1-detail.json',{'foreground':True,'attached':True,'uiNames':[x['name'] for x in ui],'mcpNames':[x['name'] for x in rr['rows']]})
 elif step==2:
  control='timeline.row.'+ids['Alice']+'.label';answers=[]
  a=await press(control);assert a['result']['handled'] and a['result']['menuId'];await wait(lambda:dom(a['result']['menuId']));answers.append(a);key(0x1b)
  await menu('Peg-A','timeline.row.changeDuration');await wait(lambda:focus()=='timelineDuration.value')
  a=await press(control);assert code(a)=='ui.modalOpen' and not menus() and focus()=='timelineDuration.value';answers.append(a);key(0x1b)
  ident=await double();await wait(lambda:focus()==ident)
  before=[await c('element.list'),await rows(),await c('cell.selection.get')];v=value(ident)
  a=await press(control);assert code(a)=='ui.busy' and not menus() and value(ident)==v and focus()==ident
  assert before==[await c('element.list'),await rows(),await c('cell.selection.get')];answers.append(a);key(0x1b)
  a=await press('timeline.unknown');assert code(a)=='view.notFound';answers.append(a)
  gone=(await c('element.create',{'type':'drawing','name':'Gone'}))['elementIds'][0]
  await wait(lambda:dom('timeline.row.'+gone));await c('element.delete',{'elementIds':[gone]});await wait(lambda:not dom('timeline.row.'+gone))
  a=await press('timeline.row.'+gone);assert code(a)=='view.notFound';answers.append(a)
  a=await press('timeline.add');assert a['result']['handled'] is False and not menus();answers.append(a)
  a=await press(42);assert code(a)=='arg.invalid';answers.append(a)
  save('step2-detail.json',answers)
 elif step==3:
  results=[]
  for name,reason in [('', 'empty'),('Bad/Name','forbidden'),('Bob','already'),('A'*65,'64')]:
   ident=await double();await typed(ident,name);pos=await t.position();key(0x0d)
   await wait(lambda:js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.getAttribute('data-tt-problem')"%json.dumps(ident)))
   status=await wait(lambda:text('status.line'));problem=js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.getAttribute('data-tt-problem')"%json.dumps(ident))
   assert reason in status.lower(),status
   assert value(ident)==name and (await c('element.get',{'elementId':ids['Alice']}))['name']=='Alice' and not await t.steps_since(pos)
   results.append({'typed':name,'status':status,'problem':problem,'historySteps':0});key(0x1b);await wait(lambda:not dom(ident))
  ident=await double();pos=await t.position();key(0x0d);await wait(lambda:not dom(ident))
  assert not text('status.line') and not await t.steps_since(pos)
  save('step3-detail.json',results)
 elif step==4:
  click('timeline.row.'+ids['Alice']+'.label');guard();d.choose_menu(['Element','Rename']);await wait(lambda:dom('timelineRename.value'))
  await typed('timelineRename.value','');click('timelineRename.submit')
  await wait(lambda:text('status.line'));assert dom('timelineRename.value') and value('timelineRename.value')==''
  rename=text('status.line');key(0x1b)
  await menu('Alice','timeline.row.markerColour');await wait(lambda:dom('timelineMarkerColour.value'))
  await typed('timelineMarkerColour.value','zzz');click('timelineMarkerColour.submit');await wait(lambda:text('status.line'))
  assert dom('timelineMarkerColour.value') and value('timelineMarkerColour.value')=='zzz';colour=text('status.line')
  await typed('timelineMarkerColour.value','#123456');click('timelineMarkerColour.submit');await wait(lambda:not dom('timelineMarkerColour.value'))
  result=await c('element.get',{'elementId':ids['Alice']});assert '#123456' in json.dumps(result).lower()
  save('step4-detail.json',{'renameRefusal':rename,'colourRefusal':colour,'element':result})
 elif step==5:
  before=await bar('Peg-A');pos=await t.position();a=barpoint('Peg-A',20);await drag(a,(a[0]+round(48*js('devicePixelRatio')),a[1]))
  await until_cmd('timeline.rows.list',{'from':1,'to':60},lambda rr:next(x['bar']['start'] for x in rr['rows'] if x['elementId']==ids['Peg-A'])==4)
  assert len(await t.steps_since(pos))==1
  await menu('Bob','timeline.row.lock');await until_cmd('element.get',{'elementId':ids['Bob']},lambda rr:rr['locked'])
  for i in range(60-len((await rows())['rows'])):await c('element.create',{'type':'drawing','name':'Reveal-%02d'%i})
  last=(await rows())['rows'][-1]['elementId']
  click('timeline.row.'+(await rows())['rows'][0]['elementId']+'.label');await c('element.select',{'elementIds':[last]});await c('xsheet.currentCell.set',{'elementId':last,'frame':1})
  def visible():return js("(()=>{const l=document.querySelector('[data-tt-id=\"timeline.layers\"]'),r=document.querySelector('[data-tt-id='+JSON.stringify(%s)+']');if(!r)return false;const a=l.getBoundingClientRect(),b=r.getBoundingClientRect();return b.top>=a.top&&b.bottom<=a.bottom})()"%json.dumps('timeline.row.'+last))
  js("document.querySelector('[data-tt-id=\"timeline.layers\"]').scrollTop=0");await wait(lambda:not visible())
  guard();d.u32.keybd_event(0xbf,0x35,0,0);guard();d.u32.keybd_event(0xbf,0x35,2,0);await wait(visible)
  await c('playback.loop.set',{'enabled':True});click('mainToolbar.playStop');await until_cmd('playback.state.get',{},lambda rr:rr['playing'])
  guard();d.click_screen(*point('timeline.ruler',(12-.5)*16,12));await until_cmd('playback.state.get',{},lambda rr:not rr['playing'])
  save('step5-detail.json',{'dragFrames':3,'undoSteps':1,'bobLocked':True,'revealInRows':60,'playStartedAndRulerStopped':True})
 t.run.check(True,'Round 3 step '+str(step)+' exercised',None)
'''
exec(prefix+extra,r.__dict__)
if __name__=='__main__':
 if sys.argv[1]=='record':
  step=int(sys.argv[2]);line=sys.argv[3]
  report=ROOT/'reviews/M5-C5.1b-r3-live.md'
  if not report.exists():
   report.write_text('# M5 C5.1b round 3 live closure\n\n## Summary\n\nP1 0; P2 0; P3 0. No confirmed findings.\nC51B-R2-01: open (verification pending).\nC51B-R2-02: open (verification pending).\nC51B-R2-03: open (verification pending).\nProduct head 578bdd51; review checkout 38d91e23.\n\n## Step results\n\n## Findings\n\nNone.\n\n## Observations\n\nRaw captures in system temp; summaries in evidence/M5-C5.1b-r3/review/.\n',encoding='ascii',newline='\n')
  content=report.read_text(encoding='ascii').replace('\n## Findings',f'{step}. {line}\n\n## Findings')
  if step==2 and line.startswith('PASS'):
   content=content.replace('C51B-R2-02: open (verification pending).','C51B-R2-02: closed.').replace('C51B-R2-03: open (verification pending).','C51B-R2-03: closed.')
  if step==3 and line.startswith('PASS'):content=content.replace('C51B-R2-01: open (verification pending).','C51B-R2-01: closed.')
  report.write_text(content,encoding='ascii',newline='\n')
  files=[p for p in r.EVID.rglob('*') if p.is_file()]
  assert all(p.stat().st_size<200000 for p in files) and sum(p.stat().st_size for p in files)<=1000000
  for p in files+[report,Path(__file__)]:
   raw=p.read_bytes();assert not raw.startswith(b'\xef\xbb\xbf') and b'\r' not in raw
   assert ('Bear'+'er ').encode() not in raw
  subprocess.run(['git','add',str(report),str(Path(__file__)),str(r.EVID)],check=True)
  subprocess.run(['git','commit','-m',f'Review C51b round 3 live step {step}'],check=True)
  subprocess.run(['git','push','origin','HEAD:review/m5-c51b-r3'],check=True)
  sys.exit(0)
 step=int(sys.argv[1])
 if step==1 and not os.environ.get('SKIP_BUILD'):
  rc=r.build()
  if rc:sys.exit(rc)
 sys.exit(r.live(step))
