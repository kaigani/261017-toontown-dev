"""Independent round 2 live checks. Raw captures stay in TEMP."""
import json, os, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EVID=ROOT/'reviews/evidence/M4-C4.4b-r2-review'
def save(name,data):
 EVID.mkdir(parents=True,exist_ok=True)
 text=json.dumps(data,ensure_ascii=True,indent=2)+'\n';assert len(text.encode())<200000
 (EVID/name).write_text(text,encoding='utf-8',newline='\n')
def regressions():
 rows=[]
 for case,steps in [('l1b',range(4,9)),('l2a',range(1,5)),('l2b',range(5,8))]:
  for step in steps:
   raw=Path(tempfile.mkdtemp(prefix='c44b-r2-regression-'))
   with (raw/'console.txt').open('w',encoding='utf-8') as f:
    r=subprocess.run([sys.executable,str(ROOT/'tests/adversarial/c44b-r1-live.py'),case,str(step)],cwd=ROOT,env={**os.environ,'REVIEW_RAW':str(raw),'PYTHONIOENCODING':'utf-8'},stdout=f,stderr=subprocess.STDOUT,timeout=530)
   changed=subprocess.check_output(['git','diff','--name-only'],cwd=ROOT,text=True).splitlines();details=[]
   for path in changed:
    if path.startswith('reviews/evidence/') and not path.startswith('reviews/evidence/M4-C4.4b-r2-review/'):
     value=json.loads((ROOT/path).read_text(encoding='utf-8'));details.append({'file':path,'checks':value.get('checks') if isinstance(value,dict) else None,'failures':value.get('failures') if isinstance(value,dict) else None})
     subprocess.run(['git','checkout','--',path],cwd=ROOT,check=True)
   row={'case':case,'step':step,'exit':r.returncode,'summaries':details,'raw':str(raw)};rows.append(row);save('step1-regressions.json',rows);print(json.dumps(row),flush=True)
 return int(any(r['exit'] for r in rows))

