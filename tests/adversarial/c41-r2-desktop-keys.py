"""Real Windows desktop review; run in the desktop gui CI job with a built binary."""
import asyncio, ctypes as c, importlib.util, json, sys, tempfile, time
from ctypes import wintypes as w
from pathlib import Path
from contextlib import asynccontextmanager
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('e2e',ROOT/'scripts/c41-gui-e2e.py'); e=importlib.util.module_from_spec(spec); spec.loader.exec_module(e)
u=c.windll.user32
u.SetProcessDPIAware()
u.GetDpiForWindow.argtypes=[w.HWND]; u.GetDpiForWindow.restype=w.UINT
u.SetWindowPos.argtypes=[w.HWND,w.HWND,c.c_int,c.c_int,c.c_int,c.c_int,w.UINT]
u.PostMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM]
def windows(pid):
 rows=[]
 @c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
 def cb(h,l):
  p=w.DWORD(); u.GetWindowThreadProcessId(h,c.byref(p))
  if p.value==pid:
   t=c.create_unicode_buffer(512); u.GetWindowTextW(h,t,512)
   if t.value:
    r=w.RECT(); q=w.RECT(); u.GetWindowRect(h,c.byref(r)); u.GetClientRect(h,c.byref(q)); scale=u.GetDpiForWindow(h)/96 or 1
    rows.append(dict(handle=int(h),title=t.value,visible=bool(u.IsWindowVisible(h)),scale=scale,x=round(r.left/scale),y=round(r.top/scale),width=round(q.right/scale),height=round(q.bottom/scale),outer=[r.right-r.left,r.bottom-r.top]))
  return True
 u.EnumWindows(cb,0); return rows
@asynccontextmanager
async def session(d):
 async with e.httpx2.AsyncClient(headers={'Authorization':'Bearer '+d['token']},timeout=90) as client:
  async with e.streamable_http_client(d['endpoint'],http_client=client) as ss:
   async def roots(ctx):
    from mcp.types import ListRootsResult,Root
    return ListRootsResult(roots=[Root(uri=ACCESS.as_uri(),name='review temp')])
   async with e.ClientSession(ss[0],ss[1],list_roots_callback=roots) as s:
    await s.initialize(); yield s
def menu_labels(pid):
 u.GetMenu.argtypes=[w.HWND]; u.GetMenu.restype=w.HMENU
 u.GetSubMenu.argtypes=[w.HMENU,c.c_int]; u.GetSubMenu.restype=w.HMENU
 u.GetMenuItemCount.argtypes=[w.HMENU]
 u.GetMenuStringW.argtypes=[w.HMENU,w.UINT,w.LPWSTR,c.c_int,w.UINT]
 labels=[]
 def walk(h):
  for i in range(u.GetMenuItemCount(h)):
   b=c.create_unicode_buffer(512); u.GetMenuStringW(h,i,b,512,0x400); labels.append(b.value)
   sub=u.GetSubMenu(h,i)
   if sub: walk(sub)
 h=next(r['handle'] for r in windows(pid) if r['title'].startswith('Toontown')); walk(u.GetMenu(h)); return labels
