"""Reviewer Windows OS-input probes. Run in the desktop gui job, with an absolute binary path."""
import sys
if len(sys.argv) < 2:
    print('skipped: no application')
    sys.exit(0)
import asyncio, base64, ctypes, hashlib, importlib.util, json, shutil, subprocess, tempfile, time
from pathlib import Path
R = Path(__file__).resolve().parents[2]
O = R/'reviews/evidence/M4-C4.2b-r1'
def load(name,path):
    s=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
c=load('c42',R/'scripts/c42-gui-e2e.py');e=c.e2e
def save(name,obj): (O/name).write_text(json.dumps(obj,indent=2),encoding='utf-8')
async def settle(s):
    end=time.monotonic()+1
    while time.monotonic()<end: await c.canvas_state(s)
def hold(points):
    u=ctypes.windll.user32
    original=u.mouse_event
    u.mouse_event=lambda flags,*args: None if flags==4 else original(flags,*args)
    try: c.os_drag(e.APP_PID['pid'],points)
    finally: u.mouse_event=original
def release(): ctypes.windll.user32.mouse_event(4,0,0,0,0)
async def main(s,part):
    rc=lambda id,p={}:e.run_cmd(s,id,p)
    await rc('project.create',{'name':'Review','onUnsaved':'fail'})
    await rc('workspace.select',{'name':'Drawing'})
    el=(await rc('element.create',{'type':'drawing','name':'Ink'}))['result']['elementIds'][0]
    dr=(await rc('element.drawing.create',{'elementId':el,'name':'1','exposeFrom':1,'exposeTo':1}))['result']['drawingId']
    await rc('app.context.set',{'elementId':el,'frame':1})
    sw=(await rc('doc.object.find',{'type':'swatch','name':'Colour 2'}))['result']
    items=sw.get('items',sw.get('matches',[]));black=items[0].get('id') or items[0].get('key')
    await rc('colour.current.set',{'colourId':black})
    await rc('draw.rectangle.add',{'from':{'x':-120,'y':-80},'to':{'x':120,'y':80}})
    await settle(s)
    mapping=(await rc('draw.viewMapping.get',{'view':'drawingView'}))['result']
    save(part+'-mapping.json',mapping)
    # Fresh private default workspace is identical to the builder's located capture.
    x0,y0=49,126
    async def state():
        return {k:(await rc(cid)) for k,cid in [('history','edit.history.get'),('drawing','draw.drawing.get'),('selection','draw.selection.get'),('activity','agent.activity.list')]}
    if part.startswith('d'):
        return await part_d(s,part,rc,state,mapping,dr)
    if part=='c2':
        rows=[]
        for tool in ['brush','pencil','rectangle','ellipse','line','select','eraser','paint']:
            await rc('tool.active.set',{'toolId':tool})
            before=await state()
            m=mapping['drawingToView']; v=lambda x,y:c.apply(m,x,y)
            points=[v(-60,0),v(-20,20),v(20,0),v(60,0)]
            if tool in ['rectangle','ellipse']: points=[v(-60,-40),v(-20,-10),v(20,20),v(60,40)]
            if tool=='select': points=[v(-120,0),v(-120,0)]
            if tool=='paint': points=[v(0,0),v(0,0)]
            c.os_drag(e.APP_PID['pid'],[(x+x0,y+y0) for x,y in points])
            await settle(s)
            after=await state()
            rows.append({'tool':tool,'sentViewPoints':points,'before':before,'after':after,'canvas':await c.canvas_state(s),'verdict':'Equivalence unverifiable until collected samples exposed'})
            save('c2-gestures.json',rows)
        save('c2-tree.json',await e.tree(s))
        await rc('tool.active.set',{'toolId':'brush'})
        before=await state()
        hold([(x0+150,y0+150),(x0+250,y0+180)])
        try: e.os_escape(e.APP_PID['pid'])
        finally: release()
        await settle(s)
        after=await state()
        unchanged=all(before[k]==after[k] for k in ['drawing','history','selection'])
        save('c2-escape.json',{'before':before,'after':after,'unchanged':unchanged})
        # Plan section 5 recorded-webview equivalence needs the actual collected samples.
        def events(v):
            if isinstance(v,dict):
                if isinstance(v.get('events'),list) and v['events']: return v['events']
                for x in v.values():
                    if found:=events(x): return found
            if isinstance(v,list):
                for x in v:
                    if found:=events(x): return found
            return None
        assert events(rows), 'C42B-R1-03: no collected OS gesture samples in canvas/history/activity/drawing receipts'
        return
    async def shot(name,target='drawingView',region=None):
        args={'target':target}
        if region: args['region']=region
        env=await e.call(s,'screenshot',args)
        r=env.get('result') or {};data=base64.b64decode(r.pop('png',''))
        if data:
            path=O/(name+'.png');path.write_bytes(data);e.strip_png(path)
        save(name+'.json',env)
        return data,r
    async def pair(name):
        await settle(s)
        a,meta=await shot(name+'-capture')
        env=await rc('render.frame.run',{'composition':'activeView','background':'#F0F0F0'})
        rr=env.get('result') or {}; b=base64.b64decode(rr.pop('png',rr.pop('data','')))
        if b:
            p=O/(name+'-final.png');p.write_bytes(b);e.strip_png(p)
        save(name+'-render.json',env)
        if a and b:
            wa,ha,ca,ar=c.png_rgba(a);wb,hb,cb,br=c.png_rgba(b)
            if (wa,ha)==(wb,hb):
                diff=sum(max(abs(ar[y][x*ca+i]-br[y][x*cb+i]) for i in range(3))>32 for y in range(ha) for x in range(wa))
                save(name+'-comparison.json',{'size':[wa,ha],'threshold':32,'different':diff,'fraction':diff/(wa*ha)})
        return a,b
    for i,y in enumerate([-50,0,50]):
        found=(await rc('doc.object.find',{'type':'swatch','name':'Colour '+str(i+22)}))['result']
        its=found.get('items',found.get('matches',[]))
        if its: await rc('colour.current.set',{'colourId':its[0].get('id') or its[0].get('key')})
        result=await rc('draw.stroke.add',{'kind':'pencil','thickness':20,'points':[{'x':-160,'y':y,'pressure':1},{'x':160,'y':y+20,'pressure':1}],'fit':False})
        save('c3-stroke-'+str(i)+'.json',result)
    await rc('colour.paint.apply',{'points':[{'x':0,'y':30}]})
    await pair('c3-1')
    await rc('view.zoom.set',{'viewId':'drawingView','zoom':2})
    await rc('view.pan.set',{'viewId':'drawingView','by':{'x':30,'y':20}})
    await pair('c3-2')
    await rc('view.rotation.set',{'viewId':'drawingView','degrees':30})
    await pair('c3-3')
    await rc('view.transform.reset',{'viewId':'drawingView'})
    other=(await rc('element.drawing.create',{'elementId':el,'name':'2','exposeFrom':2,'exposeTo':2}))['result']['drawingId']
    await rc('app.context.set',{'elementId':el,'frame':2})
    await rc('draw.ellipse.add',{'from':{'x':-220,'y':-150},'to':{'x':-140,'y':150}})
    await rc('app.context.set',{'elementId':el,'frame':1})
    await rc('drawAid.onionSkin.set',{'enabled':True,'previous':1,'next':1})
    await pair('c3-4')
    await rc('drawAid.grid.set',{'visible':True})
    await pair('c3-5')
    await rc('drawAid.grid.set',{'visible':False})
    await rc('tool.active.set',{'toolId':'brush'})
    await shot('c3-6-before')
    hold([(x0+100,y0+100),(x0+220,y0+140)])
    try:
        await settle(s)
        await pair('c3-6-held')
    finally: release()
    await pair('c3-6-after')
    await shot('c3-7-before','window')
    await rc('ui.preferences.open',{'category':'interface'})
    await settle(s)
    await shot('c3-7-dialog','window')
    e.os_escape(e.APP_PID['pid']);await settle(s)
    full,meta=await shot('c3-8-full')
    reg,rm=await shot('c3-8-region',region={'x':20,'y':20,'w':100,'h':80})
    cap=await e.call(s,'screenshot',{'target':'drawingView','region':{'x':0,'y':0,'w':2049,'h':10}})
    save('c3-8-cap.json',cap)
    save('c3-8-hash.json',{'reported':meta.get('sha256'),'actual':hashlib.sha256(full).hexdigest(),'meta':meta})
    save('c3-8-display.json',{'os':e.os_windows(e.APP_PID['pid']),'mapping':mapping})


