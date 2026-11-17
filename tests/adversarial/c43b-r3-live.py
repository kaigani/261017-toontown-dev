"""Independent round-three live review. Usage: python FILE APP a1|a2|a3."""
import asyncio, contextlib, importlib.util, json, os, subprocess, sys, tempfile, time, hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(os.environ.get('REVIEW_RAW') or tempfile.mkdtemp(prefix='c43b-r3-'))
def load(name,path):
 s=importlib.util.spec_from_file_location(name,ROOT/path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
async def wait(t,ident):
 for _ in range(100):
  if t.cdp.js("!!document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')"%json.dumps(ident)):return
  await asyncio.sleep(.1)
 raise RuntimeError('missing '+ident)
async def exercise(t,op,choice,input_kind='click'):
 c=t.cmd;work=t.work
 await c('prefs.set',{'values':{'files.backups.count':5}})
 await c('project.create',{'name':'Other','onUnsaved':'discard'})
 other=work/'Other.animpkg';flow=work/'Review.animpkg'
 await c('project.saveAs',{'path':str(other),'onExists':'overwrite'})
 await c('project.create',{'name':'Review','onUnsaved':'discard'})
 await c('project.saveAs',{'path':str(flow),'onExists':'overwrite'})
 await c('element.create',{'type':'drawing','name':'SavedMarker'})
 await c('project.save',{})
 await c('element.create',{'type':'drawing','name':'UnsavedMarker'})
 before=flow.read_bytes();(OUT/(op+'-'+choice+'-'+input_kind+'-before.animpkg')).write_bytes(before);doc=await c('doc.export.json',{'scope':'project','binaries':'omit'})
 t.clear();d.choose_menu(['File']+({'New':['New Project'],'Open':['Open'],'Recent':['Open Recent','Other'],'Backup':['Open Backup'],'Close':['Close Project'],'Quit':['Quit']}[op]))
 await wait(t,'message.project.unsavedChanges')
 focus=t.cdp.js('document.activeElement?.dataset.ttId')
 if input_kind=='click':t.click('message.'+choice)
 else:d.key(0x0D if input_kind=='enter' else 0x1B)
 await asyncio.sleep(.7)
 if choice!='cancel':
  if op=='New':
   await wait(t,'newProject.name');t.click('newProject.name');d.key(0x41,0x11);d.type_text('Next');t.click('newProject.create')
  elif op=='Open':
   await asyncio.sleep(1);d.type_text(str(other));d.key(0x0D)
  elif op=='Backup':
   await wait(t,'openBackup.open.0');t.click('openBackup.open.0')
 await asyncio.sleep(2)
 calls=t.recorded(); sent=[x for x in calls if x.get('command','').startswith('project.') or x.get('command')=='app.quit']
 sent=[x for x in sent if x.get('command') in ('project.save','project.create','project.open','project.backup.open','project.close','app.quit')]
 after=flow.read_bytes();(OUT/(op+'-'+choice+'-'+input_kind+'-after.animpkg')).write_bytes(after);state=None;afterdoc=None
 if op!='Quit' or choice=='cancel':
  state=await c('app.state.get',{});afterdoc=await c('doc.export.json',{'scope':'project','binaries':'omit'}) if state.get('project') else None
 command={'New':'project.create','Open':'project.open','Recent':'project.open','Backup':'project.backup.open','Close':'project.close','Quit':'app.quit'}[op]
 expected=[] if choice=='cancel' else (['project.save',command] if choice=='save' else [command])
 ok=[x['command'] for x in sent]==expected and ((before!=after) if choice=='save' else before==after)
 if choice=='cancel':ok=ok and afterdoc==doc and state['project']['dirty']
 else:
  params=sent[-1]['params'] if sent else {}
  if afterdoc is not None:ok=ok and 'UnsavedMarker' not in json.dumps(afterdoc)
  ok=ok and (params.get('onUnsaved')=='discard' if choice=='discard' else 'onUnsaved' not in params)
  if op in ('Open','Recent'):ok=ok and state['project']['path']==str(other)
  elif op=='Close':ok=ok and not state.get('project')
  elif op=='New':ok=ok and state['project']['path'] is None
  elif op=='Backup':ok=ok and state['project']['path'] is None
 rec={'operation':op,'choice':choice,'input':input_kind,'focus':focus,'sent':sent,'beforeBytes':len(before),'afterBytes':len(after),'beforeHash':hashlib.sha256(before).hexdigest(),'afterHash':sha(flow),'beforeDoc':doc,'afterDoc':afterdoc,'state':state,'ok':bool(ok)}
 t.run.save(op+'-'+choice+'-'+input_kind+'.json',rec);t.run.check(bool(ok),op+' '+choice+' '+input_kind,rec)
async def a2_body(t,op,choice,kind):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await exercise(t,op,choice,kind)
def a2():
 run=d.e2e.Run(OUT);d.e6.dpi_aware()
 for op in ('New','Open','Recent','Backup','Close','Quit'):
  for choice,kind in [('save','click'),('discard','click'),('cancel','click'),('save','enter'),('cancel','escape')]:
   work=Path(tempfile.mkdtemp(prefix='r3-flow-'))
   async def body(t):await a2_body(t,op,choice,kind)
   d.build_and_run=lambda t,only:body(t)
   try:asyncio.run(d.session_all(APP,run,OUT,work,set()))
   except Exception as ex:
    if not (op=='Quit' and choice!='cancel' and run.transcript and run.transcript[-1].get('ok')):run.check(False,op+' '+choice+' '+kind+' ran',repr(ex))
 run.save('transcript.json',run.transcript);return int(bool(run.failures))
def activate_new_windows(stop):
 import ctypes,time,os
 from ctypes import wintypes
 u=ctypes.windll.user32;k=ctypes.windll.kernel32
 u.GetForegroundWindow.restype=wintypes.HWND
 k.OpenProcess.restype=wintypes.HANDLE
 k.QueryFullProcessImageNameW.argtypes=[wintypes.HANDLE,wintypes.DWORD,wintypes.LPWSTR,ctypes.POINTER(wintypes.DWORD)]
 k.CloseHandle.argtypes=[wintypes.HANDLE]
 msg=wintypes.MSG();u.PeekMessageW(ctypes.byref(msg),None,0,0,0)
 app=os.path.normcase(APP)
 def visit(h,_):
  if not u.IsWindowVisible(h):return True
  r=wintypes.RECT();u.GetWindowRect(h,ctypes.byref(r))
  if r.right-r.left<800 or r.bottom-r.top<500:return True
  pid=wintypes.DWORD();tid=u.GetWindowThreadProcessId(h,ctypes.byref(pid))
  p=k.OpenProcess(0x1000,False,pid.value)
  if not p:return True
  b=ctypes.create_unicode_buffer(2048);n=wintypes.DWORD(2048);ok=k.QueryFullProcessImageNameW(p,0,b,ctypes.byref(n));k.CloseHandle(p)
  if not ok or os.path.normcase(b.value)!=app:return True
  fg=u.GetForegroundWindow();fp=wintypes.DWORD();ft=u.GetWindowThreadProcessId(fg,ctypes.byref(fp))
  if fp.value==pid.value:return True
  own=k.GetCurrentThreadId();a=u.AttachThreadInput(own,ft,True);a2=u.AttachThreadInput(own,tid,True)
  u.ShowWindow(h,9);u.BringWindowToTop(h);u.SetForegroundWindow(h)
  if a2:u.AttachThreadInput(own,tid,False)
  if a:u.AttachThreadInput(own,ft,False)
  print('activate',h,fg,u.GetForegroundWindow(),a,a2,flush=True)
  return True
 cb=ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(visit)
 while not stop.wait(.1):u.EnumWindows(cb,0)
def repeat_input():
 rows=[]
 tasks=[('c43b-r1-live.py',['case7',name]) for name in ('r2','r2-exact','r2-exact-select','r2-exact-tools','r3-labels')]
 if os.environ.get('REVIEW_REPEAT_ONLY'):tasks=[x for x in tasks if x[1][-1] in os.environ['REVIEW_REPEAT_ONLY'].split(',')]
 if not os.environ.get('REVIEW_REPEAT_ONLY'):tasks += [('c43b-r2-live.py',[name]) for name in ('a3','b3')]
 try:
  for script,args in tasks:
   log=OUT/(script+'-'+'-'.join(args)+'.log')
   with log.open('w',encoding='utf-8') as f:
    proc=subprocess.run([sys.executable,str(ROOT/'tests/adversarial'/script),APP,*args],cwd=ROOT,env={**os.environ,'PYTHONIOENCODING':'utf-8','REVIEW_RAW':str(OUT/'raw')},stdout=f,stderr=subprocess.STDOUT)
   rows.append({'probe':script+' '+' '.join(args),'exit':proc.returncode,'log':str(log)})
   (OUT/'repeat-summary.json').write_text(json.dumps(rows,indent=1),encoding='utf-8')
   print(rows[-1],flush=True)
 finally:pass
 return int(any(x['exit'] for x in rows))
def a1():
 if '--first-click' in sys.argv:return first_click()
 if '--repeat-input' in sys.argv:return repeat_input()
 rows=[]
 for script,cases in [('c43b-r1-live.py',['case3','case4','case5','case6','case7','case8']),('c43b-r2-live.py',['a2','a3','b1','b2','b3'])]:
  for case in cases:
   log=OUT/(script+'-'+case+'.log')
   with log.open('w',encoding='utf-8') as f:
    p=subprocess.run([sys.executable,str(ROOT/'tests/adversarial'/script),APP,case],cwd=ROOT,env={**os.environ,'PYTHONIOENCODING':'utf-8','REVIEW_RAW':str(OUT/'raw')},stdout=f,stderr=subprocess.STDOUT)
   rows.append({'probe':script+' '+case,'exit':p.returncode,'failLines':[l for l in log.read_text(encoding='utf-8',errors='replace').splitlines() if 'FAIL' in l]})
   (OUT/'summary.json').write_text(json.dumps(rows,indent=2),encoding='utf-8')
 return int(any(r['exit'] for r in rows))
async def first_click_body(t):
 import ctypes
 from ctypes import wintypes
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await t.cmd('project.create',{'name':'First click review'})
 await asyncio.sleep(1)
 t.cdp.js("window.reviewEvents=[]; ['pointerdown','pointerup','click','focusin','focusout'].forEach(k=>document.addEventListener(k,e=>reviewEvents.push({type:k,target:e.target.dataset?.ttId,tag:e.target.tagName}),true))")
 u=ctypes.windll.user32;u.GetForegroundWindow.restype=wintypes.HWND
 rows=[]
 for i in range(3):
  t.clear();h=d.front();fg=u.GetForegroundWindow()
  before=await t.cmd('drawAid.state.get',{})
  t.click('mainToolbar.drawAid.grid.set');await asyncio.sleep(1)
  after=await t.cmd('drawAid.state.get',{})
  rows.append({'iteration':i,'main':h,'foreground':fg,'foregroundAfter':u.GetForegroundWindow(),'before':before,'after':after,'sent':t.recorded(),'events':t.cdp.js('reviewEvents.splice(0)')})
 t.run.save('first-click.json',rows)
def first_click():
 run=d.e2e.Run(OUT);d.e6.dpi_aware();work=Path(tempfile.mkdtemp(prefix='r3-first-click-'))
 d.build_and_run=lambda t,only:first_click_body(t)
 asyncio.run(d.session_all(APP,run,OUT,work,set()))
 return 0
async def modal(t,ident):
 before=await t.cmd('doc.export.json',{'scope':'project','binaries':'omit'})
 t.clear()
 focus=[]
 for _ in range(20):
  d.key(0x09);await asyncio.sleep(.03)
  focus.append(t.cdp.js("!!document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.contains(document.activeElement)"%json.dumps(ident)))
 d.key(0x42)
 d.key(0x5A,ctrl=True) if False else None
 d.key(0x5A,0x11)
 await asyncio.sleep(.2)
 after=await t.cmd('doc.export.json',{'scope':'project','binaries':'omit'})
 sent=t.recorded();bad=[x for x in sent if t.kinds.get(x.get('command')) in ('action','ui','toggle','job')]
 t.run.check(all(focus) and before==after and not bad,'modal keys and Tab '+ident,{'focus':focus,'sent':sent})
async def a3_body(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await c('project.create',{'name':'DialogReview'})
 async def prefs():
  d.choose_menu(['File','Preferences']);await wait(t,'preferences.category.keyboard');t.click('preferences.category.keyboard');await asyncio.sleep(.5)
 async def finish():
  t.click('preferences.submit');await asyncio.sleep(.7)
 async def keys():return sorted(x['keys'] for x in await c('keymap.list',{'commandId':'project.save'}))
 path=t.work/'import.json'
 for choice in ('merge','replace','cancel','enter','space','escape'):
  await c('keymap.reset',{'all':True});await c('keymap.export',{'path':str(path),'onExists':'overwrite'})
  data=json.loads(path.read_text(encoding='utf-8'))
  for b in data['bindings']:
   if b['commandId']=='project.save':b['keys']='Primary+Shift+F11'
  path.write_text(json.dumps(data),encoding='utf-8')
  await prefs();t.click('keymap.import');await asyncio.sleep(1.8);d.type_text(str(path));d.key(0x0D)
  await wait(t,'message.keymap.importMode')
  focused=t.cdp.js('document.activeElement?.dataset.ttId')
  if choice in ('merge','replace','cancel'):await modal(t,'message.keymap.importMode');t.click('message.'+choice)
  else:d.key(0x0D if choice=='enter' else 0x20 if choice=='space' else 0x1B)
  await asyncio.sleep(.5);await finish();actual=await keys()
  wanted=['Primary+S','Primary+Shift+F11'] if choice in ('merge','enter','space') else ['Primary+Shift+F11'] if choice=='replace' else ['Primary+S']
  t.run.check(actual==wanted and focused=='message.merge','Import '+choice,{'keys':actual,'focused':focused,'sent':t.recorded()})
 for choice in ('reset','cancel','enter','escape'):
  await c('keymap.reset',{'all':True});await c('keymap.set',{'commandId':'project.save','keys':'Primary+Shift+F11','replaceExisting':True})
  await prefs();t.click('keymap.resetAll');await wait(t,'message.keymap.resetAll');t.clear()
  if choice in ('reset','cancel'):await modal(t,'message.keymap.resetAll');t.click('message.'+choice)
  else:
   d.key(0x0D if choice=='enter' else 0x1B)
   await asyncio.sleep(.3)
   if choice=='enter':
    stuck=t.cdp.js("!!document.querySelector('[data-tt-id=\"message.keymap.resetAll\"]')")
    t.run.check(not stuck,'Reset default Enter answers the question',{'stuck':stuck,'focus':t.cdp.js('document.activeElement?.dataset.ttId')})
    if stuck:d.key(0x1B)
  await asyncio.sleep(.4);await finish()
  t.run.check(await keys()==(['Primary+S'] if choice=='reset' else ['Primary+Shift+F11']),'Reset '+choice,t.recorded())
 await c('keymap.reset',{'all':True})
 for choice in ('override','cancel'):
  await prefs();t.click('keymap.filter');d.type_text('app.info.get');await asyncio.sleep(.5);t.click('keymap.command.app.info.get');t.click('keymap.capture')
  d.key(0x0D,0x10);await wait(t,'keymap.conflict');t.clear()
  await modal(t,'keymap.conflict');t.click('keymap.conflict.'+choice);await finish()
  actual=await c('keymap.list',{'commandId':'app.info.get'})
  t.run.check(any(x['keys']=='Shift+Enter' for x in actual)==(choice=='override'),'Conflict '+choice,{'bindings':actual,'sent':t.recorded()})
  await c('keymap.reset',{'all':True})
 for choice in ('replace','cancel'):
  await c('project.create',{'name':'Existing','onUnsaved':'discard'});target=t.work/('existing-'+choice+'.animpkg');await c('project.saveAs',{'path':str(target)})
  await c('project.create',{'name':'Replacement','onUnsaved':'discard'});before=target.read_bytes();t.clear()
  d.choose_menu(['File','Save As']);await asyncio.sleep(1.8);d.type_text(str(target));d.key(0x0D);await asyncio.sleep(1);d.key(0x59)
  await wait(t,'message.project.exists');await modal(t,'message.project.exists');t.click('message.'+choice);await asyncio.sleep(2)
  t.run.check((target.read_bytes()!=before)==(choice=='replace'),'Save As '+choice,{'sent':t.recorded(),'before':hashlib.sha256(before).hexdigest(),'after':sha(target)})
 await jobs(t)
async def jobs(t):
 discovery=json.loads((t.work/'discovery.json').read_text())
 before=await t.cmd('doc.export.json',{'scope':'project','binaries':'omit'})
 async with contextlib.AsyncExitStack() as stack:
  client=await stack.enter_async_context(d.e2e.httpx2.AsyncClient(headers={'Authorization':'Bearer '+discovery['token']},timeout=300))
  streams=await stack.enter_async_context(d.e2e.streamable_http_client(discovery['endpoint'],http_client=client))
  from mcp import types
  async def roots(_):return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='review')])
  agent=await stack.enter_async_context(d.e2e.ClientSession(streams[0],streams[1],list_roots_callback=roots));await agent.initialize()
  env=await d.e2e.run_cmd(agent,'render.range.run',{'from':1,'to':2400,'scale':2,'directory':str(t.work/'render'),'createDirs':True,'onConflict':'overwrite'})
  jid=env['result']['jobId'];await wait(t,'jobsWindow.cancel.'+jid)
  shown=t.cdp.js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.textContent"%json.dumps('jobsWindow.job.'+jid))
  t.clear();t.click('jobsWindow.cancel.'+jid)
  result=await d.e2e.call(agent,'job_wait',{'jobId':jid,'timeoutMs':120000});await asyncio.sleep(1)
  sent=t.recorded();after=await t.cmd('doc.export.json',{'scope':'project','binaries':'omit'})
  closed=t.cdp.js("!document.querySelector('[data-tt-id=jobsWindow]')")
  t.run.save('job-cancel.json',{'jobId':jid,'result':result,'shown':shown,'sent':sent,'before':before,'after':after,'closed':closed})
  t.run.check(bool(shown) and before==after and closed and any(x=={'command':'job.cancel','params':{'jobId':jid}} for x in sent) and result.get('result',{}).get('state')=='cancelled','Agent progress cancel',result)
