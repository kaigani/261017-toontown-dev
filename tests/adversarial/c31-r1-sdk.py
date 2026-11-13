"""Independent official-SDK C3.1 smoke/reproductions.

Default runs pure receipt helper tests. To exercise the product:
python c31-r1-sdk.py --binary <headless.exe> --output <new-report.json>
Requires mcp==2.0.0. Product projects/settings use a temporary directory.
"""
import asyncio
import importlib.metadata
import json
import pathlib
import sys
import tempfile
import unittest


def structured(result):
    return getattr(result,'structured_content',None) or getattr(result,'structuredContent',None) or {}


class ReceiptHelpers(unittest.TestCase):
    def test_sdk_2_and_compatibility_result_fields(self):
        class V2:
            structured_content={'status':'ok'}
        class V1:
            structuredContent={'status':'error'}
        self.assertEqual(structured(V2()),{'status':'ok'})
        self.assertEqual(structured(V1()),{'status':'error'})


async def exercise(binary):
    from mcp import ClientSession,StdioServerParameters,types
    from mcp.client.stdio import stdio_client
    version=importlib.metadata.version('mcp')
    if version!='2.0.0': raise RuntimeError(f'mcp==2.0.0 required, found {version}')
    records=[]; checks=[]
    with tempfile.TemporaryDirectory(prefix='tt-c31-review-sdk-') as tmp:
        async def roots(_):
            return types.ListRootsResult(roots=[types.Root(uri=pathlib.Path(tmp).resolve().as_uri(),name='review')])
        server=StdioServerParameters(command=str(binary),args=['--headless','--mcp-stdio','--user-dir',str(pathlib.Path(tmp)/'user'),'--policy','autonomous'])
        async with stdio_client(server) as (read,write):
            async with ClientSession(read,write,list_roots_callback=roots) as client:
                init=await client.initialize()
                async def run(command,params,dry=False):
                    e=structured(await client.call_tool('run_command',{'id':command,'params':params,'dryRun':dry}))
                    records.append({'command':command,'params':params,'dryRun':dry,'envelope':e})
                    return e
                def check(name,ok,details=None):
                    checks.append({'check':name,'ok':bool(ok),'details':details})
                e=await run('project.create',{'name':'Review C3.1'})
                check('project.create',e.get('status')=='ok',e)
                e=await run('element.create',{'type':'drawing','name':'Review'})
                check('element.create',e.get('status')=='ok',e)
                element=e['result']['elementIds'][0]
                drawings=[]
                for name in ['1','2','3']:
                    e=await run('element.drawing.create',{'elementId':element,'name':name})
                    drawings.append(e['result']['drawingId'])
                e=await run('xsheet.exposures.set',{'columns':[{'elementId':element,'from':1,'to':3,'exposures':[{'drawing':str(i),'from':i,'to':i} for i in range(1,4)]}]})
                check('sheet write',e.get('status')=='ok',e)
                e=await run('element.drawing.delete',{'drawingIds':[drawings[0]]})
                check('parameter confirmation',e.get('status')=='needs_confirmation' and e.get('result',{}).get('code')=='drawing.inUse',e)
                before=await run('element.typeColour.get',{})
                await run('element.typeColour.set',{'category':'pegs','colour':'#123456'},True)
                after=await run('element.typeColour.get',{})
                check('C31-R1-02 dry run session isolation',after.get('result')==before.get('result'),{'before':before,'after':after})
                e=await run('cell.columnate.apply',{'elementId':element,'frame':1,'firstDrawing':'1','lastDrawing':'3','exposeEach':4,'exposeLast':1,'loopMode':'continuous','totalFrames':12})
                check('C31-R1-01 exact columnate total',e.get('result',{}).get('totalFrames')==12,e)
                e=await run('element.get',{'elementId':'scene:#1/element:"Absent"'})
                check('element reference code',e.get('error',{}).get('code')=='element.notFound',e)
    return {'sdkVersion':version,'binary':str(binary),'checks':checks,'calls':records,'allPassed':all(c['ok'] for c in checks)}


if __name__=='__main__':
    if len(sys.argv)==5 and sys.argv[1]=='--binary' and sys.argv[3]=='--output':
        output=pathlib.Path(sys.argv[4]).resolve()
        if output.exists(): raise FileExistsError('preserve the prior receipt; use a new output file')
        report=asyncio.run(exercise(pathlib.Path(sys.argv[2]).resolve()))
        output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
        print(json.dumps({'checks':len(report['checks']),'failed':[c['check'] for c in report['checks'] if not c['ok']]}))
        raise SystemExit(0 if report['allPassed'] else 1)
    unittest.main()
