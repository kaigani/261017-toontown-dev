"""Scoped C4.4b closure; real OS input via unchanged prior helpers."""
import importlib.util, inspect, json, os, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EVID=ROOT/'reviews/evidence/M4-C4.4b-r4-review'
def load(n,p):
 s=importlib.util.spec_from_file_location(n,ROOT/p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def save(n,v):
 EVID.mkdir(parents=True,exist_ok=True);s=json.dumps(v,indent=2,ensure_ascii=True)+'\n';assert len(s.encode())<200000;(EVID/n).write_text(s,encoding='utf-8',newline='\n')
r=load('r4previous','tests/adversarial/c44b-r3-live.py');r.EVID=EVID
BRANCH=r'''
 for kind in ('element','drawing','hidden'):
  for action in (('anchorDelete','selectDelete','nativeDelete','nativeCut','contextCut','brush') if kind=='element' else ('anchorDelete','brush')):
   eid,did=await fixture('cameraView','contourEditor' if action=='anchorDelete' else 'brush' if action=='brush' else 'select')
   sid=(await c('draw.selection.get'))['selectedIds'][0]
   t.click('cameraView.canvas');await c('draw.selection.set',{'by':'ids','ids':[sid]})
   await c('edit.selection.set',{'context':'drawing','ids':[sid]})
   if action=='anchorDelete':await c('draw.pointSelection.set',{'points':[{'strokeId':sid,'anchor':1}]})
   if kind=='element':await c('element.lock.set',{'elementIds':[eid],'locked':True})
   elif kind=='drawing':await c('draw.lock.set',{'locked':True})
   else:
    await c('drawAid.selectionDisplay.set',{'keepSelectionVisible':False,'outlineOnSelection':False});await c('element.visibility.set',{'elementIds':[eid],'scope':'scene','visible':False})
   await pause();before=await c('doc.export.json',{'scope':'project','binaries':'omit'});pos=await t.position();t.clear();d.front()
   if action in ('anchorDelete','selectDelete'):d.key(0x2e)
   elif action.startswith('native'):d.choose_menu(['Edit','Delete' if action=='nativeDelete' else 'Cut'])
   elif action=='brush':await gesture('cameraView',[(-20,0),(20,20)])
   else:
    rr=t.rect('cameraView.canvas');o=wintypes.POINT();d.u32.ClientToScreen(d.window_handle(),ctypes.byref(o));d.right_click_screen(o.x+rr['x']*rr['dpr'],o.y+rr['y']*rr['dpr']);await pause()
    def flatten(n):return [n]+[x for ch in n.get('children',[]) for x in flatten(ch)]
    items=[x for x in flatten(await d.McpTable.tree(t)) if x.get('role')=='menuitem' and x.get('command',{}).get('commandId')=='edit.cut'];assert items;t.click(items[0]['id'])
   await pause();sent=t.recorded();status=t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent")
   dialogs=t.cdp.js("[...document.querySelectorAll('[role=alertdialog],[role=dialog]')].map(e=>e.textContent)")
   after=await c('doc.export.json',{'scope':'project','binaries':'omit'});history=await t.steps_since(pos)
   label='Delete points' if action=='anchorDelete' else 'Delete' if action in ('selectDelete','nativeDelete') else 'Cut' if action in ('nativeCut','contextCut') else 'Add stroke'
   reason='hidden' if kind=='hidden' else 'locked'
   check(label.lower() in str(status).lower() and reason in str(status).lower(),kind+' '+action+' label and reason',status)
   before['bundle'].pop('exportedAt',None);after['bundle'].pop('exportedAt',None)
   check(not dialogs and before==after and not history,kind+' '+action+' no dialog/document/undo change',{'dialogs':dialogs,'history':history})
   edits=[x for x in sent if t.kinds.get(x['command']) not in ('query',) and x['command'] not in ('view.active.set','draw.selection.set','draw.pointSelection.set')]
   check(not edits or (all(x['command'] in ('edit.delete','edit.cut') for x in edits) and before==after and not history),kind+' '+action+' no successful edit sent',edits);d.key(0x1b)
'''
def matrix():
 p=load('r4base','tests/adversarial/c44a-r1-live.py');p.EVID=Path(tempfile.mkdtemp());old=load('r4matrix','tests/adversarial/c44b-r2-live.py');old.PRIOR=p
 s=inspect.getsource(old.matrix);s=s[:s.index(' if step==5:')]+BRANCH;ns=dict(old.__dict__);exec(s,ns);p.live_body=ns['matrix'];result=p.live(2)
 rows=json.loads((p.OUT/'transcript.json').read_text());save('step2.json',{'raw':str(p.OUT),'checks':[x for x in rows if 'check' in x],'failures':[x for x in rows if x.get('ok')==False]});return result
if __name__=='__main__':
 assert sys.argv[1]=='p1';step=int(sys.argv[2])
 if step==1:
  rows=[]
  for n in (2,3):
   raw=Path(tempfile.mkdtemp(prefix='r4-prior-'))
   with (raw/'console.txt').open('w',encoding='utf-8') as f:q=subprocess.run([sys.executable,str(ROOT/'tests/adversarial/c44b-r3-live.py'),'n1',str(n)],cwd=ROOT,stdout=f,stderr=subprocess.STDOUT,timeout=530,env={**os.environ,'PYTHONIOENCODING':'utf-8'})
   rows.append({'probe':'n1 '+str(n),'exit':q.returncode,'log':str(raw/'console.txt'),'failLines':[x for x in (raw/'console.txt').read_text(encoding='utf-8').splitlines() if 'FAIL' in x]})
   for path in (ROOT/'reviews/evidence/M4-C4.4b-r3-review').glob('step'+str(n)+'*.json'):save('prior-'+path.name,json.loads(path.read_text()))
   subprocess.run(['git','checkout','--','reviews/evidence/M4-C4.4b-r3-review'],cwd=ROOT,check=True)
  save('step1.json',rows);sys.exit(any(x['exit'] for x in rows))
 if step==2:sys.exit(matrix())
 if step==5:sys.exit(r.run_regression() or r.run_text(8))
import asyncio, ctypes, time, socket
async def lock_body(t,step):
 p=BASE;d=p.d;c=t.cmd;check=t.run.check;t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await c('project.create',{'name':'Lock closure'})
 project=t.work/'closure.animpkg';await c('project.saveAs',{'path':str(project)});await c('project.close',{'onUnsaved':'discard'})
 lock=Path(str(project)+'.lock');helper=Path(str(lock)+'.takeover')
 original=json.dumps({'user':'review','host':socket.gethostname(),'processId':os.getpid(),'appVersion':'0.0.1','openedAt':'2000-01-01T00:00:00.000Z','heartbeatAt':'2000-01-01T00:00:00.000Z','lockId':'stale-r4'}).encode()
 lock.write_bytes(original);helper.write_bytes(b'left-behind')
 k=ctypes.WinDLL('kernel32',use_last_error=True);k.CreateFileW.argtypes=[ctypes.c_wchar_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p];k.CreateFileW.restype=ctypes.c_void_p;k.CloseHandle.argtypes=[ctypes.c_void_p]
 handle=k.CreateFileW(str(helper),0x80000000,7,None,3,0,None);assert handle!=ctypes.c_void_p(-1).value
 def close():
  nonlocal handle
  if handle:k.CloseHandle(handle);handle=None
 def dom(i):return t.cdp.js('document.querySelector('+json.dumps('[data-tt-id="'+i+'"]')+')?.textContent')
 async def native_open():
  start=time.monotonic();d.choose_menu(['File','Open']);await asyncio.sleep(1);d.type_text(str(project));d.key(0x0d)
  for _ in range(450):
   if dom('message.project.lockFileInUse'):break
   await asyncio.sleep(.1)
  elapsed=time.monotonic()-start;message=dom('message.project.lockFileInUse')
  check(bool(message) and str(helper) in message and 'Try Again' in message and 'Cancel' in message and 'not writable' not in message.lower() and 'open in another session' not in message.lower(),'file in use named with choices',{'seconds':elapsed,'message':message})
  check(elapsed<16,'open answers within about 14 seconds',elapsed)
 try:
  if step==3:
   await native_open();t.click('message.cancel');await asyncio.sleep(.7)
   state=await c('app.state.get');check(not state.get('project') and lock.read_bytes()==original,'Cancel leaves no project and unchanged lock',state)
   await native_open();close();t.click('message.retry');await asyncio.sleep(3)
   state=await c('app.state.get');check(bool(state.get('project')),'Try Again opens after reader closes',state)
  else:
   q=p.IndependentHttp(json.loads((t.work/'discovery.json').read_text()),'r4-lock');q.initialize()
   q2=p.IndependentHttp(json.loads((t.work/'discovery.json').read_text()),'r4-query');q2.initialize()
   for client in (q,q2):
    state=await asyncio.to_thread(client.command,'app.state.get');sid=state['result']['agent']['sessionId'];t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command',{json:"+json.dumps(json.dumps({'command':'agent.session.grant','params':{'sessionId':sid,'roots':[str(t.work)]}}))+"})")
   async def call(client,n,params=None):
    result=await asyncio.to_thread(client.command,n,params)
    if result.get('result',{}).get('jobId'):result=await asyncio.to_thread(client.tool,'job_wait',{'jobId':result['result']['jobId'],'timeoutMs':45000})
    return result
   try:
    start=time.monotonic();task=asyncio.create_task(call(q,'project.open',{'path':str(project)}));await asyncio.sleep(1)
    answer=await asyncio.wait_for(call(q2,'app.state.get'),10);check(answer.get('status')=='ok' and not task.done(),'other MCP call answered while open waits',answer)
    result=await asyncio.wait_for(task,45);elapsed=time.monotonic()-start
    check('fs.ioError' in json.dumps(result) and 'lockFileInUse' in json.dumps(result) and str(helper) in json.dumps(result).replace('\\\\','\\'),'MCP code reason path',result)
    check(elapsed<16 and lock.read_bytes()==original,'MCP bounded and lock unchanged',elapsed)
    close();result=await call(q,'project.open',{'path':str(project)});check(result.get('status')=='ok','MCP retry succeeds',result)
   finally:q.close();q2.close()
 finally:close()
def lock_run(step):
 global BASE
 BASE=load('r4lockbase','tests/adversarial/c44a-r1-live.py');BASE.EVID=Path(tempfile.mkdtemp());BASE.live_body=lock_body;result=BASE.live(step)
 rows=json.loads((BASE.OUT/'transcript.json').read_text());save('step'+str(step)+'.json',{'raw':str(BASE.OUT),'checks':[x for x in rows if 'check' in x],'failures':[x for x in rows if x.get('ok')==False]});return result
if __name__=='__main__' and int(sys.argv[2]) in (3,4):sys.exit(lock_run(int(sys.argv[2])))
