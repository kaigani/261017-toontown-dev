"""Independent C4.3b review probes. Usage: probe APP case1-vite."""
import sys
if len(sys.argv) < 3:
    print("skipped: no application")
    sys.exit(0)
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "reviews/evidence/M4-C4.3b-r1"
def case1_vite():
    results = []
    with tempfile.TemporaryDirectory(prefix="c43b-review-") as td:
        tmp = Path(td)
        tracked = subprocess.check_output(["git", "ls-files", "desktop", "crates/executor/src", "pnpm-workspace.yaml", "package.json", "pnpm-lock.yaml"], cwd=ROOT, text=True).splitlines()
        for name in tracked:
            src, dst = ROOT/name, tmp/name
            if src.is_file():
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(src,dst)
        for rel in ["node_modules", "desktop/node_modules"]:
            if os.name == "nt":
                subprocess.run(["cmd", "/c", "mklink", "/J", str(tmp/rel), str(ROOT/rel)], check=True, capture_output=True)
            else:
                (tmp/rel).symlink_to(ROOT/rel, target_is_directory=True)
        manifest = tmp/"desktop/src/ui-bindings/parity.json"
        original = manifest.read_text(encoding="utf-8")
        for fixture in ["baseline", "missing-rendered", "stale-element"]:
            data = json.loads(original)
            if fixture == "missing-rendered":
                data["dom"].pop()
            if fixture == "stale-element":
                data["dom"].append({"id":"review.absent", "surface":"review", "commands":["edit.undo"]})
            manifest.write_text(json.dumps(data,indent=2)+"\n" if fixture != "baseline" else original, encoding="utf-8", newline="\n")
            command = ["pnpm.cmd" if os.name == "nt" else "pnpm", "--dir", "desktop", "test", "src/ui-bindings/parity.test.tsx"]
            run = subprocess.run(command,cwd=tmp,capture_output=True,text=True,encoding="utf-8",timeout=180)
            (OUT/("case1-vite-"+fixture+".txt")).write_text(run.stdout+run.stderr,encoding="utf-8",newline="\n")
            results.append({"fixture":fixture,"exit":run.returncode})
        # Remove junctions before TemporaryDirectory walks the fixture.
        for rel in ["desktop/node_modules", "node_modules"]:
            if os.name == "nt": os.rmdir(tmp/rel)
            else: (tmp/rel).unlink()
    (OUT/"case1-vite.json").write_text(json.dumps(results,indent=2)+"\n",encoding="utf-8",newline="\n")
    assert results[0]["exit"] == 0, results
    assert all(r["exit"] != 0 for r in results[1:]), results
    print(results)

def load(name, file):
    import importlib.util
    spec = importlib.util.spec_from_file_location(name, ROOT/file)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m

async def scenes(session, run, out):
    import asyncio
    m = CAMERA
    rc = m.e2e.run_cmd
    async def command(name, params):
        r = await rc(session, name, params)
        run.check(r.get('status') == 'ok', 'scene construction '+name, r.get('error'))
        return r.get('result') or {}
    results = []
    for scene in range(1,5):
        if scene == 1:
            await command('project.create', {'name':'ReviewScenes','onUnsaved':'fail'})
        else:
            hist=await command('edit.history.get', {'limit':1000})
            for _ in range(hist.get('position',0)):
                await command('edit.undo', {})
        found = await command('doc.object.find', {'type':'swatch','name':'Colour 2'})
        sw = (found.get('items') or found.get('matches'))[0]
        colour = sw.get('id') or sw.get('key') or sw.get('colourId')
        ids = []
        for i in range(3):
            el = await command('element.create', {'type':'drawing','name':'Panel'+str(i)})
            eid = el['elementIds'][0]; ids.append(eid)
            await command('element.drawing.create', {'elementId':eid,'name':'review','exposeFrom':1,'exposeTo':24})
            await command('app.context.set', {'elementId':eid,'frame':1})
            await command('colour.current.set', {'colourId':colour})
            await command('draw.rectangle.add', {'from':{'x':-95+i*15,'y':-65},'to':{'x':80,'y':75+i*8}})
            await command('colour.paint.apply', {'points':[{'x':0,'y':0}]})
            await command('anim.transform.set', {'nodeIds':[eid],'target':'static','position':{'x':(i-1)*1.8,'y':(i-1)*.3,'z':[-2,0,15.8][i] if scene==4 else -i*.3}})
        if scene == 1:
            pegs=[]
            for i in range(2):
                p=await command('element.create', {'type':'peg','name':'Nested'+str(i)})
                pegs.append(p['elementIds'][0])
                await command('anim.transform.set', {'nodeIds':[pegs[-1]],'target':'static','rotation':23-i*37,'scale':{'x':1.4-i*.3,'y':.7+i*.2}})
            await command('element.parent.set', {'elementIds':[pegs[1]],'parentId':pegs[0]})
            await command('element.parent.set', {'elementIds':ids,'parentId':pegs[1]})
        if scene == 2:
            await command('anim.camera.create', {'name':'Spare','makeCurrent':False})
            await command('anim.camera.create', {'name':'Current','makeCurrent':True,'position':{'x':.4,'y':.2,'z':16},'fov':35})
            cam=(await command('anim.camera.current.get', {}))
            run.save('camera-id.json',cam)
            await command('anim.camera.set', {'cameraId':cam.get('cameraId') or cam.get('id'),'roll':17})
        if scene == 3:
            await command('element.visibility.set', {'elementIds':[ids[0]],'scope':'scene','visible':False})
            await command('element.lock.set', {'elementIds':[ids[1]],'locked':True})
            await command('drawAid.currentOnTop.set', {'enabled':True})
            await command('drawAid.selectionDisplay.set', {'boxHighlighting':True,'keepSelectionVisible':False})
        await command('app.context.set', {'elementId':ids[1],'frame':1})
        await command('edit.selection.set', {'context':'staging','ids':[]})
        await command('view.active.set', {'viewId':'cameraView'})
        await command('view.transform.reset', {'viewId':'cameraView'})
        m.MASK=[]
        if scene == 3:
            await command('edit.selection.set', {'context':'staging','ids':[ids[1]]})
            await command('anim.tool.set', {'tool':'select'})
            b=await m.box_of(session,'cameraView')
            if b:
                x0,y0,x1,y1=b
                m.MASK=[(x0-15,y0-15,x1+15,y0+15),(x0-15,y1-15,x1+15,y1+15),
                        (x0-15,y0-15,x0+15,y1+15),(x1-15,y0-15,x1+15,y1+15)]
                mp=((await command('draw.viewMapping.get',{'view':'cameraView'})).get('drawingToView'))
                px,py=m.c42.apply(mp,0,0); m.MASK.append((px-8,py-8,px+8,py+8))
            run.save('selection-overlay-mask.json',m.MASK)
        for variant in (['plain','zoom-pan-resize'] if scene==1 else ['plain']):
            if variant != 'plain':
                await command('view.zoom.set', {'viewId':'cameraView','zoom':1.3})
                await command('view.pan.set', {'viewId':'cameraView','by':{'x':35,'y':-23}})
                await command('window.geometry.set', {'maximized':False,'width':1280,'height':900})
            d=await m.compare_camera(session,run,out,f'scene{scene}-{variant}',f'independent scene {scene} {variant}')
            summary=await m.canvas_summary(session)
            run.save(f'scene{scene}-{variant}-summary.json',summary)
            run.check(summary.get('backend')==m.REQUESTED['backend'] and not summary.get('fallback'),'requested actual backend',summary.get('backend'))
            results.append({'scene':scene,'variant':variant,**d})
            run.save('figures.json',results)

