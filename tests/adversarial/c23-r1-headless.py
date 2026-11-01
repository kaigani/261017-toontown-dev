"""C2.3 E2 slice through the official MCP SDK; receipts default to a temporary directory."""
import argparse
import asyncio
import base64
import hashlib
import io
import json
import pathlib
import shutil
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
D = 'dr_01K742SG0KKMCAP9X1E6Z5P4SW'
SKIN = 'col_01K742SG059XGSNZ6M9MACZ2ZY'
INK = 'col_01K742SG09H7QE4RRR6XSYYS88'

async def exercise(binary, out, work):
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    from PIL import Image
    base = work / 'base.animpkg'
    shutil.copy2(ROOT / 'reviews/evidence/M2-C2.2-r3/headless-base.animpkg', base)
    async def elicit(context, params):
        return types.ElicitResult(action='accept', content={'confirm': True})
    async def roots(context):
        return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(), name='review')])
    params = StdioServerParameters(command=str(binary), args=['--mcp-stdio', '--project', str(base), '--policy', 'autonomous', '--allow-path', str(work), '--user-dir', str(work/'user')])
    observations = []
    try:
        async with stdio_client(params) as (read, write):
            async with ClientSession(read, write, elicitation_callback=elicit, list_roots_callback=roots) as s:
                await s.initialize()
                async def run(command, p, success=True):
                    result = await s.call_tool('run_command', {'id':command, 'params':p})
                    env = getattr(result, 'structured_content', None) or getattr(result, 'structuredContent', None)
                    observations.append({'command':command, 'params':p, 'envelope':env})
                    if success: assert env['status']=='ok', env
                    return env
                added = (await run('draw.rectangle.add', {'drawingId':D, 'from':{'x':2000,'y':2000}, 'to':{'x':2100,'y':2100}, 'layer':'new'}))['result']
                zone = added['zoneIds'][0]
                await run('colour.paint.apply', {'drawingId':D,'objectIds':[zone],'colourId':SKIN})
                replaced = await run('colour.recolour.apply', {'fromColourId':SKIN,'toColourId':INK,'scope':'drawing','drawingIds':[D],'targets':['zones'],'respectProtected':False})
                p = {'drawingId':D,'width':100,'height':100,'region':{'x':2000,'y':2000,'width':100,'height':100}}
                rendered = (await run('draw.drawing.render', p))['result']
                data = base64.b64decode(rendered['png'])
                image = Image.open(io.BytesIO(data)).convert('RGBA')
                centre = image.getpixel((50,50))
                observations.append({'centrePixel':centre, 'pngSha256':hashlib.sha256(data).hexdigest(), 'pixelsSha256':rendered['pixelsSha256']})
                assert centre==(0,0,0,255), centre
                again = (await run('draw.drawing.render',p))['result']
                assert again['png']==rendered['png']
                default = await s.call_tool('render_drawing', {'drawing':D})
                structured = getattr(default,'structured_content',None) or getattr(default,'structuredContent',None)
                observations.append({'tool':'render_drawing','result':structured})
                # Tool envelopes use the common executor envelope.
                assert structured['result']['width']==512, structured
                await run('draw.stroke.add', {'drawingId':D,'kind':'invisible','layer':added['layerId'],'fit':False,'points':[{'x':2050,'y':1990},{'x':2050,'y':2110}]})
                stale = await run('colour.paint.apply', {'drawingId':D,'objectIds':[zone],'colourId':SKIN},False)
                assert stale['error']['code']=='colour.staleZoneId', stale
    finally:
        (out/'headless-sdk.json').write_text(json.dumps({'binary':str(binary),'observations':observations},indent=2),encoding='utf-8')

if __name__=='__main__':
    import sys, unittest
    if '--binary' not in sys.argv:
        class Format(unittest.TestCase):
            def test_base_is_an_existing_reusable_package(self):
                import zipfile
                self.assertTrue(zipfile.is_zipfile(ROOT/'reviews/evidence/M2-C2.2-r3/headless-base.animpkg'))
        unittest.main(verbosity=2)
    parser=argparse.ArgumentParser()
    parser.add_argument('--binary',type=pathlib.Path,required=True)
    parser.add_argument('--out',type=pathlib.Path)
    args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='tt-c23-r1-sdk-') as tmp:
        work=pathlib.Path(tmp).resolve()
        out=args.out.resolve() if args.out else work/'receipts'
        out.mkdir(parents=True,exist_ok=True)
        asyncio.run(exercise(args.binary.resolve(),out,work))
        print('E2 draw, paint-by-ID, recolour, render, pixels, repeat determinism and stale-ID checks passed')
