"""Independent SDK 2.0.0 request-cancellation/session/audit probe."""
import asyncio,json,sys,tempfile,importlib.metadata
from pathlib import Path
from mcp import ClientSession,StdioServerParameters,types
from mcp.client.stdio import stdio_client

def sc(r): return r.structured_content
async def main():
    root=Path(__file__).resolve().parents[2]
    work=Path(tempfile.mkdtemp(prefix='tt-c34-r2-sdk-'))
    evidence=[]
    async def roots(ctx):return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(),name='review')])
    params=StdioServerParameters(command=str((root/'target/release/toontown-headless.exe').resolve()),args=['--headless','--mcp-stdio','--user-dir',str(work/'user'),'--policy','autonomous'])
    async with stdio_client(params) as (read,write):
        async with ClientSession(read,write,list_roots_callback=roots) as s:
            await s.initialize();await asyncio.sleep(.5)
            async def run(command,p):return sc(await s.call_tool('run_command',{'id':command,'params':p}))
            created=await run('project.create',{'name':'Review','cameraWidth':256,'cameraHeight':144})
            assert created['status']=='ok',created
            for kind in ['run_command','batch','nonjob']:
                out=work/kind
                render={'from':1,'to':100000,'width':256,'directory':str(out),'createDirs':True,'wait':True}
                if kind=='run_command':name,args='run_command',{'id':'render.range.run','params':render}
                elif kind=='batch':name,args='batch',{'label':'review cancellation','atomic':False,'steps':[{'id':'render.range.run','params':render},{'id':'element.create','params':{'type':'peg','name':'Late','sceneId':'scene:#1'}}]}
                else:name,args='run_command',{'id':'render.frame.run','params':{'frame':1,'width':8192,'height':8192,'output':'file','path':str(work/'long.png')}}
                task=asyncio.create_task(s.call_tool(name,args))
                if kind!='nonjob':
                    for _ in range(3000):
                        if task.done():raise AssertionError(str(task.result()))
                        if out.exists() and any(out.iterdir()):break
                        await asyncio.sleep(.01)
                    else:raise AssertionError('no staging')
                else:await asyncio.sleep(.03)
                task.cancel()
                try:await task
                except asyncio.CancelledError:pass
                else:raise AssertionError('call answered before cancellation')
                await asyncio.sleep(.2)
                state=await run('doc.state.get',{})
                assert state['status']=='ok',state
                jobs=sc(await s.call_tool('job_status',{}))['result']
                settled=[]
                for j in jobs:
                    if j['state'] in ['queued','running']:
                        await run('job.cancel',{'jobId':j['jobId']})
                        terminal=await run('job.wait',{'jobId':j['jobId'],'timeoutMs':60000})
                        assert terminal['status']=='ok',terminal
                        settled.append({k:terminal['result'].get(k) for k in ['jobId','state','error','warnings']})
                audit=await run('agent.activity.list',{'limit':100})
                audit['result']=[{k:v for k,v in a.items() if k not in ['params','result','arguments']} for a in audit['result']]
                evidence.append({'kind':kind,'following_state':state['status'],'jobs':[{k:j.get(k) for k in ['jobId','commandId','state','progress','error']} for j in jobs],'activity':audit,'terminal':settled})
                assert not out.exists() or not any('.tt-partial-' in p.name for p in out.iterdir())
    (root/'reviews/evidence/M3-C3.4-r2/sdk.json').write_text(json.dumps({'sdk':importlib.metadata.version('mcp'),'calls':evidence},indent=2)+'\n')
    print('SDK cancellation/session/staging probes completed')
if __name__=='__main__':asyncio.run(main())




