"""Live round-three keys, freshness, policy persistence and attached off probe."""
import asyncio, importlib.util, json, sys, tempfile, time, http.client
from urllib.parse import urlsplit
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sp=importlib.util.spec_from_file_location('floats',ROOT/'tests/adversarial/c41-r2-desktop-floats.py'); f=importlib.util.module_from_spec(sp); sp.loader.exec_module(f); e=f.e
async def main(app,out):
 receipt=[]
 def check(name,ok,detail=None):
  receipt.append(dict(check=name,ok=bool(ok),detail=detail)); print(('PASS ' if ok else 'FAIL ')+name,flush=True)
 async def poll(fn,pred,bound=10):
  end=time.monotonic()+bound; v=None
  while time.monotonic()<end:
   v=await fn()
   if pred(v): break
  return v
 with tempfile.TemporaryDirectory(prefix='tt-r3-c3-') as temp:
  user=Path(temp); (user/'Settings').mkdir(); seed=dict(policy='autonomous',maxJobs=3,maxBatchSteps=789,maxInlineImagePx=1024,maxInlineBytes=123456,askInApp=False,letAgentMoveView=False,highlightAgentChanges=False)
  (user/'Settings/preferences.json').write_text(json.dumps(dict(format='toontown-settings',kind='preferences',formatVersion='1.0',data={'agent.policy.defaults':seed})),encoding='utf-8')
  disc=user/'discovery.json'; proc=e.launch(app,user,disc)
  try:
   d=e.wait_discovery(disc,proc)
   async with f.session(d) as s:
    async def cmd(cid,p=None): return await e.run_cmd(s,cid,p)
    async def tree(): return (await e.tree(s)).get('result') or {}
    t=await poll(tree,lambda t:not t.get('stale') and t.get('source')=='gui')
    check('fresh renderedSeq is number',not t.get('stale') and isinstance(t.get('renderedSeq'),int),t)
    v=await e.call(s,'key',{'keys':'F24'}); check('unbound key handled false',(v.get('result') or {}).get('handled') is False,v)
    await cmd('keymap.set',dict(commandId='edit.undo',keys='F6',onConflict='override'))
    v=await e.call(s,'key',{'keys':'F6'}); err=v.get('error') or {}; check('disabled Undo typed reason',err.get('code')=='command.disabled' and bool((err.get('details') or {}).get('reason')),v)
    await cmd('keymap.set',dict(commandId='ui.statusBar.set',params={'visible':False},keys='F7',onConflict='override'))
    await cmd('ui.preferences.open',{'category':'interface'})
    await poll(tree,lambda t:e.by_id(t.get('root') or {},'preferences'))
    v=await e.call(s,'key',{'keys':'F7'}); check('agent key under modal ui.modalOpen',(v.get('error') or {}).get('code')=='ui.modalOpen',v)
    e.os_escape(proc.pid); await poll(tree,lambda t:e.by_id(t.get('root') or {},'preferences') is None)
    v=await e.call(s,'key',{'keys':'F7'}); check('same key after OS Escape works',v.get('status')=='ok' and (v.get('result') or {}).get('visible') is False,v)
    await poll(tree,lambda t:not t.get('stale'))
    v=await cmd('window.panel.set',dict(panelId='history',floating=True,visible=True)); check('float command accepted',v.get('status')=='ok',v)
    t=await tree(); check('new float never falsely fresh without float', (t.get('stale') is True) or (isinstance(t.get('renderedSeq'),int) and e.by_id(t.get('root') or {},'float-history') is not None),t)
    v=await cmd('agent.policy.defaults.set',{'maxJobs':4}); receipt.append(dict(observation='MCP app scope refusal',envelope=v))
    for key,cid,params in [('F8','agent.policy.defaults.set',{'maxJobs':4}),('F9','agent.server.set',{'enabled':False}),('F10','agent.server.set',{'enabled':True})]:
     v=await cmd('keymap.set',dict(keys=key,commandId=cid,params=params,onConflict='override')); assert v.get('status')=='ok',v
    e.os_escape(proc.pid); f.u.keybd_event(0x77,0,0,0); f.u.keybd_event(0x77,0,2,0)
    async def policy(): return await cmd('prefs.get',{'keys':['agent.policy.defaults']})
    v=await poll(policy,lambda v:((v.get('result') or {}).get('values',{}).get('agent.policy.defaults') or {}).get('maxJobs')==4)
    check('single policy member user shortcut applied',((v.get('result') or {}).get('values',{}).get('agent.policy.defaults') or {}).get('maxJobs')==4,v)
   e.stop(proc); proc=e.launch(app,user,disc); d=e.wait_discovery(disc,proc)
   async with f.session(d) as s:
    v=await e.run_cmd(s,'prefs.get',{'keys':['agent.policy.defaults']}); expected={**seed,'maxJobs':4}; got=(v.get('result') or {}).get('values',{}).get('agent.policy.defaults'); check('policy siblings survive restart',got==expected,dict(expected=expected,actual=got))
    e.os_escape(proc.pid); f.u.keybd_event(0x79,0,0,0); f.u.keybd_event(0x79,0,2,0)
    async def enabled(): return await e.run_cmd(s,'prefs.get',{'keys':['agent.server.enabled']})
    v=await poll(enabled,lambda v:(v.get('result') or {}).get('values',{}).get('agent.server.enabled') is True)
    check('user enable shortcut reaches preference',(v.get('result') or {}).get('values',{}).get('agent.server.enabled') is True,v)
    e.os_escape(proc.pid); f.u.keybd_event(0x78,0,0,0); f.u.keybd_event(0x78,0,2,0)
    url=urlsplit(d['endpoint'])
    def responding():
     conn=http.client.HTTPConnection(url.hostname,url.port,timeout=.25)
     try:
      conn.request('DELETE',url.path,headers={'Authorization':'Bearer '+d['token'],'Mcp-Session-Id':'absent'}); r=conn.getresponse(); r.read(); return True
     except OSError: return False
     finally: conn.close()
    start=time.monotonic(); end=start+3
    while time.monotonic()<end:
     live=responding(); advertised=bool(json.loads(disc.read_text()).get('endpoint'))
     if not live and not advertised: break
    if live:
     v=await enabled(); check('disable shortcut cleared preference',(v.get('result') or {}).get('values',{}).get('agent.server.enabled') is False,v)
    check('disabled endpoint stops and discovery clears within 3s',not live and not advertised,dict(durationS=time.monotonic()-start,responding=live,advertised=advertised))
    e.os_escape(proc.pid); f.u.keybd_event(0x79,0,0,0); f.u.keybd_event(0x79,0,2,0)
    end=time.monotonic()+10
    while time.monotonic()<end:
     new=json.loads(disc.read_text())
     if new.get('endpoint'): break
    async with f.session(new) as fresh:
     v=await e.run_cmd(fresh,'agent.server.status')
     check('reenable with same bearer works',v.get('status')=='ok' and new['token']==d['token'],v)
  finally: e.stop(proc)
 out.write_text(json.dumps(receipt,indent=2),encoding='utf-8',newline='\n'); return int(any(not r.get('ok',True) for r in receipt))
if __name__=='__main__':
 if len(sys.argv)<2 or not Path(sys.argv[1]).is_file(): print('skipped: no application'); sys.exit(0)
 sys.exit(asyncio.run(main(str(Path(sys.argv[1]).resolve()),Path(sys.argv[2]))))