def load(name,path):
 import importlib.util
 spec=importlib.util.spec_from_file_location(name,ROOT/path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
async def matrix(t,step):
 import asyncio,ctypes
 from ctypes import wintypes
 d=PRIOR.d;c=t.cmd;check=t.run.check
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 eq=load('r2eq','scripts/c44b-gesture-equivalence.py')
 async def pause():await asyncio.sleep(.6)
 async def gesture(view,pts):
  d.front();m=(await c('draw.viewMapping.get',{'view':view}))['drawingToView']
  rr=t.cdp.js("document.querySelector('[data-tt-id=\""+view+".canvas\"]').getBoundingClientRect().toJSON()")
  o=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o));scale=t.cdp.js('devicePixelRatio')
  for i,pt in enumerate(pts):
   x,y=eq.c42.apply(m,*pt);assert 0<=x<rr['width'] and 0<=y<rr['height'],(view,pt,rr)
   d.u32.SetCursorPos(round(o.x+(rr['x']+x)*scale),round(o.y+(rr['y']+y)*scale))
   if i==0:d.e6._send_button(True)
   await asyncio.sleep(.12)
  d.e6._send_button(False);await pause()
 async def fixture(view,tool):
  await PRIOR.r3.preserve_fixture(t)
  await c('project.create',{'name':'R2 live matrix','onUnsaved':'discard'})
  eid=(await c('element.create',{'type':'drawing','name':'Target'}))['elementIds'][0]
  await c('element.drawing.create',{'elementId':eid,'name':'1','exposeFrom':1,'exposeTo':24})
  await c('app.context.set',{'elementId':eid,'frame':1})
  await c('workspace.select',{'name':'Drawing' if view=='drawingView' else 'Default'})
  await c('window.panel.set',{'panelId':view,'visible':True,'focus':True});await c('view.active.set',{'viewId':view});await c('view.transform.reset',{'viewId':view})
  sid=(await c('draw.polyline.add',{'points':[{'x':-80,'y':-60},{'x':80,'y':-60},{'x':80,'y':60},{'x':-80,'y':60}],'closed':True,'snap':False}))['strokeIds'][0]
  await c('draw.selection.set',{'by':'ids','ids':[sid]});await c('tool.active.set',{'toolId':tool});await c('tool.snapToContour.set',{'enabled':False});await pause()
  return eid,(await c('draw.drawing.get'))['drawingId']
 tools={'brush':[(-20,0),(20,20)],'rectangle':[(-20,0),(20,20)],'eraser':[(-90,-60),(-60,-60)],'select':[(-80,-60),(-60,-40)],'contourEditor':[(-80,-60),(-60,-40)],'perspective':[(-80,-60),(-60,-40)],'repositionAll':[(0,0),(20,20)],'text':[(0,0),(0,0)]}
 if step==5:
  await fixture('drawingView','contourEditor');await c('window.panel.set',{'panelId':'properties','visible':True});await pause();t.click('properties.page.toolOptions');await pause()
  for tool in ('contourEditor','perspective','repositionAll','text'):
   await c('tool.active.set',{'toolId':tool});await c('draw.selection.set',{'by':'ids','ids':[]});await pause()
   controls=t.cdp.js("[...document.querySelectorAll('[data-tt-id=toolOptions] input,[data-tt-id=toolOptions] select,[data-tt-id=toolOptions] button')].map(e=>({id:e.dataset.ttId,disabled:e.disabled,reason:e.title||e.closest('label')?.title}))")
   check(bool(controls),tool+' options controls present',controls)
   if tool=='contourEditor':check(all(any(x['id']==i and x['disabled'] and x['reason'] for x in controls) for i in ('toolOptions.selectedLine.thickness','toolOptions.selectedLine.invisible')),'D-79 no line controls disabled with reason',controls)
   t.run.save(tool+'-options.json',controls)
   for ident in ('toolOptions.snapToContour','toolOptions.placeBehind'):
    if any(x['id']==ident and not x['disabled'] for x in controls):
     before=await c('tool.options.get',{'toolId':tool});t.click(ident);await pause();after=await c('tool.options.get',{'toolId':tool});check(before!=after,tool+' enabled '+ident+' acts',{'before':before,'after':after})
  t.click('properties.page.drawing');await pause()
  browse=t.cdp.js("(()=>{let e=document.querySelector('[data-tt-id=\"inspector.browse\"]');return e?{disabled:e.disabled,reason:e.title||e.closest('label')?.title}:null})()")
  check(bool(browse) and browse['disabled'] and bool(browse['reason']),'D-79 Browse present disabled reason',browse)
  return
 for view in ('cameraView','drawingView'):
  for tool,pts in tools.items():
   eid,did=await fixture(view,tool)
   if step==2:await c('element.lock.set',{'elementIds':[eid],'locked':True})
   elif step==3:await c('draw.lock.set',{'locked':True})
   elif step==4:
    if tool not in ('contourEditor','perspective','repositionAll','text') or view!='cameraView':continue
    await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':False,'outlineOnSelection':False});await c('drawAid.currentOnTop.set',{'enabled':False});await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False})
   await pause();before=await c('draw.drawing.get',{'drawingId':did});pos=await t.position();t.clear()
   await gesture(view,pts)
   if tool=='text':d.front();d.type_text('X');await pause()
   after=await c('draw.drawing.get',{'drawingId':did});history=await t.steps_since(pos)
   node=d.McpTable.node(await d.McpTable.tree(t),view+'.canvas');canvas=json.loads(node.get('value') or '{}')
   status=t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent")
   commands=t.recorded();refused=step in (3,4) or (step==2 and view=='cameraView')
   name=view+' '+tool
   if step==4:check(not canvas.get('toolBox') and not canvas.get('contours') and not canvas.get('textEdit') and not canvas.get('textBoxes'),'D-79 hidden no overlays '+tool,{k:canvas.get(k) for k in ('drawings','toolBox','contours','textEdit','textBoxes')})
   if refused:
    check(before==after and not history,name+' D-80/D-79 no edit or undo',{'history':history,'lastGesture':canvas.get('lastGesture'),'status':status})
    code='drawing.locked' if step==3 else 'element.hidden' if step==4 else 'element.locked'
    check(bool(status) and (code in str(status) or any(w in str(status).lower() for w in ('locked','hidden'))),name+' person told '+code,status)
   else:check(before!=after and bool(history),name+' D-80 Drawing view edits',{'history':history,'status':status})
   t.run.save(name.replace(' ','-')+'.json',{'before':before,'after':after,'history':history,'canvas':canvas,'status':status,'commands':commands})
   if step==2 and view=='cameraView' and tool=='select':
    await c('draw.selection.set',{'by':'ids','ids':[]});await gesture(view,[(-80,-60),(-80,-60)]);picked=await c('draw.selection.get');check(not picked.get('selectedIds'),'D-80 Camera pick locked content refused',picked)



 if step==3:
  eid,did=await fixture('drawingView','repositionAll');ids=[did]
  for i in (2,3):
   await c('element.drawing.create',{'elementId':eid,'name':str(i),'exposeFrom':i,'exposeTo':i});await c('app.context.set',{'elementId':eid,'frame':i});await c('draw.rectangle.add',{'from':{'x':-80,'y':-60},'to':{'x':80,'y':60}});ids.append((await c('draw.drawing.get'))['drawingId'])
  await c('app.context.set',{'elementId':eid,'frame':1});await c('element.drawingLock.set',{'drawingIds':[ids[1]],'locked':True});await pause()
  before=[await c('draw.drawing.get',{'drawingId':i}) for i in ids];await gesture('drawingView',[(0,0),(20,20)]);after=[await c('draw.drawing.get',{'drawingId':i}) for i in ids]
  check(before[0]!=after[0] and before[1]==after[1] and before[2]!=after[2],'SI-189 mixed bank edits two skips locked',{'ids':ids})
  canvas=json.loads(d.McpTable.node(await d.McpTable.tree(t),'drawingView.canvas').get('value') or '{}');t.run.save('mixed-bank.json',canvas)
  answer=await c('draw.repositionAll.apply',{'translate':{'x':1,'y':0}});check(ids[1] in json.dumps(answer),'SI-189 explicit command answer lists skipped drawing',answer);await c('edit.undo')
  await c('element.drawingLock.set',{'drawingIds':ids,'locked':True});after=[await c('draw.drawing.get',{'drawingId':i}) for i in ids];pos=await t.position();await gesture('drawingView',[(0,0),(20,20)]);final=[await c('draw.drawing.get',{'drawingId':i}) for i in ids]
  check(after==final and await t.position()==pos,'SI-189 all locked refused',None)
 if step==4:
  for option in ('keepSelectionVisible','outlineOnSelection'):
   for tool in ('contourEditor','perspective','repositionAll','text'):
    eid,did=await fixture('cameraView',tool);await c('edit.selection.set',{'context':os.environ.get('C44B_SELECTION_CONTEXT','timeline'),'ids':[eid]});await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':option=='keepSelectionVisible','outlineOnSelection':option=='outlineOnSelection'});await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False});await pause()
    canvas=json.loads(d.McpTable.node(await d.McpTable.tree(t),'cameraView.canvas').get('value') or '{}');t.run.save(option+'-'+tool+'.json',canvas);before=await c('draw.drawing.get',{'drawingId':did});await gesture('cameraView',tools[tool]);after=await c('draw.drawing.get',{'drawingId':did})
    if tool=='text' and option=='keepSelectionVisible':d.front();d.type_text('X');await pause();after=await c('draw.drawing.get',{'drawingId':did})
    check((before!=after)==(option=='keepSelectionVisible'),'02 10.7 '+option+' '+tool+' edit policy',canvas)
  for tool in ('brush','text'):
   eid,did=await fixture('cameraView',tool);await c('draw.selection.set',{'by':'all'});await c('draw.object.delete',{'ids':(await c('draw.selection.get'))['selectedIds']});await pause();before=await c('draw.drawing.get');await gesture('cameraView',tools[tool])
   if tool=='text':d.front();d.type_text('X');await pause()
   check(before!=await c('draw.drawing.get'),'D-79 visible empty '+tool+' adds',None)

