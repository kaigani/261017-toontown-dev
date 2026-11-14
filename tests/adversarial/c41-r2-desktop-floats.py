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
   async with e.ClientSession(ss[0],ss[1]) as s:
    await s.initialize(); yield s
async def main(app,out):
 receipt=[]
 def check(name,ok,detail=None):
  receipt.append(dict(check=name,ok=bool(ok),detail=detail)); print(('PASS ' if ok else 'FAIL ')+name,flush=True)
 async def poll(fn,pred,bound=10):
  end=time.monotonic()+bound; v=None
  while time.monotonic()<end:
   v=await fn()
   if pred(v): return v
  return v
 with tempfile.TemporaryDirectory(prefix='tt-c2-review-') as temp:
  user=Path(temp); (user/'Settings').mkdir(); (user/'Settings/preferences.json').write_text(json.dumps(dict(format='toontown-settings',kind='preferences',formatVersion='1.0',data={'agent.policy.defaults':{'policy':'autonomous'}})))
  disc=user/'discovery.json'; proc=e.launch(app,user,disc)
  try:
   d=e.wait_discovery(disc,proc)
   async with session(d) as s:
    async def cmd(cid,p=None): return await e.run_cmd(s,cid,p)
    async def ws(): return windows(proc.pid)
    async def panels(): return (await cmd('window.panel.list')).get('result') or []
    async def geom(): return next(p for p in await panels() if p['panelId']=='history')
    async def tree(): return (await e.tree(s)).get('result') or {}
    await poll(tree,lambda t:e.by_id(t.get('root') or {},'app'))
    await cmd('workspace.create',{'name':'C2Floats'})
    await cmd('workspace.select',{'name':'C2Floats'})
    baseline=len([r for r in windows(proc.pid) if r['visible']])
    g=dict(x=140,y=120,width=420,height=300)
    await cmd('window.panel.set',dict(panelId='history',visible=True,floating=True,geometry=g))
    rows=await poll(ws,lambda rows:len([r for r in rows if r['visible']])==baseline+1)
    h=next((r for r in rows if r['title']=='History'),{})
    check('one additional OS float',len([r for r in rows if r['visible']])==baseline+1,rows)
    t=await poll(tree,lambda t:e.by_id(t.get('root') or {},'float-history'))
    check('combined float tree',e.by_id(t.get('root') or {},'float-history') is not None)
    check('initial geometry matches logical OS within 2px',all(abs(h.get(k,-999)-v)<=2 for k,v in g.items()),dict(os=h,kernel=await geom()))
    g=dict(x=200,y=180,width=380,height=280); await cmd('window.panel.set',dict(panelId='history',geometry=g))
    rows=await poll(ws,lambda rs:any(r['title']=='History' and all(abs(r[k]-v)<=2 for k,v in g.items()) for r in rs))
    check('command moves and resizes OS float',any(r['title']=='History' and all(abs(r[k]-v)<=2 for k,v in g.items()) for r in rows),rows)
    # Observe a settled interval by polling, never sleep within timing bounds.
    end=time.monotonic()+.7
    while time.monotonic()<end: await geom()
    h=next(r for r in windows(proc.pid) if r['title']=='History'); scale=h['scale']
    before=await cmd('agent.activity.list',{'limit':100})
    u.SetWindowPos(h['handle'],None,round(260*scale),round(230*scale),round(450*scale),round(350*scale),4)
    p=await poll(geom,lambda p:abs((p.get('geometry') or {}).get('x',0)-260)<=2)
    check('OS move written back',abs((p.get('geometry') or {}).get('x',0)-260)<=2,p)
    baseline_geometry=p.get('geometry')
    baseline_os=next(r for r in windows(proc.pid) if r['title']=='History')
    stable=True; samples=0; end=time.monotonic()+2
    while time.monotonic()<end:
     current=(await geom()).get('geometry')
     current_os=next((r for r in windows(proc.pid) if r['title']=='History'),None)
     stable=stable and current==baseline_geometry and current_os==baseline_os
     samples+=1
    check('move activity settles',stable and samples>1 and abs((baseline_geometry or {}).get('x',0)-260)<=2 and abs((baseline_geometry or {}).get('y',0)-230)<=2,dict(geometry=baseline_geometry,os=baseline_os,samples=samples))
    await cmd('window.panel.set',dict(panelId='history',geometry=dict(x=99999,y=99999,width=380,height=280)))
    p=await geom(); check('far geometry clamped',(p.get('geometry') or {}).get('x')!=99999,p)
    for setting in [dict(floating=False),dict(floating=True,visible=True),dict(visible=False),dict(visible=True)]:
     await cmd('window.panel.set',dict(panelId='history',**setting)); want=setting.get('visible',setting.get('floating',True))
     rows=await poll(ws,lambda rs:any(r['title']=='History' for r in rs)==want)
     check('float lifecycle '+str(setting),any(r['title']=='History' for r in rows)==want,rows)
    end=time.monotonic()+1
    while time.monotonic()<end: await tree()
    h=next(r for r in windows(proc.pid) if r['title']=='History'); u.PostMessageW(h['handle'],0x10,0,0)
    p=await poll(geom,lambda p:p.get('visible') is False)
    check('WM_CLOSE hides float and app lives',p.get('visible') is False and proc.poll() is None,p)
    for panel in ['history','colour','properties']:
     check('float '+panel,(await cmd('window.panel.set',dict(panelId=panel,floating=True,visible=True))).get('status')=='ok')
    for cid in ['window.minimized.set','window.floatingPanels.set']:
     for value in [False,True] if 'floatingPanels' in cid else [True,False]:
      env=await cmd(cid,{'visible':value} if 'floatingPanels' in cid else {'minimized':value})
      wantshown=value if 'floatingPanels' in cid else not value
      rows=await poll(ws,lambda rs:len([r for r in rs if r['visible'] and r['title'] in ['History','Colour','Properties']])==(3 if wantshown else 0))
      check(cid+' '+str(value),env.get('status')=='ok' and len([r for r in rows if r['visible'] and r['title'] in ['History','Colour','Properties']])==(3 if wantshown else 0),dict(env=env,windows=rows))
    check('raise all',(await cmd('window.raiseAll.run')).get('status')=='ok')
    await cmd('workspace.save'); saved=await panels(); (user/'saved.json').write_text(json.dumps(saved))
    await cmd('workspace.select',{'name':'Drawing'}); rows=await poll(ws,lambda rs:len([r for r in rs if r['visible']])==1)
    check('workspace switch closes old floats',len([r for r in rows if r['visible']])==1,rows)
   e.stop(proc); proc=e.launch(app,user,disc); d=e.wait_discovery(disc,proc)
   async with session(d) as s:
    await e.run_cmd(s,'workspace.select',{'name':'C2Floats'})
    rows=await poll(lambda:asyncio.to_thread(windows,proc.pid),lambda rs:len([r for r in rs if r['visible']])==4)
    check('three saved floats restore',len([r for r in rows if r['visible']])==4,rows)
    restored=(await e.run_cmd(s,'window.panel.list')).get('result') or []
    expected=json.loads((user/'saved.json').read_text())
    check('restored float geometry',[(p['panelId'],p.get('geometry')) for p in restored if p.get('floating')]==[(p['panelId'],p.get('geometry')) for p in expected if p.get('floating')],restored)
    env=await e.run_cmd(s,'ui.preferences.open',{'category':'interface'})
    await e.tree(s); env=await e.run_cmd(s,'window.panel.set',{'panelId':'history','visible':False})
    check('main modal refuses agent with float', (env.get('error') or {}).get('code')=='ui.modalOpen',env)
    e.os_escape(proc.pid)
    t=await poll(lambda:e.tree(s),lambda t:e.by_id((t.get('result') or {}).get('root') or {},'preferences') is None)
    env=await e.run_cmd(s,'ui.statusBar.set',{'visible':True}); check('agent accepted after modal',env.get('status')=='ok',env)
   h=next(r for r in windows(proc.pid) if r['title'].startswith('Toontown')); u.PostMessageW(h['handle'],0x10,0,0)
   end=time.monotonic()+15
   while time.monotonic()<end and proc.poll() is None: pass
   check('main close with floats exits process',proc.poll() is not None)
  finally:
   e.stop(proc)
 out.write_text(json.dumps(receipt,indent=2),encoding='utf-8',newline='\n')
 return int(any(not r['ok'] for r in receipt))
if __name__=='__main__':
 if len(sys.argv)<2 or not Path(sys.argv[1]).is_file(): print('skipped: no application'); sys.exit(0)
 sys.exit(asyncio.run(main(str(Path(sys.argv[1]).resolve()),Path(sys.argv[2]) if len(sys.argv)>2 else ROOT/'reviews/evidence/M4-C4.1-r2/c2b.json')))
