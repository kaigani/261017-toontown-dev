"""Independent C4.4b round-one live probes. Raw output stays in TEMP."""
import importlib.util, json, os, subprocess, sys, tempfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
EVID = ROOT / 'reviews/evidence/M4-C4.4b-r1-review'
APP = ROOT / 'target/release/toontown.exe'
HEAD = ROOT / 'target/release/toontown-headless.exe'
def load(name, rel):
    s = importlib.util.spec_from_file_location(name, ROOT / rel)
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
def save(name, value):
    EVID.mkdir(parents=True, exist_ok=True)
    txt = json.dumps(value, indent=2, ensure_ascii=True) + '\n'
    assert len(txt.encode()) <= 200000
    (EVID / name).write_text(txt, encoding='utf-8', newline='\n')
def regressions():
    rows=json.loads((EVID/'step2.json').read_text(encoding='utf-8')) if os.environ.get('C44B_REGRESSION_ONLY') and (EVID/'step2.json').exists() else []
    for script,args in [('c44a-r4-live.py',['p1a','2']),('c44a-r4-live.py',['p1b','7']),('c44a-r4-live.py',['p1b','9']),('c44a-r3-live.py',['n2a','2']),('c44a-r2-live.py',['m1a','3']),('c43b-r1-live.py',[str(APP),'case4',str(HEAD)])]:
        if os.environ.get('C44B_REGRESSION_ONLY') and script!=os.environ['C44B_REGRESSION_ONLY']:continue
        raw=Path(tempfile.mkdtemp(prefix='c44b-regression-'))
        path=ROOT/'tests/adversarial'/script
        if script=='c43b-r1-live.py':
            code="import sys;sys.argv="+repr([str(path),*args])+";p=__import__('pathlib').Path(sys.argv[0]);s=p.read_text(encoding='utf-8').replace('OUT = ROOT / \"reviews/evidence/M4-C4.3b-r1\"', 'OUT = __import__(\"pathlib\").Path('+repr("+repr(str(raw))+")+')');exec(compile(s,str(p),'exec'),{'__file__':str(p),'__name__':'__main__'})"
            cmd=[sys.executable,'-c',code]
        else: cmd=[sys.executable,str(path),*args]
        with (raw/'console.txt').open('w',encoding='utf-8') as f:
            result=subprocess.run(cmd,cwd=ROOT,env={**os.environ,'REVIEW_RAW':str(raw)},stdout=f,stderr=subprocess.STDOUT,timeout=530)
        failures=[]
        for p in raw.rglob('transcript.json'):
            failures += [r for r in json.loads(p.read_text(encoding='utf-8')) if not r.get('ok',False)]
        row={'script':script,'args':args,'exit':result.returncode,'failures':failures,'raw':str(raw)}
        rows=[r for r in rows if not (r['script']==script and r['args']==args)]
        rows.append(row); print(json.dumps(row),flush=True)
        changed=subprocess.check_output(['git','diff','--name-only'],cwd=ROOT,text=True).splitlines()
        for p in changed:
            if p.startswith('reviews/evidence/') and not p.startswith('reviews/evidence/M4-C4.4b-r1-review/'):
                subprocess.run(['git','checkout','--',p],cwd=ROOT,check=True)
        save('step2.json',rows)
    for row in rows:
        row['acceptedStop']=row['script']=='c44a-r4-live.py' and row['args']==['p1b','9'] and 'line 322' in (Path(row['raw'])/'console.txt').read_text(encoding='utf-8') and len(row['failures'])==1
        row['transcriptSha256']={str(p.relative_to(row['raw'])):__import__('hashlib').sha256(p.read_bytes()).hexdigest() for p in Path(row['raw']).rglob('transcript.json')}
    save('step2.json',rows)
    return int(any(r['exit'] and not r.get('acceptedStop') for r in rows))
def build():
    raw=Path(tempfile.mkdtemp(prefix='c44b-build-'));rows=[]
    for args,cwd in [(['pnpm.cmd','install','--frozen-lockfile'],ROOT/'desktop'),(['pnpm.cmd','build'],ROOT/'desktop'),(['cargo','build','--release','--locked','-p','tt-desktop','-p','tt-headless'],ROOT)]:
        with (raw/(str(len(rows))+'.txt')).open('w',encoding='utf-8') as f:
            r=subprocess.run(args,cwd=cwd,stdout=f,stderr=subprocess.STDOUT,timeout=530)
        rows.append({'command':args,'exit':r.returncode});save('step1.json',{'results':rows,'raw':str(raw)})
        if r.returncode:return r.returncode
    return 0
