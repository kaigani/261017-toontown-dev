"""Independent live closure probe. Uses only a disposable settings directory."""
import asyncio,importlib.util,json,sys,tempfile,time,threading,statistics,os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
sp=importlib.util.spec_from_file_location('floats',ROOT/'tests/adversarial/c41-r2-desktop-floats.py'); f=importlib.util.module_from_spec(sp); sp.loader.exec_module(f); e=f.e
async def main(app,out):
 rows=[]
 def check(name,ok,detail=None):
  rows.append(dict(check=name,ok=bool(ok),detail=detail)); out.write_text(json.dumps(rows,indent=2),encoding='utf-8',newline='\n'); print(name,ok,flush=True)
 async def poll(fn,pred,bound=15):
  end=time.monotonic()+bound; v=None
  while time.monotonic()<end:
   v=await fn()
   if pred(v): break
  return v
 with tempfile.TemporaryDirectory(prefix='tt-r6-') as temp:
  user=Path(temp); (user/'Settings').mkdir(); (user/'Settings/preferences.json').write_text(json.dumps(dict(format='toontown-settings',kind='preferences',formatVersion='1.0',data={'agent.policy.defaults':{'policy':'autonomous'}})),encoding='utf-8')
  disc=user/'discovery.json'; proc=e.launch(app,user,disc)
  try:
   d=e.wait_discovery(disc,proc)
   async with f.session(d) as s:
    async def cmd(cid,p=None): return await e.run_cmd(s,cid,p)
    async def tree(): return (await e.tree(s)).get('result') or {}
    def walk(n):
     yield n
     for c in n.get('children') or []: yield from walk(c)
    ids=['edit.undo','edit.redo','project.save','edit.copy','edit.cut','edit.paste','edit.delete']
    await cmd('window.toolbar.items.set',{'items':ids})
    for i,cid in enumerate(ids): await cmd('keymap.set',dict(commandId=cid,keys='F'+str(i+13),onConflict='override'))
    for state in ['none','new','edit','undo']:
     if state=='new': check('create project',(await cmd('project.create',{'name':'R6'})).get('status')=='ok')
     if state=='edit':
      v=await cmd('element.create',{'type':'peg','count':1}); check('edit fixture',v.get('status')=='ok',v)
      r=v.get('result') or {}; created=r.get('elementIds') or r.get('createdIds') or []
      if created: await cmd('edit.selection.set',{'context':'staging','ids':created})
     if state=='undo': check('undo fixture',(await cmd('edit.undo')).get('status')=='ok')
     t=await poll(tree,lambda t:not t.get('stale') and t.get('source')=='gui')
     for i,cid in enumerate(ids):
      nodes=[n for n in walk(t.get('root') or {}) if (n.get('command') or {}).get('commandId')==cid]
      # Native menus are included by the host; preserve every reference's fixed parameters.
      check(state+' '+cid+' present',bool(nodes))
      for n in nodes:
       dry=await e.call(s,'run_command',{'id':cid,'params':(n.get('command') or {}).get('params') or {},'dryRun':True})
       code=(dry.get('error') or {}).get('code'); allowed=dry.get('status')=='ok' or code in ['project.pathRequired','project.unsavedChanges']
       enabled=n.get('enabled',True); check(state+' '+cid+' parity',enabled==allowed,dict(node=n,dry=dry))
       if not enabled:
        key=await e.call(s,'key',{'keys':'F'+str(i+13)}); err=key.get('error') or {}; reason=n.get('disabledReason') or n.get('reason')
        check(state+' '+cid+' disabled key',err.get('code')=='command.disabled' and (not reason or (err.get('details') or {}).get('reason')==reason),key)
    async def measure(label):
     result={}
     for cid,count in [('app.state.get',50),('app.frame.set',20)]:
      times=[]
      for i in range(count):
       start=time.monotonic(); v=await cmd(cid,{'frame':i%2+1} if cid=='app.frame.set' else {}); times.append((time.monotonic()-start)*1000)
       assert v.get('status')=='ok',v
      result[cid]=dict(medianMs=statistics.median(times),worstMs=max(times),samples=times)
     check(label+' latency',True,result); return result
    empty=await measure('empty')
    batches=[]
    for i in range(4):
     v=await e.call(s,'batch',{'label':'R6 large fixture','steps':[{'id':'element.create','params':{'type':'peg','count':99}} for _ in range(6)]})
     batches.append(v); assert v.get('status')=='ok',v
    check('2376 elements created in four batches',True,[v.get('status') for v in batches])
    large=await measure('large')
    check('ADR-0021 decision 9 responsiveness',large['app.frame.set']['worstMs']<=100+empty['app.frame.set']['worstMs'],dict(empty=empty,large=large))
    for key,enabled in [('F9',False),('F10',True)]: await cmd('keymap.set',dict(keys=key,commandId='agent.server.set',params={'enabled':enabled},onConflict='override'))
   stop=threading.Event(); errors=[]; reads=[0]; endpoints=[0]
   def reader():
    while not stop.is_set():
     try:
      raw=disc.read_bytes(); v=json.loads(raw); reads[0]+=1
      if len(raw)!=4096: errors.append('size '+str(len(raw)))
      if v.get('endpoint'):
       endpoints[0]+=1
       if v.get('processId')!=proc.pid or proc.poll() is not None: errors.append('dead or wrong pid '+str(v.get('processId')))
     except Exception as ex:
      if len(errors)<30: errors.append(type(ex).__name__+': '+str(ex))
   th=threading.Thread(target=reader); th.start()
   try:
    for i in range(20):
     for vk,enabled in [(0x78,False),(0x79,True)]:
      e.os_escape(proc.pid); f.u.keybd_event(vk,0,0,0); f.u.keybd_event(vk,0,2,0)
      end=time.monotonic()+10; matched=False
      while time.monotonic()<end:
       try:
        new=json.loads(disc.read_bytes())
        if bool(new.get('endpoint'))==enabled: matched=True; break
       except (ValueError,OSError): pass
      check('toggle '+str(i)+' '+str(enabled),matched)
   finally: stop.set(); th.join()
   check('discovery always complete',not errors,dict(reads=reads[0],endpointReads=endpoints[0],errors=errors[:30]))
   new=json.loads(disc.read_bytes()); same=new['token']==d['token']; new['token']=d['token']
   async with f.session(new) as s: v=await e.run_cmd(s,'agent.server.status'); check('same bearer after twenty toggles',same and v.get('status')=='ok',v)
  finally: e.stop(proc)
 return int(any(not r['ok'] for r in rows))
if __name__=='__main__': sys.exit(asyncio.run(main(str(Path(sys.argv[1]).resolve()),Path(sys.argv[2]))))