def a3():
 run=d.e2e.Run(OUT);d.e6.dpi_aware();work=Path(tempfile.mkdtemp(prefix='r3-dialogs-'))
 d.build_and_run=lambda t,only:a3_body(t)
 try:asyncio.run(d.session_all(APP,run,OUT,work,set()))
 except Exception as ex:
  import traceback;traceback.print_exc();run.check(False,'A3 ran',repr(ex))
 run.save('transcript.json',run.transcript);return int(bool(run.failures))
if __name__=='__main__':
 if len(sys.argv)<3:sys.exit(0)
 APP=str(Path(sys.argv[1]).resolve());OUT.mkdir(parents=True,exist_ok=True)
 d=load('r3d39','scripts/c43-d39-bindings.py')
 server=subprocess.Popen([sys.executable,'-m','http.server','1420','--bind','127.0.0.1','--directory',str(ROOT/'desktop/dist')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 import threading
 stop=threading.Event();worker=None
 if '--first-click' not in sys.argv:
  worker=threading.Thread(target=activate_new_windows,args=(stop,),daemon=True);worker.start()
 try:sys.exit({'a1':a1,'a2':a2,'a3':a3}[sys.argv[2]]())
 finally:
  stop.set()
  if worker:worker.join()
  server.terminate();server.wait()