async def contour_body(t,step):
    import asyncio, ctypes, math, shutil
    from ctypes import wintypes
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    p=PRIOR; d=p.d; c=t.cmd; check=t.run.check
    t.cdp=d.Cdp(d.CDP_PORT); t.cdp.record()
    eq=load('review_equivalence','scripts/c44b-gesture-equivalence.py')
    async def roots(_): return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='work')])
    async def gesture(view,pts,mods=(),escape=False,matrix_override=None):
        await asyncio.sleep(.35); d.front()
        matrix=matrix_override or (await c('draw.viewMapping.get',{'view':view}))['drawingToView']
        rect=t.cdp.js("document.querySelector('[data-tt-id=\""+view+".canvas\"]').getBoundingClientRect().toJSON()")
        origin=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(origin))
        scale=t.cdp.js('devicePixelRatio')
        xy=[eq.c42.apply(matrix,*q) for q in pts]
        assert all(0<=x<rect['width'] and 0<=y<rect['height'] for x,y in xy), ('gesture outside visible canvas',view,rect,xy)
        previous=json.dumps(json.loads((d.McpTable.node(await d.McpTable.tree(t),view+'.canvas') or {}).get('value') or '{}').get('lastGesture'))
        screen=[(origin.x+(rect['x']+x)*scale,origin.y+(rect['y']+y)*scale) for x,y in xy]
        for vk in mods:d.u32.keybd_event(vk,0,0,0)
        try:
            d.u32.SetCursorPos(round(screen[0][0]),round(screen[0][1]));d.e6._send_button(True)
            for x,y in screen[1:]:
                await asyncio.sleep(.1);d.u32.SetCursorPos(round(x),round(y))
            if escape:d.key(0x1b)
            await asyncio.sleep(.1);d.e6._send_button(False)
        finally:
            for vk in reversed(mods):d.u32.keybd_event(vk,0,2,0)
        await asyncio.sleep(.65)
        for _ in range(100):
            tree=await d.McpTable.tree(t);node=d.McpTable.node(tree,view+'.canvas'); assert node, view
            state=json.loads(node.get('value') or '{}')
            if json.dumps(state.get('lastGesture'))!=previous:return state.get('lastGesture')
            await asyncio.sleep(.05)
        return None
    def unstamped(doc):return eq.unstamped(doc)
    def stroke(doc):
        def walk(x):
            if isinstance(x,dict):
                if isinstance(x.get('bezier'),list):return x
                for v in x.values():
                    found=walk(v)
                    if found:return found
            if isinstance(x,list):
                for v in x:
                    found=walk(v)
                    if found:return found
        return walk(doc)
    def xy(v):return (v['x'],v['y'])
    def curve(b,t):
        return tuple(sum(w*p[k] for w,p in zip(((1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3),map(xy,b))) for k in (0,1))
    async def fixture(view,closed=False,two=False):
        await p.r3.preserve_fixture(t)
        await c('project.create',{'name':'Contour review','onUnsaved':'fail'})
        eid=(await c('element.create',{'type':'drawing','name':'Curve'}))['elementIds'][0]
        await c('element.drawing.create',{'elementId':eid,'name':'1','exposeFrom':1,'exposeTo':24})
        await c('app.context.set',{'elementId':eid,'frame':1})
        target_drawing=(await c('draw.drawing.get'))['drawingId']
        await c('workspace.select',{'name':'Drawing' if view=='drawingView' else 'Default'})
        await c('window.panel.set',{'panelId':view,'visible':True,'focus':True})
        await c('view.active.set',{'viewId':view});await c('view.transform.reset',{'viewId':view})
        points=[{'x':-80,'y':0,'outHandle':{'x':20,'y':30}}, {'x':0,'y':0,'inHandle':{'x':-25,'y':15},'outHandle':{'x':25,'y':35}}, {'x':80,'y':0,'inHandle':{'x':-20,'y':-30}}]
        if two:points=points[:2]
        added=await c('draw.polyline.add',{'points':points,'closed':closed,'snap':False})
        sid=added['strokeIds'][0]
        await c('draw.selection.set',{'by':'ids','ids':[sid]})
        await c('tool.active.set',{'toolId':'contourEditor'});await c('tool.snapToContour.set',{'enabled':False})
        await asyncio.sleep(.6)
        return sid
    async def compare(view,name,action,expected,selected=(),closed=False,two=False,extra=None,label='Contour Editing'):
        sid=await fixture(view,closed,two)
        if selected:await c('draw.pointSelection.set',{'points':[{'strokeId':sid,'anchor':i} for i in selected]})
        if extra:await extra(sid)
        base=t.work/'base.animpkg';await c('project.saveAs',{'path':str(base),'onExists':'overwrite'})
        before=await c('draw.drawing.get');selection_before=await c('draw.selection.get');pos=await t.position();t.clear()
        rec=await action(sid)
        after=await c('draw.drawing.get'); labels=await t.steps_since(pos)
        commands=(rec or {}).get('commands') or []
        if expected is None: commands=[]
        check(any(x.get('commandId')==expected for x in commands) if expected else not commands,name+' command',commands)
        want=[] if not expected or expected in ('draw.pointSelection.set','draw.selection.set') else [label]
        check([x[0] for x in labels]==want,name+' undo labels',labels)
        if not want:check(before==after,name+' selection/cancel leaves document unchanged',None)
        if name.endswith('-symmetric') or name.endswith('-broken'):
            mode='symmetric' if name.endswith('-symmetric') else 'broken'
            check(any(x.get('params',{}).get('handleMode')==mode for x in commands),name+' handle mode',commands)
        size=await c('view.list');vs=size.get('views',[]) if isinstance(size,dict) else size
        copy=t.work/'direct.animpkg';shutil.copyfile(base,copy)
        args=['--mcp-stdio','--project',str(copy),'--policy','autonomous','--allow-path',str(t.work),'--user-dir',str(t.work/('head-'+str(len(list(t.work.glob('head-*'))))))]
        for v in vs:
            if v.get('size',{}).get('width'):args+=['--view-size',v['viewId']+'='+str(v['size']['width'])+'x'+str(v['size']['height'])]
        async with stdio_client(StdioServerParameters(command=str(HEAD),args=args)) as streams:
            async with ClientSession(*streams,list_roots_callback=roots) as hs:
                await hs.initialize(); ht=d.Table(hs,t.run,p.OUT,t.work)
                await ht.cmd('draw.selection.set',{'by':'ids','ids':selection_before.get('selectedIds',[])})
                await ht.cmd('draw.pointSelection.set',{'points':selection_before.get('selectedPoints',[])})
                directpos=await ht.position()
                for cmd in commands:await ht.cmd(cmd['commandId'],cmd.get('params') or {})
                direct=await ht.cmd('draw.drawing.get')
                check(unstamped(after)==unstamped(direct),name+' D-32 exact direct content',{'equal':unstamped(after)==unstamped(direct)})
                directlabels=await ht.steps_since(directpos);check([x[0] for x in directlabels]==want,name+' direct undo labels',directlabels)
        selection=await c('draw.selection.get')
        canvas=json.loads((d.McpTable.node(await d.McpTable.tree(t),view+'.canvas') or {}).get('value') or '{}')
        if step==4:
            desired=([0,1] if name.endswith('add-click') or name.endswith('marquee') else [] if name.endswith('empty') else [0])
            check(sorted(x['anchor'] for x in selection.get('selectedPoints',[]))==desired,name+' selection reported',selection)
            check((canvas.get('contours') or {}).get('selected',0)==len(desired),name+' rendered contour selection equals reported selection',canvas.get('contours'))
        t.run.save(name+'.json',{'record':rec,'commands':commands,'before':before,'after':after,'labels':labels,'selection':selection,'display':canvas.get('contours')})
        if want and labels:
            for _ in labels:await c('edit.undo')
            restored=await c('draw.drawing.get')
            check(unstamped(restored)==unstamped(before),name+' undo exact content',None)
        return before,after,rec
    for view in ('drawingView','cameraView'):
        if step==3 and view=='cameraView':continue
        def act(pts,mods=(),escape=False):return lambda sid:gesture(view,pts,mods,escape)
        async def delete(sid):
            d.front();t.click(view+'.canvas');await c('draw.pointSelection.set',{'points':[{'strokeId':sid,'anchor':i} for i in deletion]});t.clear();d.key(0x2e);await asyncio.sleep(.7)
            return {'commands':[{'commandId':x['command'],'params':x.get('params',{})} for x in t.recorded() if x['command']=='draw.point.delete']}
        if step==3:
            for tool,cmd,label,pts in [('brush','draw.stroke.add','Brush Tool',[(-80,60),(-40,80),(0,60)]),('select','draw.transform.apply','Move Object',[(-40,17),(-20,37)]),('rectangle','draw.rectangle.add','Rectangle Tool',[(-100,-60),(100,60)])]:
                async def tool_extra(sid):await c('tool.active.set',{'toolId':tool})
                await compare(view,view+'-old-'+tool,act(pts),cmd,extra=tool_extra,label=label)
            await c('window.panel.set',{'panelId':'properties','visible':True})
            await asyncio.sleep(.5)
            t.click('properties.page.toolOptions')
            for tool in (await c('tool.list'))['tools']:
                tid=tool['toolId'];t.clear();t.click('drawingTools.'+tid);await asyncio.sleep(.2)
                check((await c('tool.active.get'))['toolId']==tid,'toolbar '+tid,t.recorded())
                page=t.cdp.js("document.querySelector('[data-tt-id=\"toolOptions\"]')?.outerHTML");check(bool(page),'options page available '+tid,page)
                required='toolOptions.rectRule' if tid=='select' else 'toolOptions.selectedLine' if tid=='contourEditor' else 'toolOptions.pen' if tid in ('brush','eraser') else 'toolOptions.thicknessSource' if tid in ('pencil','rectangle','ellipse','line','polyline') else 'toolOptions.gapClosing' if tid in ('paint','unpaint','paintUnpainted') else 'toolOptions.snapToContour'
                check(required in (page or ''),'02 tool options page matches '+tid,required)
                t.run.save(view+'-options-'+tid+'.json',page)
                await c('keymap.set',{'commandId':'tool.active.set','params':{'toolId':tid},'keys':'F6','context':'global','onConflict':'override'})
                t.click('drawingTools.'+('select' if tid!='select' else 'contourEditor'));t.click(view+'.canvas');t.clear();d.key(0x75);await asyncio.sleep(.2)
                check((await c('tool.active.get'))['toolId']==tid,'shortcut '+tid,await c('tool.options.get',{'toolId':tid}))
                check(required in (t.cdp.js("document.querySelector('[data-tt-id=\"toolOptions\"]')?.outerHTML") or ''),'02 shortcut options match '+tid,required)
        elif step==4:
            for name,pts,mods,selected in [('click',[(-80,0)]*2,(),()),('add-click',[(0,0)]*2,(0x10,),(0,)),('marquee',[(-100,-50),(20,50)],(),()),('empty',[(120,100)]*2,(),(0,1))]:
                await compare(view,view+'-'+name,act(pts,mods),'draw.selection.set' if name=='empty' else 'draw.pointSelection.set',selected)
        elif step==5:
            for name,pts,mods,selected,cmd in [('anchor',[(-80,0),(-65,15)],(),(),'draw.point.move'),('two',[(-80,0),(-65,15)],(),(0,1),'draw.point.move'),('segment',[(-40,16.875),(-40,36.875)],(),(),'draw.segment.move'),('symmetric',[(25,35),(35,45)],(),(1,),'draw.point.set'),('broken',[(25,35),(35,45)],(0x12,),(1,),'draw.point.set')]:
                await compare(view,view+'-'+name,act(pts,mods),cmd,selected)
            await compare(view,view+'-escape',act([(-80,0),(-60,20)],escape=True),None)
            for enabled in (False,True):
                async def snap_extra(sid):
                    doc=await c('draw.drawing.get');layer=next(n['id'] for n in doc['nodes'] if n.get('type')=='layer')
                    await c('draw.line.add',{'from':{'x':-40,'y':50},'to':{'x':40,'y':50},'layer':layer})
                    await c('draw.selection.set',{'by':'ids','ids':[sid]})
                    await c('tool.snapToContour.set',{'enabled':enabled})
                b,a,r=await compare(view,view+'-snap-other-'+str(enabled),act([(-80,0),(-20,45)]),'draw.point.move',extra=snap_extra)
                point=xy(stroke(a)['bezier'][0]);check(abs(point[1]-(50 if enabled else 45))<1.1,view+' other stroke snapping '+str(enabled),point)
                async def own_extra(sid):await c('tool.snapToContour.set',{'enabled':enabled})
                b,a,r=await compare(view,view+'-snap-own-'+str(enabled),act([(-80,0),(-5,0)]),'draw.point.move',extra=own_extra)
                point=xy(stroke(a)['bezier'][0]);check(abs(point[0]+5)<1.1,view+' D-76 own stroke excluded '+str(enabled),point)
        elif step==6:
            for name,point in [('reset',(0,0)),('end',(-80,0))]:
                before,after,rec=await compare(view,view+'-'+name,act([point]*2,(0x12,)),'draw.point.set')
                if name=='reset':
                    b=stroke(before)['bezier'];a=stroke(after)['bezier'];anchor=xy(b[3])
                    vectors=lambda arr:[tuple(arr[i][k]-anchor[j] for j,k in enumerate(('x','y'))) for i in (2,4)]
                    bv,av=vectors(b),vectors(a)
                    check(abs(av[0][0]*av[1][1]-av[0][1]*av[1][0])<1e-6 and all(abs(math.hypot(*x)-math.hypot(*y))<1e-6 for x,y in zip(bv,av)),view+' D-75 reset collinear lengths',{'before':bv,'after':av})
            async def zero(sid):await c('draw.point.set',{'strokeId':sid,'anchor':1,'inHandle':{'x':0,'y':0},'outHandle':{'x':0,'y':0},'handleMode':'broken'})
            await compare(view,view+'-zero-reset',act([(0,0)]*2,(0x12,)),'draw.point.set',extra=zero)
            before,after,rec=await compare(view,view+'-insert',act([(-41.875,16.875)]*2,(0x11,)),'draw.point.add')
            cmds=(rec or {}).get('commands') or []
            if cmds and cmds[0]['commandId']=='draw.point.add':
                split=cmds[0]['params']['t'];b=stroke(before)['bezier'];a=stroke(after)['bezier'];errors=[]
                for i in range(101):
                    u=i/100;old=curve(b[:4],u);new=curve(a[:4],u/split) if u<=split else curve(a[3:7],(u-split)/(1-split));errors.append(math.dist(old,new))
                check(max(errors)<1e-6,view+' insert shape unchanged 101 samples',max(errors))
        elif step==7:
            for name,deletion,closed,two in [('one',(1,),False,False),('several',(0,1),False,False),('all',(0,1,2),False,False),('closed',(1,),True,False),('two',(0,),False,True)]:
                await compare(view,view+'-delete-'+name,delete,'draw.point.delete',deletion,closed,two)
            for zoom in (.5,1,2):
                for kind,radius,center,direction,expected in [('anchor',6,(-80,0),(-1,0),'draw.point.move'),('handle',5,(25,35),(0,1),'draw.point.set'),('segment',4,(-41.875,16.875),(0,1),'draw.segment.move')]:
                    for inside in (True,False):
                        async def setzoom(sid):await c('view.zoom.set',{'viewId':view,'zoom':zoom})
                        async def radius_action(sid):
                            matrix=(await c('draw.viewMapping.get',{'view':view}))['drawingToView'];scale=math.hypot(matrix[0],matrix[1])
                            offset=(radius+(-.8 if inside else .8))/scale
                            start=tuple(center[k]+direction[k]*offset for k in (0,1))
                            return await gesture(view,[start,(start[0]+12/scale,start[1]+12/scale)])
                        await compare(view,view+'-radius-'+kind+'-'+str(zoom)+'-'+str(inside),radius_action,expected if inside else 'draw.pointSelection.set',(1,) if kind=='handle' else (),extra=setzoom)
        elif step==8:
            sid=await fixture(view);await c('window.panel.set',{'panelId':'properties','visible':True});await asyncio.sleep(.6)
            async def options(sid):
                await c('window.panel.set',{'panelId':'properties','visible':True});await asyncio.sleep(.5)
                t.click('properties.page.toolOptions');await asyncio.sleep(.3)
            async def thickness(sid):
                t.click('toolOptions.selectedLine.thickness');d.key(0x41,0x11);t.clear();d.type_text('7');d.key(0x0d);await asyncio.sleep(.6)
                return {'commands':[{'commandId':x['command'],'params':x['params']} for x in t.recorded() if x['command']=='draw.stroke.set']}
            await compare(view,view+'-thickness-direct',thickness,'draw.stroke.set',extra=options,label='Edit Stroke')
            async def invisible_action(sid):
                t.clear();t.click('toolOptions.selectedLine.invisible');await asyncio.sleep(.5)
                return {'commands':[{'commandId':x['command'],'params':x.get('params',{})} for x in t.recorded() if x['command']=='draw.stroke.set']}
            b,a,r=await compare(view,view+'-invisible-direct',invisible_action,'draw.stroke.set',extra=options,label='Edit Stroke')
            check(stroke(a)['kind']=='invisible',view+' Invisible effect',stroke(a)['kind'])
            sid=await fixture(view);await options(sid)
            for val in ('7','abc','-1','101'):
                ident='toolOptions.selectedLine.thickness';t.click(ident)
                d.key(0x41,0x11);t.clear();d.type_text(val);d.key(0x0d);await asyncio.sleep(.6)
                t.run.save(view+'-thickness-'+val+'.json',{'sent':t.recorded(),'drawing':await c('draw.drawing.get'),'field':t.cdp.js('document.activeElement.value')})
                writes=[x for x in t.recorded() if t.kinds.get(x['command'])!='query']
                check(any(x['command']=='draw.stroke.set' for x in writes) if val=='7' else not writes,view+' thickness '+val,writes)
                check(t.cdp.js("document.querySelector('[data-tt-id=\"toolOptions.selectedLine.thickness\"]').value")=='7',view+' numeric retained/restored '+val,None)
            t.clear();t.click('toolOptions.selectedLine.invisible');await asyncio.sleep(.6)
            check(any(x['command']=='draw.stroke.set' and x['params'].get('kind')=='invisible' for x in t.recorded()),view+' invisible',t.recorded())
            await gesture(view,[(120,100)]*2);await asyncio.sleep(.5)
            t.run.save(view+'-no-single-line.json',t.cdp.js("document.querySelector('[data-tt-id=\"toolOptions.selectedLine\"]')?.outerHTML"))
            check(not t.cdp.js("!!document.querySelector('[data-tt-id=\"toolOptions.selectedLine.thickness\"]:enabled')"),view+' no single line control unavailable',None)
            disabled=t.cdp.js("['thickness','invisible'].map(s=>{const e=document.querySelector('[data-tt-id=\"toolOptions.selectedLine.'+s+'\"]');return {id:s,present:!!e,disabled:e?.disabled,reason:e?.title}})")
            check(all(x['present'] and x.get('disabled') and x.get('reason') for x in disabled),view+' no-single-line controls disabled rather than hidden (M4 plan UI conventions)',disabled)
            d.front();rr=t.rect(view+'.canvas');origin=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(origin))
            d.right_click_screen(origin.x+rr['x']*rr['dpr'],origin.y+rr['y']*rr['dpr']);await asyncio.sleep(.4)
            tree=await d.McpTable.tree(t)
            def flatten(n):return [n]+[x for ch in n.get('children',[]) for x in flatten(ch)]
            items=[x for x in flatten(tree) if '.drawingMenu.' in x.get('id','')]
            check({'edit.cut','edit.copy','edit.paste','edit.selectAll','edit.deselectAll'}.issubset({x.get('command',{}).get('commandId') for x in items}),view+' contour context drawing actions',items)
            t.run.save(view+'-context.json',items);d.key(0x1b)
            t.clear();pos=await t.position();await t.do(('context',view+'.canvas',view+'.drawingMenu.selectAll'));await asyncio.sleep(.4)
            check(bool((await c('draw.selection.get')).get('selectedIds')) and await t.position()==pos,view+' context Select All selects without undo',t.recorded())
            for mode in (('locked','hidden','layer-locked') if view=='cameraView' else ('locked','hidden')):
                sid=await fixture(view)
                visible=await c('draw.drawing.get');drawing_id=visible['drawingId'];cur=await c('draw.current.get')
                visible_matrix=(await c('draw.viewMapping.get',{'view':view}))['drawingToView']
                if mode=='locked':await c('draw.lock.set',{'locked':True})
                elif mode=='layer-locked':await c('element.lock.set',{'elementIds':[cur['elementId']],'locked':True})
                else:
                    await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':False,'outlineOnSelection':False})
                    await c('drawAid.currentOnTop.set',{'enabled':False})
                    await c('element.visibility.set',{'elementIds':[cur['elementId']],'scope':'xsheet' if view=='drawingView' else 'scene','visible':False})
                await asyncio.sleep(.5)
                canvas=d.McpTable.node(await d.McpTable.tree(t),view+'.canvas')
                t.run.save(view+'-'+mode+'-canvas.json',canvas)
                t.run.shot(view+'-'+mode,d.e2e.APP_PID['pid'])
                saved=t.work/(view+'-'+mode+'.animpkg');await c('project.saveAs',{'path':str(saved),'onExists':'overwrite'})
                copied=t.work/(view+'-'+mode+'-direct.animpkg');shutil.copyfile(saved,copied)
                before=await c('draw.drawing.get',{'drawingId':drawing_id});pos=await t.position();rec=await gesture(view,[(-80,0),(-60,20)],matrix_override=visible_matrix)
                after=await c('draw.drawing.get',{'drawingId':drawing_id})
                t.run.save(view+'-'+mode+'.json',{'record':rec,'commands':(rec or {}).get('commands',[]),'before':before,'after':after,'labels':await t.steps_since(pos)})
                check(before==after and await t.position()==pos,view+' '+mode+' no edit',rec)
                args=['--mcp-stdio','--project',str(copied),'--policy','autonomous','--allow-path',str(t.work),'--user-dir',str(t.work/(view+'-'+mode+'-head'))]
                async with stdio_client(StdioServerParameters(command=str(HEAD),args=args)) as streams:
                    async with ClientSession(*streams,list_roots_callback=roots) as hs:
                        await hs.initialize();ht=d.Table(hs,t.run,p.OUT,t.work)
                        for cmd in (rec or {}).get('commands',[]):await d.e2e.run_cmd(hs,cmd['commandId'],cmd.get('params',{}))
                        direct=await ht.cmd('draw.drawing.get',{'drawingId':drawing_id})
                        check(unstamped(after)==unstamped(direct),view+' '+mode+' D-32 exact direct content',None)