def case3():
    global CAMERA
    import asyncio
    CAMERA=load('review_camera','scripts/c43-camera-e2e.py')
    import inspect
    body=inspect.getsource(CAMERA.compare_mapped).replace('            n += 1', '            if any(x0 <= vx <= x1 and y0 <= vy <= y1 for x0,y0,x1,y1 in MASK):\n                continue\n            n += 1')
    CAMERA.MASK=[]
    exec(compile(body,__file__,'exec'),CAMERA.__dict__)
    async def session(url,token,run,out,os_input):
        from contextlib import AsyncExitStack
        e=CAMERA.e2e
        async with AsyncExitStack() as stack:
            client=await stack.enter_async_context(e.httpx2.AsyncClient(headers={'Authorization':f'Bearer {token}'},timeout=300))
            streams=await stack.enter_async_context(e.streamable_http_client(url,http_client=client))
            s=await stack.enter_async_context(e.ClientSession(streams[0],streams[1])); await s.initialize()
            await e.gui_tree(s,run,lambda r:e.by_id(r,'startScreen') is not None,'start screen')
            await scenes(s,run,out)
    CAMERA.session_run=session
    app=str(Path(sys.argv[1]).resolve()); codes=[]
    for backend in ('webgl2','webgpu'):
        sys.argv=['camera',app,str(OUT/'case3'/backend),'--backend',backend]
        codes.append(CAMERA.main())
    sys.exit(int(any(codes)))

def case4():
    if len(sys.argv)<4:
        print('skipped: no application'); return
    import inspect
    m=load('review_equiv','scripts/c43-gesture-equivalence.py')
    body=inspect.getsource(m.main)
    a=body.index('    rows = ['); b=body.index('    results = []',a)
    body=body[:a]+'''    def rotation(u,o,m):
        import math
        c=((u[0]+u[2])/2,(u[1]+u[3])/2)
        p=(u[2]+9,u[1]-9); dx,dy=p[0]-c[0],p[1]-c[1]; t=-.31
        return [p,(c[0]+dx*math.cos(t)-dy*math.sin(t),c[1]+dx*math.sin(t)+dy*math.cos(t))]
    corner=lambda u,o,m:[(u[2],u[1]),(u[2]+21,u[1]-13)]
    edge=lambda u,o,m:[(u[2],(u[1]+u[3])/2),(u[2]+24,(u[1]+u[3])/2)]
    skew=lambda u,o,m:[((u[0]+u[2])/2,u[1]-12),((u[0]+u[2])/2+22,u[1]-12)]
    move=du((40,30),(65,44))
    rows=[('move','scene','select',move,True),('rotate','scene','transform',rotation,True),
          ('corner','scene','transform',corner,True),('edge','scene','transform',edge,True),
          ('skew','scene','transform',skew,True),('pivot','scene','transform',lambda u,o,m:[o,(o[0]+17,o[1]-9)],True),
          ('constrain scale','scene','transform',corner,True),('constrain rotate','scene','transform',rotation,True),
          ('temporary select','scene','transform',move,True),('temporary skew','scene','transform',skew,True),
          ('keyed frame 8','scene','transform',move,True),('child move','scene','select',move,True),
          ('line under peg','drawing','line',du((-70,35),(55,50)),False)]
''' + body[b:]
    body=body.replace('    saved = work /', '''    peg=((await app.result('element.create',{'type':'peg','name':'ReviewParent'})).get('elementIds'))[0]
    await rc('anim.transform.set',{'nodeIds':[peg],'target':'static','rotation':27,'scale':{'x':1.35,'y':.72}})
    await rc('element.parent.set',{'elementIds':[ids['Bob']],'parentId':peg})
    await rc('anim.transform.set',{'nodeIds':[ids['Bob']],'target':'key','frame':1,'position':{'x':0,'y':0,'z':0}})
    saved = work /''')
    body=body.replace('        for engine in engines:\n            await engine.run("edit.selection.set"', '''        for engine in engines:
            await engine.run('app.context.set',{'elementId':ids['Bob'],'frame':8 if name=='keyed frame 8' else 1})
            await engine.run("edit.selection.set"''')
    body=body.replace('        before = json.dumps', '''        before_content=[(await e.result('doc.object.get',{'ref':ids['Bob']})).get('object') if family=='scene' else await e.result('draw.drawing.get',{}) for e in engines]
        before = json.dumps''')
    body=body.replace('            # The Paint row', '''            await engine.run('view.zoom.set',{'viewId':'cameraView','zoom':.5})
            # The Paint row''')
    body=body.replace('            e6.os_drag(e2e.APP_PID', '''            modifier=0x10 if name.startswith('constrain') else 0x11 if name=='temporary select' else 0x12 if name=='temporary skew' else None
            import ctypes
            if modifier: ctypes.windll.user32.keybd_event(modifier,0,0,0)
            e6.os_drag(e2e.APP_PID''')
    body=body.replace('            how = "os"', '''            if modifier: ctypes.windll.user32.keybd_event(modifier,0,2,0)
            how = "os"''')
    body=body.replace('        results.append(row)\n        run.save', '''        row['directResults']=direct
        await review_expect(m=None, row=row, run=run, u=u, mapping=m, ids=ids, name=name, events=events, frame=8 if name=='keyed frame 8' else 1)
        run.check(how=='os','real OS input '+name,how)
        results.append(row)
        run.save''')
    body=body.replace('    return results', '''        restored=[(await e.result('doc.object.get',{'ref':ids['Bob']})).get('object') if family=='scene' else await e.result('draw.drawing.get',{}) for e in engines]
        run.save('undo-'+name.replace(' ','-')+'.json',{'before':before_content,'after':restored})
        row['undoRestored']=canonical(restored)==canonical(before_content)
        run.check(row['undoRestored'],'undo restores prior content '+name,None)
        run.save('rows.json',results)
    return results''')
    m.review_expect=review_expect
    m.canonical=canonical
    body=body.replace("        before_content=[", "        before_bundle=[canonical((await e.result('doc.export.json',{'scope':'project','binaries':'omit'})).get('bundle')) for e in engines]\n        before_content=[")
    body=body.replace("        labels = [await e.labels_since", "        bundles=[canonical((await e.result('doc.export.json',{'scope':'project','binaries':'omit'})).get('bundle')) for e in engines]\n        row['wholeDocumentSame']=bundles[0]==bundles[1]==bundles[2]\n        run.check(row['wholeDocumentSame'], 'whole document three-way '+name, None)\n        labels = [await e.labels_since")
    body=body.replace("        row['undoRestored']=", "        restored_bundle=[canonical((await e.result('doc.export.json',{'scope':'project','binaries':'omit'})).get('bundle')) for e in engines]\n        run.check(restored_bundle==before_bundle,'whole document undo '+name,None)\n        row['undoRestored']=")
    exec(compile(body,__file__,'exec'),m.__dict__)
    app=str(Path(sys.argv[1]).resolve()); head=str(Path(sys.argv[3]).resolve())
    sys.argv=['equiv',app,head,str(OUT/'case4')]
    sys.exit(m.run_main())

