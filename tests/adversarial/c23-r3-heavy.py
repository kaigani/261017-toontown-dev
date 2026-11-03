"""Raw official-SDK observations for long-text conversion, render size limit and queued reads."""
import argparse,asyncio,json,pathlib,shutil,tempfile,time,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2];D='dr_01K742SG0KKMCAP9X1E6Z5P4SW'
def sc(r):return getattr(r,'structured_content',None) or getattr(r,'structuredContent',None)
async def exercise(binary,out,work,counts):
 from mcp import ClientSession,StdioServerParameters,types
 from mcp.client.stdio import stdio_client
 async def elicit(context,params):return types.ElicitResult(action='accept',content={'confirm':True})
 observations=[]
 try:
  for count in counts:
   folder=work/str(count);folder.mkdir();base=folder/'base.animpkg';shutil.copy2(ROOT/'reviews/evidence/M2-C2.2-r3/headless-base.animpkg',base)
   p=StdioServerParameters(command=str(binary),args=['--mcp-stdio','--project',str(base),'--policy','autonomous','--allow-path',str(work),'--user-dir',str(folder/'user')])
   async with stdio_client(p) as (read,write):
    async with ClientSession(read,write,elicitation_callback=elicit) as s:
     await s.initialize()
     async def run(id,params):
      t=time.perf_counter();e=sc(await s.call_tool('run_command',{'id':id,'params':params}));seconds=time.perf_counter()-t;observations.append({'command':id,'textLength':count,'seconds':seconds,'params':params,'envelope':e});print(id,count,seconds,e['status'],flush=True);return e
     before=(await run('doc.state.get',{}))['documentRev']
     added=await run('draw.text.add',{'drawingId':D,'position':{'x':2000,'y':2000},'text':'O'*count})
     assert added['status']=='ok',added
     start=(await run('doc.state.get',{}))['documentRev']
     e=await run('draw.breakApart.apply',{'drawingId':D,'textIds':[added['result']['textId']]})
     following=await run('doc.state.get',{})
     if e['status']=='error':assert following['documentRev']==start
     assert following['status']=='ok',following
     if count==1000:
      e=await run('draw.drawing.render',{'drawingId':D,'width':8192,'height':8192});assert e['status']=='ok',e
      await run('doc.state.get',{})
 finally:
  out.mkdir(parents=True,exist_ok=True);(out/'heavy-sdk.json').write_text(json.dumps(observations,indent=2),encoding='utf-8')
class Format(unittest.TestCase):
 def test_limit(self):self.assertEqual(8192*8192,67108864)
if __name__=='__main__':
 import sys
 if '--binary' not in sys.argv:unittest.main(verbosity=2)
 else:
  p=argparse.ArgumentParser();p.add_argument('--binary',type=pathlib.Path,required=True);p.add_argument('--out',type=pathlib.Path);p.add_argument('--counts',default='1000,10000');a=p.parse_args()
  with tempfile.TemporaryDirectory(prefix='tt-c23-r3-heavy-') as t:
   work=pathlib.Path(t).resolve();asyncio.run(exercise(a.binary.resolve(),a.out.resolve() if a.out else work/'receipts',work,[int(n) for n in a.counts.split(',')]))