def live(step):
    global PRIOR
    PRIOR=load('c44b_prior','tests/adversarial/c44a-r1-live.py')
    PRIOR.EVID=EVID; PRIOR.live_body=contour_body
    result=PRIOR.live(step)
    compact(step,PRIOR.OUT)
    return result or int(bool(json.loads((EVID/('step'+str(step)+'.json')).read_text(encoding='utf-8'))['failures']))
def compact(step,out):
    rows=[r for r in json.loads((out/'transcript.json').read_text(encoding='utf-8')) if 'check' in r]
    import hashlib
    digest=lambda obj:hashlib.sha256(json.dumps(obj,sort_keys=True).encode()).hexdigest()
    actions=[]
    def bezier(x):
        if isinstance(x,dict):
            if 'bezier' in x:return x['bezier']
            for v in x.values():
                found=bezier(v)
                if found:return found
        if isinstance(x,list):
            for v in x:
                found=bezier(v)
                if found:return found
    def geometry(x):
        result=[]
        if isinstance(x,dict):
            if isinstance(x.get('bezier'),list):return [{'id':x.get('id'),'kind':x.get('kind'),'closed':x.get('closed'),'anchors':(len(x['bezier'])+2)//3}]
            for v in x.values():result.extend(geometry(v))
        elif isinstance(x,list):
            for v in x:result.extend(geometry(v))
        return result
    for path in out.glob('*.json'):
        data=json.loads(path.read_text(encoding='utf-8'))
        if isinstance(data,dict) and 'commands' in data and 'before' in data:
            actions.append({'name':path.stem,'commands':data['commands'],'labels':data.get('labels'),'selection':data.get('selection'),'display':data.get('display'),'geometryBefore':geometry(data['before']),'geometryAfter':geometry(data['after']),'beforeSha256':digest(data['before']),'afterSha256':digest(data['after'])})
            if path.stem.endswith(('-symmetric','-broken')):
                b,a=bezier(data['before']),bezier(data['after'])
                ok=all(abs(a[2][k]+a[4][k]-2*a[3][k])<1e-6 for k in ('x','y')) if path.stem.endswith('-symmetric') else a[2]==b[2]
                rows.append({'check':path.stem+' opposite handle geometric invariant (D-75 item 4)','ok':ok,'detail':{'before':b[2:5],'after':a[2:5]}})
            if path.stem.endswith('-insert'):
                selected=data.get('selection',{}).get('selectedPoints',[])
                rows.append({'check':path.stem+' requested new-anchor selection (observation; spec and design note silent)','ok':len(selected)==1 and selected[0]['anchor']==1,'detail':selected})
    measurements=[{'check':r['check'],'detail':r.get('detail')} for r in rows if any(s in r['check'] for s in ('101 samples','collinear lengths','geometric invariant','other stroke snapping','own stroke excluded'))]
    canvases=[]
    for path in out.glob('*-canvas.json'):
        node=json.loads(path.read_text(encoding='utf-8'));v=json.loads((node or {}).get('value') or '{}')
        canvases.append({'name':path.stem,'drawings':v.get('drawings'),'contours':v.get('contours'),'overlay':v.get('overlay')})
    captures=[{'name':path.name,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()} for path in out.glob('*.png')]
    supplemental=[]
    if str(step).startswith('l2-'):
        for path in out.glob('*.json'):
            if any(token in path.stem for token in ('direct-answer','held-','options','empty-','drawTopLayer','placeBehind','snapToContour','context-action','active-after-switch')):
                value=json.loads(path.read_text(encoding='utf-8'))
                if path.stem.endswith('direct-answer'):
                    value={k:value.get(k) for k in ('status','result','undo','error','errors')}
                supplemental.append({'name':path.stem,'value':value})
        for path in out.glob('*-context.json'):
            tree=json.loads(path.read_text(encoding='utf-8'))
            def menus(n):
                if not isinstance(n,dict):return []
                return ([{k:n.get(k) for k in ('id','label','command','disabled','reason')}] if '.drawingMenu.' in n.get('id','') else [])+[v for ch in n.get('children',[]) for v in menus(ch)]
            supplemental.append({'name':path.stem,'value':menus(tree)})
        save('l2-'+str(step)+'-supplemental.json',supplemental)
    save('step'+str(step)+'.json',{'checks':len(rows),'failures':[r for r in rows if not r.get('ok')],'results':[{'check':r['check'],'ok':r['ok']} for r in rows],'actions':actions,'measurements':measurements,'canvases':canvases,'captures':captures,'transcriptSha256':hashlib.sha256((out/'transcript.json').read_bytes()).hexdigest(),'raw':str(out)})
async def envelope_body(t,step):
    import asyncio, ctypes, math, shutil
    from ctypes import wintypes
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    p=PRIOR; d=p.d; c=t.cmd; check=t.run.check
    t.cdp=d.Cdp(d.CDP_PORT); t.cdp.record()
    hold_action=None
    eq=load('review_equivalence','scripts/c44b-gesture-equivalence.py')
    async def roots(_): return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='work')])
    async def gesture(view,pts,mods=(),escape=False,matrix_override=None):
        await asyncio.sleep(.35); d.front()
        matrix=matrix_override or (await c('draw.viewMapping.get',{'view':view}))['drawingToView']
        rect=t.cdp.js("document.querySelector('[data-tt-id=\""+view+".canvas\"]').getBoundingClientRect().toJSON()")
        origin=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(origin))
        scale=t.cdp.js('devicePixelRatio')
        xy=[eq.c42.apply(matrix,*q) for q in pts]
        assert all(0<=x<rect['width'] and 0<=y<rect['height'] for x,y in xy), ('gesture outside visible canvas',view,rect,xy)
        previous=json.dumps(json.loads((d.McpTable.node(await d.McpTable.tree(t),view+'.canvas') or {}).get('value') or '{}').get('lastGesture'))
        screen=[(origin.x+(rect['x']+x)*scale,origin.y+(rect['y']+y)*scale) for x,y in xy]
        for vk in mods:d.u32.keybd_event(vk,0,0,0)
        try:
            d.u32.SetCursorPos(round(screen[0][0]),round(screen[0][1]));d.e6._send_button(True)
            for x,y in screen[1:]:
                await asyncio.sleep(.1);d.u32.SetCursorPos(round(x),round(y))
            if hold_action:await hold_action()
            if escape:d.key(0x1b)
            await asyncio.sleep(.3)
            if step==4:
                from PIL import ImageGrab
                region=(round(origin.x+rect['x']*scale),round(origin.y+rect['y']*scale),round(origin.x+(rect['x']+rect['width'])*scale),round(origin.y+(rect['y']+rect['height'])*scale))
                ImageGrab.grab(bbox=region).save(p.OUT/(view+'-held.png'))
            d.e6._send_button(False)
        finally:
            for vk in reversed(mods):d.u32.keybd_event(vk,0,2,0)
        await asyncio.sleep(.65)
        if step==4:
            ImageGrab.grab(bbox=region).save(p.OUT/(view+'-committed.png'))
        for _ in range(100):
            tree=await d.McpTable.tree(t);node=d.McpTable.node(tree,view+'.canvas'); assert node, view
            state=json.loads(node.get('value') or '{}')
            if json.dumps(state.get('lastGesture'))!=previous:return state.get('lastGesture')
            await asyncio.sleep(.05)
        return None
    async def fixture(view,tool='perspective'):
        await p.r3.preserve_fixture(t)
        await c('project.create',{'name':'Envelope review','onUnsaved':'fail'})
        await c('tool.placeBehind.set',{'enabled':False});await c('tool.drawTopLayer.set',{'enabled':False});await c('tool.snapToContour.set',{'enabled':False})
        eid=(await c('element.create',{'type':'drawing','name':'Shapes'}))['elementIds'][0]
        found=await c('doc.object.find',{'type':'swatch','name':'Colour 2'})
        items=found.get('items') or found.get('matches') or []
        await c('colour.current.set',{'colourId':items[0].get('id') or items[0].get('key')})
        ids=[]
        for i,(x,y,w,h) in enumerate([(-80,-60,160,120),(-30,-20,90,50),(15,10,40,80)]):
            did=(await c('element.drawing.create',{'elementId':eid,'name':str(i+1),'exposeFrom':i*8+1,'exposeTo':i*8+8}))
            await c('app.context.set',{'elementId':eid,'frame':i*8+1})
            added=await c('draw.rectangle.add',{'from':{'x':x,'y':y},'to':{'x':x+w,'y':y+h},'thickness':1})
            if i==0:first=added['strokeIds']
            ids.append((await c('draw.drawing.get'))['drawingId'])
        await c('app.context.set',{'elementId':eid,'frame':1})
        target_drawing=(await c('draw.drawing.get'))['drawingId']
        await c('workspace.select',{'name':'Drawing' if view=='drawingView' else 'Default'})
        await c('window.panel.set',{'panelId':view,'visible':True,'focus':True})
        await c('view.active.set',{'viewId':view});await c('view.transform.reset',{'viewId':view})
        if step==2 and view=='cameraView':await c('view.zoom.set',{'viewId':view,'zoom':0.625})
        if step==4:
            inset=await c('draw.polyline.add',{'points':[{'x':-50,'y':-30},{'x':50,'y':-30},{'x':50,'y':30},{'x':-50,'y':30}],'closed':True,'thickness':2,'snap':False})
            first+=inset['strokeIds']
        await c('draw.selection.set',{'by':'ids','ids':first});await c('tool.active.set',{'toolId':tool})
        await asyncio.sleep(.7)
        return eid,ids
    async def drawings(ids,cmd=c):return [eq.unstamped(await cmd('draw.drawing.get',{'drawingId':i})) for i in ids]
    async def compare(view,name,pts,mods=(),escape=False,tool='perspective',setup=None,expect=True,command_override=None,label_override=None):
        eid,ids=await fixture(view,tool)
        if setup:await setup(eid,ids)
        selection=await c('draw.selection.get')
        base=t.work/'base.animpkg';await c('project.saveAs',{'path':str(base),'onExists':'overwrite'})
        before=await drawings(ids);pos=await t.position();t.clear()
        rec=await gesture(view,pts,mods,escape)
        after=await drawings(ids);labels=await t.steps_since(pos);commands=(rec or {}).get('commands') or []
        expected='draw.envelope.apply' if tool=='perspective' else 'draw.repositionAll.apply'
        label='Perspective Tool' if tool=='perspective' else 'Reposition All Drawings'
        expected=command_override or expected;label=label_override or label
        check(len(commands)==1 and commands[0]['commandId']==expected if expect else ((rec or {}).get('status')!='ok' or not commands),name+' command',commands)
        check([x[0] for x in labels]==([label] if expect else []),name+' undo labels',labels)
        if step==2 and commands and commands[0]['commandId']=='draw.envelope.apply':
            corners=commands[0]['params']['corners']
            if name.endswith('-coincident'):
                check(any(math.dist((u['x'],u['y']),(v['x'],v['y']))<1e-8 for i,u in enumerate(corners) for v in corners[i+1:]),name+' target truly coincident',corners)
            if name.endswith('-all-collinear'):
                check(max(v['y'] for v in corners)-min(v['y'] for v in corners)<1e-8,name+' target truly collinear',corners)

        vs=await c('view.list');vs=vs.get('views',[]) if isinstance(vs,dict) else vs
        copy=t.work/'direct.animpkg';shutil.copyfile(base,copy)
        args=['--mcp-stdio','--project',str(copy),'--policy','autonomous','--allow-path',str(t.work),'--user-dir',str(t.work/('head-'+name))]
        for v in vs:
            if v.get('size',{}).get('width'):args+=['--view-size',v['viewId']+'='+str(v['size']['width'])+'x'+str(v['size']['height'])]
        async with stdio_client(StdioServerParameters(command=str(HEAD),args=args)) as streams:
            async with ClientSession(*streams,list_roots_callback=roots) as hs:
                await hs.initialize();ht=d.Table(hs,t.run,p.OUT,t.work)
                await ht.cmd('draw.selection.set',{'by':'ids','ids':selection.get('selectedIds',[])})
                for cmd in commands:
                    result=await d.e2e.run_cmd(hs,cmd['commandId'],cmd.get('params') or {})
                    t.run.save(name+'-direct-answer.json',result)
                direct=await drawings(ids,ht.cmd)
                check(after==direct,name+' D-32 exact all drawing content',None)
        if tool=='repositionAll' and commands and commands[0]['commandId']=='draw.repositionAll.apply' and expect:
            import numpy as np
            q=commands[0]['params'];box=before[0]['bounds']
            x,y,w,h=[box[k] for k in ('x','y','width','height')]
            def mapped(pt):
                px,py=pt['x'],pt['y']
                if 'translate' in q:return px+q['translate']['x'],py+q['translate']['y']
                if 'corners' in q:
                    src=[(x,y),(x+w,y),(x+w,y+h),(x,y+h)];dst=[(v['x'],v['y']) for v in q['corners']]
                    aa=[];bb=[]
                    for (u,v),(xx,yy) in zip(src,dst):
                        aa.extend([[u,v,1,0,0,0,-xx*u,-xx*v],[0,0,0,u,v,1,-yy*u,-yy*v]]);bb.extend([xx,yy])
                    hh=np.linalg.solve(aa,bb);den=hh[6]*px+hh[7]*py+1
                    return (hh[0]*px+hh[1]*py+hh[2])/den,(hh[3]*px+hh[4]*py+hh[5])/den
                pivot=q.get('pivot',{'x':0,'y':0});u,v=px-pivot['x'],py-pivot['y']
                if 'scale' in q:u*=q['scale']['x'];v*=q['scale']['y']
                if 'rotate' in q:
                    a=math.radians(q['rotate']);u,v=u*math.cos(a)-v*math.sin(a),u*math.sin(a)+v*math.cos(a)
                return u+pivot['x'],v+pivot['y']
            def points(z):
                if isinstance(z,dict):
                    if 'bezier' in z:return z['bezier']
                    return [p for v in z.values() for p in points(v)]
                if isinstance(z,list):return [p for v in z for p in points(v)]
                return []
            errors=[]
            for b,a in zip(before,after):
                if b.get('locked'):continue
                errors.extend(math.dist(mapped(p),(r['x'],r['y'])) for p,r in zip(points(b),points(a)))
            check(bool(errors) and max(errors)<1e-8,name+' D-75 one independent current-bounds map all drawings',max(errors) if errors else None)

        if step==3 and commands and commands[0]['commandId']=='draw.envelope.apply':
            box=before[0]['bounds'];x,y,w,h=[box[k] for k in ('x','y','width','height')]
            cs=commands[0]['params']['corners'];o,r,u=cs[0],cs[1],cs[3]
            aa,bb=(r['x']-o['x'])/w,(r['y']-o['y'])/w
            cc,dd=(u['x']-o['x'])/h,(u['y']-o['y'])/h
            ee,ff=o['x']-aa*x-cc*y,o['y']-bb*x-dd*y
            results=[]
            for kind in ('text','bitmap'):
                old=next((n for n in before[0]['nodes'] if n.get('type')==kind),None)
                if old is None:continue
                new=next(n for n in after[0]['nodes'] if n.get('type')==kind)
                a,b,c0,d0,e,f=old['transform']
                wanted=[aa*a+cc*b,bb*a+dd*b,aa*c0+cc*d0,bb*c0+dd*d0,aa*e+cc*f+ee,bb*e+dd*f+ff]
                err=max(abs(u-v) for u,v in zip(wanted,new['transform']))
                check(err<1e-8,name+' '+kind+' three-corner affine fit',{'maxError':err,'before':old['transform'],'after':new['transform']})
                results.append({'kind':kind,'maxError':err,'before':old['transform'],'after':new['transform']})
            t.run.save(name+'-affine-fit-options.json',results)
        check(all(__import__('math').isfinite(x) for x in numbers(after)),name+' finite saved content',None)
        t.run.save(name+'.json',{'commands':commands,'record':rec,'before':before,'after':after,'labels':labels})
        for _ in labels:await c('edit.undo')
        check(await drawings(ids)==before,name+' undo exact all drawings',None)
        return before,after,rec
    def numbers(v):
        if isinstance(v,dict):return [n for x in v.values() for n in numbers(x)]
        if isinstance(v,list):return [n for x in v for n in numbers(x)]
        return [v] if isinstance(v,(float,int)) else []
    for view in ('drawingView','cameraView'):
        if step==7 and os.environ.get('C44B_FRAME_ONLY'):
            for tool in ('perspective','repositionAll'):
                eid,ids=await fixture(view,tool)
                await c('keymap.set',{'commandId':'app.frame.set','params':{'frame':9},'keys':'F7','context':'global','onConflict':'override'})
                base=t.work/'frame-base.animpkg';await c('project.saveAs',{'path':str(base),'onExists':'overwrite'})
                before=await drawings(ids);pos=await t.position()
                async def switch_frame():d.key(0x76);await asyncio.sleep(.4)
                hold_action=switch_frame
                rec=await gesture(view,[(-80,-60),(-50,-35)])
                hold_action=None
                after=await drawings(ids);labels=await t.steps_since(pos)
                if tool=='perspective':check(after==before,view+' '+tool+' user frame switch leaves drawings unchanged',rec)
                args=['--mcp-stdio','--project',str(base),'--policy','autonomous','--allow-path',str(t.work),'--user-dir',str(t.work/(view+'-'+tool+'-frame-head'))]
                vs=await c('view.list');vs=vs.get('views',[]) if isinstance(vs,dict) else vs
                for v in vs:
                    if v.get('size',{}).get('width'):args+=['--view-size',v['viewId']+'='+str(v['size']['width'])+'x'+str(v['size']['height'])]
                async with stdio_client(StdioServerParameters(command=str(HEAD),args=args)) as streams:
                    async with ClientSession(*streams,list_roots_callback=roots) as hs:
                        await hs.initialize();ht=d.Table(hs,t.run,p.OUT,t.work)
                        await ht.cmd('app.context.set',{'elementId':eid,'frame':9})
                        for command in (rec or {}).get('commands',[]):await d.e2e.run_cmd(hs,command['commandId'],command.get('params',{}))
                        check(await drawings(ids,ht.cmd)==after,view+' '+tool+' user frame switch exact direct content at new frame',None)

                t.run.save(view+'-'+tool+'-user-frame.json',{'commands':(rec or {}).get('commands',[]),'record':rec,'before':before,'after':after,'labels':labels,'current':await c('draw.current.get')})
                for _ in labels:await c('edit.undo')
                check(await drawings(ids)==before,view+' '+tool+' user frame switch exact undo',None)
            continue
        if step==1:
            for name,pts,mods in [('corner',[(-80,-60),(-50,-35)],()),('side',[(0,-60),(0,-35)],()),('alternate-side',[(0,-60),(20,-40)],(0x12,))]:
                await compare(view,view+'-'+name,pts,mods)
            await compare(view,view+'-escape',[(-80,-60),(-50,-35)],escape=True,expect=False)
        elif step==2:
            for name,target in [('crossed',(100,-60)),('concave',(20,20)),('coincident',(80.5,-60.5))]:
                await compare(view,view+'-'+name,[(-80.5,-60.5),target])
            await compare(view,view+'-all-collinear',[(0,60.5),(0,-60.5)])
        elif step==3:
            async def content(eid,ids):
                await c('draw.stroke.add',{'points':[{'x':-60,'y':-30},{'x':0,'y':35},{'x':60,'y':-10}]})
                await c('draw.polyline.add',{'points':[{'x':-50,'y':-20},{'x':0,'y':20},{'x':50,'y':-15}],'thickness':7,'snap':False})
                await c('draw.text.add',{'position':{'x':-20,'y':-10},'text':'Test'})
                from PIL import Image
                png=t.work/'fixture.png';Image.new('RGBA',(8,8),(30,60,120,255)).save(png)
                image=(await c('element.create',{'type':'image','name':'Bitmap source'}))['elementIds'][0]
                imported=await d.e2e.run_cmd(t.s,'import.file.run',{'paths':[str(png)],'elementId':image,'frame':1,'wait':True})
                t.run.save(view+'-bitmap-placement-options.json',imported)
                if imported.get('status')=='ok':
                    bank=await c('element.drawing.list',{'elementId':image})
                    bitmap=await c('draw.drawing.get',{'drawingId':bank['drawings'][0]['id']})
                    node=next(n for n in bitmap['nodes'] if n['type']=='bitmap');node['transform']=[2,0,0,-2,-8,8]
                    await c('draw.content.add',{'drawingId':ids[0],'content':{'nodes':[node]}})
                await c('app.context.set',{'elementId':eid,'frame':1})

                await c('draw.selection.set',{'by':'rect','rect':{'x':-90,'y':-70,'width':180,'height':140},'rectRule':'inside','mode':'replace'})
            await compare(view,view+'-mixed-content',[(-80,-60),(-45,-30)],setup=content)
        elif step==4:
            await compare(view,view+'-keystone',[(-80,-60),(-45,-30)])
            for kind in ('held','committed'):shutil.copyfile(p.OUT/(view+'-'+kind+'.png'),p.OUT/(view+'-keystone-'+kind+'.png'))
            import numpy as np
            from PIL import Image
            from scipy.ndimage import distance_transform_edt, binary_dilation
            ia=np.asarray(Image.open(p.OUT/(view+'-held.png')).convert('RGB'))
            ib=np.asarray(Image.open(p.OUT/(view+'-committed.png')).convert('RGB'))
            def ink(im):return (im.max(2)<120)&(im.max(2)-im.min(2)<25)
            # Exclude selection and handle accent pixels, plus a 10-device-pixel guard.
            accent=lambda im:(im.max(2)-im.min(2)>35)
            mask=binary_dilation(accent(ia)|accent(ib),iterations=10)
            mask[:5]=True;mask[-5:]=True;mask[:,:5]=True;mask[:,-5:]=True
            aa=ink(ia)&~mask;bb=ink(ib)&~mask
            wide=binary_dilation(mask,iterations=3)
            dist=max(float(distance_transform_edt(~aa)[bb&~wide].max()),float(distance_transform_edt(~bb)[aa&~wide].max()))/t.cdp.js('devicePixelRatio') if aa.any() and bb.any() else None
            check(dist is not None and dist<=1,view+' independent preview Hausdorff within 1 view pixel',{'distance':dist,'heldInk':int(aa.sum()),'committedInk':int(bb.sum())})
            await fixture(view)
            await gesture(view,[(120,100),(135,110)])
            check(not (await c('draw.selection.get')).get('selectedIds'),view+' missed press empties selection',None)
            rec=await gesture(view,[(-100,-80),(100,80)])
            check(bool((await c('draw.selection.get')).get('selectedIds')),view+' following marquee selects',rec)
            rec=await gesture(view,[(-80,-60),(-45,-30)])
            check(any(x['commandId']=='draw.envelope.apply' for x in (rec or {}).get('commands',[])),view+' following corner works',rec)
        elif step==5:
            for name,pts,mods in [('offset',[(0,0),(20,15)],()),('rotate',[(-90,-70),(-60,-90)],()),('scale',[(-80,-60),(-100,-75)],()),('envelope',[(-80,-60),(-50,-35)],(0x12,))]:
                await compare(view,view+'-'+name,pts,mods,tool='repositionAll')
        elif step==6:
            for tool in ('perspective','repositionAll'):
                for mode in ('one-locked','current-locked','all-locked','element-locked','hidden'):
                    if mode=='hidden' and view!='cameraView':continue
                    async def setup(eid,ids):
                        if mode=='one-locked':await c('draw.lock.set',{'drawingId':ids[1],'locked':True})
                        elif mode=='current-locked':await c('draw.lock.set',{'drawingId':ids[0],'locked':True})
                        elif mode=='all-locked':
                            for did in ids:await c('draw.lock.set',{'drawingId':did,'locked':True})
                        elif mode=='element-locked':await c('element.lock.set',{'elementIds':[eid],'locked':True})
                        else:
                            await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':False,'outlineOnSelection':False})
                            await c('drawAid.currentOnTop.set',{'enabled':False})
                            await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False})
                        await asyncio.sleep(.5)
                        node=d.McpTable.node(await d.McpTable.tree(t),view+'.canvas')
                        t.run.save(view+'-'+tool+'-'+mode+'-canvas.json',node)
                        if mode=='hidden':check(not json.loads((node or {}).get('value') or '{}').get('toolBox'),view+' '+tool+' hidden has no overlay box',node)
                    b,a,r=await compare(view,view+'-'+tool+'-'+mode,[(-80,-60),(-50,-35)],tool=tool,setup=setup,expect=mode=='one-locked' or (mode=='current-locked' and tool=='repositionAll') or (mode=='element-locked' and view=='drawingView'))
                    if (mode not in ('one-locked','current-locked') and not (mode=='element-locked' and view=='drawingView')) or (mode=='current-locked' and tool=='perspective'):check(b==a,view+' '+tool+' '+mode+' forbidden edit unchanged',r)
                    elif tool=='repositionAll' and mode in ('one-locked','current-locked'):
                        locked_index=1 if mode=='one-locked' else 0
                        check(b[locked_index]==a[locked_index] and all(b[i]!=a[i] for i in range(3) if i!=locked_index),view+' partial lock skips only locked',r)
        elif step==7:
            for tool in ('perspective','repositionAll'):
                await fixture(view,tool)
                await c('window.panel.set',{'panelId':'properties','visible':True});await asyncio.sleep(.5)
                t.click('properties.page.toolOptions');await asyncio.sleep(.3)
                for ident,cmd in [('drawTopLayer','tool.drawTopLayer.set'),('placeBehind','tool.placeBehind.set'),('snapToContour','tool.snapToContour.set')]:
                    t.clear();pos=await t.position();t.click('toolOptions.'+ident);await asyncio.sleep(.3)
                    sent=[x for x in t.recorded() if x['command']==cmd]
                    check(len(sent)==1 and await t.position()==pos,view+' '+tool+' options '+ident+' command no undo',sent)
                    t.run.save(view+'-'+tool+'-'+ident+'.json',{'sent':sent,'options':await c('tool.options.get',{'toolId':tool})})
                page=t.cdp.js("document.querySelector('[data-tt-id=\"toolOptions\"]')?.outerHTML")
                t.run.save(view+'-'+tool+'-options.json',page)
                check('mapping' not in (page or ''),view+' '+tool+' no undeclared mapping control D-75 item 5',None)
                d.front();rr=t.rect(view+'.canvas');origin=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(origin))
                d.right_click_screen(origin.x+rr['x']*rr['dpr'],origin.y+rr['y']*rr['dpr']);await asyncio.sleep(.3)
                t.run.save(view+'-'+tool+'-context.json',await d.McpTable.tree(t));d.key(0x1b)
                await c('draw.selection.set',{'by':'ids','ids':[]});t.clear();pos=await t.position()
                await t.do(('context',view+'.canvas',view+'.drawingMenu.selectAll'));await asyncio.sleep(.3)
                check(bool((await c('draw.selection.get')).get('selectedIds')) and pos==await t.position(),view+' '+tool+' context Select All selects with no undo',t.recorded())
                t.run.save(view+'-'+tool+'-context-action.json',t.recorded())
                await c('draw.selection.set',{'by':'ids','ids':[]})
                rec=await gesture(view,[(-80,-60),(-50,-35)])
                t.run.save(view+'-'+tool+'-empty-selection.json',rec)
                for mode in ('tool','frame','agent'):
                    async def install(eid,ids):
                        nonlocal hold_action
                        async def held():
                            command,params=('tool.active.set',{'toolId':'select'}) if mode=='tool' else ('app.context.set',{'elementId':eid,'frame':9}) if mode=='frame' else ('draw.stroke.add',{'points':[{'x':0,'y':0},{'x':10,'y':10}]})
                            answer=await d.e2e.run_cmd(t.s,command,params)
                            t.run.save(view+'-'+tool+'-held-'+mode+'.json',answer)
                            check(answer.get('status')!='ok' and 'ui.busy' in json.dumps(answer),view+' '+tool+' held '+mode+' is refused busy',answer)
                        hold_action=held
                    await compare(view,view+'-'+tool+'-lifecycle-'+mode,[(-80,-60),(-50,-35)],tool=tool,setup=install)
                    hold_action=None
                    check((await c('draw.current.get')).get('frame')==1,view+' '+tool+' held '+mode+' stays on original frame',None)
                async def switch_setup(eid,ids):
                    nonlocal hold_action
                    await c('keymap.set',{'commandId':'tool.active.set','params':{'toolId':'select'},'keys':'F6','context':'global','onConflict':'override'})
                    async def switch():d.key(0x75);await asyncio.sleep(.3)
                    hold_action=switch
                await compare(view,view+'-'+tool+'-keyboard-switch',[(-80,-60),(-50,-35)],tool=tool,setup=switch_setup,command_override='draw.transform.apply',label_override='Scale Object')
                hold_action=None
                t.run.save(view+'-'+tool+'-active-after-switch.json',await c('tool.active.get'))
                eid,ids=await fixture(view,tool)
                await c('element.drawingContent.delete',{'drawingIds':ids})
                before=await drawings(ids);pos=await t.position()
                rec=await gesture(view,[(-80,-60),(-50,-35)])
                check(before==await drawings(ids) and pos==await t.position(),view+' '+tool+' empty drawings unchanged',rec)
                t.run.save(view+'-'+tool+'-empty-drawings.json',rec)