async def review_expect(m,row,run,u,mapping,ids,name,events,frame):
    import math
    cmd=row['commands']['window']; p=(cmd[0]['params'] if cmd else {})
    target='static' if name in ('move','temporary select','child move','pivot') else 'key'
    field='position' if name in ('move','temporary select','child move','keyed frame 8') else 'rotation' if 'rotate' in name else 'scale' if name in ('corner','edge','constrain scale') else 'skew' if 'skew' in name else 'rotationPivot'
    label={'position':'Move Element','rotation':'Rotate Element','scale':'Scale Element','skew':'Skew Element','rotationPivot':'Move Pivot Element'}[field]
    expected={'spec':'05_scene_planning.md:394-419; 1174-1199; M4-design-gestures.md:113-118 (SI-144)','command':'anim.transform.set','nodeIds':[ids['Bob']],'target':target,'channel':field,'frame':frame,'label':label}
    ok=bool(cmd) and cmd[0]['commandId']=='anim.transform.set' and p.get('target')==target and p.get('nodeIds')==[ids['Bob']] and field in p and row['labels']==[[label]]*3
    if name=='line under peg':
        expected={'spec':'02_drawing.md:540-543,1321; M4-design-gestures.md:92-94','command':'draw.line.add','label':'Line Tool'}
        ok=bool(cmd) and cmd[0]['commandId']=='draw.line.add' and row['labels']==[['Line Tool']]*3
    elif target=='key':
        ok=ok and p.get('frame')==frame
        if name=='edge': ok=ok and set(p.get('scale',{}))=={'x'}
    if name!='line under peg':
        channels=[] if target=='static' else ['scale.x'] if name=='edge' else ['scale.x','scale.y'] if field=='scale' else [field]
        keyed=(row['directResults'][0].get('result') or {}).get('keyedChannels',[])
        expected['keyedChannels']=channels
        ok=ok and sorted(keyed)==sorted(channels)
    if events:
        start,end=events[0],events[-1]
        x,y=start['x'],start['y']; X,Y=end['x'],end['y']
        a,b,c,d,e,f=mapping; det=a*d-b*c
        dx=(d*(X-x)-c*(Y-y))/det; dy=(-b*(X-x)+a*(Y-y))/det
        values={}
        if field=='position': values={'position':{'x':dx/100,'y':dy/100}}
        elif field=='rotationPivot': values={'rotationPivot':{'x':dx/100,'y':dy/100},'scalePivot':{'x':dx/100,'y':dy/100}}
        elif field=='rotation':
            cx,cy=(u[0]+u[2])/2,(u[1]+u[3])/2
            angle=math.degrees(math.atan2(y-cy,x-cx)-math.atan2(Y-cy,X-cx))
            if name.startswith('constrain'): angle=round(angle/15)*15
            values={'rotation':angle}
        elif field=='scale':
            sx=(X-u[0])/(x-u[0]); sy=(Y-u[3])/(y-u[3])
            if name=='constrain scale': sx=sy=max(sx,sy,key=abs)
            values={'scale':{'x':sx} if name=='edge' else {'x':sx,'y':sy}}
        elif field=='skew': values={'skew':math.degrees(math.atan((X-x)/((u[3]-u[1])/2)))}
        if name=='line under peg':
            values={'from':{'x':(d*(x-e)-c*(y-f))/det,'y':(-b*(x-e)+a*(y-f))/det},'to':{'x':(d*(X-e)-c*(Y-f))/det,'y':(-b*(X-e)+a*(Y-f))/det}}
        def close(a,b):
            if isinstance(a,dict): return isinstance(b,dict) and all(k in b and close(v,b[k]) for k,v in a.items())
            return isinstance(b,(float,int)) and abs(a-b)<=1e-9*max(1,abs(a),abs(b))
        expected['numeric']=values;expected['independentTolerance']=1e-9
        ok=ok and close(values,p)
    row['expected']=expected; row['specVerdict']='PASS' if ok else 'FAIL'
    run.check(ok,'D-23 stated command target channels label '+name,{'expected':expected,'observed':cmd,'labels':row['labels']})

def canonical(value):
    if isinstance(value,dict): return {k:canonical(v) for k,v in value.items() if k not in ('rev','exportedAt')}
    if isinstance(value,list): return [canonical(v) for v in value]
    return value

def case5():
    import inspect
    m=load('review_preview','scripts/c43-camera-e2e.py')
    body=inspect.getsource(m.session_run)
    # Keep the launch/session construction and V9 OS click check, replace render comparisons.
    a=body.index('        # 1. Captures against renders.'); b=body.index('        # 3. V9:',a)
    body=body[:a]+body[b:]
    body=body.replace('            await preview_rows(session, run, ids)','            await live_preview(REVIEW_MODULE, session, run, out, ids)')
    m.REVIEW_MODULE=m; m.live_preview=live_preview
    exec(compile(body,__file__,'exec'),m.__dict__)
    app=str(Path(sys.argv[1]).resolve());sys.argv=['preview',app,str(OUT/'case5'),'--backend','webgl2']
    sys.exit(m.main())

