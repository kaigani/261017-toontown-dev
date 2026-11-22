"""Reviewer live runner. Original reviewer and builder files remain read-only.

Usage: c42b-r2-live.py APP MODE [HEADLESS]
Modes: c2, d3-webgl2, d3-webgpu, d1-long-webgl2, d1-long-webgpu,
gui, byte, regression, exact, exact-select, exact-tools, selection, observer,
measure-webgl2, measure-webgpu.
"""
import sys
if len(sys.argv) < 2:
    print('skipped: no application')
    sys.exit(0)
import importlib.util
import json
from pathlib import Path
import subprocess
import time
import asyncio
import base64
import ctypes
import math
import inspect

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M4-C4.2b-r2'

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def idle():
    deadline = time.monotonic() + 300
    while True:
        listing = subprocess.check_output(['tasklist', '/FI', 'IMAGENAME eq toontown.exe'], text=True)
        if 'toontown.exe' not in listing:
            return
        if time.monotonic() >= deadline:
            raise RuntimeError('other application still running; no launch attempted')

async def selection(m, s, part):
    c, e = m.c, m.e
    rc = lambda cid, p={}: e.run_cmd(s, cid, p)
    await rc('project.create', {'name':'Selection review','onUnsaved':'fail'})
    await rc('workspace.select', {'name':'Drawing'})
    eid = (await rc('element.create', {'type':'drawing','name':'Three'}))['result']['elementIds'][0]
    await rc('element.drawing.create', {'elementId':eid,'name':'1','exposeFrom':1,'exposeTo':1})
    await rc('app.context.set', {'elementId':eid,'frame':1})
    sw=(await rc('doc.object.find',{'type':'swatch','name':'Colour 2'}))['result']
    black=(sw.get('items') or sw.get('matches'))[0]
    await rc('colour.current.set',{'colourId':black.get('id') or black.get('key')})
    for x in (-250, -70, 110):
        await rc('draw.rectangle.add', {'from':{'x':x,'y':-70},'to':{'x':x+100,'y':30}})
    await rc('tool.active.set', {'toolId':'select'})
    await m.settle(s)
    eq = load('equivalence_helpers', ROOT/'scripts/c42-gesture-equivalence.py')
    wp = c.png_of(await rc('inspect.screenshot', {'target':'window'}))
    vp = c.png_of(await rc('inspect.screenshot', {'target':'drawingView'}))
    offset = eq.canvas_offset(wp, vp)
    assert offset is not None
    rows = []
    async def mapping():
        return (await rc('draw.viewMapping.get', {'view':'drawingView'}))['result']['drawingToView']
    async def gesture(points, shift=False):
        mat = await mapping()
        pts = [c.apply(mat, *p) for p in points]
        if shift: ctypes.windll.user32.keybd_event(0x10,0,0,0)
        try: c.os_drag(e.APP_PID['pid'], [(x+offset[0],y+offset[1]) for x,y in pts])
        finally:
            if shift: ctypes.windll.user32.keybd_event(0x10,0,2,0)
        await m.settle(s)
    async def capture(n):
        env = await e.call(s,'screenshot',{'target':'drawingView'})
        a = c.png_of(env)
        b = c.png_of(await rc('render.frame.run',{'composition':'activeView','background':'#F0F0F0'}))
        for label, data in [('capture',a),('final',b)]:
            p = m.O/f'{n}-{label}.png'
            p.write_bytes(data); e.strip_png(p)
            assert p.stat().st_size < 1000000
        sel = (await rc('draw.selection.get'))['result']
        box = sel.get('bounds')
        handles = [] if not box else c.handle_points([box['x'],box['y'],box['x']+box['width'],box['y']+box['height']],await mapping())
        w,h,ch,pixels = c.png_rgba(a)
        extents = []
        white_extents = []
        for x,y in handles:
            accent = [(dx,dy) for dy in range(-7,8) for dx in range(-7,8)
                      if 0<=round(x)+dx<w and 0<=round(y)+dy<h
                      and (q:=pixels[round(y)+dy][(round(x)+dx)*ch:(round(x)+dx)*ch+3])[2]>=180
                      and q[2]-q[0]>=60 and 90<=q[1]<=210]
            extents.append([max(t[i] for t in accent)-min(t[i] for t in accent)+1 for i in (0,1)] if accent else [0,0])
            white = [(dx,dy) for dy in range(-6,7) for dx in range(-6,7) if 0<=round(x)+dx<w and 0<=round(y)+dy<h and min(pixels[round(y)+dy][(round(x)+dx)*ch:(round(x)+dx)*ch+3])>=250]
            white_extents.append([max(t[i] for t in white)-min(t[i] for t in white)+1 for i in (0,1)] if white else [0,0])
        return {'selection':sel,'difference':c.differing(a,b),'handles':c.handles_in(a,handles,1),'finalHandles':c.handles_in(b,handles,1),'handleExtents':extents,'whiteCentreExtents':white_extents,'mappedHandles':handles,
                'record':(await c.canvas_state(s)).get('lastGesture')}
    async def record(n, data, ok):
        rows.append({'case':n,'verdict':'PASS' if ok else 'FAIL','observed':data})
        m.save('selection-cases.json',rows)
    await gesture([(-250,-20),(-250,-20)])
    a = await capture(1)
    await record(1,a,a['handles']==8 and a['finalHandles']==0 and a['difference']['different']>0)
    await gesture([(-70,-20),(-70,-20)],True)
    b = await capture(2)
    await record(2,b,b['selection'].get('bounds',{}).get('width',0)>200 and b['handles']==8)
    await gesture([(300,160),(300,160)])
    d = await capture(3)
    await record(3,d,not d['selection'].get('bounds') and d['difference']['fraction']<.01)
    await gesture([(-70,-20),(-70,-20)])
    await rc('view.zoom.set',{'viewId':'drawingView','zoom':2})
    await rc('view.rotation.set',{'viewId':'drawingView','degrees':30})
    await m.settle(s)
    z = await capture(4)
    await record(4,z,z['handles']==8 and z['finalHandles']==0)
    await rc('view.transform.reset',{'viewId':'drawingView'})
    await m.settle(s)
    drags = [([ (34,34),(80,80)],'scale'), ([(38,38),(-128,38)],'rotate'), ([(-20,-20),(0,-10)],'translate')]
    for n,(pts,key) in enumerate(drags,5):
        before = (await rc('edit.history.get'))['result']['position']
        box = (await rc('draw.selection.get'))['result']['bounds']
        if key=='rotate':
            cx,cy = box['x']+box['width']/2,box['y']+box['height']/2
            px,py = box['x']+box['width']+8,box['y']+box['height']+8
            pts = [(px,py),(cx-(py-cy),cy+(px-cx))]
        await gesture(pts)
        data = await capture(n)
        after = (await rc('edit.history.get'))['result']['position']
        data['historyDelta'] = after-before
        cmds = (data.get('record') or {}).get('commands') or []
        rec = data.get('record') or {}
        ev = rec.get('events') or []
        expected = None
        if ev:
            inv = eq.inverse(rec['drawingToView'])
            p,q = [c.apply(inv,t['x'],t['y']) for t in (ev[0],ev[-1])]
            if key=='scale':
                pivot = {'x':box['x'],'y':box['y']}
                expected = {'scale':{'x':(q[0]-pivot['x'])/(p[0]-pivot['x']),'y':(q[1]-pivot['y'])/(p[1]-pivot['y'])},'pivot':pivot}
            elif key=='rotate':
                pivot = {'x':box['x']+box['width']/2,'y':box['y']+box['height']/2}
                u,v = (p[0]-pivot['x'],p[1]-pivot['y']),(q[0]-pivot['x'],q[1]-pivot['y'])
                expected = {'rotate':math.degrees(math.atan2(u[0]*v[1]-u[1]*v[0],u[0]*v[0]+u[1]*v[1])),'pivot':pivot}
            else: expected = {'translate':{'x':q[0]-p[0],'y':q[1]-p[1]}}
        data['derivedParameters'] = expected
        match = any(x.get('commandId')=='draw.transform.apply' and expected and all(eq.close(x.get('params',{}).get(k),v) for k,v in expected.items()) for x in cmds)
        await record(n,data,after-before==1 and match)
        if after>before: await rc('edit.undo')
        await m.settle(s)
    rows.append({'case':8,'verdict':'PENDING exact mode','observed':{'note':'Exact three-engine comparisons run separately in exact mode; see its rows.'}})
    m.save('selection-cases.json',rows)
    assert all(r['verdict']!='FAIL' for r in rows), 'selection case failed; see selection-cases.json'

