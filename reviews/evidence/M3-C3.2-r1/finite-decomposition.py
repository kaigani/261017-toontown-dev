"""Finite world matrix with overflowing decomposition norm; ADR-0019 output refusal oracle."""
import asyncio,importlib.util,json,sys,tempfile,math
from pathlib import Path
R=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('e4',R/'scripts/e4-cutout-numeric.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
async def main():
 work=Path(tempfile.mkdtemp(prefix='tt-c32-finite-'));out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
 async def roots(ctx):return m.types.ListRootsResult(roots=[m.types.Root(uri=work.as_uri(),name='work')])
 async with m.stdio_client(m.session_params(str(Path(sys.argv[1]).resolve()),work)) as (read,write):
  async with m.ClientSession(read,write,list_roots_callback=roots) as s:
   await s.initialize();await asyncio.sleep(.5)
   async def invoke(cid,p):return m.sc(await s.call_tool('run_command',{'id':cid,'params':p}))
   async def cmd(cid,p):
    r=await invoke(cid,p);assert r['status']=='ok',r;v=r['result']
    if v.get('jobId'):await s.call_tool('job_wait',{'jobId':v['jobId'],'timeoutMs':60000})
    return v
   await cmd('project.create',{'name':'Finite decomposition','path':str(work/'base.animpkg')})
   parent=None
   for i in range(26):
    node=(await cmd('element.create',{'type':'peg','name':f'N{i}'}))['elementIds'][0]
    if parent:await cmd('element.parent.set',{'elementIds':[node],'parentId':parent})
    await cmd('anim.transform.set',{'nodeIds':[node],'target':'static','scale':{'x':1000000,'y':1000000}})
    parent=node
   result=await invoke('anim.evaluate',{'nodeIds':[parent],'frames':[1],'include':['world']})
   acceptable=result.get('status')=='error' and result.get('error',{}).get('code')=='anim.valueOutOfRange'
   if result.get('status')=='ok':
    world=result['result']['frames'][0]['nodes'][0]['world'];sx=world['scale']['x'];sy=world['scale']['y']
    acceptable=isinstance(sx,(int,float)) and isinstance(sy,(int,float)) and math.isfinite(sx) and math.isfinite(sy) and abs(sx/1e156-1)<1e-12 and abs(sy/1e156-1)<1e-12
   receipt={'chainLength':26,'perNodeScale':1000000,'analyticScale':1e156,'ok':acceptable,'observed':result}
   (out/'result.json').write_text(json.dumps(receipt,indent=2));print(json.dumps(receipt,indent=2));return int(not acceptable)
sys.exit(asyncio.run(main()))