async def part_d(s,part,rc,state,mapping,dr):
    rows=[]
    async def record(case,fn):
        try:
            data=await fn(); verdict=data.pop('verdict',None)
            if verdict is None:
                nested=[v.get('verdict') for v in data.values() if isinstance(v,dict) and 'verdict' in v]
                verdict=next((v for v in nested if v!='Pass'),'Pass')
        except Exception as ex: data={'error':repr(ex)};verdict='Inconclusive: probe error'
        rows.append({'case':case,'verdict':verdict,'observed':data});save(part+'-cases.json',rows)
    async def snap(name):
        env=await e.call(s,'screenshot',{'target':'drawingView'})
        data=base64.b64decode((env.get('result') or {}).pop('png',''))
        if data:
            path=O/(name+'.png');path.write_bytes(data);e.strip_png(path)
        return data
    async def pair(name):
        await settle(s);a=await snap(name)
        env=await rc('render.frame.run',{'composition':'activeView','background':'#F0F0F0'})
        r=env.get('result') or {};b=base64.b64decode(r.get('png',r.get('data','')))
        if not a or not b: return {'verdict':'Inconclusive: image missing','render':env}
        wa,ha,ca,ar=c.png_rgba(a);wb,hb,cb,br=c.png_rgba(b)
        if (wa,ha)!=(wb,hb): return {'verdict':'Inconclusive: dimensions differ','sizes':[[wa,ha],[wb,hb]]}
        diff=sum(max(abs(ar[y][x*ca+i]-br[y][x*cb+i]) for i in range(3))>32 for y in range(ha) for x in range(wa))/(wa*ha)
        return {'fraction':diff,'verdict':'Pass' if diff<.01 else 'Fail'}
    def events(k=0,n=4):
        m=mapping['drawingToView'];return c.drag([c.apply(m,-180+i*240/max(1,n-1),-140+k*12+((i%2)*5 if n<100 else 0)) for i in range(n)])
    async def pointer(tool='brush',ev=None): return await e.call(s,'pointer',{'view':'drawingView','tool':tool,'events':ev or events()})
    if part.startswith('d1-long-'):
        for k in range(100):
            await rc('draw.stroke.add',{'kind':'brush','points':[{'x':-600+i*2.4,'y':-400+k*8+(i%20)*.3,'pressure':.6} for i in range(500)],'fit':False})
        await settle(s)
        before=await c.canvas_state(s)
        responses=[]
        for _ in range(5):
            responses.append(await pointer(ev=events(n=9998)))
            await settle(s)
        env=responses
        await settle(s)
        after=await c.canvas_state(s)
        save(part+'.json',{'before':before,'response':env,'after':after})
        commits=(after.get('metrics') or {}).get('commits') or []
        assert commits and max(commits)<=150, 'C42B-R1-04: large long-stroke release-to-committed exceeds 150 ms'
        return
    if part=='d1-extra':
        end=time.monotonic()+90
        prefs={}
        while time.monotonic()<end:
            prefs=await rc('prefs.get',{'keys':['display.canvasBackendMeasured']})
            values=(prefs.get('result') or {}).get('values',{})
            if (values.get('display.canvasBackendMeasured') or {}).get('fingerprint'): break
        save('d1-auto.json',{'prefs':prefs,'canvas':await c.canvas_state(s)})
        return
    if part=='d2':
        async def equivalence(tools):
            out=[]
            for tool in tools:
                ev=events();before=await state();env=await pointer(tool,ev);after=await state();await snap('d2-'+tool)
                # A hidden replay in the same engine is supplementary; second-engine exact ids remain unverified.
                pos=lambda st:(st['history'].get('result') or {}).get('position',0)
                for _ in range(pos(after)-pos(before)): await rc('edit.undo')
                await rc('window.panel.set',{'panelId':'drawingView','visible':False});await settle(s)
                head=await pointer(tool,ev);hd=await state()
                for _ in range(pos(hd)-pos(before)): await rc('edit.undo')
                await rc('window.panel.set',{'panelId':'drawingView','visible':True});await settle(s)
                out.append({'tool':tool,'window':env,'headless':head,'contentEqualModuloIds':c.strip_ids(after['drawing'])==c.strip_ids(hd['drawing'])})
            return {'results':out,'verdict':'Inconclusive: second-engine exact replay not exercised'}
        await record(1,lambda:equivalence(['brush']))
        await record(2,lambda:equivalence(['rectangle','select']))
        async def cancel():
            before=await state();task=asyncio.create_task(pointer(ev=events(n=9000)))
            end=time.monotonic()+10
            while time.monotonic()<end:
                st=await c.canvas_state(s)
                if st.get('gesture') or task.done(): break
            hold([(249,326),(259,336)])
            try: env=await task;mid=await state()
            finally: e.os_escape(e.APP_PID['pid']);release()
            await settle(s);after=await state()
            ok=(env.get('error') or {}).get('code')=='ui.busy' and (env.get('error') or {}).get('details',{}).get('reason')=='userInput' and before['drawing']==after['drawing'] and before['history']==after['history']
            return {'response':env,'before':before,'after':after,'verdict':'Pass' if ok else 'Fail'}
        await record(3,cancel)
        async def busy():
            await rc('tool.active.set',{'toolId':'brush'});hold([(249,326),(259,336)])
            results=[]
            try:
                for name,fn in [('query',lambda:rc('draw.drawing.get')),('tool',lambda:rc('tool.active.set',{'toolId':'rectangle'})),('pointer',lambda:pointer())]:
                    t=time.monotonic();env=await fn();results.append({'name':name,'ms':(time.monotonic()-t)*1000,'env':env})
            finally: release()
            await settle(s);env=await pointer()
            ok=all((r['env'].get('error') or {}).get('code')=='ui.busy' for r in results[1:]) and env.get('status')=='ok'
            return {'results':results,'afterRelease':env,'verdict':'Pass' if ok else 'Fail'}
        await record(4,busy)
        async def hidden():
            await rc('window.panel.set',{'panelId':'drawingView','visible':False});await settle(s);before=await state();env=await pointer();after=await state()
            await rc('window.panel.set',{'panelId':'drawingView','visible':True});await settle(s)
            return {'response':env,'changed':before['drawing']!=after['drawing'],'verdict':'Pass' if env.get('status')=='ok' and before['drawing']!=after['drawing'] else 'Fail'}
        await record(5,hidden)
        async def twenty():
            before=await state();responses=[await pointer(ev=events(k)) for k in range(20)];after=await state();comparison=await pair('d2-twenty')
            delta=after['history']['result']['position']-before['history']['result']['position']
            return {'responses':responses,'historyDelta':delta,'comparison':comparison,'verdict':'Pass' if delta==20 and all(x.get('status')=='ok' for x in responses) and comparison['verdict']=='Pass' else 'Fail'}
        await record(6,twenty)
    else:
        async def floating():
            env=await rc('window.panel.set',{'panelId':'drawingView','floating':True,'geometry':{'x':100,'y':100,'width':900,'height':700}});await settle(s)
            return {'response':env,'comparison':await pair(part+'-float'),'windows':e.os_windows(e.APP_PID['pid'])}
        await record(1,floating)
        async def osstroke():
            before=await state();await rc('tool.active.set',{'toolId':'brush'});c.os_drag(e.APP_PID['pid'],[(200,200),(300,250)]);await settle(s);after=await state()
            return {'before':before,'after':after,'verdict':'Pass' if before['drawing']!=after['drawing'] else 'Fail'}
        await record(2,osstroke)
        async def agent():
            env=await pointer();return {'response':env,'canvas':await c.canvas_state(s),'verdict':'Pass' if env.get('status')=='ok' else 'Fail'}
        await record(3,agent)
        async def close():
            await rc('tool.active.set',{'toolId':'brush'});before=await state();hold([(200,200),(300,250)])
            try:
                # OS close bypasses agent-write coexistence, as a user closing the float does.
                from ctypes import wintypes
                u=ctypes.windll.user32;handles=[]
                def cb(h,_):
                    pid=wintypes.DWORD();u.GetWindowThreadProcessId(h,ctypes.byref(pid))
                    if pid.value==e.APP_PID['pid'] and u.IsWindowVisible(h): handles.append(h)
                    return True
                u.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(cb),0)
                u.PostMessageW(handles[0],0x10,0,0)
            finally: release()
            await settle(s);after=await state();env=await rc('tool.active.set',{'toolId':'brush'})
            ok=before['drawing']==after['drawing'] and before['history']==after['history'] and env.get('status')=='ok'
            return {'before':before,'after':after,'afterCall':env,'verdict':'Pass' if ok else 'Fail'}
        await record(4,close)
        async def dock():
            env=await rc('window.panel.set',{'panelId':'drawingView','visible':True,'floating':False});await settle(s);return {'response':env,'gesture':await pointer(),'comparison':await pair(part+'-dock')}
        await record(5,dock)
        async def backend(): return {'canvas':await c.canvas_state(s),'comparison':await pair(part+'-backend')}
        await record(6,backend)