async def live_preview(m,s,run,out,ids):
    import asyncio,threading,time,ctypes
    from ctypes import wintypes
    rc=m.e2e.run_cmd; e6=m.e6; c=m.c42
    async def cmd(n,p=None):
        env=await rc(s,n,p or {})
        run.check(env.get('status')=='ok','preview setup/read '+n,env.get('error'))
        return env.get('result') or {}
    async def history(): return await cmd('edit.history.get',{'limit':1000})
    async def state(): return await cmd('doc.state.get')
    async def capture(name,view):
        data=c.png_of(await rc(s,'inspect.screenshot',{'target':view}))
        if data: (out/(name+'.png')).write_bytes(data)
        return data
    def difference(a,b):
        if not a or not b: return 1.0
        w,h,ch,aa=c.png_rgba(a); W,H,C,bb=c.png_rgba(b)
        if (w,h)!=(W,H): return 1.0
        bad=sum(max(abs(aa[y][x*ch+k]-bb[y][x*C+k]) for k in range(3))>32 for y in range(h) for x in range(w))
        return bad/(w*h)
    for view in ('cameraView','drawingView'):
        current=ids['Bob']
        if view=='drawingView':
            el=await cmd('element.create',{'type':'drawing','name':'UnpaintedPreview'})
            current=el['elementIds'][0]
            await cmd('element.drawing.create',{'elementId':current,'name':'unpainted','exposeFrom':1,'exposeTo':24})
            await cmd('app.context.set',{'elementId':current,'frame':1})
            await cmd('draw.rectangle.add',{'from':{'x':-120,'y':-90},'to':{'x':120,'y':90}})
        await cmd('workspace.select',{'name':'Default' if view=='cameraView' else 'Drawing'})
        await cmd('view.active.set',{'viewId':view})
        await cmd('app.context.set',{'elementId':current,'frame':1})
        await cmd('view.zoom.set',{'viewId':view,'zoom':.6})
        if view=='cameraView':
            await cmd('anim.tool.set',{'tool':'select'})
            await cmd('edit.selection.set',{'context':'staging','ids':[ids['Bob']]})
        else:
            await cmd('tool.active.set',{'toolId':'select'})
            await cmd('draw.selection.set',{'by':'all'})
        await asyncio.sleep(1)
        for ending in ('commit','escape'):
            place=await e6.locate_view(s,rc,view); b=await m.box_of(s,view)
            run.check(bool(place and b),'located '+view,place)
            if not place or not b: continue
            x,y=(b[0]+b[2])/2+18,(b[1]+b[3])/2+12
            pts=[(place[0]+x,place[1]+y),(place[0]+x+32,place[1]+y+17)]
            before=await state(); h0=await history(); pos=h0['position']
            base=await capture(view+'-'+ending+'-before',view)
            times={}; th=threading.Thread(target=e6.os_drag,args=(m.e2e.APP_PID['pid'],pts,.15,2.2,times));th.start()
            await asyncio.sleep(1)
            held=await state(); hh=await history(); summary=await m.canvas_summary(s,view)
            mid=await capture(view+'-'+ending+'-held',view)
            run.check(held==before and hh==h0,'05:400 PV1 held revision and history unchanged '+view+' '+ending,{'before':before,'held':held})
            run.check(bool(summary.get('preview')),'preview visible '+view+' '+ending,summary.get('preview'))
            if ending=='escape': c.os_key(0x1B)
            await asyncio.to_thread(th.join); await asyncio.sleep(.8)
            after=await capture(view+'-'+ending+'-after',view); ha=await history()
            if ending=='commit':
                frac=difference(mid,after)
                run.check(frac<.01,'SI-154 exact-class capture agrees '+view,{'fraction':frac,'RGBThreshold':32,'bound':.01})
                run.check(ha['position']==pos+1,'one commit step '+view,ha['position']-pos)
                await cmd('edit.undo')
            else:
                run.check((await state())==before and ha==h0,'Escape document unchanged '+view,None)
                run.check(not (await m.canvas_summary(s,view)).get('preview') and difference(base,after)<.001,'Escape clears preview next capture '+view,{'fraction':difference(base,after)})
    # A relative skew crossing 89 degrees must be refused (05:1236).
    await cmd('workspace.select',{'name':'Default'})
    await cmd('view.active.set',{'viewId':'cameraView'})
    await cmd('app.context.set',{'elementId':ids['Bob'],'frame':1})
    await cmd('anim.transform.set',{'nodeIds':[ids['Bob']],'target':'key','frame':1,'skew':88})
    await cmd('view.zoom.set',{'viewId':'cameraView','zoom':.1})
    await cmd('anim.tool.set',{'tool':'transform'})
    await cmd('edit.selection.set',{'context':'staging','ids':[ids['Bob']]})
    await asyncio.sleep(1)
    place=await e6.locate_view(s,rc,'cameraView'); b=await m.box_of(s,'cameraView')
    if place and b:
        x,y=(b[0]+b[2])/2,b[1]-12
        h=await history(); times={}
        await asyncio.to_thread(e6.os_drag,m.e2e.APP_PID['pid'],[(place[0]+x,place[1]+y),(place[0]+x+32,place[1]+y+17)],.15,.1,times)
        await asyncio.sleep(1)
        rec=(await m.canvas_summary(s)).get('lastGesture')
        run.save('refused.json',rec)
        await capture('refused-after','cameraView')
        run.check((await history())['position']==h['position'] and rec and rec.get('status')!='ok' and not (await m.canvas_summary(s)).get('preview'),'refused commit leaves no preview',rec)
    else: run.check(False,'refusal fixture located',{'place':place,'box':b})
    await cmd('app.context.set',{'elementId':ids['Bob'],'frame':1})
    await cmd('anim.tool.set',{'tool':'select'})
    await cmd('edit.undo')
    await cmd('view.zoom.set',{'viewId':'cameraView','zoom':.6})
    await asyncio.sleep(.5)
    await rapid_drags(m,s,run,out,ids)

async def rapid_drags(m,s,run,out,ids):
    import asyncio,ctypes,threading,time
    from ctypes import wintypes
    rc=m.e2e.run_cmd; e6=m.e6; u=ctypes.windll.user32
    async def cmd(n,p=None): return (await rc(s,n,p or {})).get('result') or {}
    results=[]
    for i in range(20):
        await cmd('edit.selection.set',{'context':'staging','ids':[ids['Bob']]})
        b=await m.box_of(s,'cameraView'); place=await e6.locate_view(s,rc,'cameraView')
        if not b or not place:
            run.check(False,'rapid fixture located '+str(i),None); continue
        h0=await cmd('edit.history.get',{'limit':1000}); before=(await cmd('doc.object.get',{'ref':ids['Bob']})).get('object')
        h=e6._main_window(m.e2e.APP_PID['pid']);u.SetForegroundWindow(h)
        origin=wintypes.POINT(0,0);u.ClientToScreen(h,ctypes.byref(origin))
        x=int(origin.x+place[0]+(b[0]+b[2])/2+18); y=int(origin.y+place[1]+(b[1]+b[3])/2+12)
        times={}
        def pair():
            u.SetCursorPos(x,y);time.sleep(.05);e6._send_button(True);time.sleep(.04);u.SetCursorPos(x+14,y+8);time.sleep(.04)
            e6._send_button(False);times['release1']=time.monotonic()
            e6._send_button(True);times['press2']=time.monotonic()
            time.sleep(.04);u.SetCursorPos(x+29,y+17)
            time.sleep(.65);u.SetCursorPos(x+34,y+20)
            time.sleep(.1);u.SetCursorPos(x+29,y+17)
            time.sleep(.8);e6._send_button(False)
        th=threading.Thread(target=pair);th.start();await asyncio.sleep(.5)
        preview=(await m.canvas_summary(s)).get('preview')
        await asyncio.sleep(.4)
        preview_late=(await m.canvas_summary(s)).get('preview')
        if i in (0,19):
            shot=m.c42.png_of(await rc(s,'inspect.screenshot',{'target':'cameraView'}))
            if shot: (out/f'rapid-{i}-held.png').write_bytes(shot)
        await asyncio.to_thread(th.join);await asyncio.sleep(.5)
        hist=await cmd('edit.history.get',{'limit':1000}); after=(await cmd('doc.object.get',{'ref':ids['Bob']})).get('object')
        steps=hist['position']-h0['position']
        mapping=(await cmd('draw.viewMapping.get',{'view':'cameraView'}))['drawingToView']
        a,b,c,d,_,_=mapping; det=a*d-b*c
        delta=lambda dx,dy: ((d*dx-c*dy)/det/100,(-b*dx+a*dy)/det/100)
        expected=delta(29,17)
        off=lambda obj: obj['transform']['offset']
        observed=(off(after)['x']-off(before)['x'],off(after)['y']-off(before)['y'])
        ordered=steps==2 and max(abs(v-w) for v,w in zip(expected,observed))<1e-9
        if steps==2:
            await cmd('edit.undo')
            first=(await cmd('doc.object.get',{'ref':ids['Bob']})).get('object')
            expected_first=delta(14,8)
            observed_first=(off(first)['x']-off(before)['x'],off(first)['y']-off(before)['y'])
            ordered=ordered and max(abs(v-w) for v,w in zip(expected_first,observed_first))<1e-9
            await cmd('edit.undo')
        else:
            for _ in range(steps): await cmd('edit.undo')
        ok=ordered and bool(preview)
        row={'iteration':i,'times':times,'previewHeld':preview,'previewAfterMoreMovement':preview_late,'steps':steps,'ordered':ordered,'expectedDelta':expected,'observedDelta':observed,'before':before,'after':after}
        results.append(row);run.save('rapid.json',results)
        run.check(ok,'PV1 consecutive drags preserve second preview and two steps '+str(i),row)