async def observer(m,s,part):
    c,e=m.c,m.e
    rc=lambda cid,p={}:e.run_cmd(s,cid,p)
    e.os_escape(e.APP_PID['pid'])
    def key(vk):
        ctypes.windll.user32.keybd_event(vk,0,0,0)
        ctypes.windll.user32.keybd_event(vk,0,2,0)
    key(0x75)  # F6, user shortcut opens the private fixture copy.
    deadline=time.monotonic()+30
    while time.monotonic()<deadline:
        st=(await rc('app.state.get')).get('result') or {}
        if st.get('project'):break
    assert st.get('project'),st
    key(0x76)  # F7, user shortcut selects Drawing workspace.
    await m.settle(s)
    key(0x78)  # F9, user shortcut chooses the fixture's exposed drawing.
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        state=await c.canvas_state(s)
        if state.get('drawings'):break
    assert state.get('drawings'),state
    key(0x77)  # F8, user shortcut selects Brush.
    await m.settle(s)
    mapping=(await rc('draw.viewMapping.get',{'view':'drawingView'}))['result']['drawingToView']
    pts=[c.apply(mapping,x,150) for x in (-100,-70,-40)]
    c.os_drag(e.APP_PID['pid'],[(x+49,y+126) for x,y in pts])
    await m.settle(s)
    st=(await rc('app.state.get'))['result']
    record=(await c.canvas_state(s)).get('lastGesture')
    m.save('read-only-samples.json',{'agent':st.get('agent'),'lastGesture':record})
    assert st['agent']['scopes']==['read'],st['agent']
    assert record and record.get('events'),record

