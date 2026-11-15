"""Round three live probes; existing helpers are loaded without modifying their files."""
import sys
if len(sys.argv)<2:
    print('skipped: no application');sys.exit(0)
import importlib.util,json,time,inspect,asyncio
from pathlib import Path
R=Path(__file__).resolve().parents[2]
def load(n,p):
    s=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
r2=load('r2',R/'tests/adversarial/c42b-r2-live.py')
O=R/'reviews/evidence/M4-C4.2b-r3'
r2.OUT=O
mode=sys.argv[2]
if mode.startswith('fixed-'):
    original=r2.load
    def fixed(n,p):
        m=original(n,p)
        if p.name=='c42-canvas-measure.py':
            src=inspect.getsource(m.measure)
            src=src.replace('                long_commits = []', "                for _ in range(5):\n                    await rc(session, 'edit.undo', {})\n                long_commits = []")
            exec(compile(src,__file__,'exec'),m.__dict__)
        return m
    r2.load=fixed
    sys.argv[2]='measure-'+mode.split('-')[-1]
    r2.OUT=O/'fixed'
if mode=='endings':
    original=r2.load
    def custom(n,p):
        m=original(n,p)
        if p.name=='c42-gui-e2e.py':
            src=inspect.getsource(m.first_session)
            src=src.replace('("focus loss", [])', '("busy between clicks", ["Polyline Tool"])')
            src=src.replace('                            if ending == "Enter":', '                            pc = (pa[0]+30, pa[1]+40)\n                            os_drag(e2e.APP_PID["pid"], [pc,pc])\n                            await asyncio.sleep(0.6)\n                            if ending == "busy between clicks":\n                                t=time.monotonic()\n                                query=await rc(session,\'app.state.get\',{})\n                                query_ms=(time.monotonic()-t)*1000\n                                t=time.monotonic()\n                                busy=await e2e.call(session,\'run_command\',{\'id\':\'tool.active.set\',\'params\':{\'toolId\':\'brush\'}})\n                                busy_ms=(time.monotonic()-t)*1000\n                                run.check(query.get(\'result\',{}).get(\'modal\') is not None and (busy.get(\'error\') or {}).get(\'code\')==\'ui.busy\' and busy_ms>=9900 and query_ms<150, \'between-click query and delayed busy write\', {\'queryMs\':query_ms,\'busyMs\':busy_ms,\'busy\':busy})\n                                os_key(0x0D)\n                            elif ending == "Enter":')
            src=src.replace('                            hist = (await rc(session,', '                            run.save("ending-"+ending.replace(" ","-")+"-canvas.json", await canvas_state(session))\n                            hist = (await rc(session,')
            src=src.replace('                            if modal is not None:', "                            rect = await rc(session,'draw.rectangle.add',{'from':{'x':-10,'y':-10},'to':{'x':10,'y':10}})\n                            run.check(rect.get('status')=='ok','rectangle write after '+ending,rect.get('error'))\n                            run.save('endings-progress.json',run.transcript)\n                            if modal is not None:")
            exec(compile(src,__file__,'exec'),m.__dict__)
        return m
    r2.load=custom
    sys.argv[2]='gui'
    r2.OUT=O/'endings'
if mode!='labels':
    sys.exit(r2.main())
r2.idle()
m=load('eq3',R/'scripts/c42-gesture-equivalence.py')
src=inspect.getsource(m.main)
src=src.replace('    results = []',"    rows += [('select: move','select',[(cx,cy),(cx+25,cy-15)],[],True)]\n    rows = [r for r in rows if r[0] in ('rectangle','ellipse','line','polyline','eraser','cutter','scissor','select: move')]\n    results = []")
src=src.replace('            await engine.run("colour.current.set", {"colourId": black})','            await engine.run("tool.active.set", {"toolId": tool})\n            await engine.run("colour.current.set", {"colourId": black})')
src=src.replace('"tool": rec["tool"], ', '')
src=src.replace('        # Back to the fixture on every engine.', '''        expected = {'rectangle':'Rectangle Tool','ellipse':'Ellipse Tool','line':'Line Tool','polyline':'Polyline Tool','eraser':'Eraser Tool','cutter':'Cutter Tool','scissor':'Scissor Tool','select':'Move Object'}[tool]
        row['expectedLabel'] = expected
        row['undoRedo'] = []
        for engine, lab in zip(engines, labels):
            await engine.run('edit.undo', {})
            await engine.run('edit.redo', {})
            row['undoRedo'].append(await engine.labels_since(pos[engines.index(engine)]))
        row['verdict'] = 'PASS' if same_content and labels == [[expected]]*3 and row['undoRedo'] == labels and how == 'os' else 'FAIL'
        run.check(row['verdict']=='PASS', 'specified label and undo/redo '+name, row['undoRedo'])
        run.save('rows.json', results)
        # Back to the fixture on every engine.''')
exec(compile(src,__file__,'exec'),m.__dict__)
out=O/'labels';sys.argv=['eq',str(Path(sys.argv[1]).resolve()),str(Path(sys.argv[3]).resolve()),str(out)]
code=m.run_main()
(out/'review-run.json').write_text(json.dumps({'exitCode':code}),encoding='utf-8')
sys.exit(code)
