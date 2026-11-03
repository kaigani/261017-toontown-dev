"""Real headless E2 painting half via official MCP SDK and exported --run replay.

Run: python tests/adversarial/c22-r3-headless.py --binary target/debug/toontown-headless.exe --out <directory>
Without arguments, CI checks the batch-format construction without requiring an SDK install or built binary.
"""
import argparse
import asyncio
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
D = 'dr_01K742SG0KKMCAP9X1E6Z5P4SW'

def sc(r):
    return getattr(r, 'structured_content', None) or getattr(r, 'structuredContent', None)

def square():
    return {'drawingId':D,'kind':'pencil','layer':'new','fit':False,'closed':True,
            'points':[{'x':1000,'y':1000},{'x':1100,'y':1000},{'x':1100,'y':1100},{'x':1000,'y':1100}]}

async def exercise(binary, out, work):
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    async def elicit(context, params):
        return types.ElicitResult(action='accept', content={'confirm':True})
    async def roots(context):
        return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(), name='review')])
    params=StdioServerParameters(command=str(binary),args=['--mcp-stdio','--policy','autonomous','--allow-path',str(work),'--user-dir',str(work/'user')])
    # Replay requires the same project ID and revision; establish and reopen a saved base first.
    base=work/'base.animpkg'
    async with stdio_client(params) as (read,write):
        async with ClientSession(read,write,elicitation_callback=elicit,list_roots_callback=roots) as s:
            await s.initialize()
            for id,p in [('project.create',{'name':'R3 E2'}),('doc.import.json',{'source':json.loads((ROOT/'target/evidence/review-c22/fixture-bundle.json').read_bytes()),'idPolicy':'preserve'})]:
                env=sc(await s.call_tool('run_command',{'id':id,'params':p}));assert env['status']=='ok',env
            saved=sc(await s.call_tool('run_command',{'id':'project.saveAs','params':{'path':str(base)}}))
            done=sc(await s.call_tool('run_command',{'id':'job.wait','params':{'jobId':saved['result']['jobId'],'timeoutMs':60000}}))
            assert done['result']['state']=='succeeded',done
    params.args += ['--project',str(base)]
    observations=[]
    async with stdio_client(params) as (read,write):
        async with ClientSession(read,write,elicitation_callback=elicit,list_roots_callback=roots) as s:
            await s.initialize()
            async def run(id, params, success=True):
                env=sc(await s.call_tool('run_command',{'id':id,'params':params}))
                observations.append({'command':id,'params':params,'envelope':env})
                if success: assert env['status']=='ok',env
                return env
            added=(await run('draw.stroke.add',square()))['result']
            layer=added['layerId']; old=added['zoneIds'][0]
            await run('draw.stroke.add',{'drawingId':D,'kind':'invisible','layer':layer,'fit':False,'points':[{'x':1040,'y':990},{'x':1040,'y':1110}]})
            stale=await run('colour.paint.apply',{'drawingId':D,'objectIds':[old]},False)
            assert stale['error']['code']=='colour.staleZoneId',stale
            readback=(await run('draw.drawing.get',{'drawingId':D}))['result']
            zones=next(n for n in readback['nodes'] if n['id']==layer)['zones']
            ids=[z['id'] for z in zones]
            assert len(ids)==2
            lint=(await run('doc.validate',{'checks':['unpaintedZones']}))['result']
            assert any(z in json.dumps(lint) for z in ids),lint
            painted=await run('colour.paint.apply',{'drawingId':D,'objectIds':ids})
            assert painted['documentRev']==readback['rev']+1,painted
            lint=(await run('doc.validate',{'checks':['unpaintedZones']}))['result']
            assert not any(z in json.dumps(lint) for z in ids),lint
            for id in ['edit.undo','edit.redo']: await run(id,{})
            resource=await s.read_resource(f'app://drawing/{D}')
            assert D in resource.contents[0].text
            export=await run('agent.activity.export',{'path':str(work/'replay.json'),'format':'batch'})
            assert export['status']=='ok'
    # The exported activity uses the same transaction seeds and committed commands; stale failure is omitted.
    batch=json.loads((work/'replay.json').read_bytes())
    (out/'headless-batch.json').write_text(json.dumps(batch,indent=2),encoding='utf-8')
    command=[str(binary),'--run',str(work/'replay.json'),'--project',str(base),'--policy','autonomous','--allow-path',str(work),'--user-dir',str(work/'replay-user')]
    result=subprocess.run(command,capture_output=True,timeout=120)
    shutil.copy2(base,out/'headless-base.animpkg')
    (out/'headless-replay.stdout').write_bytes(result.stdout)
    (out/'headless-replay.stderr').write_bytes(result.stderr)
    assert result.returncode==0,(result.returncode,result.stdout.decode(errors='replace'),result.stderr.decode(errors='replace'))
    (out/'headless-sdk.json').write_text(json.dumps({'binary':str(binary),'commands':observations,'replayExit':result.returncode},indent=2),encoding='utf-8')
    print('SDK E2 stale/fresh IDs, validation, undo/redo, resource and --run replay passed')

class FormatTest(unittest.TestCase):
    def test_square_is_closed_and_explicitly_targeted(self):
        p=square()
        self.assertEqual(len(p['points']),4)
        self.assertTrue(p['closed'])
        self.assertEqual(p['drawingId'],D)

if __name__=='__main__':
    if '--binary' not in sys.argv: unittest.main(verbosity=2)
    else:
        parser=argparse.ArgumentParser()
        parser.add_argument('--binary',type=pathlib.Path,required=True)
        parser.add_argument('--out',type=pathlib.Path,required=True)
        args=parser.parse_args()
        args.out.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='tt-c22-r3-sdk-') as tmp:
            asyncio.run(exercise(args.binary.resolve(),args.out.resolve(),pathlib.Path(tmp).resolve()))
