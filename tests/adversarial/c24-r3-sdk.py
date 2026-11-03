"""Round-3 real-release SDK probes. Run with --binary EXE --out RECEIPT; no implicit writes."""
import asyncio
import json
from pathlib import Path
import sys
import tempfile
import unittest


async def probe(binary, output):
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    calls, checks = [], []
    def check(name, passed, detail):
        checks.append({'name': name, 'ok': bool(passed), 'detail': detail})
        print(name, passed, flush=True)
    with tempfile.TemporaryDirectory(prefix='tt-c24-r3-sdk-') as tmp:
        work = Path(tmp)
        async def roots(ctx):
            return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(), name='review')])
        args = ['--headless', '--mcp-stdio', '--user-dir', str(work / 'user'), '--policy', 'autonomous']
        async with stdio_client(StdioServerParameters(command=str(binary), args=args)) as (reader, writer):
            async with ClientSession(reader, writer, list_roots_callback=roots) as session:
                await session.initialize()
                await asyncio.sleep(.5)
                async def cmd(id, params=None):
                    r = await session.call_tool('run_command', {'id': id, 'params': params or {}})
                    env = getattr(r, 'structuredContent', None) or getattr(r, 'structured_content', None) or {}
                    calls.append({'id': id, 'params': params, 'envelope': env})
                    assert env['status'] == 'ok', env
                    return env['result']
                async def done(id, params):
                    r = await cmd(id, params)
                    if 'jobId' not in r:
                        return r
                    r = await cmd('job.wait', {'jobId': r['jobId'], 'timeoutMs': 60000})
                    r = r.get('record', r)
                    assert r['state'] == 'succeeded', r
                    return r['result']
                await cmd('project.create', {'name': 'C24 R3', 'path': str(work / 'Review.animpkg')})
                state = await cmd('doc.state.get')
                history = await cmd('edit.history.get')
                exported = await cmd('doc.export.json', {'scope': 'project'})
                await cmd('tool.symmetry.set', {'mode': 'kaleidoscope', 'angle': 17.0, 'mirrors': 7})
                check('symmetry has no document revision', (await cmd('doc.state.get'))['documentRev'] == state['documentRev'], state)
                check('symmetry has no undo step', await cmd('edit.history.get') == history, history)
                after = await cmd('doc.export.json', {'scope': 'project'})
                for e in (exported, after):
                    e.get('bundle', {}).pop('exportedAt', None)
                check('symmetry does not alter saved document content', exported == after, {})
                await cmd('tool.symmetry.set', {'mode': 'none'})
                # A single line segment overlapping a brush edge; the line is visible above and below the brush.
                # Boundary overlap is not a crossing and must not erase the visible portions.
                point = lambda p: dict(zip(('x', 'y'), p))
                line = {'id': 'r3_line', 'kind': 'pencil', 'closed': False,
                    'bezier': [point(p) for p in [(40, 0), (40, 0), (40, 100), (40, 100)]],
                    'colourId': None, 'lineStyle': {'thickness': 1, 'lineType': 'solid'}}
                # Obtain an existing opaque colour without depending on current-colour implementation.
                pal = exported['bundle']
                def find_colour(v):
                    if isinstance(v, dict):
                        if isinstance(v.get('colourId'), str) and v.get('kind') == 'solid':
                            return v['colourId']
                        for x in v.values():
                            c = find_colour(x)
                            if c:
                                return c
                    if isinstance(v, list):
                        for x in v:
                            c = find_colour(x)
                            if c:
                                return c
                    return None
                colour = find_colour(pal)
                if colour is None:
                    # The schema's actual swatch list is authoritative; record it on an unexpected fixture shape.
                    raise AssertionError(pal)
                pts = [(40, 25), (60, 25), (60, 75), (40, 75)]
                bez = []
                for a, b in zip(pts, pts[1:] + pts[:1]):
                    bez += [point(a), point(a), point(b)]
                brush = {'id': 'r3_brush', 'kind': 'brush', 'closed': True, 'colourId': colour, 'outline': [bez]}
                layers = [{'type': 'layer', 'id': 'r3_lower', 'layerKind': 'pencil', 'strokes': [line]},
                          {'type': 'layer', 'id': 'r3_upper', 'layerKind': 'brush', 'strokes': [brush]}]
                r = await cmd('draw.content.add', {'elementId': 'scene:#1/element:"Drawing"', 'frame': 1,
                    'createDrawing': True, 'select': False, 'content': {'nodes': layers}})
                d = r['drawingId']
                before = await cmd('draw.drawing.get', {'drawingId': d})
                f = await cmd('draw.flatten.apply', {'drawingId': d, 'ids': [r['idMap']['r3_lower'], r['idMap']['r3_upper']]})
                after = await cmd('draw.drawing.get', {'drawingId': d})
                pencils = [s for n in after['nodes'] for s in n.get('strokes', []) if s['kind'] == 'pencil']
                check('Flatten keeps a partially covered single segment', bool(pencils), {'before': before, 'after': after, 'flatten': f})
                await done('project.saveAs', {'path': str(work / 'saved.animpkg'), 'onExists': 'overwrite'})
                await cmd('tool.symmetry.set', {'mode': 'mirror', 'angle': 13.0})
        # A fresh session on the same user directory must have no persisted symmetry default.
        args += ['--project', str(work / 'saved.animpkg')]
        async with stdio_client(StdioServerParameters(command=str(binary), args=args)) as (reader, writer):
            async with ClientSession(reader, writer, list_roots_callback=roots) as session:
                await session.initialize()
                await asyncio.sleep(.5)
                r = await session.call_tool('run_command', {'id': 'draw.stroke.add', 'params': {'drawingId': d,
                    'kind': 'pencil', 'fit': False, 'layer': 'new', 'symmetry': True,
                    'points': [{'x': 400, 'y': 400}, {'x': 430, 'y': 420}]}})
                env = getattr(r, 'structuredContent', None) or getattr(r, 'structured_content', None) or {}
                check('symmetry has no saved user default', env.get('status') == 'ok' and len(env['result']['strokeIds']) == 1, env)
    output.write_text(json.dumps({'binary': str(binary), 'sdk': '2.0.0', 'checks': checks, 'calls': calls}, indent=2) + '\n')
    return 0 if all(x['ok'] for x in checks) else 1


if __name__ == '__main__':
    if len(sys.argv) == 5 and sys.argv[1] == '--binary' and sys.argv[3] == '--out':
        output = Path(sys.argv[4])
        if output.exists():
            raise RuntimeError('refusing to overwrite receipt')
        sys.exit(asyncio.run(probe(Path(sys.argv[2]).resolve(), output)))
    else:
        class ExplicitOnly(unittest.TestCase):
            def test_output_is_explicit(self):
                self.assertIn('--out', __doc__)
        unittest.main()
