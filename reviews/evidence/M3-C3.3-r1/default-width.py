import asyncio,json,sys,tempfile
from pathlib import Path
from mcp import ClientSession,StdioServerParameters,types
from mcp.client.stdio import stdio_client
async def main():
 work=Path(tempfile.mkdtemp(prefix='c33-r1-default-width-'))
 async def roots(context): return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(),name='work')])
 server=StdioServerParameters(command=str(Path('target/release/toontown-headless.exe').resolve()),args=['--headless','--mcp-stdio','--user-dir',str(work/'user'),'--policy','autonomous'])
 async with stdio_client(server) as (read,write):
  async with ClientSession(read,write,list_roots_callback=roots) as client:
   await client.initialize(); await asyncio.sleep(.5)
   def sc(v): return getattr(v,'structuredContent',None) or getattr(v,'structured_content',None) or {}
   made=sc(await client.call_tool('run_command',{'id':'project.create','params':{'name':'DefaultWidth','path':str(work/'Default.animpkg')}}))
   assert made.get('status')=='ok',made
   tool=sc(await client.call_tool('render_frame',{})); own=sc(await client.call_tool('run_command',{'id':'render.frame.run','params':{}}))
   assert tool['status']==own['status']=='ok'
   receipt=dict(toolWidth=tool['result']['width'],commandWidth=own['result']['width'],samePng=tool['result']['data']==own['result']['data'],section11TableDefault=960,section09RequiresDefaultParity=True)
   assert receipt['samePng'] and receipt['toolWidth']==receipt['commandWidth']
   print(json.dumps(receipt,indent=2))
   (Path(tempfile.gettempdir())/'toontown-c33-r1/default-width.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8',newline='\n')
asyncio.run(main())