async def text_body(t,step):
    import asyncio, ctypes, time
    from ctypes import wintypes
    p=PRIOR; d=p.d; c=t.cmd; check=t.run.check
    source=(ROOT/'scripts/c43-d39-bindings.py').read_text(encoding='utf-8')
    source=source[source.index('class Cdp:'):source.index('# ----- OS input')].replace('class Cdp:','class TextCdp:').replace('if "/run_command" in (req.get("url") or ""):', 'if any(p in (req.get("url") or "") for p in ("/run_command", "/text_input")):').replace('req.get("command"), "params": req.get("params") or {}', 'req.get("command") or "text_input", "params": req.get("params") or req')
    namespace=dict(d.__dict__);exec(source,namespace);t.cdp=namespace['TextCdp'](d.CDP_PORT);t.cdp.record()
    rows=[]
    async def pause():await asyncio.sleep(.6)
    async def state(name):
        tree=await d.McpTable.tree(t)
        canvas=d.McpTable.node(tree,view+'.canvas') or {}
        v=json.loads(canvas.get('value') or '{}')
        row={'name':name,'commands':[x for x in t.recorded() if t.kinds.get(x['command'])!='query'],'history':await c('edit.history.get',{'limit':1000}),'drawing':await c('draw.drawing.get',{'flatten':True,**({'drawingId':target_drawing} if target_drawing else {})}),'editor':t.cdp.js("(()=>{const e=document.querySelector('[data-tt-id=\"canvas.textEditor\"]');return e?{anchor:Number(e.dataset.ttAnchor),caret:Number(e.dataset.ttCaret),textId:e.dataset.ttTextId}:null})()"),'canvas':v,'panelState':t.cdp.js("[...document.querySelectorAll('[data-tt-id^=\"text.\"]')].map(e=>({id:e.dataset.ttId,value:e.value,checked:e.getAttribute('aria-checked'),placeholder:e.getAttribute('placeholder')}))"),'status':t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent"),'role':d.McpTable.node(tree,'text.role'),'tool':await c('tool.active.get')}
        rows.append(row);t.clear();return row
    async def gesture(pts,double=False):
        m=(await c('draw.viewMapping.get',{'view':view}))['drawingToView']
        eq=load('text_eq','scripts/c44b-gesture-equivalence.py')
        rr=t.cdp.js("document.querySelector('[data-tt-id=\""+view+".canvas\"]').getBoundingClientRect().toJSON()")
        o=wintypes.POINT();d.front();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o));scale=t.cdp.js('devicePixelRatio')
        xy=[eq.c42.apply(m,*q) for q in pts]
        for i,(x,y) in enumerate(xy):
            d.u32.SetCursorPos(round(o.x+(rr['x']+x)*scale),round(o.y+(rr['y']+y)*scale))
            if i==0:d.e6._send_button(True)
            await asyncio.sleep(.1)
        d.e6._send_button(False)
        if double:
            await asyncio.sleep(.08);d.e6._send_button(True);d.e6._send_button(False)
        await pause()
    async def fixture(text='',width=None,v='drawingView'):
        nonlocal view,eid,target_drawing
        view=v
        await p.r3.preserve_fixture(t)
        await c('project.create',{'name':'Text live review','onUnsaved':'discard'})
        eid=(await c('element.create',{'type':'drawing','name':'Text'}))['elementIds'][0]
        await c('element.drawing.create',{'elementId':eid,'name':'1','exposeFrom':1,'exposeTo':24})
        await c('app.context.set',{'elementId':eid,'frame':1})
        target_drawing=(await c('draw.drawing.get'))['drawingId']
        await c('workspace.select',{'name':'Drawing' if view=='drawingView' else 'Default'})
        await c('window.panel.set',{'panelId':view,'visible':True,'focus':True});await c('view.active.set',{'viewId':view});await c('view.transform.reset',{'viewId':view})
        await c('tool.active.set',{'toolId':'text'});await pause();t.clear()
        await gesture([(-100,80)] if width is None else [(-100,80),(-100+width,20)])
        if text:
            for i,line in enumerate(text.split('\n')):
                if i:d.key(0x0d)
                d.type_text(line);await pause()
        return await state('fixture '+text[:20])
    def texts(row):return [n for n in row['drawing'].get('nodes',[]) if 'text' in n and 'runs' in n]
    def content(row):return ''.join(n['text'] for n in texts(row))
    async def key(vk,*mods):
        d.front();t.cdp.js("document.querySelector('[data-tt-id=\"canvas.textEditor\"]')?.focus()");d.key(vk,*mods);await pause();return await state('key '+hex(vk)+' '+str(mods))
    async def typed(txt):
        d.front();t.cdp.js("document.querySelector('[data-tt-id=\"canvas.textEditor\"]')?.focus()");d.type_text(txt);await pause();return await state('typed '+txt[:24])
    def clip_set(txt):
        u=ctypes.windll.user32;k=ctypes.windll.kernel32
        k.GlobalAlloc.restype=ctypes.c_void_p;k.GlobalLock.restype=ctypes.c_void_p;k.GlobalLock.argtypes=[ctypes.c_void_p];k.GlobalUnlock.argtypes=[ctypes.c_void_p]
        u.SetClipboardData.argtypes=[ctypes.c_uint,ctypes.c_void_p]
        data=txt.encode('utf-16-le')+b'\0\0';h=k.GlobalAlloc(0x42,len(data));v=k.GlobalLock(h);ctypes.memmove(v,data,len(data));k.GlobalUnlock(h)
        assert u.OpenClipboard(None);u.EmptyClipboard();assert u.SetClipboardData(13,h);u.CloseClipboard()
        class Done:
            def destroy(self):pass
        return Done()
    def clip_get():
        import tkinter
        r=tkinter.Tk();r.withdraw()
        try:return r.clipboard_get()
        finally:r.destroy()
    view='drawingView';eid=None;target_drawing=None
    try:
        if step==1:
            for width in (None,240):
                r=await fixture('Hello',width);ns=texts(r)
                check(len(ns)==1 and ns[0]['text']=='Hello','02 7.6 real creation',ns)
                check(width is None or abs(ns[0].get('width',0)-width)<=1,'02 7.6 drag width',ns)
                # Independent direct command on a clean drawing; compare text content excluding generated ids.
                gui=ns[0]
                await c('tool.active.set',{'toolId':'select'});await c('draw.object.delete',{'ids':[gui['id']]})
                await c('tool.active.set',{'toolId':'text'})
                rec=r['canvas']['lastGesture']
                answer=await d.e2e.call(t.s,'pointer',{'view':view,'tool':'text','space':'view','events':rec['events']});rows.append({'name':'MCP pointer','answer':answer})
                answer=await d.e2e.call(t.s,'type_text',{'text':'Hello'});rows.append({'name':'MCP type_text','answer':answer})
                agent=await state('agent pointer type_text');normalize=lambda n:{k:v for k,v in n.items() if k not in ('id','bounds')}
                check(normalize(gui)==normalize(texts(agent)[0]),'D-32 MCP pointer exact text node',{'gui':gui,'agent':texts(agent)})
                await c('tool.active.set',{'toolId':'select'});await c('draw.object.delete',{'ids':[texts(agent)[0]['id']]})
                params={'position':{'x':gui['transform'][4],'y':gui['transform'][5]},'text':'Hello'}
                if width is not None:params['width']=width
                await c('draw.text.add',params);direct=await state('direct draw.text.add')
                normalize=lambda n:{k:v for k,v in n.items() if k not in ('id','bounds')}
                check(normalize(gui)==normalize(texts(direct)[0]),'D-32 direct exact text node',{'gui':gui,'direct':texts(direct)})
        elif step==2:
            for mode in ('continuous','left','click','timer','save'):
                await fixture();pos=await t.position();await typed('ab')
                if mode=='left':await key(0x25)
                if mode=='click':await gesture([(-90,65)]);await state('click caret')
                if mode=='timer':await asyncio.sleep(10)
                if mode=='save':await c('project.saveAs',{'path':str(t.work/(mode+'.animpkg'))});await state('save')
                await typed('c');labels=await t.steps_since(pos)
                check([x[0] for x in labels]==(['Typing'] if mode in ('continuous','timer') else ['Typing','Typing']),'SI-191 '+mode,labels)
                await c('edit.undo');r=await state('undo '+mode)
                if mode in ('continuous','timer'):check(content(r)=='','SI-191 undo whole run',texts(r))
        elif step==3:
            await fixture('one two\nthree four')
            for vk,mods in [(0x24,()),(0x27,()),(0x25,()),(0x26,()),(0x28,()),(0x23,()),(0x25,(0x10,)),(0x27,(0x10,)),(0x26,(0x10,)),(0x28,(0x10,)),(0x24,(0x10,)),(0x23,(0x10,)),(0x25,(0x11,)),(0x27,(0x11,)),(0x08,()),(0x2e,()),(0x08,(0x11,)),(0x0d,()),(0x09,()),(0x41,(0x11,))]:
                await key(vk,*mods)
            await gesture([(-70,65)],double=True);r=await state('double click word')
            check(r['editor'] is not None and r['editor']['anchor']!=r['editor']['caret'],'02 7.6 double click selects word',r['editor'])
        elif step==4:
            await fixture('hello world')
            await typed('b');await key(0x20);r=await state('tool letter and space')
            check(r['tool'].get('toolId')=='text' and content(r)=='hello worldb ','SI-193 letter space',r['tool'])
            await key(0x25);r=await key(0x2e);check(content(r)=='hello worldb','SI-193 Delete character',texts(r))
            await key(0x41,0x11);before=await c('draw.selection.get');await key(0x43,0x11);check(clip_get()=='hello worldb','SI-193 OS Copy',clip_get())
            await key(0x58,0x11);r=await key(0x56,0x11);check(content(r)=='hello worldb','SI-193 Cut Paste',texts(r));check(before==await c('draw.selection.get'),'SI-193 selection untouched',None)
            await key(0x1b);await gesture([(180,-80)]);await state('outside new object');await key(0x1b)
            r=await state('empty deletion');check(any(x.get('label')=='Delete Text Object' for x in r['history'].get('entries',[])),'SI-194 empty deletion label',r['history'])
            await fixture('keep');await c('tool.active.set',{'toolId':'brush'});await typed('z');r=await state('tool switch');check(content(r)=='keep','02 7.6 switch keeps text',texts(r))
            await fixture('abc');r=await key(0x5a,0x11);check(content(r)=='','STATIC-04 document undo',texts(r))
        elif step==5:
            value='e\u0301 \U0001f469\u200d\U0001f4bb \U00010400 \u65e5\u672c\u8a9e'
            for mode in ('paste','unicode'):
                await fixture()
                if mode=='paste':root=clip_set(value);r=await key(0x56,0x11);root.destroy()
                else:
                    units=value.encode('utf-16-le');events=[]
                    for i in range(0,len(units),2):
                        for flags in (4,6):
                            event=d.INPUT(type=1);event.ki=d.KEYBDINPUT(0,int.from_bytes(units[i:i+2],'little'),flags,0,None);events.append(event)
                    batch=(d.INPUT*len(events))(*events);assert d.u32.SendInput(len(events),batch,ctypes.sizeof(d.INPUT))==len(events)
                    await pause();r=await state('synthetic unicode')
                check(content(r)==__import__('unicodedata').normalize('NFC',value),'10 section 1 NFC Unicode '+mode,{'input':value,'observed':content(r),'exactInput':content(r)==value})
                for _ in range(len(value)):await key(0x25)
                for _ in range(7):await key(0x27)
                await key(0x08);await key(0x2e)
        elif step==6:
            await fixture('one two');await c('window.panel.set',{'panelId':'text','visible':True});await pause()
            await key(0x24)
            for _ in range(3):await key(0x27,0x10)
            for ident in ('text.bold','text.italic'):
                t.click(ident);await pause();await state(ident)
            for ident,value in [('text.size','48'),('text.kerning','7')]:
                t.click(ident);d.key(0x41,0x11);d.type_text(value);d.key(0x0d);await pause();await state(ident)
            await t.do(('select','text.align','centre'));await pause();await state('alignment')
            await key(0x41,0x11);await state('mixed')
            await key(0x23);t.click('text.bold');await pause();await state('pending bold');await typed('B')
            await fixture('A');await c('window.panel.set',{'panelId':'text','visible':True});await pause();pos=await t.position();t.click('text.bold');await pause();await typed('B');check(len(await t.steps_since(pos))==1,'STATIC-01 format ends typing run',await t.steps_since(pos))
            doc=await c('doc.export.json',{'scope':'project','binaries':'omit'});t.run.save('colour-source.json',doc)
            colour=doc['bundle']['palettes'][0]['swatches'][2]['colourId'];answer=await d.e2e.run_cmd(t.s,'colour.current.set',{'colourId':colour});rows.append({'name':'colour attempt','answer':answer})
            if answer.get('status')=='ok':
                r=await typed('C');check(texts(r)[0]['runs'][-1]['colourId']==answer['result']['colourId'],'STATIC-03 current colour',texts(r))
        elif step==7:
            for mode in ('normal','element-locked','drawing-locked','hidden','frame','tool'):
                r=await fixture('abcd',v='cameraView');node=texts(r)[0]
                if mode=='element-locked':await c('element.lock.set',{'elementIds':[eid],'locked':True})
                if mode=='drawing-locked':await c('draw.lock.set',{'drawingId':r['drawing']['drawingId'],'locked':True})
                if mode=='hidden':
                    await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':False,'outlineOnSelection':False});await c('drawAid.currentOnTop.set',{'enabled':False});await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False})
                if mode=='frame':await c('app.context.set',{'elementId':eid,'frame':25})
                if mode=='tool':await c('tool.active.set',{'toolId':'brush'})
                await pause();r=await typed('X')
                if mode in ('element-locked','drawing-locked','hidden'):check(content(r)=='abcd','02 7.6 / design 7 no edit '+mode,texts(r))
                await state('camera '+mode)
                if mode in ('element-locked','drawing-locked','hidden'):
                    await gesture([(-80,65)]);r=await state('reenter '+mode);check(r['editor'] is None,'04 10.4 / design 7 no editor '+mode,r['editor'])
                if mode=='normal':
                    await c('view.zoom.set',{'viewId':view,'zoom':1.5});await c('view.rotation.set',{'viewId':view,'degrees':25});await pause();await gesture([(-80,65)]);await typed('Q');await key(0x41,0x11);await c('window.panel.set',{'panelId':'text','visible':True});await pause();t.click('text.bold');await pause();await state('camera transformed format')
        elif step==8:
            for mode in ('own','change','delete','undo'):
                r=await fixture('abcdef');node=texts(r)[0]
                if mode=='own':
                    q=p.IndependentHttp(json.loads((t.work/'discovery.json').read_text()),'independent-text-review')
                    try:
                        q.initialize();m=(await c('draw.viewMapping.get',{'view':view}))['drawingToView'];eq=load('agent_eq','scripts/c44b-gesture-equivalence.py');x,y=eq.c42.apply(m,150,-80)
                        answer=q.tool('pointer',{'view':view,'tool':'text','space':'view','events':[{'type':'down','x':x,'y':y},{'type':'up','x':x,'y':y}]});rows.append({'name':'independent pointer','answer':answer})
                        rows.append({'name':'independent type_text','answer':q.tool('type_text',{'text':'AGENT'})})
                    finally:q.close()
                if mode=='change':await c('draw.text.set',{'textId':node['id'],'text':'a'})
                if mode=='delete':await c('draw.object.delete',{'ids':[node['id']]})
                if mode=='undo':await c('edit.undo')
                await typed('Z');await state('agent '+mode)
                result=await c('doc.validate');check(not result.get('errors'),'agent '+mode+' validates',{'errors':result.get('errors'),'valid':result.get('valid')})
        elif step==9:
            await fixture(width=240);root=clip_set('x'*100000);start=time.monotonic();await key(0x56,0x11);elapsed=time.monotonic()-start;root.destroy();r=await state('100k paste');check(elapsed<8,'100k responsive',{'elapsed':elapsed,'length':len(content(r))})
            await fixture('long single line '*40,width=160);await state('wrap')
            await c('window.panel.set',{'panelId':'text','visible':True});await pause();await key(0x41,0x11);t.click('text.font');d.key(0x41,0x11);d.type_text('ReviewMissingFont987');d.key(0x0d);await pause();r=await state('missing font');check(texts(r)[0]['runs'][0]['font']=='ReviewMissingFont987','02 7.6.1 font name retained',texts(r)[0]['runs'])
            answer=await d.e2e.run_cmd(t.s,'draw.textFormat.set',{'textId':texts(r)[0]['id'],'start':0,'length':1,'font':'ReviewMissingFont988'});rows.append({'name':'missing font envelope','answer':answer})
            check(any(w.get('code')=='text.fontMissing' for w in answer.get('warnings',[])),'D-75 item 9 missing font code (severity absent from MCP warning schema)',answer.get('warnings'))
            await key(0x23);await typed('A');t.click('text.bold');await pause();await state('format');root=clip_set('paste');await key(0x56,0x11);root.destroy();before=await state('before history')
            await c('edit.undo');u=await state('undo');await c('edit.redo');r=await state('redo');check({k:v for k,v in r['drawing'].items() if k!='rev'}=={k:v for k,v in before['drawing'].items() if k!='rev'},'undo redo exact content',None);await c('edit.undo');r=await state('undo again');check({k:v for k,v in r['drawing'].items() if k!='rev'}=={k:v for k,v in u['drawing'].items() if k!='rev'},'second undo exact content',None)
    finally:
        # Cap repository receipts; full text and captures remain in TEMP.
        t.run.save('text-actions.json',rows)
        def small(x):
            if isinstance(x,str) and len(x)>1000:return {'length':len(x),'prefix':x[:80],'sha256':__import__('hashlib').sha256(x.encode()).hexdigest()}
            if isinstance(x,list):return [small(y) for y in x]
            if isinstance(x,dict):return {k:small(v) for k,v in x.items()}
            return x
        reduced=small(rows)
        for row in reduced:
            if 'canvas' in row:row['canvas']={k:v for k,v in row['canvas'].items() if k in ('lastGesture','lastError','selection','overlay')}
            if 'history' in row:row['history']={'position':row['history']['position'],'entries':[{k:e.get(k) for k in ('label','source','stepId')} for e in row['history'].get('entries',[])]}
        for offset in range(0,len(reduced),5):
            payload=json.dumps(reduced[offset:offset+5],ensure_ascii=True,separators=(',',':'))+'\n';assert len(payload)<200000
            (EVID/('l3-step'+str(step)+'-actions-'+str(offset//5)+'.json')).write_text(payload,encoding='utf-8',newline='\n')

if __name__=='__main__':
    case,step=sys.argv[1],int(sys.argv[2]); assert case in ('l1a','l1b','l2a','l2b','l3a','l3b')
    if case in ('l3a','l3b'):
        PRIOR=load('c44b_prior','tests/adversarial/c44a-r1-live.py');PRIOR.EVID=EVID;PRIOR.live_body=text_body
        result=PRIOR.live(step)
        transcript=json.loads((PRIOR.OUT/'transcript.json').read_text());checks=[{k:r.get(k) for k in ('check','ok')} for r in transcript if 'check' in r]
        save(case+'-step'+str(step)+'.json',{'raw':str(PRIOR.OUT),'results':checks,'failures':[r['check'] for r in checks if not r['ok']]})
        sys.exit(result)
    if case in ('l2a','l2b'):
        PRIOR=load('c44b_prior','tests/adversarial/c44a-r1-live.py');PRIOR.EVID=EVID;PRIOR.live_body=envelope_body
        phase='-user-frame' if os.environ.get('C44B_FRAME_ONLY') else ''
        if phase:PRIOR.EVID=Path(tempfile.mkdtemp(prefix='c44b-summary-'))
        result=PRIOR.live(step);compact('l2-'+str(step)+phase,PRIOR.OUT);sys.exit(result)
    if case=='l1a' and step==1:sys.exit(build())
    if case=='l1a' and step==2: sys.exit(regressions())
    if step in (3,4,5,6,7,8): sys.exit(live(step))
    raise ValueError('Step not implemented yet')