def main():
    app, mode = str(Path(sys.argv[1]).resolve()), sys.argv[2]
    out = OUT / mode
    out.mkdir(parents=True, exist_ok=True)
    idle()
    started = time.monotonic()
    code = 0
    try:
        if mode.startswith('measure-'):
            m = load('measure_suite', ROOT/'scripts/c42-canvas-measure.py')
            sys.argv = ['measure',app,str(out),'--backend',mode.split('-')[-1],'--bounds','workstation','--long-samples','30']
            code = m.main()
        elif mode in ('gui', 'byte', 'regression', 'exact', 'exact-select', 'exact-tools'):
            file = {'gui':'c42-gui-e2e.py','byte':'c42-byte-gate.py',
                    'regression':'c41-gui-e2e.py','exact':'c42-gesture-equivalence.py',
                    'exact-select':'c42-gesture-equivalence.py','exact-tools':'c42-gesture-equivalence.py'}[mode]
            m = load('suite', ROOT / 'scripts' / file)
            if mode.startswith('exact'):
                # D-4 is exact, including identities. Do not accept builder tolerances.
                m.close = lambda a, b: a == b
                m.c42.strip_ids = lambda v: v
                if mode != 'exact':
                    source = inspect.getsource(m.main)
                    if mode == 'exact-select':
                        source = source.replace('    results = []', "    rows += [('select: move','select',[(cx,cy),(cx+25,cy-15)],[],True)]\n    results = []")
                    source = source.replace('            await engine.run("colour.current.set", {"colourId": black})', '            await engine.run("tool.active.set", {"toolId": tool})\n            await engine.run("colour.current.set", {"colourId": black})')
                    # OS records omit the optional tool override; replay that request shape too.
                    source = source.replace('"tool": rec["tool"], ', '')
                    if mode == 'exact-tools':
                        source = source.replace('vpts = [v((170, 100)), v((220, 140))]', 'vpts = [v((170, 100)), v((220, 140)), v((260,100))]')
                        source = source.replace('os_double_click(e2e.APP_PID["pid"], client(vpts[1]))', 'c42.os_drag(e2e.APP_PID["pid"], [client(vpts[2]), client(vpts[2])])\n                os_double_click(e2e.APP_PID["pid"], client(vpts[2]))')
                        source = source.replace('        # 2. Engine 2:', '''        if tool == "polyline":
            deadline = time.monotonic()+1
            state = await app.result("app.state.get", {})
            while state.get("modal") is not None and time.monotonic()<deadline:
                state = await app.result("app.state.get", {})
            row["afterDoubleClickModal"] = state.get("modal")
            run.check(state.get("modal") is None, "Polyline double-click ends the gesture (02 7.2.5:535)", state.get("modal"))
            if state.get("modal") is not None:
                e2e.os_escape(e2e.APP_PID["pid"])
                deadline = time.monotonic()+5
                while time.monotonic()<deadline and (await app.result("app.state.get",{})).get("modal") is not None:
                    pass
        # 2. Engine 2:''')
                        source = source.replace('        labels = [await engine.labels_since(p)', '        row["contents"] = contents\n        labels = [await engine.labels_since(p)')
                    condition = "name in ('select: corner scale','select: rotate','select: move')" if mode == 'exact-select' else "name in ('brush','pencil','rectangle','ellipse','line','polyline','eraser','paint')"
                    source = source.replace('for name, tool, pts, mods, selected in rows:',f'for name, tool, pts, mods, selected in [r for r in rows if (lambda name: {condition})(r[0])]:')
                    exec(compile(source,str(__file__),'exec'),m.__dict__)
                    if mode == 'exact-tools':
                        source = inspect.getsource(m.session_all)
                        old = 'return await main(Engine("window", a), heads[0], heads[1], run, out, work, sys.platform == "win32")'
                        new = '''rows = await main(Engine("window", a), heads[0], heads[1], run, out, work, sys.platform == "win32")
            app = Engine("window", a)
            defaults = (await app.result("prefs.get", {"keys":["agent.policy.defaults"]})).get("values",{}).get("agent.policy.defaults",{})
            defaults["policy"] = "observe"
            changed = await app.run("prefs.set", {"values":{"agent.policy.defaults":defaults}})
            obs_streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
            obs = await stack.enter_async_context(ClientSession(obs_streams[0],obs_streams[1]))
            await obs.initialize()
            observer = Engine("observer",obs)
            state = await observer.tool("get_state", {})
            tree = await observer.tool("inspect_ui", {"depth":20})
            node = c42.find((tree.get("result") or {}).get("root") or {}, lambda n:n.get("id")=="drawingView.canvas") or {}
            record = json.loads(node.get("value") or "{}").get("lastGesture")
            run.save("observe-samples.json", {"prefsChange":changed,"state":state,"treeStatus":tree.get("status"),"lastGesture":record})
            return rows'''
                        exec(compile(source.replace(old,new),str(__file__),'exec'),m.__dict__)
            sys.argv = [file, app] + ([str(Path(sys.argv[3]).resolve())] if mode == 'byte' or mode.startswith('exact') else []) + [str(out)]
            code = (m.run_main() if mode.startswith('exact') else m.main()) or 0
        else:
            m = load('original', ROOT / 'tests/adversarial/c42b-r1-live.py')
            m.O = out
            if mode == 'selection':
                m.main = lambda s, p: selection(m,s,p)
            if mode == 'observer':
                original_launch=m.e.launch
                def read_launch(app,user,disc):
                    import shutil
                    prefs=user/'Settings/preferences.json'
                    d=json.loads(prefs.read_text(encoding='utf-8'))
                    d['data']['agent.policy.defaults']['policy']='observe'
                    prefs.write_text(json.dumps(d),encoding='utf-8',newline='\n')
                    fixture=user/'fixture.animpkg'
                    shutil.copyfile(ROOT/'reviews/evidence/M3-C3.4-r1/ci-Windows/projects/E1-replay.animpkg',fixture)
                    binds=[{'commandId':cid,'params':params,'keys':key,'context':'global'} for key,cid,params in [
                        ('F6','project.open',{'path':str(fixture),'onUnsaved':'fail'}),
                        ('F7','workspace.select',{'name':'Drawing'}),('F8','tool.active.set',{'toolId':'brush'}),
                        ('F9','app.context.set',{'elementId':'el_01K7JJN800YHNMXD22AQ3X6TGN','frame':1})]]
                    (user/'Settings/keymap.json').write_text(json.dumps({'format':'toontown-settings','kind':'keymap','formatVersion':'1.0','data':{'baseSet':'default','added':binds,'removed':[]}}),encoding='utf-8',newline='\n')
                    return original_launch(app,user,disc)
                m.e.launch=read_launch
                m.main=lambda s,p:observer(m,s,p)
            if mode.startswith('d1-long-'):
                # Same original probe body, 30 samples instead of five; no product mutation.
                source = (ROOT / 'tests/adversarial/c42b-r1-live.py').read_text(encoding='utf-8')
                start = source.index('async def part_d(')
                stop = source.index('\ndef run():', start)
                exec(compile(source[start:stop].replace('for _ in range(5):', 'for _ in range(30):'), str(__file__), 'exec'), m.__dict__)
            m.run()
    except Exception as ex:
        code = 1
        (out/'error.json').write_text(json.dumps({'error':repr(ex)}), encoding='utf-8')
        print(repr(ex), flush=True)
    finally:
        # Helpers can leave discovery artifacts containing bearer credentials.
        for p in out.rglob('discovery.json'):
            d = json.loads(p.read_text(encoding='utf-8'))
            d['token'] = '<redacted>'
            p.write_text(json.dumps(d, indent=2), encoding='utf-8')
        (out/'review-run.json').write_text(json.dumps({'mode':mode,'seconds':time.monotonic()-started,'exitCode':code}, indent=2), encoding='utf-8')
    return code

if __name__ == '__main__':
    sys.exit(main())
