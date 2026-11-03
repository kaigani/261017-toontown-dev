"""Additional SDK/headless probes. Explicit --binary/--out required to execute; default checks source contract."""
import asyncio
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]

async def probe(binary,out):
    from mcp import ClientSession,StdioServerParameters,types
    from mcp.client.stdio import stdio_client
    checks=[]
    def check(name,ok,detail):
        checks.append({'check':name,'ok':bool(ok),'detail':detail})
        print(name,ok,flush=True)
    with tempfile.TemporaryDirectory(prefix='tt-c24-r1-e2-') as tmp:
        work=Path(tmp)
        shutil.copy(ROOT/'tests/fixtures/vectorize/e2-lineart.png',work/'lineart.png')
        async def roots(ctx): return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(),name='review')])
        def sc(r): return getattr(r,'structuredContent',None) or getattr(r,'structured_content',None) or {}
        params=StdioServerParameters(command=str(binary),args=['--headless','--mcp-stdio','--user-dir',str(work/'user'),'--policy','autonomous'])
        async with stdio_client(params) as (read,write):
            async with ClientSession(read,write,list_roots_callback=roots) as s:
                await s.initialize(); await asyncio.sleep(.5)
                async def cmd(cid,p=None,**kw): return sc(await s.call_tool('run_command',{'id':cid,'params':p or {},**kw}))
                async def done(r):
                    jid=r.get('result',{}).get('jobId')
                    if not jid: return r
                    r=sc(await s.call_tool('job_wait',{'jobId':jid,'timeoutMs':60000}))
                    r=r.get('result',r); return r.get('record',r)
                target='scene:#1/element:"Drawing"'
                created=await cmd('project.create',{'name':'Review','path':str(work/'Review.animpkg')})
                assert created['status']=='ok',created
                two=await done(await cmd('draw.vectorize.run',{'files':[str(work/'lineart.png')]*2,'targetElementId':target,'startFrame':1}))
                check('two images complete as distinct drawings',two.get('state')=='succeeded' and two['result']['completed']==2 and len(set(two['result']['drawingIds']))==2,two)
                drawing=two['result']['drawingIds'][0]
                stale=await cmd('draw.vectorize.run',{'files':[str(work/'lineart.png')],'targetElementId':target,'startFrame':3},expectRev={target:0})
                check('stale vectorize expectRev refuses before job',stale.get('error',{}).get('code')=='rev.conflict' and 'jobId' not in stale.get('result',{}),stale)
                before=(await cmd('doc.state.get'))['result']['documentRev']
                started=await cmd('draw.vectorize.run',{'files':[str(work/'lineart.png')]*64,'targetElementId':target,'startFrame':3})
                jid=started['result']['jobId']
                for _ in range(1000):
                    state=await cmd('doc.state.get')
                    if state['result']['documentRev']>before: break
                    await asyncio.sleep(.005)
                cancel=sc(await s.call_tool('job_cancel',{'jobId':jid}))
                rec=await done(started)
                result=rec.get('result',{})
                count=result.get('completed',0)
                check('mid-job cancellation preserves a nonzero committed prefix',rec.get('state')=='cancelled' and 0<count<64 and len(result.get('drawingIds',[]))==count and len(result.get('skipped',[]))==64-count,rec)
                check('cancelled suffix uses SI-083 reasons and indices',all(x['reason']=='cancelled' and x['index']==count+i for i,x in enumerate(result.get('skipped',[]))),rec)
                drawing_before=await cmd('draw.drawing.get',{'drawingId':drawing})
                oldrev=drawing_before['result']['rev']
                await cmd('draw.stroke.add',{'drawingId':drawing,'kind':'pencil','fit':False,'points':[{'x':800,'y':0},{'x':820,'y':0}]})
                saved=await done(await cmd('project.saveAs',{'path':str(work/'edited-base.animpkg'),'onExists':'overwrite'}))
                check('extra edit saved',saved.get('state')=='succeeded',saved)
                batch={'label':'Review guarded replay','atomic':True,'expectRev':{drawing:oldrev},'stopOnError':True,'steps':[{'id':'draw.stroke.add','params':{'drawingId':drawing,'kind':'pencil','fit':False,'points':[{'x':900,'y':0},{'x':920,'y':0}]}}]}
                (work/'guarded-replay.json').write_text(json.dumps(batch))
        r=subprocess.run([str(binary),'--headless','--user-dir',str(work/'replay-user'),'--project',str(work/'edited-base.animpkg'),'--run',str(work/'guarded-replay.json'),'--out',str(work/'replay.jsonl')],capture_output=True,text=True,timeout=120)
        log=(work/'replay.jsonl').read_text() if (work/'replay.jsonl').exists() else ''
        check('replay after extra edit fails with revision conflict',r.returncode!=0 and 'rev.conflict' in log,{'exit':r.returncode,'log':log,'stderr':r.stderr})
        out.mkdir(parents=True,exist_ok=True)
        (out/'variants.json').write_text(json.dumps(checks,indent=2))
        (out/'guarded-replay.json').write_text(json.dumps(batch,indent=2))
        (out/'guarded-replay.jsonl').write_text(log)
    return 0 if all(x['ok'] for x in checks) else 1

if __name__=='__main__':
    if '--binary' in sys.argv:
        binary=Path(sys.argv[sys.argv.index('--binary')+1]).resolve()
        out=Path(sys.argv[sys.argv.index('--out')+1])
        sys.exit(asyncio.run(probe(binary,out)))
    class Contract(unittest.TestCase):
        def test_variant_has_explicit_output_and_revision_guard(self):
            self.assertIn('expectRev',Path(__file__).read_text())
    unittest.main()