def run():
    deadline=time.monotonic()+180
    while True:
        tasks=subprocess.check_output(['tasklist','/FI','IMAGENAME eq toontown.exe'],text=True)
        if 'toontown.exe' not in tasks: break
        if time.monotonic()>deadline: raise RuntimeError('other application still running')
    user=Path(tempfile.mkdtemp(prefix='tt-review-c42b-'));disc=user/'discovery.json'
    (user/'Settings').mkdir()
    seed={'agent.policy.defaults':{'policy':'autonomous','maxJobs':2,'maxBatchSteps':1000,'maxInlineImagePx':2048,'maxInlineBytes':52428800,'askInApp':True,'letAgentMoveView':True,'highlightAgentChanges':True}}
    (user/'Settings/preferences.json').write_text(json.dumps({'format':'toontown-settings','kind':'preferences','formatVersion':'1.0','data':seed}),encoding='utf-8')
    if sys.argv[2].startswith(('d3-','d1-long-')) or sys.argv[2]=='d1-extra':
        measure=load('measure',R/'scripts/c42-canvas-measure.py')
        p=measure.launch(str(Path(sys.argv[1]).resolve()),user,disc,None if sys.argv[2]=='d1-extra' else sys.argv[2].split('-')[-1])
    else: p=e.launch(str(Path(sys.argv[1]).resolve()),user,disc)
    async def connect(d):
        async with e.httpx2.AsyncClient(headers={'Authorization':'Bearer '+d['token']},timeout=300) as client:
            async with e.streamable_http_client(d['endpoint'],http_client=client) as streams:
                async with e.ClientSession(streams[0],streams[1]) as s:
                    await s.initialize();await main(s,sys.argv[2])
    try: asyncio.run(connect(e.wait_discovery(disc,p)))
    finally: e.stop(p);shutil.rmtree(user)
if __name__=='__main__': run()