def case8():
    import inspect
    os.environ['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS']='--remote-debugging-port=9223'
    m=load('review_parity','scripts/c43-e6-coexistence.py')
    body=inspect.getsource(m.session_run)
    body=body[:body.index('        own =')]+ '        await parity_window(session,run,out,bob,REVIEW_E6)\n'
    m.parity_window=parity_window;m.REVIEW_E6=m
    exec(compile(body,__file__,'exec'),m.__dict__)
    sys.argv=['parity',str(Path(sys.argv[1]).resolve()),str(OUT/'case8')]
    sys.exit(m.main())

async def parity_window(s,run,out,bob,m):
    import asyncio,ctypes,urllib.request,websockets
    from ctypes import wintypes
    rc=m.e2e.run_cmd
    async def cmd(n,p=None): return (await rc(s,n,p or {})).get('result') or {}
    pages=json.load(urllib.request.urlopen('http://127.0.0.1:9223/json'))
    page=next(p for p in pages if p['type']=='page' and '1420' in p['url'])
    async with websockets.connect(page['webSocketDebuggerUrl'],max_size=30000000) as ws:
        seq=0
        async def js(expr):
            nonlocal seq
            seq+=1; await ws.send(json.dumps({'id':seq,'method':'Runtime.evaluate','params':{'expression':expr,'returnByValue':True}}))
            while True:
                r=json.loads(await ws.recv())
                if r.get('id')==seq:
                    if r.get('result',{}).get('exceptionDetails'): raise RuntimeError(r)
                    return r.get('result',{}).get('result',{}).get('value')
        await js("window.reviewIPC=[];window.reviewInvoke=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async function(c,a,...z){let v=await window.reviewInvoke.call(this,c,a,...z);if(c==='run_command'||c==='dispatch_key')window.reviewIPC.push({command:c,args:a,result:v});return v};true")
        u=ctypes.windll.user32;u.GetMenu.restype=wintypes.HMENU;u.GetSubMenu.restype=wintypes.HMENU
        h=m._main_window(m.e2e.APP_PID['pid']);u.SetForegroundWindow(h)
        native_trace=[]
        phase='enabled'
        def has_menu_command(value,command):
            if isinstance(value,dict):
                ref=value.get('command') or {}
                if value.get('role')=='menuitem' and (value.get('commandId')==command or isinstance(ref,dict) and ref.get('commandId')==command): return True
                return any(has_menu_command(v,command) for v in value.values())
            return isinstance(value,list) and any(has_menu_command(v,command) for v in value)
        def native_state(path):
            handle=u.GetMenu(h)
            for label in path:
                found=None
                for i in range(u.GetMenuItemCount(handle)):
                    b=ctypes.create_unicode_buffer(256);u.GetMenuStringW(handle,i,b,256,0x400)
                    if b.value.replace('&','').split('\t')[0].rstrip('…').strip()==label:found=i;break
                if found is None:return {'missing':label}
                flags=u.GetMenuState(handle,found,0x400)
                state={'label':label,'flags':flags,'enabled':not bool(flags&3)}
                handle=u.GetSubMenu(handle,found)
            return state
        async def click_screen(x,y):
            u.SetCursorPos(int(x),int(y));await asyncio.sleep(.12);m._send_button(True);m._send_button(False);await asyncio.sleep(.35)
        async def menu(path):
            handle=u.GetMenu(h)
            for label in path:
                found=None
                for i in range(u.GetMenuItemCount(handle)):
                    b=ctypes.create_unicode_buffer(256);u.GetMenuStringW(handle,i,b,256,0x400)
                    if b.value.replace('&','').split('\t')[0].rstrip('ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â¦').strip()==label:
                        found=i;break
                if found is None: raise RuntimeError('native menu label absent: '+label)
                rect=wintypes.RECT()
                if not u.GetMenuItemRect(h,handle,found,ctypes.byref(rect)): raise RuntimeError('native menu rectangle absent: '+label)
                await click_screen((rect.left+rect.right)/2,(rect.top+rect.bottom)/2)
                native_trace.append({'label':label,'treeWhileOpen':await m.e2e.call(s,'inspect_ui',{'target':'window','depth':32})})
                handle=u.GetSubMenu(handle,found)
                if handle: run.shot('native-'+phase+'-'+name.lower().replace(' ','-')+'-'+label.lower().replace(' ','-'),m.e2e.APP_PID['pid'])
        async def dom(command):
            rect=await js("(()=>{let e=[...document.querySelectorAll('[data-tt-command]')].find(e=>e.getAttribute('data-tt-command')==="+json.dumps(command)+"&&e.getBoundingClientRect().width"+("&&e.title==='brush'" if command=='tool.active.set' else '')+");if(!e)return null;let r=e.getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()")
            if not rect: raise RuntimeError('rendered control absent: '+command)
            visible=await js('(()=>{let e=document.elementFromPoint('+str(rect[0])+','+str(rect[1])+');return !!e&&!!e.closest(\'[data-tt-command="'+command+'"]\')})()')
            if not visible:
                for _ in range(100):
                    m.c42.os_key(0x09);await asyncio.sleep(.03)
                    focused=await js("document.activeElement?.getAttribute('data-tt-command')")
                    if focused==command: break
                rect=await js("(()=>{let e=document.activeElement;if(e?.getAttribute('data-tt-command')!=="+json.dumps(command)+")return null;let r=e.getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()")
                if not rect: raise RuntimeError('control not reached with real Tab keys: '+command)
            p=wintypes.POINT(0,0);u.ClientToScreen(h,ctypes.byref(p));await click_screen(p.x+rect[0],p.y+rect[1])
        rows=[]
        citations={}
        for spec in ('01_application_shell.md','02_drawing.md','10_data_model_project_format.md'):
            heading=''
            for number,line in enumerate((ROOT/'spec'/spec).read_text(encoding='utf-8').splitlines(),1):
                if line.startswith('#'): heading=line.lstrip('# ').strip()
                if line.startswith('| `'):
                    command=line.split('`')[1]
                    citations[command]='spec/'+spec+' section '+heading+' line '+str(number)
        actions=[('Flatten','draw.flatten.apply',['Drawing','Flatten']),('Arrange','draw.arrange.apply',['Drawing','Arrange','Send to Back']),('Flip','draw.flip.apply',['Drawing','Flip Horizontal']),('Group','draw.group.create',['Drawing','Group']),('Cut Special','edit.cutSpecial',['Edit','Cut Special']),('Project Info','project.info.get',['File','Project Info']),('Check Project','doc.validate',['File','Check Project']),('Open Backup','project.backup.open',['File','Open Backup']),('About','app.info.get',['Help','About Toontown']),('Grid','drawAid.grid.set',None),('Symmetry','tool.symmetry.set',None),('Brush','tool.active.set',None),('Select All','edit.selectAll','key'),('Brush gesture','draw.stroke.add','gesture')]
        await cmd('workspace.select',{'name':'Drawing'});await cmd('view.active.set',{'viewId':'drawingView'})
        work=tempfile.TemporaryDirectory(prefix='tt-review-parity-')
        await cmd('prefs.set',{'values':{'files.backups.count':5}})
        await cmd('project.saveAs',{'path':str(Path(work.name)/'Parity.animpkg')})
        await cmd('draw.rectangle.add',{'from':{'x':-50,'y':-45},'to':{'x':35,'y':25},'layer':'new'})
        await cmd('project.save')
        for name,command,path in actions:
            citation=citations[command]
            row={'action':name,'expectedCommand':command,'spec':citation+'; D-25; D-30'}
            start_check=len(run.transcript)
            try:
                native_trace.clear()
                added=await cmd('draw.rectangle.add',{'from':{'x':-90,'y':-65},'to':{'x':60,'y':40},'layer':'new'})
                await cmd('draw.selection.set',{'by':'all'})
                if name=='Arrange': await cmd('draw.selection.set',{'by':'ids','ids':added['strokeIds']})
                if name=='Select All': await cmd('draw.selection.set',{'by':'none'})
                if name=='Open Backup': await cmd('project.save')
                await js('window.reviewIPC=[]')
                row['before']=await cmd('doc.export.json',{'scope':'project','binaries':'omit'})
                row['beforeSelection']=await cmd('draw.selection.get')
                if path=='key':
                    u.keybd_event(0x11,0,0,0);m.c42.os_key(0x41);u.keybd_event(0x11,0,2,0)
                elif path=='undo':
                    u.keybd_event(0x11,0,0,0);m.c42.os_key(0x5A);u.keybd_event(0x11,0,2,0)
                elif path=='gesture':
                    await cmd('tool.active.set',{'toolId':'brush'})
                    place=await m.locate_view(s,rc,'drawingView')
                    mapping=(await cmd('draw.viewMapping.get',{'view':'drawingView'}))['drawingToView']
                    pts=[m.c42.apply(mapping,*p) for p in ((-60,10),(-20,35),(30,5))]
                    times={};await asyncio.to_thread(m.os_drag,m.e2e.APP_PID['pid'],[(place[0]+x,place[1]+y) for x,y in pts],.5,.1,times)
                    row['osTimes']=times
                elif path: await menu(path)
                else: await dom(command)
                if name=='Open Backup':
                    await asyncio.sleep(.5)
                    await dom('project.backup.open')
                await asyncio.sleep(.7)
                row['ipc']=await js('window.reviewIPC');row['tree']=await m.e2e.tree(s)
                row['nativeMenuTrace']=list(native_trace)
                if isinstance(path,list):
                    row['inspectExposesMenuItem']=any(has_menu_command(t['treeWhileOpen'],command) for t in native_trace[:-1])
                    run.check(row['inspectExposesMenuItem'],'spec/11_agent_interface_mcp.md section 7.5 line 339: open menu widget exposes command '+command,row['inspectExposesMenuItem'])
                row['after']=await cmd('doc.export.json',{'scope':'project','binaries':'omit'})
                row['history']=await cmd('edit.history.get',{'limit':1000})
                row['afterSelection']=await cmd('draw.selection.get')
                run.shot(name.lower().replace(' ','-'),m.e2e.APP_PID['pid'])
                observed=[json.loads(e['args'].get('json','{}')).get('command') for e in row['ipc'] if e['command']=='run_command']
                if path=='gesture':
                    camera=load('parity_canvas','scripts/c43-camera-e2e.py')
                    row['gesture']=await camera.canvas_summary(s,'drawingView')
                    observed += [c['commandId'] for c in (row['gesture'].get('lastGesture') or {}).get('commands',[])]
                row['observed']=observed
                run.check(command in observed,'D-25 expected '+command+' from '+name+'; '+citation,observed)
                values=[json.loads(e['args'].get('json','{}')) for e in row['ipc'] if e['command']=='run_command']
                expected={'Arrange':{'to':'back'},'Flip':{'axis':'horizontal'},'Brush':{'toolId':'brush'}}.get(name,{})
                row['expectedParams']=expected
                matching=[v for v in values if v.get('command')==command and all(v.get('params',{}).get(k)==x for k,x in expected.items())]
                if path=='gesture': matching=(row['gesture'].get('lastGesture') or {}).get('commands',[])
                run.check(bool(matching),'D-23 expected parameters '+json.dumps(expected)+' for '+name+'; '+citation,values)
                if path=='gesture':
                    strokes=[c for c in matching if c.get('commandId')=='draw.stroke.add']
                    run.check(bool(strokes) and strokes[0].get('params',{}).get('kind')=='brush','02:1207,1318 Brush gesture sends kind brush',strokes)
                if name in ('Flatten','Arrange','Flip','Group','Cut Special','Brush gesture','Open Backup'):
                    row['documentChanged']=canonical(row['before'])!=canonical(row['after'])
                    run.check(row['documentChanged'],'document effect '+name,row['documentChanged'])
                if name=='Select All':
                    run.check(not row['beforeSelection'].get('bounds') and bool(row['afterSelection'].get('bounds')),'02:1329 Select All selects previously empty selection',row['afterSelection'])
                if name in ('Project Info','Check Project','About'):
                    run.check(canonical(row['before'])==canonical(row['after']),'query leaves document unchanged; '+citation,None)
                    root=(row['tree'].get('result') or {}).get('root') or {}
                    ident={'Project Info':'projectInfo','Check Project':'checkProject','About':'about'}[name]
                    run.check(m.e2e.by_id(root,ident) is not None,'expected '+name+' dialog '+ident+'; '+citation,None)
                label={'Flatten':'Flatten Drawing','Arrange':'Send to Back','Flip':'Flip Horizontal','Group':'Group Selection','Cut Special':'Cut Special','Brush gesture':'Brush Tool'}.get(name)
                if label:
                    got=row['history']['entries'][-1]['label'];row['expectedLabel']=label
                    run.check(got==label,'D-23 expected undo label '+label+'; '+citation,got)
                row['verdict']='PASS' if command in observed else 'FAIL'
            except Exception as ex:
                row['error']=repr(ex);row['verdict']='FAIL';run.check(False,'parity action completed '+name,repr(ex))
            finally:
                m.c42.os_key(0x1B);await asyncio.sleep(.2)
                await js("document.querySelector('[role=dialog] button')?.focus();true")
                # Dismiss dialogs with a real Escape; retain their inspected tree above.
                m.c42.os_key(0x1B)
            row['checks']=run.transcript[start_check:]
            row['verdict']='PASS' if all(c.get('ok',True) for c in row['checks']) else 'FAIL'
            rows.append(row);run.save('rows.json',rows)
        await cmd('project.close',{'onUnsaved':'discard'})
        await asyncio.sleep(.5)
        phase='disabled'
        for row,(_,command,path) in zip(rows,actions):
            name=row['action']
            if isinstance(path,list):
                row['nativeNoProject']=native_state(path)
                native_trace.clear();await menu(path[:-1])
            row['disabledTree']=await m.e2e.call(s,'inspect_ui',{'target':'window','depth':32})
            m.c42.os_key(0x1B);m.c42.os_key(0x1B);m.c42.os_key(0x1B)
            if command in ('app.info.get','project.backup.open'):
                row['disabledState']='Not applicable: globally available under spec/01_application_shell.md section 1.3 lines 71-75'
                run.save('rows.json',rows);continue
            params={'draw.arrange.apply':{'to':'back'},'draw.flip.apply':{'axis':'horizontal'},'tool.active.set':{'toolId':'brush'},'tool.symmetry.set':{'mode':'mirror'},'drawAid.grid.set':{'visible':True},'draw.stroke.add':{'kind':'brush','points':[{'x':0,'y':0},{'x':10,'y':5}]}}.get(command,{})
            expression="window.__TAURI_INTERNALS__.invoke('availability',{json:"+json.dumps(json.dumps({'refs':[{'commandId':command,'params':params}]}))+"})"
            # Runtime read only: retain the kernel's precheck result beside the inspected tree.
            # Promise evaluation is awaited by CDP below.
            seq+=1;await ws.send(json.dumps({'id':seq,'method':'Runtime.evaluate','params':{'expression':expression,'returnByValue':True,'awaitPromise':True}}))
            while True:
                result=json.loads(await ws.recv())
                if result.get('id')==seq: break
            row['noProjectAvailability']=result
            before=await cmd('doc.state.get')
            refusal=await rc(s,command,params)
            after=await cmd('doc.state.get')
            row['noProjectInvocation']={'response':refusal,'unchanged':before==after}
            run.check(refusal.get('status')=='error' and (refusal.get('error') or {}).get('code')=='project.none','spec/01_application_shell.md section 1.3 lines 71-75: '+command+' refuses project.none without project',refusal.get('error'))
            if refusal.get('status')=='error': run.check(before==after,'refused command changes no document '+command,None)
            run.save('rows.json',rows)
        work.cleanup()