async def main(app,out):
 receipt=[]
 def check(name,ok,detail=None):
  receipt.append(dict(check=name,ok=bool(ok),detail=detail)); print(('PASS ' if ok else 'FAIL ')+name,flush=True)
 with tempfile.TemporaryDirectory(prefix='tt-c2-keys-') as temp:
  global ACCESS
  ACCESS=Path(temp)
  user=Path(temp); (user/'Settings').mkdir(); (user/'Settings/preferences.json').write_text(json.dumps(dict(format='toontown-settings',kind='preferences',formatVersion='1.0',data={'agent.policy.defaults':{'policy':'autonomous'}})))
  disc=user/'discovery.json'; proc=e.launch(app,user,disc)
  try:
   d=e.wait_discovery(disc,proc); oldpid=proc.pid
   async with session(d) as s:
    async def cmd(cid,p=None): return await e.run_cmd(s,cid,p)
    await cmd('project.create',{'name':'C2Keys','onUnsaved':'fail'})
    await cmd('project.properties.set',{'frameRate':12})
    bindings=(await cmd('keymap.list')).get('result') or []; receipt.append({'bindings':bindings,'nativeMenu':menu_labels(proc.pid)})
    menu=json.loads((ROOT/'desktop/src/ui-bindings/menu.json').read_text())
    def walk(n):
     if isinstance(n,dict):
      yield n
      for v in n.values(): yield from walk(v)
     elif isinstance(n,list):
      for v in n: yield from walk(v)
    commands=[n.get('command') for n in walk(menu) if n.get('kind')=='command']
    wanted=['edit.undo','edit.redo','project.save','project.create','ui.preferences.open','window.panel.set','view.zoom.set','view.transform.reset','ui.help.open','window.fullScreen.set']
    chosen=[]
    for cid in wanted:
     b=next((b for b in bindings if b['commandId']==cid),None)
     if b: chosen.append(b)
    for b in bindings:
     if len(chosen)>=10: break
     if b not in chosen and b['commandId'] in commands: chosen.append(b)
    for b in chosen:
     env=await e.call(s,'key',{'keys':b['keys']}); act=await e.last_activity(s,'key')
     check('key resolves '+b['keys'],(env.get('result') or {}).get('commandId',act.get('commandId'))==b['commandId'],dict(binding=b,env=env,activity=act,manifest=b['commandId'] in commands))
     receipt.append({'effectOf':b['commandId'],'state':await e.call(s,'get_state',{}),'tree':await e.tree(s),'nativeMenu':menu_labels(proc.pid)})
     e.os_escape(proc.pid)
     end=time.monotonic()+2
     while time.monotonic()<end:
      state=(await e.call(s,'get_state',{})).get('result') or {}
      if state.get('modal') is None: break
      await e.tree(s)
    await cmd('ui.statusBar.set',{'visible':True})
    for i in range(10):
     value=bool(i%2); await cmd('ui.statusBar.set',{'visible':value}); env=await e.tree(s); t=env.get('result') or {}; shown=e.by_id(t.get('root') or {},'statusBar') is not None
     check('rapid freshness '+str(i),shown==value or t.get('stale') is True,dict(value=value,shown=shown,stale=t.get('stale')))
    old=next((b['keys'] for b in bindings if b['commandId']=='project.save'),None)
    env=await cmd('keymap.set',dict(commandId='project.save',keys='F2',onConflict='override',replaceExisting=True))
    check('rebind override',env.get('status')=='ok',env)
    if old:
     env=await cmd('keymap.get',{'keys':old}); check('old sequence no longer invokes rebound command',((env.get('result') or {}).get('effectiveCommand') or {}).get('commandId')!='project.save',env)
    end=time.monotonic()+2
    while time.monotonic()<end: await e.tree(s)
    env=await e.call(s,'key',{'keys':'F2'}); act=await e.last_activity(s,'key'); check('native menu follows F2',any('F2' in label and 'Save' in label for label in menu_labels(proc.pid)),menu_labels(proc.pid))
    check('new sequence resolves',(env.get('result') or {}).get('commandId',act.get('commandId'))=='project.save',dict(env=env,activity=act))
    path=str(user/'exchange.json'); before=(await cmd('keymap.list')).get('result'); await cmd('keymap.export',{'path':path}); await cmd('keymap.set',{'commandId':'project.save','keys':'F4','onConflict':'override'}); env=await cmd('keymap.import',{'path':path,'mode':'replace'}); after=(await cmd('keymap.list')).get('result')
    check('export import identity',[{k:v for k,v in b.items() if k!='isDefault'} for b in before]==[{k:v for k,v in b.items() if k!='isDefault'} for b in after],env)
    stored=json.loads((user/'Settings/keymap.json').read_text()); check('stored map only deltas',len(json.dumps(stored))<len(json.dumps(bindings)),stored)
   import http.client
   from urllib.parse import urlsplit
   def wire(d,sid=None):
    url=urlsplit(d['endpoint']); conn=http.client.HTTPConnection(url.hostname,url.port,timeout=10)
    body={'jsonrpc':'2.0','id':1,'method':'tools/list'} if sid else {'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'review','version':'1'}}}
    headers={'Authorization':'Bearer '+d['token']}
    if sid: headers['Mcp-Session-Id']=sid
    conn.request('POST',url.path,json.dumps(body),headers); r=conn.getresponse(); status=r.status; ident=r.getheader('Mcp-Session-Id'); r.read(); conn.close(); return status,ident
   _,oldsession=wire(d)
   e.stop(proc); proc=e.launch(app,user,disc); d=e.wait_discovery(disc,proc)
   check('old session answers 404 after restart',wire(d,oldsession)[0]==404)
   check('discovery names new process',d.get('pid',d.get('processId'))==proc.pid and proc.pid!=oldpid,{k:v for k,v in d.items() if k!='token'})
   async with session(d) as s:
    env=await e.run_cmd(s,'keymap.get',{'keys':'F2'}); check('binding survives restart',((env.get('result') or {}).get('effectiveCommand') or {}).get('commandId')=='project.save',env)
    end=time.monotonic()+15
    t=await e.tree(s)
    while time.monotonic()<end and (((t.get('result') or {}).get('source')!='gui') or (t.get('result') or {}).get('stale')): t=await e.tree(s)
    check('restart fresh GUI',((t.get('result') or {}).get('source')=='gui') and not (t.get('result') or {}).get('stale'),t)
    env=await e.run_cmd(s,'keymap.reset',{'all':True}); after=(await e.run_cmd(s,'keymap.list')).get('result'); check('reset restores defaults',bindings==after,dict(env=env,before=bindings,after=after))
  finally: e.stop(proc)
 out.write_text(json.dumps(receipt,indent=2),encoding='utf-8',newline='\n')
 return int(any(not r.get('ok',True) for r in receipt))
if __name__=='__main__':
 if len(sys.argv)<2 or not Path(sys.argv[1]).is_file(): print('skipped: no application'); sys.exit(0)
 sys.exit(asyncio.run(main(str(Path(sys.argv[1]).resolve()),Path(sys.argv[2]) if len(sys.argv)>2 else ROOT/'reviews/evidence/M4-C4.1-r2/c2c.json')))
