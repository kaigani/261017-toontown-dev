"""Independent C4.3b review probes. Usage: probe APP case1-vite."""
import sys
if len(sys.argv) < 2:
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
    body=body.replace('            e6.os_drag(e2e.APP_PID', '''            modifier=0x10 if name.startswith('constrain') else 0x11 if name=='temporary select' else 0x12 if name=='temporary skew' else None
            import ctypes
            if modifier: ctypes.windll.user32.keybd_event(modifier,0,0,0)
            e6.os_drag(e2e.APP_PID''')
    body=body.replace('            how = "os"', '''            if modifier: ctypes.windll.user32.keybd_event(modifier,0,2,0)
            how = "os"''')
    body=body.replace('        results.append(row)\n        run.save', '''        await review_expect(m=None, row=row, run=run, u=u, mapping=m, ids=ids, name=name, events=events, frame=8 if name=='keyed frame 8' else 1)
        run.check(how=='os','real OS input '+name,how)
        results.append(row)
        run.save''')
    body=body.replace('    return results', '''        restored=[(await e.result('doc.object.get',{'ref':ids['Bob']})).get('object') if family=='scene' else await e.result('draw.drawing.get',{}) for e in engines]
        row['undoRestored']=restored==before_content
        run.check(row['undoRestored'],'undo restores prior content '+name,None)
        run.save('rows.json',results)
    return results''')
    m.review_expect=review_expect
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
        expected={'spec':'02_drawing.md section 7.4; M4-design-gestures.md:92-94','command':'draw.line.add','label':'Line Tool'}
        ok=bool(cmd) and cmd[0]['commandId']=='draw.line.add' and row['labels']==[['Line Tool']]*3
    elif target=='key':
        ok=ok and p.get('frame')==frame
        if name=='edge': ok=ok and set(p.get('scale',{}))=={'x'}
    row['expected']=expected; row['specVerdict']='PASS' if ok else 'FAIL'
    run.check(ok,'D-23 stated command target channels label '+name,{'expected':expected,'observed':cmd,'labels':row['labels']})

if __name__ == '__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    {'case1-vite':case1_vite,'case3':case3,'case4':case4}[sys.argv[2]]()
