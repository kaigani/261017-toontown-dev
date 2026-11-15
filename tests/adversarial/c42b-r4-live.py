"""Run existing live probes with receipts confined to round four."""
import sys
if len(sys.argv) < 2:
    print('skipped: no application')
    sys.exit(0)
from pathlib import Path
R = Path(__file__).resolve().parents[2]
source = (R/'tests/adversarial/c42b-r3-live.py').read_text(encoding='utf-8')
source = source.replace("O=R/'reviews/evidence/M4-C4.2b-r3'", "O=R/'reviews/evidence/M4-C4.2b-r4/live'")
mode = sys.argv[2]
if mode.startswith('d2-exact-'):
    source = source.replace("mode=sys.argv[2]", "mode=sys.argv[2]\nbody = inspect.getsource(r2.main).replace('m.O = out', 'm.O = OUT / \"d2-exact\"; m.O.mkdir(parents=True, exist_ok=True)')\nexec(compile(body, __file__, 'exec'), r2.__dict__)")
if mode in ('alternate', 'named'):
    sys.argv[2] = 'labels'
    source = source.replace("O/'labels'", "O/" + repr(mode))
    extra = '''
original_load = load
def custom_load(n,p):
    m = original_load(n,p)
    if p.name == 'c42-gesture-equivalence.py':
        body = inspect.getsource(m.main)
        body = body.replace('    results = []', "    rows = [r for r in rows if r[0] == 'rectangle']\\n    results = []")
        if MODE == 'alternate':
            body = body.replace("rows = [r for r in rows if r[0] == 'rectangle']", "rows = [('scissor','scissor',[(-170,-80),(-90,-80),(-90,80),(-170,80),(-170,-80)],['alt'],False)]")
            body = body.replace('and not mods and on_canvas', 'and on_canvas')
            body = body.replace('same_labels = labels[0] == labels[1] == labels[2] and len(labels[0]) <= 1', "same_labels = labels == [['Scissor Tool'],['Scissor Tool'],['Cutter Tool']]")
            old_drag = m.c42.os_drag
            def alt_drag(pid, points):
                import ctypes
                u = ctypes.windll.user32
                u.keybd_event(0x12,0,0,0)
                try: return old_drag(pid,points)
                finally: u.keybd_event(0x12,0,2,0)
            m.c42.os_drag = alt_drag
        else:
            body = body.replace('if os_input and offset is not None', 'if False and offset is not None')
            body = body.replace('            await app.tool("pointer",', '            await rc("tool.active.set", {"toolId":"brush"})\\n            await app.tool("pointer",')
            body = body.replace('            how = "agent"', '            run.check((await app.result("tool.active.get", {})).get("toolId") == tool, "named tool stays active after routed gesture")\\n            how = "agent"')
        exec(compile(body,str(p),'exec'),m.__dict__)
    return m
load = custom_load
'''.replace('MODE', repr(mode))
    # Transform the final function text after the round-three runner has prepared it.
    transform = extra[extra.index('        body = body.replace'):extra.index('        exec(compile')]
    transform = transform.replace('        ', '', 1)
    transform = '\n'.join(line[8:] if line.startswith('        ') else line for line in transform.splitlines())
    # Keep indentation predictable by wrapping the transformation in a function.
    injected = 'def transform_body(body):\n' + '\n'.join('    '+line for line in transform.splitlines()) + '\n    return body\nsrc = transform_body(src)\n'
    marker = 'exec(compile(src,__file__,\'exec\'),m.__dict__)'
    before, after = source.rsplit(marker, 1)
    source = before + injected + marker + after
    if mode == 'alternate':
        source = source.replace("labels == [[expected]]*3", "labels == [['Scissor Tool'],['Scissor Tool'],['Cutter Tool']]")
    else:
        source = source.replace("how == 'os'", "how == 'agent'")
exec(compile(source, str(R/'tests/adversarial/c42b-r3-live.py'), 'exec'))