def case7():
    app=str(Path(sys.argv[1]).resolve()); head=str(ROOT/'target/release/toontown-headless.exe')
    out=OUT/'case7';out.mkdir(parents=True,exist_ok=True)
    results=[]
    jobs=[('measure-'+b,['scripts/c43-camera-measure.py',app,str(out/('measure-'+b)),'--backend',b,'--bounds','workstation','--drags','30']) for b in ('webgl2','webgpu')]
    jobs += [('one-drag-gate',['scripts/c43-camera-measure.py',app,str(out/'one-drag-gate'),'--backend','webgl2','--bounds','workstation','--drags','1'])]
    jobs += [('c41',['scripts/c41-gui-e2e.py',app,str(out/'c41')]),('c42',['scripts/c42-gui-e2e.py',app,str(out/'c42')]),('byte',['scripts/c42-byte-gate.py',app,head,str(out/'byte')])]
    jobs += [(n,['tests/adversarial/'+f,app]+args) for n,f,args in [('r1','c42b-r1-live.py',['c2']),('r2','c42b-r2-live.py',['selection',head]),('r3','c42b-r3-live.py',['endings',head]),('r4','c42b-r4-live.py',['named',head])]]
    jobs += [(n,['tests/adversarial/'+n+'.py',app]) for n in ('c42b-r1s-selection','c42b-r1s-generation','c42b-r1s-experiments','c42b-r1-conformance')]
    jobs += [('r1-'+mode,['tests/adversarial/c42b-r1-live.py',app,mode]) for mode in ('c3','d2','d2-exact-attached','d2-exact-headless','d3-webgl2')]
    jobs += [('r2-'+mode,['tests/adversarial/c42b-r2-live.py',app,mode,head]) for mode in ('c2','d3-webgl2','gui','regression','byte','exact','exact-select','exact-tools')]
    jobs += [('r3-labels',['tests/adversarial/c42b-r3-live.py',app,'labels',head])]
    if len(sys.argv)>3:
        jobs=[j for j in jobs if j[0]==sys.argv[3]]
        if not jobs: raise SystemExit('unknown Case 7 check: '+sys.argv[3])
    for name,args in jobs:
        try:
            if args[0].startswith('tests/adversarial/'):
                source_path=str(ROOT/args[0])
                args=[str(ROOT/'tests/adversarial/c43b-r1-regression-runner.py'),source_path]+args[1:]
            p=subprocess.run([sys.executable]+args,cwd=ROOT,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=530)
            (out/(name+'.txt')).write_text(p.stdout+p.stderr,encoding='utf-8',newline='\n')
            results.append({'name':name,'exit':p.returncode})
            if name=='one-drag-gate':
                results[-1]['builderExit']=p.returncode
                results[-1]['assertion']='D-33 workstation gate must reject fewer than 30 samples per scene'
                results[-1]['exit']=0 if p.returncode!=0 else 1
            if name.startswith('measure-'):
                receipt=json.loads((out/name/'measurements.json').read_text(encoding='utf-8'))
                r=receipt.get('record',{})
                validations=[{'assertion':'D-33 requested backend ran','pass':r.get('backend')==name[8:],'requested':name[8:],'actual':r.get('backend')}]
                for scene in ('sample','groups1k'):
                    data=r.get(scene,{})
                    validations.append({'assertion':'D-33 30 actual commits '+scene,'pass':data.get('commits',{}).get('n',0)>=30})
                results[-1]['independentChecks']=validations
                if any(not v['pass'] for v in validations):results[-1]['exit']=1
        except subprocess.TimeoutExpired as ex:
            results.append({'name':name,'timeout':530})
        (out/(name+'-result.json')).write_text(json.dumps(results[-1],indent=2)+'\n',encoding='utf-8',newline='\n')
        print(results[-1],flush=True)
    sys.exit(1 if any(r.get('exit')!=0 for r in results) else 0)

