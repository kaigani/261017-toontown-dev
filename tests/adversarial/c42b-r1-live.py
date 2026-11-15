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
        env=await rc('render.frame.run',{'composition':'activeView','background':'#404040'})
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
        found=(await rc('doc.object.find',{'type':'swatch','name':'Colour '+str(i+2)}))['result']
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
        await shot('c3-6-held')
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
    p=e.launch(str(Path(sys.argv[1]).resolve()),user,disc)
    async def connect(d):
        async with e.httpx2.AsyncClient(headers={'Authorization':'Bearer '+d['token']},timeout=300) as client:
            async with e.streamable_http_client(d['endpoint'],http_client=client) as streams:
                async with e.ClientSession(streams[0],streams[1]) as s:
                    await s.initialize();await main(s,sys.argv[2])
    try: asyncio.run(connect(e.wait_discovery(disc,p)))
    finally: e.stop(p);shutil.rmtree(user)
if __name__=='__main__': run()