def live(step):
 global PRIOR
 PRIOR=load('r2prior','tests/adversarial/c44a-r1-live.py');PRIOR.EVID=Path(tempfile.mkdtemp(prefix='c44b-r2-summary-'));PRIOR.live_body=matrix
 result=PRIOR.live(step)
 transcript=json.loads((PRIOR.OUT/'transcript.json').read_text());checks=[r for r in transcript if 'check' in r]
 save('step'+str(step)+'.json',{'raw':str(PRIOR.OUT),'checks':checks,'failures':[r for r in checks if not r.get('ok')]})
 return result

async def text_checks(t,step):
 import inspect
 old=load('r2text','tests/adversarial/c44b-r1-live.py');old.PRIOR=PRIOR
 source=inspect.getsource(old.text_body)
 start=source.index('        if step==1:');end=source.index('    finally:',start)
 branches="""        if step==6:
            await fixture();pos=await t.position();await typed('A')
            await c('window.panel.set',{'panelId':'text','visible':True});await pause();t.click('text.bold');await pause();r=await typed('B')
            labels=await t.steps_since(pos);check([x[0] for x in labels]==['Typing','Typing'],'D-78 pending format two Typing steps',labels)
            check(content(r)=='AB' and len(texts(r)[0]['runs'])>=2 and texts(r)[0]['runs'][-1].get('bold')==True,'D-78 B alone bold',texts(r))
            await key(0x5a,0x11);r=await state('undo B');check(content(r)=='A','D-78 undo removes only B',texts(r))
            await fixture('ABC');pos=await t.position();r=await key(0x5a,0x11)
            check(content(r)=='' and await t.position()==pos-1,'D-78 canvas Ctrl Z document undo',r['history'])
            r=await key(0x5a,0x11,0x10);check(content(r)=='ABC' and await t.position()==pos,'D-78 canvas redo',r['history'])
            r=await typed('X');check(content(r)=='ABCX' and r['editor'] is not None,'D-78 valid caret after redo',r['editor'])
            t.click('text.size');d.key(0x41,0x11);d.type_text('42');await pause();pos=await t.position();d.key(0x5a,0x11);await pause()
            check(await t.position()==pos and t.cdp.js("document.querySelector('[data-tt-id=\"text.size\"]').value")!='42','D-78 ordinary numeric field keeps undo',None)
            await c('tool.active.set',{'toolId':'brush'});await c('window.panel.set',{'panelId':'pen','visible':True});await pause()
            pens=(await c('tool.pen.list'))['pens'];t.click('pen.row.'+pens[0]['penId']);d.key(0x71);await pause();t.click('pen.rename');d.key(0x41,0x11);d.type_text('Review temporary');pos=await t.position();d.key(0x5a,0x11);await pause()
            check(await t.position()==pos and t.cdp.js("document.querySelector('[data-tt-id=\"pen.rename\"]').value")!='Review temporary','D-78 ordinary rename field keeps undo',None);d.key(0x1b)

        elif step==7:
            for v in ('drawingView','cameraView'):
                r=await fixture('hello world',v=v);node=texts(r)[0]
                doc=await c('doc.export.json',{'scope':'project','binaries':'omit'});colour=doc['bundle']['palettes'][0]['swatches'][2]['colourId']
                await c('colour.current.set',{'colourId':colour});r=await typed('X');check(texts(r)[0]['runs'][-1]['colourId']==colour,'D-78 current colour '+v,texts(r))
                pos=await t.position();await gesture([(-70,65)],double=True);r=await state('word double click')
                check(r['editor'] is not None and sorted([r['editor']['anchor'],r['editor']['caret']])==[0,5] and await t.position()==pos,'D-81 exact word no undo '+v,r['editor'])
                q=p.IndependentHttp(json.loads((t.work/'discovery.json').read_text()),'r2-independent-word')
                try:
                    q.initialize();m=(await c('draw.viewMapping.get',{'view':view}))['drawingToView'];eq=load('r2wordeq','scripts/c44b-gesture-equivalence.py');x,y=eq.c42.apply(m,-70,65)
                    answer=await asyncio.to_thread(q.tool,'pointer',{'view':view,'tool':'text','space':'view','events':[{'type':'down','x':x,'y':y},{'type':'up','x':x,'y':y},{'type':'doubleClick','x':x,'y':y}]});rows.append({'name':'independent double click','answer':answer});r=await state('agent word')
                    check(r['editor'] is not None and sorted([r['editor']['anchor'],r['editor']['caret']])==[0,5] and await t.position()==pos,'D-81 MCP exact word no undo '+v,r['editor'])
                finally:q.close()
                await c('draw.lock.set',{'locked':True});r=await state('locked session');check(r['editor'] is None,'D-81 lock ends session '+v,r['editor'])
                await gesture([(-70,65)]);r=await state('locked click');check(r['editor'] is None and 'locked' in str(r['status']).lower(),'D-81 locked click refused and told '+v,{'editor':r['editor'],'status':r['status']})
        elif step==8:
            r=await fixture('abcdef');node=texts(r)[0];await c('draw.text.set',{'textId':node['id'],'text':'a'});r=await typed('X')
            check(content(r)=='aX' and r['editor'] is not None and r['editor']['caret']==2,'D-78 other origin shorten caret reconciled',{'text':content(r),'editor':r['editor']})
            await c('draw.object.delete',{'ids':[node['id']]});r=await typed('Y');check(content(r)=='' and r['editor'] is None,'D-78 other origin deletion ends session',r['editor'])
            await fixture('one two, three');r=await key(0x25,0x11);check(r['editor']['caret']==9,'D-81 word left',r['editor'])
            r=await key(0x25,0x11,0x10);check(sorted([r['editor']['anchor'],r['editor']['caret']])==[7,9],'D-81 shift word left punctuation',r['editor'])
            r=await key(0x27,0x11);check(r['editor']['caret']==8,'D-81 word right punctuation',r['editor'])
            r=await key(0x08,0x11);check(content(r)=='one two three','D-81 word backspace punctuation',content(r))
            r=await key(0x2e,0x11);check(content(r)=='one two','D-81 word delete',content(r))
            await fixture();r=await typed(chr(0x1f642));check(content(r)==chr(0x1f642),'D-81 paired UTF16 events one scalar',content(r))
"""
 source=source[:start]+branches+source[end:]
 namespace=dict(old.__dict__);exec(source,namespace);await namespace['text_body'](t,step)

if __name__=='__main__':
 case,step=sys.argv[1],int(sys.argv[2]);assert case in ('m1','m2')
 if case=='m1' and step==1:sys.exit(regressions())
 if case=='m1' and step in (2,3,4,5):sys.exit(live(step))
 if case=='m2' and step in (6,7,8):
  matrix=text_checks
  sys.exit(live(step))
 raise ValueError('step implementation pending')