def case6():
    import inspect
    m=load('review_e6','scripts/c43-e6-coexistence.py')
    body=inspect.getsource(m.session_run)
    body=body.replace('        run.save("tree-final.json"', '        await extra_coexistence(session,run,out,bob,REVIEW_E6)\n        run.save("tree-final.json"')
    body=body.replace('        env = await rc(session, "edit.undo", {"ifTop": agent_step})', '        protected=canonical((await rc(session,"doc.export.json",{"scope":"project","binaries":"omit"})).get("result"))\n        env = await rc(session, "edit.undo", {"ifTop": agent_step})')
    body=body.replace('        hist = await history()\n        after =', '        surviving=canonical((await rc(session,"doc.export.json",{"scope":"project","binaries":"omit"})).get("result"))\n        run.check(protected==surviving,"11:587 refused ifTop preserves entire document",None)\n        run.save("protected-document.json",surviving)\n        hist = await history()\n        after =')
    m.canonical=canonical
    m.extra_coexistence=extra_coexistence; m.REVIEW_E6=m
    exec(compile(body,__file__,'exec'),m.__dict__)
    app=str(Path(sys.argv[1]).resolve())
    sys.argv=['e6',app,str(OUT/'case6-camera')]
    first=m.main()
    body=body.replace('cameraView','drawingView').replace('"transform"','"brush"')
    body=body.replace('await rc(session, "anim.tool.set", {"tool": "brush"})','await rc(session, "workspace.select", {"name":"Drawing"})\n        await rc(session, "tool.active.set", {"toolId":"brush"})')
    # The drawing variant still queues the agent transform; only the user's tool changes.
    exec(compile(body,__file__,'exec'),m.__dict__)
    sys.argv=['e6',app,str(OUT/'case6-drawing')]
    second=m.main()
    labels=[]
    for name,expected in [('camera',['Move Element']*3),('drawing',['Brush Tool','Move Element','Brush Tool'])]:
        hist=json.loads((OUT/('case6-'+name)/'history.json').read_text(encoding='utf-8'))
        got=[e['label'] for e in hist['entries'][-3:]]
        labels.append({'view':name,'spec':'spec/05_scene_planning.md:1135; spec/02_drawing.md:1318','expected':expected,'observed':got,'pass':got==expected})
    (OUT/'case6-labels.json').write_text(json.dumps(labels,indent=2)+'\n',encoding='utf-8',newline='\n')
    first=max(first,int(any(not r['pass'] for r in labels)))
    sys.exit(max(first,second))

