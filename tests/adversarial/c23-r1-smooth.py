"""Bounded real-binary Smooth responsiveness probe; writes only to --out or a temp directory."""
import argparse, asyncio, json, pathlib, shutil, tempfile, time
ROOT=pathlib.Path(__file__).resolve().parents[2]
D='dr_01K742SG0KKMCAP9X1E6Z5P4SW'
async def exercise(binary,out,work,n):
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client
    base=work/'base.animpkg';shutil.copy2(ROOT/'reviews/evidence/M2-C2.2-r3/headless-base.animpkg',base)
    params=StdioServerParameters(command=str(binary),args=['--mcp-stdio','--project',str(base),'--policy','autonomous','--allow-path',str(work),'--user-dir',str(work/'user')])
    receipt={'samples':n,'observations':[]}
    async with stdio_client(params) as (read,write):
        async with ClientSession(read,write) as s:
            await s.initialize()
            async def run(cmd,p):
                r=await s.call_tool('run_command',{'id':cmd,'params':p})
                e=getattr(r,'structured_content',None) or getattr(r,'structuredContent',None)
                receipt['observations'].append({'command':cmd,'envelope':e});return e
            a=await run('draw.stroke.add',{'drawingId':D,'kind':'invisible','fit':False,'layer':'new','points':[{'x':2000+i,'y':2000} for i in range(n)]})
            assert a['status']=='ok',a
            receipt['params']={'drawingId':D,'ids':a['result']['strokeIds'],'tolerance':1}
            start=time.perf_counter()
            try:
                receipt['response']=await asyncio.wait_for(run('draw.smooth.apply',receipt['params']),40)
            except TimeoutError:
                receipt['externalTimeoutSeconds']=40
            finally:
                receipt['wallSeconds']=time.perf_counter()-start
                (out/'smooth-never-block.json').write_text(json.dumps(receipt,indent=2),encoding='utf-8')
            assert receipt['wallSeconds']<30,receipt['wallSeconds']
if __name__=='__main__':
    import sys, unittest
    if '--binary' not in sys.argv:
        class Format(unittest.TestCase):
            def test_fixture_keeps_every_anchor(self):
                points=[{'x':2000+i,'y':2000} for i in range(2000)]
                self.assertEqual(len({(p['x'],p['y']) for p in points}),2000)
        unittest.main(verbosity=2)
    p=argparse.ArgumentParser();p.add_argument('--binary',type=pathlib.Path,required=True);p.add_argument('--out',type=pathlib.Path);p.add_argument('--samples',type=int,default=2000);a=p.parse_args()
    with tempfile.TemporaryDirectory(prefix='tt-c23-r1-smooth-') as tmp:
        work=pathlib.Path(tmp).resolve();out=a.out.resolve() if a.out else work/'receipts';out.mkdir(parents=True,exist_ok=True)
        asyncio.run(exercise(a.binary.resolve(),out,work,a.samples))
