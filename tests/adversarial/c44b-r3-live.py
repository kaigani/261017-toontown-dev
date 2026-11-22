"""Round 3 live probes. Fresh sessions; raw captures remain in TEMP."""
import importlib.util, inspect, json, os, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EVID=ROOT/'reviews/evidence/M4-C4.4b-r3-review'
def load(name,path):
 s=importlib.util.spec_from_file_location(name,ROOT/path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def save(name,data):
 EVID.mkdir(parents=True,exist_ok=True)
 value=json.dumps(data,ensure_ascii=True,indent=2)+'\n';assert len(value.encode())<200000
 (EVID/name).write_text(value,encoding='utf-8',newline='\n')
def prior_runs():
 rows=[]
 for case,step in [('m1',2),('m1',3),('m1',4),('m1',5),('m2',6),('m2',7),('m2',8)]:
  raw=Path(tempfile.mkdtemp(prefix='c44b-r3-old-'))
  with (raw/'console.txt').open('w',encoding='utf-8') as f:
   r=subprocess.run([sys.executable,str(ROOT/'tests/adversarial/c44b-r2-live.py'),case,str(step)],cwd=ROOT,stdout=f,stderr=subprocess.STDOUT,timeout=530,env={**os.environ,'REVIEW_RAW':str(raw),'PYTHONIOENCODING':'utf-8'})
  receipt=json.loads((ROOT/f'reviews/evidence/M4-C4.4b-r2-review/step{step}.json').read_text())
  rows.append({'case':case,'step':step,'exit':r.returncode,**receipt});save('step1.json',rows)
  subprocess.run(['git','checkout','--','reviews/evidence/M4-C4.4b-r2-review'],cwd=ROOT,check=True)
  print(case,step,r.returncode,receipt['failures'],flush=True)
 return int(any(x['exit'] for x in rows))
def run_text(step):
 p=load('r3prior','tests/adversarial/c44a-r1-live.py');p.EVID=Path(tempfile.mkdtemp(prefix='r3summary-'))
 old=load('r3oldtext','tests/adversarial/c44b-r1-live.py');old.PRIOR=p;old.EVID=p.EVID
 source=inspect.getsource(old.text_body);a=source.index('        if step==1:');b=source.rindex('    finally:')
 branches=TEXT_BRANCHES
 source=source[:a]+branches+source[b:]
 ns=dict(old.__dict__);exec(source,ns);p.live_body=ns['text_body']
 result=p.live(step)
 transcript=json.loads((p.OUT/'transcript.json').read_text());checks=[r for r in transcript if 'check' in r]
 save('step'+str(step)+'-text.json',{'raw':str(p.OUT),'checks':checks,'failures':[r for r in checks if not r.get('ok')]})
 return result
def run_matrix(step):
 p=load('r3prior','tests/adversarial/c44a-r1-live.py');p.EVID=Path(tempfile.mkdtemp(prefix='r3summary-'))
 old=load('r3matrix','tests/adversarial/c44b-r2-live.py');old.PRIOR=p
 source=inspect.getsource(old.matrix);source=source[:source.index(' if step==5:')]+MATRIX_BRANCHES
 ns=dict(old.__dict__);exec(source,ns);p.live_body=ns['matrix']
 result=p.live(step);transcript=json.loads((p.OUT/'transcript.json').read_text());checks=[r for r in transcript if 'check' in r]
 save('step'+str(step)+'.json',{'raw':str(p.OUT),'checks':checks,'failures':[r for r in checks if not r.get('ok')]})
 return result
def run_regression():
 p=load('r3prior','tests/adversarial/c44a-r1-live.py');p.EVID=Path(tempfile.mkdtemp(prefix='r3summary-'))
 old=load('r3tools','tests/adversarial/c44b-r1-live.py');old.PRIOR=p
 s=inspect.getsource(old.contour_body);s=s[:s.index("    for view in ('drawingView','cameraView'):")]+"    view='cameraView'\n    await compare(view,'camera-anchor',lambda sid:gesture(view,[(-80,0),(-65,15)]),'draw.point.move')\n"
 ns=dict(old.__dict__);exec(s,ns);contour=ns['contour_body']
 s=inspect.getsource(old.envelope_body);s=s[:s.index("    for view in ('drawingView','cameraView'):")]+"    view='cameraView'\n    await compare(view,'camera-corner',[(-80,-60),(-50,-35)])\n    await compare(view,'camera-offset',[(0,0),(20,15)],tool='repositionAll')\n"
 exec(s,ns);envelope=ns['envelope_body']
 async def body(t,step):
  await contour(t,5);await envelope(t,1)
 p.live_body=body;result=p.live(8);rows=json.loads((p.OUT/'transcript.json').read_text());checks=[r for r in rows if 'check' in r]
 save('step8.json',{'raw':str(p.OUT),'checks':checks,'failures':[r for r in checks if not r.get('ok')]});return result
MATRIX_BRANCHES=r'''
 async def condition(kind,eid):
  if kind=='element':await c('element.lock.set',{'elementIds':[eid],'locked':True})
  elif kind=='drawing':await c('draw.lock.set',{'locked':True})
  else:
   await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':False,'outlineOnSelection':False})
   await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False})
  await pause()
 async def record(name,did,before,pos,refused=True):
  after=await c('draw.drawing.get',{'drawingId':did});history=await t.steps_since(pos)
  status=t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent")
  check((before==after and not history) if refused else (before!=after and bool(history)),name+' document and undo',{'history':history,'status':status})
  if refused:check(any(x in str(status).lower() for x in ('locked','hidden')),name+' reason',status)
  t.run.save(name+'.json',{'before':before,'after':after,'history':history,'status':status,'commands':t.recorded(),'visibleText':t.cdp.js('document.body.innerText')})
 if step==2:
  for view,kind in [('cameraView','element'),('cameraView','drawing'),('cameraView','hidden'),('drawingView','element')]:
   for action in ('anchorDelete','selectDelete','cut','paste','duplicate','nudge'):
    tool='contourEditor' if action=='anchorDelete' else 'select'
    eid,did=await fixture(view,tool);sid=(await c('draw.selection.get'))['selectedIds'][0]
    await c('edit.selection.set',{'context':'drawing','ids':[sid]})
    if action=='anchorDelete':await c('draw.pointSelection.set',{'points':[{'strokeId':sid,'anchor':1}]})
    if action=='paste':await c('edit.copy',{'context':'drawing','objectIds':[sid]})
    t.click(view+'.canvas');await c('draw.selection.set',{'by':'ids','ids':[sid]})
    if action=='anchorDelete':await c('draw.pointSelection.set',{'points':[{'strokeId':sid,'anchor':1}]})
    await condition(kind,eid);before=await c('draw.drawing.get');pos=await t.position();t.clear();d.front()
    vk,mods={'anchorDelete':(0x2e,()),'selectDelete':(0x2e,()),'cut':(0x58,(0x11,)),'paste':(0x56,(0x11,)),'duplicate':(0x44,(0x11,)),'nudge':(0x27,())}[action]
    d.key(vk,*mods);await pause()
    if action in ('duplicate','nudge'):
     check(before==await c('draw.drawing.get') and await t.position()==pos,view+' '+kind+' unbound '+action+' observation',None)
    else:await record(view+'-'+kind+'-'+action,did,before,pos,view=='cameraView')
 elif step==3:
  for kind in ('element','drawing','hidden'):
   for action in ('thickness','invisible'):
    eid,did=await fixture('cameraView','contourEditor');await c('window.panel.set',{'panelId':'properties','visible':True});await pause();t.click('properties.page.toolOptions');await pause()
    await condition(kind,eid);before=await c('draw.drawing.get');pos=await t.position();t.clear()
    t.click('toolOptions.selectedLine.'+action)
    if action=='thickness':d.key(0x41,0x11);d.type_text('17');d.key(0x0d)
    await pause();await record(kind+'-'+action,did,before,pos)
   await c('window.panel.set',{'panelId':'pen','visible':True});await pause()
   settings=await c('tool.settings.get');t.run.save(kind+'-pen-settings.json',settings)
   pens=(await c('tool.pen.list'))['pens'];t.click('pen.row.'+pens[0]['penId']);await pause()
   check(bool(await c('tool.settings.get')),'D-83 '+kind+' Pen selection works',None)
   for menu in ('Cut','Paste','Delete'):
    eid,did=await fixture('cameraView','select');sid=(await c('draw.selection.get'))['selectedIds'][0]
    await c('edit.copy',{'context':'drawing','objectIds':[sid]});await condition(kind,eid)
    before=await c('draw.drawing.get');pos=await t.position();t.clear();d.choose_menu(['Edit',menu]);await pause();d.key(0x1b)
    await record(kind+'-native-'+menu,did,before,pos)
   eid,did=await fixture('cameraView','select');await condition(kind,eid);before=await c('draw.drawing.get');pos=await t.position()
   rr=t.rect('cameraView.canvas');o=wintypes.POINT();d.front();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o));d.right_click_screen(o.x+rr['x']*rr['dpr'],o.y+rr['y']*rr['dpr']);await pause()
   def flatten(n):return [n]+[x for ch in n.get('children',[]) for x in flatten(ch)]
   items=[x for x in flatten(await d.McpTable.tree(t)) if x.get('role')=='menuitem' and x.get('command',{}).get('commandId')=='edit.cut']
   check(bool(items),'context Cut offered '+kind,items)
   if items:t.click(items[0]['id']);await pause();await record(kind+'-context-cut',did,before,pos)
   d.key(0x1b)
 elif step==4:
  for kind in ('element','hidden','drawing'):
   eid,did=await fixture('cameraView','contourEditor');sid=(await c('draw.selection.get'))['selectedIds'][0];await condition(kind,eid)
   before=await c('draw.drawing.get');pos=await t.position()
   # Independent transport, explicit drawing and stroke ids.
   q=PRIOR.IndependentHttp(json.loads((t.work/'discovery.json').read_text()),'r3-explicit')
   try:
    q.initialize();answer=await asyncio.to_thread(q.tool,'run_command',{'command':'draw.stroke.set','params':{'drawingId':did,'strokeIds':[sid],'thickness':13}})
   finally:q.close()
   after=await c('draw.drawing.get');check(before!=after if kind!='drawing' else before==after and 'drawing.locked' in str(answer),'D-83 independent explicit '+kind,answer)
  for context in ('timeline','xsheet'):
   for option in ('keepSelectionVisible','off','outlineOnSelection'):
    for tool in ('contourEditor','perspective','repositionAll','text'):
     eid,did=await fixture('cameraView',tool);await c('edit.selection.set',{'context':context,'ids':[eid]})
     await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':option=='keepSelectionVisible','outlineOnSelection':option=='outlineOnSelection'})
     await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False});await pause()
     canvas=json.loads(d.McpTable.node(await d.McpTable.tree(t),'cameraView.canvas').get('value') or '{}')
     before=await c('draw.drawing.get');pts=[(-84,-64),(-64,-44)] if tool=='perspective' else tools[tool]
     await gesture('cameraView',pts)
     if tool=='text':d.front();d.type_text('X');await pause()
     after=await c('draw.drawing.get');check((before!=after)==(option=='keepSelectionVisible'),'D-85 '+context+' '+option+' '+tool,{'drawings':canvas.get('drawings'),'toolBox':canvas.get('toolBox')})
'''
TEXT_BRANCHES=r'''
        if step in (2,3):
            for kind in ('element','drawing','hidden'):
                await fixture('word',v='cameraView');await key(0x24);await key(0x23,0x10)
                await c('window.panel.set',{'panelId':'text','visible':True});await pause()
                if kind=='element':await c('element.lock.set',{'elementIds':[eid],'locked':True})
                elif kind=='drawing':await c('draw.lock.set',{'locked':True})
                else:
                    await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':False,'outlineOnSelection':False});await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False})
                await pause();before=await c('draw.drawing.get',{'flatten':True});pos=await t.position()
                if step==2:r=await typed('X')
                else:t.click('text.bold');await pause();r=await state('locked Bold')
                check(before==r['drawing'] and await t.position()==pos,'D-83 '+kind+' text unchanged',r['status'])
                check(any(x in str(r['status']).lower() for x in ('locked','hidden')),'D-83 '+kind+' text reason',r['status'])
        elif step==5:
            await fixture(v='cameraView');await c('window.panel.set',{'panelId':'text','visible':False});await typed('A')
            await c('window.panel.set',{'panelId':'text','visible':True});await pause();r=await state('mounted caret')
            check('next' in str(r['role']).lower(),'D-85 mounted caret role',r['role'])
            pos=await t.position();t.clear();t.click('text.bold');await pause();r=await typed('B')
            check(content(r)=='AB' and texts(r)[0]['runs'][-1].get('bold')==True and not texts(r)[0]['runs'][0].get('bold',False),'D-85 B alone bold',texts(r))
            check([x[0] for x in await t.steps_since(pos-1)]==['Typing','Typing'],'D-78 two typing steps',await t.steps_since(pos-1))
            check(any(x['command']=='draw.textFormat.set' for x in r['commands']) and not any(x['command']=='tool.text.options.set' for x in r['commands']),'D-85 session format command',r['commands'])
            await fixture('word',v='cameraView');await key(0x24);await key(0x23,0x10)
            await c('window.panel.set',{'panelId':'text','visible':False});await c('window.panel.set',{'panelId':'text','visible':True});await pause();r=await state('mounted highlight')
            check('selected' in str(r['role']).lower(),'D-85 highlighted role',r['role'])
            pos=await t.position();t.click('text.bold');await pause();r=await state('formatted highlight')
            check(all(x.get('bold') for x in texts(r)[0]['runs']) and [x[0] for x in await t.steps_since(pos)]==['Text Property Modified'],'D-85 highlight one format step',r['history'])
            await c('window.panel.set',{'panelId':'text','visible':False});await c('window.panel.set',{'panelId':'text','visible':True});await pause();r=await state('remount')
            check('selected' in str(r['role']).lower(),'D-85 remount keeps session',r['role'])
        elif step==6:
            clusters=['A','q\u0301','\U0001f469\u200d\U0001f4bb','\U0001f1fa\U0001f1f8','\u2708\ufe0f','\ud55c','Z']
            text=''.join(clusters);bounds=[0]
            for x in clusters:bounds.append(bounds[-1]+len(x))
            await fixture(v='cameraView');clip_set(text);r=await key(0x56,0x11)
            check(content(r)==text,'D-82 paste corpus exact',content(r))
            await key(0x24);stops=[]
            for _ in clusters:
                r=await key(0x27);stops.append(r['editor']['caret'])
            check(stops==bounds[1:],'D-82 Right whole clusters',stops)
            stops=[]
            for _ in clusters:
                r=await key(0x25);stops.append(r['editor']['caret'])
            check(stops==list(reversed(bounds[:-1])),'D-82 Left whole clusters',stops)
            selections=[]
            for _ in clusters:
                r=await key(0x27,0x10);selections.append([r['editor']['anchor'],r['editor']['caret']])
            check(selections==[[0,i] for i in bounds[1:]],'D-82 Shift Right clusters',selections)
            await key(0x24)
            for _ in range(3):await key(0x27)
            pos=await t.position();r=await key(0x08)
            check(content(r)==''.join(clusters[:2]+clusters[3:]) and await t.position()==pos+1,'D-82 Backspace whole emoji',content(r))
            r=await key(0x5a,0x11);check(content(r)==text and await t.position()==pos,'D-82 undo restores emoji',content(r))
            await key(0x24)
            for _ in range(3):await key(0x27)
            r=await key(0x2e);check(content(r)==''.join(clusters[:3]+clusters[4:]),'D-82 Delete whole flag',content(r))
        elif step==7:
            emoji='\U0001f469\u200d\U0001f4bb'
            await fixture('word '+emoji+' next',v='cameraView')
            await gesture([(-70,65)],double=True);r=await state('word next to emoji')
            check(sorted([r['editor']['anchor'],r['editor']['caret']])==[0,4],'D-82 double click whole word',r['editor'])
            # Use actual scene caret geometry to locate the two sides of the emoji.
            await key(0x24)
            for _ in range(5):await key(0x27)
            left=await state('emoji left')
            def scene():
                value=t.cdp.js("window.__TAURI_INTERNALS__.invoke('canvas_scene',{json:JSON.stringify({viewId:'cameraView'})})")
                return json.loads(value) if isinstance(value,str) else value
            ls=scene()
            await key(0x27);right=await state('emoji right')
            rs=scene();t.run.save('emoji-scenes.json',{'left':ls,'right':rs})
            l=ls.get('scene',ls)['textEdit']['caret'];r0=rs.get('scene',rs)['textEdit']['caret']
            await gesture([((l[0][0]+r0[0][0])/2,(l[0][1]+l[1][1])/2)])
            r=await state('real emoji middle click');check(r['editor']['caret'] in (5,8),'D-82 real middle click never inside',r['editor'])
            t.run.save('emoji-click-geometry.json',{'left':left['canvas'],'right':right['canvas']})
            check(left['editor']['caret']==5 and right['editor']['caret']==8,'D-82 emoji bounds usable',{'left':left['editor'],'right':right['editor']})
            for action in (0x27,0x08):
                await fixture('word abc next',v='cameraView');await key(0x24)
                for _ in range(6):await key(0x27)
                r=await state('codepoint 6');node=texts(r)[0]
                await c('draw.text.set',{'textId':node['id'],'text':'word '+emoji+' next'})
                r=await state('agent puts cluster around caret');t.run.save('agent-interior-'+str(action)+'.json',r)
                check(r['editor']['caret'] in (5,8),'D-82 agent content interior reconciles',r['editor'])
                r=await key(action);check(content(r)==('word '+emoji+' next' if action==0x27 else 'word  next'),'D-82 real key after agent interior '+str(action),{'text':content(r),'editor':r['editor']})
        elif step==8:
            import shutil
            from mcp import ClientSession,StdioServerParameters,types
            from mcp.client.stdio import stdio_client
            await fixture(v='cameraView');base=t.work/'text-base.animpkg';await c('project.saveAs',{'path':str(base)})
            initial=await state('empty text');tid=texts(initial)[0]['id'];pos=await t.position();r=await typed('Hi')
            check([x[0] for x in await t.steps_since(pos)]==['Typing'],'02 12.1 Text one Typing step',await t.steps_since(pos))
            copy=t.work/'text-direct.animpkg';shutil.copyfile(base,copy)
            async def roots(_):return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='work')])
            args=['--mcp-stdio','--project',str(copy),'--policy','autonomous','--allow-path',str(t.work),'--user-dir',str(t.work/'text-head')]
            async with stdio_client(StdioServerParameters(command=str(ROOT/'target/release/toontown-headless.exe'),args=args)) as streams:
                async with ClientSession(*streams,list_roots_callback=roots) as hs:
                    await hs.initialize();ht=d.Table(hs,t.run,p.OUT,t.work)
                    await ht.cmd('draw.text.set',{'textId':tid,'text':'Hi'});direct=await ht.cmd('draw.drawing.get',{'flatten':True})
                    eq=load('text-direct-eq','scripts/c44b-gesture-equivalence.py')
                    check(eq.unstamped(r['drawing'])==eq.unstamped(direct),'D-32 Text exact headless document',None)
'''
if __name__=='__main__':
 case,step=sys.argv[1],int(sys.argv[2]);assert case in ('n1','n2')
 if step==1:sys.exit(prior_runs())
 if step in (2,3):
  a=run_matrix(step);b=run_text(step);sys.exit(a or b)
 if step==4:sys.exit(run_matrix(step))
 if step in (5,6,7):sys.exit(run_text(step))
 if step==8:
  a=run_regression();b=run_text(step);sys.exit(a or b)
 raise ValueError('unimplemented step')