async def extra_coexistence(s,run,out,bob,m):
    import asyncio,threading,time
    rc=m.e2e.run_cmd
    async def cmd(n,p=None): return (await rc(s,n,p or {})).get('result') or {}
    for view in ('cameraView','drawingView'):
        await cmd('workspace.select',{'name':'Default' if view=='cameraView' else 'Drawing'})
        await cmd('view.active.set',{'viewId':view})
        await cmd('app.context.set',{'elementId':bob,'frame':1})
        await cmd('view.zoom.set',{'viewId':view,'zoom':.6})
        await cmd('anim.tool.set',{'tool':'select'}) if view=='cameraView' else await cmd('tool.active.set',{'toolId':'brush'})
        for variant in ('read','queue','escape','pointer'):
            await cmd('edit.selection.set',{'context':'staging','ids':[bob]})
            await asyncio.sleep(.6)
            place=await m.locate_view(s,rc,view)
            mapping=(await cmd('draw.viewMapping.get',{'view':view}))['drawingToView']
            pts=[m.c42.apply(mapping,*p) for p in ((60,40),(85,55))]
            pts=[(place[0]+x,place[1]+y) for x,y in pts]
            before=await cmd('edit.history.get',{'limit':1000})
            times={}; th=threading.Thread(target=m.os_drag,args=(m.e2e.APP_PID['pid'],pts,.3,2.5,times));th.start()
            while 'press' not in times and th.is_alive(): await asyncio.sleep(.02)
            await asyncio.sleep(.5)
            tasks=[]; reads=None
            if variant=='read':
                reads=await cmd('doc.export.json',{'scope':'project','binaries':'omit'})
                run.check('release' not in times and bool(reads),'busy note section 2 queries never blocked '+view,times)
            elif variant=='pointer':
                tasks=[asyncio.create_task(m.e2e.call(s,'pointer',{'view':view,'events':m.c42.drag([(100,100),(120,110)])}))]
                await asyncio.sleep(.2)
                run.check(not tasks[0].done(),'busy note section 2 pointer waits during user drag '+view,None)
            else:
                tasks=[asyncio.create_task(rc(s,'anim.transform.set',{'nodeIds':[bob],'target':'static','position':{'x':v,'y':0}})) for v in ((.7,.9) if variant=='queue' else (.8,))]
                await asyncio.sleep(.2)
                run.check(all(not t.done() for t in tasks),'11:586 busy mutation waits '+view+' '+variant,None)
                if variant=='escape': m.c42.os_key(0x1B)
            responses=await asyncio.gather(*tasks)
            await asyncio.to_thread(th.join);await asyncio.sleep(.6)
            after=await cmd('edit.history.get',{'limit':1000})
            delta=after['position']-before['position']
            if variant in ('read','escape','queue'):
                run.check(delta=={'read':1,'escape':1,'queue':3}[variant],'busy note section 5 full history count '+view+' '+variant,delta)
            doc=await cmd('doc.export.json',{'scope':'project','binaries':'omit'})
            run.save(view+'-'+variant+'.json',{'times':times,'before':before,'after':after,'responses':responses,'document':doc})
            run.check('error' not in times and all(r.get('status')=='ok' for r in responses),'11:586 waiting calls succeed '+view+' '+variant,responses)
            if variant=='queue':
                obj=(await cmd('doc.object.get',{'ref':bob}))['object']
                run.check(obj['transform']['offset']['x']==.9,'busy note section 5 no reorder queued positions .7 then .9 '+view,obj)
                await cmd('edit.undo')
                middle=(await cmd('doc.object.get',{'ref':bob}))['object']
                run.check(middle['transform']['offset']['x']==.7,'busy note section 5 preceding queued position is .7 '+view,middle)
                await cmd('edit.redo')
        # Reverse overlap: user input during an already running atomic mutation.
        await cmd('edit.selection.set',{'context':'staging','ids':[bob]})
        await asyncio.sleep(.4)
        place=await m.locate_view(s,rc,view)
        mapping=(await cmd('draw.viewMapping.get',{'view':view}))['drawingToView']
        pts=[m.c42.apply(mapping,*p) for p in ((60,40),(85,55))]
        pts=[(place[0]+x,place[1]+y) for x,y in pts]
        import time as walltime
        from ctypes import wintypes
        import ctypes
        u=ctypes.windll.user32;h=m._main_window(m.e2e.APP_PID['pid'])
        u.SetForegroundWindow(h);origin=wintypes.POINT(0,0);u.ClientToScreen(h,ctypes.byref(origin))
        u.SetCursorPos(int(origin.x+pts[0][0]),int(origin.y+pts[0][1]));await asyncio.sleep(.3)
        times={}
        reverse_start=(await cmd('edit.history.get',{'limit':1000}))['position']
        steps=[{'id':'draw.rectangle.add','params':{'from':{'x':-100+i*.01,'y':-70},'to':{'x':100,'y':70}}} for i in range(150)]
        def reverse_input():
            walltime.sleep(.03);m._send_button(True);times['press']=time.monotonic()
            walltime.sleep(.3);u.SetCursorPos(int(origin.x+pts[1][0]),int(origin.y+pts[1][1]));walltime.sleep(1)
            times['release']=time.monotonic();m._send_button(False)
        times['send']=time.monotonic()
        task=asyncio.create_task(m.e2e.call(s,'batch',{'steps':steps,'label':'Review long mutation','atomic':True}))
        th=threading.Thread(target=reverse_input); th.start()
        response=await task;times['answer']=time.monotonic()
        await asyncio.to_thread(th.join);await asyncio.sleep(.5)
        run.save(view+'-reverse.json',{'times':times,'response':response,'history':await cmd('edit.history.get',{'limit':1000}),'document':await cmd('doc.export.json',{'scope':'project','binaries':'omit'})})
        run.check('press' in times and times['send']<times['press']<times['answer'],'busy note section 2 reverse overlap occurred '+view,times)
        run.check(response.get('status')=='ok' and 'error' not in times,'busy note section 2 user input while mutation completes '+view,response.get('error'))
        now=(await cmd('edit.history.get',{'limit':1000}))['position']
        run.check(now>=reverse_start+2,'busy note section 2 reverse overlap preserves agent and user steps '+view,{'before':reverse_start,'after':now})
        for _ in range(now-reverse_start): await cmd('edit.undo')
        control_start=(await cmd('edit.history.get',{'limit':1000}))['position']
        control_times={}
        await asyncio.to_thread(m.os_drag,m.e2e.APP_PID['pid'],pts,.3,.3,control_times)
        await asyncio.sleep(.6)
        control_history=await cmd('edit.history.get',{'limit':1000})
        run.save(view+'-reverse-control.json',{'times':control_times,'before':control_start,'history':control_history})
        run.check(control_history['position']==control_start+1,'reverse control identical OS drag commits without concurrent batch '+view,control_history)

if __name__ == '__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    # A direct cargo release build uses the development URL. Reuse the built files,
    # and own the helper process so each subcommand can run in isolation.
    if sys.argv[2] in ('case3','case4','case5','case6','case7','case8'):
        import socket,atexit,time
        sock=socket.socket(); listening=sock.connect_ex(('127.0.0.1',1420))==0; sock.close()
        if not listening:
            server=subprocess.Popen([sys.executable,'-m','http.server','1420','--bind','127.0.0.1','--directory',str(ROOT/'desktop/dist')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW if os.name=='nt' else 0)
            atexit.register(server.terminate)
            time.sleep(.5)
    {'case1-vite':case1_vite,'case3':case3,'case4':case4,'case5':case5,'case6':case6,'case7':case7,'case8':case8}[sys.argv[2]]()
