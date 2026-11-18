"""Independent round-three live review. Usage: python FILE APP a1|a2|a3."""
import asyncio, contextlib, importlib.util, json, os, subprocess, sys, tempfile, time, hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(os.environ.get('REVIEW_RAW') or tempfile.mkdtemp(prefix='c43b-r3-'))
def load(name,path):
 s=importlib.util.spec_from_file_location(name,ROOT/path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def spec_queries():
 import re
 names=set()
 for path in (ROOT/'spec').glob('*.md'):
  for line in path.read_text(encoding='utf-8').splitlines():
   match=re.match(r'\|\s*`([^`]+)`\s*\|\s*query\s*\|',line)
   if match:names.add(match.group(1))
 return names
SPEC_QUERIES=spec_queries()
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def content(doc):
 import copy
 out=copy.deepcopy(doc)
 if isinstance(out,dict) and isinstance(out.get('bundle'),dict):out['bundle'].pop('exportedAt',None)
 return out
async def preserve_fixture(t):
 state=await t.cmd('app.state.get',{})
 if (state.get('project') or {}).get('dirty'):
  n=len(list(t.work.glob('fixture-save-*.animpkg')))
  await t.cmd('project.saveAs',{'path':str(t.work/('fixture-save-'+str(n)+'.animpkg')),'onExists':'overwrite'})
async def modal_fixture(t,name):
 await preserve_fixture(t)
 await t.cmd('project.create',{'name':name,'onUnsaved':'fail'})
 eid=(await t.cmd('element.create',{'type':'drawing','name':'ModalUndoSentinel'}))['elementIds'][0]
 await t.cmd('element.drawing.create',{'elementId':eid,'name':'1','exposeFrom':1,'exposeTo':24})
 await t.cmd('app.context.set',{'elementId':eid,'frame':1})
 await t.cmd('workspace.select',{'name':'Drawing'})
 await t.cmd('view.active.set',{'viewId':'drawingView'})
 await t.cmd('draw.rectangle.add',{'from':{'x':-40,'y':-40},'to':{'x':40,'y':40}})
async def wait(t,ident):
 for _ in range(100):
  if t.cdp.js("!!document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')"%json.dumps(ident)):return
  await asyncio.sleep(.1)
 t.run.save('missing-'+ident+'.json',{'body':t.cdp.js('document.body.innerText'),'focus':t.cdp.js('document.activeElement?.outerHTML')})
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
 alive=None
 if op=='Quit':
  import ctypes
  from ctypes import wintypes
  k=ctypes.windll.kernel32;k.OpenProcess.restype=wintypes.HANDLE;k.WaitForSingleObject.argtypes=[wintypes.HANDLE,wintypes.DWORD];k.CloseHandle.argtypes=[wintypes.HANDLE]
  h=k.OpenProcess(0x100000,False,d.e2e.APP_PID['pid']);alive=bool(h and k.WaitForSingleObject(h,0)==258)
  if h:k.CloseHandle(h)
 command={'New':'project.create','Open':'project.open','Recent':'project.open','Backup':'project.backup.open','Close':'project.close','Quit':'app.quit'}[op]
 expected=[] if choice=='cancel' else (['project.save',command] if choice=='save' else [command])
 ok=[x['command'] for x in sent]==expected and ((before!=after) if choice=='save' else before==after)
 if choice=='cancel':ok=ok and content(afterdoc)==content(doc) and state['project']['dirty']
 else:
  params=sent[-1]['params'] if sent else {}
  if afterdoc is not None:ok=ok and 'UnsavedMarker' not in json.dumps(afterdoc)
  ok=ok and (params.get('onUnsaved')=='discard' if choice=='discard' else 'onUnsaved' not in params)
  if op in ('Open','Recent'):ok=ok and state['project']['path']==str(other)
  elif op=='Close':ok=ok and not state.get('project')
  elif op=='New':ok=ok and state['project']['path'] is None
  elif op=='Backup':ok=ok and state['project']['path'] is None
 if op=='Quit':ok=ok and alive==(choice=='cancel')
 rec={'alive':alive,'operation':op,'choice':choice,'input':input_kind,'focus':focus,'sent':sent,'beforeBytes':len(before),'afterBytes':len(after),'beforeHash':hashlib.sha256(before).hexdigest(),'afterHash':sha(flow),'beforeDoc':doc,'afterDoc':afterdoc,'state':state,'ok':bool(ok)}
 t.run.save(op+'-'+choice+'-'+input_kind+'.json',rec);t.run.check(bool(ok),op+' '+choice+' '+input_kind,rec)
async def a2_body(t,op,choice,kind):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await exercise(t,op,choice,kind)
def a2():
 run=d.e2e.Run(OUT);d.e6.dpi_aware()
 for op in os.environ.get('REVIEW_FLOWS','New,Open,Recent,Backup,Close,Quit').split(','):
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
 recorded_before=t.recorded()
 focus=[]
 for _ in range(20):
  d.key(0x09);await asyncio.sleep(.03)
  focus.append(t.cdp.js("(() => {const e=document.querySelector('[data-tt-id='+JSON.stringify(%s)+']');const root=e?.closest('[role=alertdialog],[role=dialog]')||e;return !!root?.contains(document.activeElement)})()"%json.dumps(ident)))
 if ident!='keymap.conflict':
  d.key(0x42)
  d.key(0x5A,0x11)
 route=[]
 for _ in range(100):
  a=t.cdp.js("({id:document.activeElement?.dataset.ttId,command:document.activeElement?.dataset.ttCommand,disabled:document.activeElement?.disabled})")
  route.append(a)
  if a.get('command')=='playback.loop.set' and not a.get('disabled'):
   d.key(0x20);break
  d.key(0x09)
 await asyncio.sleep(.2)
 after=await t.cmd('doc.export.json' ,{'scope':'project','binaries':'omit'})
 sent=t.recorded()[len(recorded_before):];bad=[x for x in sent if x.get('command') not in SPEC_QUERIES and x.get('command')!='review.dispatch_key']
 t.run.check(all(focus) and content(before)==content(after) and not bad,'modal keys and Tab '+ident,{'focus':focus,'tabRoute':route,'sent':sent,'contentUnchanged':content(before)==content(after),'unexpected':bad,'keyResolutionRequests':[x for x in sent if x.get('command')=='review.dispatch_key']})
def native_dialogs():
 import ctypes
 from ctypes import wintypes
 u=ctypes.windll.user32;out=[]
 def text(h):
  b=ctypes.create_unicode_buffer(2048);u.GetWindowTextW(h,b,2048);return b.value
 def visit(h,_):
  pid=wintypes.DWORD();u.GetWindowThreadProcessId(h,ctypes.byref(pid))
  if pid.value!=d.e2e.APP_PID['pid'] or not u.IsWindowVisible(h):return True
  title=text(h)
  if title=='Toontown':return True
  children=[]
  def child(c,_):
   value=text(c)
   if value:children.append(value)
   return True
  callback=ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(child);u.EnumChildWindows(h,callback,0)
  out.append({'hwnd':int(h),'title':title,'children':children});return True
 cb=ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)(visit);u.EnumWindows(cb,0)
 return out
async def overwrite(t):
 c=t.cmd
 for choice in ('yes','no'):
  await preserve_fixture(t)
  await c('project.create',{'name':'Existing','onUnsaved':'fail'});target=t.work/('overwrite-'+choice+'.animpkg');await c('project.saveAs',{'path':str(target)})
  await modal_fixture(t,'Replacement');before=target.read_bytes();t.clear()
  d.choose_menu(['File','Save As']);await asyncio.sleep(1.8);d.key(0x4E,0x12);d.key(0x41,0x11);d.type_text(str(target));d.key(0x0D);await asyncio.sleep(.7)
  native=native_dialogs()
  questions=[x for x in native if 'confirm save' in x['title'].lower() or any('already exists' in y.lower() for y in x['children'])]
  d.key(0x59 if choice=='yes' else 0x4E);await asyncio.sleep(1.5)
  application=t.cdp.js("!!document.querySelector('[role=alertdialog]')")
  if choice=='no':d.key(0x1B);await asyncio.sleep(.3)
  sent=[x for x in t.recorded() if x.get('command') not in SPEC_QUERIES and x.get('command')!='review.dispatch_key']
  expected=[{'command':'project.saveAs','params':{'path':str(target),'onExists':'overwrite'}}] if choice=='yes' else []
  rec={'choice':choice,'native':native,'questionCount':len(questions),'applicationQuestion':application,'beforeHash':hashlib.sha256(before).hexdigest(),'afterHash':sha(target),'sent':sent}
  t.run.save('overwrite-'+choice+'.json',rec)
  t.run.check(len(questions)==1 and not application and sent==expected,'Save As one native question '+choice,rec)
  t.run.check((target.read_bytes()!=before)==(choice=='yes'),'Save As bytes '+choice,rec)
async def overwrite_only(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();await overwrite(t)
async def jobs_only(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();await jobs(t)
async def a3_body(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await modal_fixture(t,'DialogReview')
 async def prefs():
  d.choose_menu(['File','Preferences']);await wait(t,'preferences.category.keyboard');t.click('preferences.category.keyboard');await asyncio.sleep(.5)
 async def finish():
  t.click('preferences.submit');await asyncio.sleep(.7)
 async def keys():return sorted(x['keys'] for x in await c('keymap.list',{'commandId':'project.save'}))
 path=t.work/'import.json'
 for choice in ('merge','replace','cancel','enter','space','escape'):
  await c('keymap.reset',{'all':True});await c('keymap.export',{'path':str(path),'onExists':'overwrite'})
  data=json.loads(path.read_text(encoding='utf-8'));baseline=await c('keymap.list',{})
  for b in data['bindings']:
   if b['commandId']=='project.save':b['keys']='Primary+Shift+F11'
  path.write_text(json.dumps(data),encoding='utf-8')
  await prefs();t.clear();t.click('keymap.import');await asyncio.sleep(1.8);d.key(0x4E,0x12);d.key(0x41,0x11);d.type_text(str(path));d.key(0x0D)
  await wait(t,'message.keymap.importMode')
  focused=t.cdp.js('document.activeElement?.dataset.ttId')
  if choice in ('merge','replace','cancel'):await modal(t,'message.keymap.importMode');t.click('message.'+choice)
  else:d.key(0x0D if choice=='enter' else 0x20 if choice=='space' else 0x1B)
  await asyncio.sleep(.5);await finish();actual=await keys()
  wanted=['Primary+S','Primary+Shift+F11'] if choice in ('merge','enter','space') else ['Primary+Shift+F11'] if choice=='replace' else ['Primary+S']
  t.run.check(actual==wanted and focused=='message.merge','Import '+choice,{'keys':actual,'focused':focused,'sent':t.recorded()})
  normalize=lambda b:{k:b.get(k,{} if k=='params' else None) for k in ('commandId','params','keys','context')}
  canonical=lambda rows:sorted(json.dumps(normalize(b),sort_keys=True) for b in rows)
  imported=data['bindings'];expected=baseline if choice in ('cancel','escape') else imported
  if choice in ('merge','enter','space'):
   expected=baseline+[b for b in imported if normalize(b) not in [normalize(x) for x in baseline]]
  all_actual=await c('keymap.list',{})
  t.run.save('bindings-'+choice+'.json',{'before':baseline,'input':imported,'expected':expected,'actual':all_actual})
  t.run.check(canonical(expected)==canonical(all_actual),'Import complete binding set '+choice,{'expectedCount':len(expected),'actualCount':len(all_actual)})
 for choice in ('reset','cancel','enter','space','escape'):
  await c('keymap.reset',{'all':True});await c('keymap.set',{'commandId':'project.save','keys':'Primary+Shift+F11','replaceExisting':True})
  await prefs();t.click('keymap.resetAll');await wait(t,'message.keymap.resetAll');t.clear()
  if choice in ('reset','cancel'):await modal(t,'message.keymap.resetAll');t.click('message.'+choice)
  else:
   d.key(0x0D if choice=='enter' else 0x20 if choice=='space' else 0x1B)
   await asyncio.sleep(.3)
   if choice in ('enter','space'):
    stuck=t.cdp.js("!!document.querySelector('[data-tt-id=\"message.keymap.resetAll\"]')")
    t.run.check(not stuck,'Reset focused default answers '+choice,{'stuck':stuck,'focus':t.cdp.js('document.activeElement?.dataset.ttId')})
    if stuck:d.key(0x1B)
  await asyncio.sleep(.4);await finish()
  t.run.check(await keys()==(['Primary+S'] if choice=='reset' else ['Primary+Shift+F11']),'Reset '+choice,t.recorded())
 await c('keymap.reset',{'all':True})
 for choice in ('override','cancel','override-space','cancel-space'):
  await prefs();t.click('keymap.filter');d.type_text('app.info.get');await asyncio.sleep(.5);t.click('keymap.command.app.info.get');t.click('keymap.capture')
  d.key(0x0D,0x10);await wait(t,'keymap.conflict');t.clear()
  await modal(t,'keymap.conflict')
  selected=choice.split('-')[0]
  if choice.endswith('-space'):
   for _ in range(150):
    if t.cdp.js('document.activeElement?.dataset.ttId')=='keymap.conflict.'+selected:break
    d.key(0x09)
   d.key(0x20)
  else:t.click('keymap.conflict.'+selected)
  await finish()
  actual=await c('keymap.list',{'commandId':'app.info.get'})
  t.run.check(any(x['keys']=='Shift+Enter' for x in actual)==(choice.startswith('override')),'Conflict '+choice,{'bindings':actual,'sent':t.recorded()})
  await c('keymap.reset',{'all':True})
 await overwrite(t)
 await jobs(t)
 await modal_extra(t)
async def keys_only(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await modal_fixture(t,'ModalKeysReview')
 async def prefs():
  d.choose_menu(['File','Preferences']);await wait(t,'preferences.category.keyboard');t.click('preferences.category.keyboard');await asyncio.sleep(.5)
 async def finish():
  t.click('preferences.submit');await asyncio.sleep(.7)
 async def keys():return sorted(x['keys'] for x in await c('keymap.list',{'commandId':'project.save'}))
 for choice in ('reset','cancel','enter','space','escape'):
  await c('keymap.reset',{'all':True});await c('keymap.set',{'commandId':'project.save','keys':'Primary+Shift+F11','replaceExisting':True})
  await prefs();t.click('keymap.resetAll');await wait(t,'message.keymap.resetAll');t.clear()
  if choice in ('reset','cancel'):await modal(t,'message.keymap.resetAll');t.click('message.'+choice)
  else:
   d.key(0x0D if choice=='enter' else 0x20 if choice=='space' else 0x1B)
   await asyncio.sleep(.3)
   if choice in ('enter','space'):
    stuck=t.cdp.js("!!document.querySelector('[data-tt-id=\"message.keymap.resetAll\"]')")
    t.run.check(not stuck,'Reset focused default answers '+choice,{'stuck':stuck,'focus':t.cdp.js('document.activeElement?.dataset.ttId')})
    if stuck:d.key(0x1B)
  await asyncio.sleep(.4);await finish()
  t.run.check(await keys()==(['Primary+S'] if choice=='reset' else ['Primary+Shift+F11']),'Reset '+choice,t.recorded())
 await c('keymap.reset',{'all':True})
 for choice in ('override','cancel','override-space','cancel-space'):
  await prefs();t.click('keymap.filter');d.type_text('app.info.get');await asyncio.sleep(.5);t.click('keymap.command.app.info.get');t.click('keymap.capture')
  d.key(0x0D,0x10);await wait(t,'keymap.conflict');t.clear()
  await modal(t,'keymap.conflict')
  selected=choice.split('-')[0]
  if choice.endswith('-space'):
   for _ in range(150):
    if t.cdp.js('document.activeElement?.dataset.ttId')=='keymap.conflict.'+selected:break
    d.key(0x09)
   d.key(0x20)
  else:t.click('keymap.conflict.'+selected)
  await finish()
  actual=await c('keymap.list',{'commandId':'app.info.get'})
  t.run.check(any(x['keys']=='Shift+Enter' for x in actual)==(choice.startswith('override')),'Conflict '+choice,{'bindings':actual,'sent':t.recorded()})
  await c('keymap.reset',{'all':True})

async def imports_only(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await modal_fixture(t,'DialogReview')
 async def prefs():
  d.choose_menu(['File','Preferences']);await wait(t,'preferences.category.keyboard');t.click('preferences.category.keyboard');await asyncio.sleep(.5)
 async def finish():
  t.click('preferences.submit');await asyncio.sleep(.7)
 async def keys():return sorted(x['keys'] for x in await c('keymap.list',{'commandId':'project.save'}))
 path=t.work/'import.json'
 for choice in ('merge','replace','cancel','enter','space','escape'):
  await c('keymap.reset',{'all':True});await c('keymap.export',{'path':str(path),'onExists':'overwrite'})
  data=json.loads(path.read_text(encoding='utf-8'));baseline=await c('keymap.list',{})
  for b in data['bindings']:
   if b['commandId']=='project.save':b['keys']='Primary+Shift+F11'
  path.write_text(json.dumps(data),encoding='utf-8')
  await prefs();t.clear();t.click('keymap.import');await asyncio.sleep(1.8);d.key(0x4E,0x12);d.key(0x41,0x11);d.type_text(str(path));d.key(0x0D)
  await wait(t,'message.keymap.importMode')
  focused=t.cdp.js('document.activeElement?.dataset.ttId')
  if choice in ('merge','replace','cancel'):await modal(t,'message.keymap.importMode');t.click('message.'+choice)
  else:d.key(0x0D if choice=='enter' else 0x20 if choice=='space' else 0x1B)
  await asyncio.sleep(.5);await finish();actual=await keys()
  wanted=['Primary+S','Primary+Shift+F11'] if choice in ('merge','enter','space') else ['Primary+Shift+F11'] if choice=='replace' else ['Primary+S']
  t.run.check(actual==wanted and focused=='message.merge','Import '+choice,{'keys':actual,'focused':focused,'sent':t.recorded()})
  normalize=lambda b:{k:b.get(k,{} if k=='params' else None) for k in ('commandId','params','keys','context')}
  canonical=lambda rows:sorted(json.dumps(normalize(b),sort_keys=True) for b in rows)
  imported=data['bindings'];expected=baseline if choice in ('cancel','escape') else imported
  if choice in ('merge','enter','space'):
   expected=baseline+[b for b in imported if normalize(b) not in [normalize(x) for x in baseline]]
  all_actual=await c('keymap.list',{})
  t.run.save('bindings-'+choice+'.json',{'before':baseline,'input':imported,'expected':expected,'actual':all_actual})
  t.run.check(canonical(expected)==canonical(all_actual),'Import complete binding set '+choice,{'expectedCount':len(expected),'actualCount':len(all_actual)})

async def modal_extra(t):
 import ctypes
 from ctypes import wintypes
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await modal_fixture(t,'UnsavedKeys')
 path=t.work/'unsaved.animpkg';await c('project.saveAs',{'path':str(path)})
 await c('draw.rectangle.add',{'from':{'x':60,'y':0},'to':{'x':80,'y':20}})
 before=await c('doc.export.json',{'scope':'project','binaries':'omit'});disk=path.read_bytes()
 t.clear();d.choose_menu(['File','New Project']);await wait(t,'message.project.unsavedChanges')
 await modal(t,'message.project.unsavedChanges');d.key(0x1B);await asyncio.sleep(.4)
 stuck=t.cdp.js("!!document.querySelector('[data-tt-id=\"message.project.unsavedChanges\"]')")
 t.run.check(not stuck,'Escape cancels unsaved question after Tab New',{'questionStillOpen':stuck,'focused':t.cdp.js('document.activeElement?.outerHTML')})
 if stuck:t.click('message.cancel')
 state=await c('app.state.get',{});after=await c('doc.export.json',{'scope':'project','binaries':'omit'})
 t.run.check(content(before)==content(after) and path.read_bytes()==disk and state['project']['dirty'],'Unsaved modal keys and Cancel preserve dirty content',{'sent':t.recorded()})
 t.clear();d.choose_menu(['File','Open']);await wait(t,'message.project.unsavedChanges');tab_focus=[]
 for _ in range(3):
  d.key(0x09);tab_focus.append(t.cdp.js('document.activeElement?.outerHTML'))
 d.key(0x1B);await asyncio.sleep(.3)
 stuck=t.cdp.js("!!document.querySelector('[data-tt-id=\"message.project.unsavedChanges\"]')")
 t.run.check(not stuck,'Escape cancels unsaved question after Tab Open',{'questionStillOpen':stuck,'tabFocus':tab_focus})
 if stuck:t.click('message.cancel')
 t.clear();d.choose_menu(['File','New Project']);await wait(t,'message.project.unsavedChanges');focused=t.cdp.js('document.activeElement?.dataset.ttId');d.key(0x20)
 await wait(t,'newProject.name');d.key(0x1B);await asyncio.sleep(.4)
 after=await c('doc.export.json',{'scope':'project','binaries':'omit'});state=await c('app.state.get',{})
 actions=[x for x in t.recorded() if x.get('command') not in SPEC_QUERIES and x.get('command')!='review.dispatch_key']
 t.run.check(focused=='message.save' and content(before)==content(after) and not state['project']['dirty'] and actions==[{'command':'project.save','params':{}}] and not state['playback']['playing'],'Unsaved focused Space saves without background playback',{'focused':focused,'sent':actions})
 await preserve_fixture(t);await c('project.create',{'name':'NativeExisting','onUnsaved':'fail'})
 target=t.work/'native-existing.animpkg';await c('project.saveAs',{'path':str(target)})
 await modal_fixture(t,'NativeReplacement');before=await c('doc.export.json',{'scope':'project','binaries':'omit'});disk=target.read_bytes();t.clear()
 d.choose_menu(['File','Save As']);await asyncio.sleep(1.5);d.key(0x4E,0x12);d.key(0x41,0x11);d.type_text(str(target));d.key(0x0D);await asyncio.sleep(.5)
 dialogs=native_dialogs();question=next(x for x in dialogs if 'confirm save' in x['title'].lower())
 class Info(ctypes.Structure):
  _fields_=[('cbSize',wintypes.DWORD),('flags',wintypes.DWORD),('hwndActive',wintypes.HWND),('hwndFocus',wintypes.HWND),('hwndCapture',wintypes.HWND),('hwndMenuOwner',wintypes.HWND),('hwndMoveSize',wintypes.HWND),('hwndCaret',wintypes.HWND),('rcCaret',wintypes.RECT)]
 u=ctypes.windll.user32;u.GetAncestor.restype=wintypes.HWND;roots=[]
 for _ in range(6):
  d.key(0x09);info=Info();info.cbSize=ctypes.sizeof(info);u.GetGUIThreadInfo(0,ctypes.byref(info));roots.append(u.GetAncestor(info.hwndFocus,2))
 d.key(0x42);d.key(0x5A,0x11);d.key(0x20);await asyncio.sleep(.3);d.key(0x1B);await asyncio.sleep(.5)
 after=await c('doc.export.json',{'scope':'project','binaries':'omit'});state=await c('app.state.get',{})
 actions=[x for x in t.recorded() if x.get('command') not in SPEC_QUERIES and x.get('command')!='review.dispatch_key']
 rec={'questionTitle':question['title'],'tabRoots':roots,'questionHwnd':question['hwnd'],'contentUnchanged':content(before)==content(after),'fileUnchanged':target.read_bytes()==disk,'actions':actions,'playing':state['playback']['playing']}
 t.run.save('native-modal-keys.json',rec)
 t.run.check(all(h==question['hwnd'] for h in roots) and rec['contentUnchanged'] and rec['fileUnchanged'] and not actions and not rec['playing'],'Native overwrite modal keys and No preserve document',rec)
async def jobs(t):
 await preserve_fixture(t)
 await t.cmd('project.create',{'name':'JobReview','onUnsaved':'fail'})
 bob=(await t.cmd('element.create',{'type':'drawing','name':'JobDrawing'}))['elementIds'][0]
 await t.cmd('element.drawing.create',{'elementId':bob,'name':'1','exposeFrom':1,'exposeTo':24})
 await t.cmd('app.context.set',{'elementId':bob,'frame':1})
 await t.cmd('draw.rectangle.add',{'from':{'x':-40,'y':-40},'to':{'x':40,'y':40}})
 discovery=json.loads((t.work/'discovery.json').read_text())
 before=await t.cmd('doc.export.json',{'scope':'project','binaries':'omit'})
 async with contextlib.AsyncExitStack() as stack:
  client=await stack.enter_async_context(d.e2e.httpx2.AsyncClient(headers={'Authorization':'Bearer '+discovery['token']},timeout=300))
  streams=await stack.enter_async_context(d.e2e.streamable_http_client(discovery['endpoint'],http_client=client))
  from mcp import types
  async def roots(_):return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name='review')])
  agent=await stack.enter_async_context(d.e2e.ClientSession(streams[0],streams[1],list_roots_callback=roots));await agent.initialize()
  attempts=[]
  for _ in range(300):
   env=await d.e2e.run_cmd(agent,'render.range.run',{'from':1,'to':2400,'scale':2,'directory':str(t.work/'render'),'createDirs':True,'onConflict':'overwrite'})
   if env.get('status')=='ok':break
   attempts.append(env)
   if env.get('error',{}).get('code')!='fs.outsideRoots':break
   await asyncio.sleep(.1)
  t.run.save('roots-bootstrap.json',attempts)
  t.run.save('job-start.json',env)
  if env.get('status')!='ok':raise RuntimeError('job start '+json.dumps(env))
  jid=env['result']['jobId'];await wait(t,'jobsWindow.cancel.'+jid)
  shown=t.cdp.js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.textContent"%json.dumps('jobsWindow.job.'+jid))
  t.clear();t.click('jobsWindow.cancel.'+jid)
  result=await d.e2e.call(agent,'job_wait',{'jobId':jid,'timeoutMs':120000});await asyncio.sleep(1)
  sent=t.recorded();after=await t.cmd('doc.export.json',{'scope':'project','binaries':'omit'})
  closed=t.cdp.js("!document.querySelector('[data-tt-id=jobsWindow]')")
  t.run.save('job-cancel.json',{'jobId':jid,'result':result,'shown':shown,'sent':sent,'before':before,'after':after,'closed':closed})
  t.run.check(bool(shown) and content(before)==content(after) and closed and any(x=={'command':'job.cancel','params':{'jobId':jid}} for x in sent) and result.get('result',{}).get('state')=='cancelled','Agent progress cancel',result)
  env=await d.e2e.run_cmd(agent,'render.range.run',{'from':1,'to':12000,'scale':0.05,'directory':str(t.work/'committed'),'createDirs':True,'onConflict':'overwrite'})
  t.run.save('commit-start.json',env)
  if env.get('status')!='ok':raise RuntimeError('commit job start '+json.dumps(env))
  committed_id=env['result']['jobId'];samples=[];past=None;deadline=time.monotonic()+600
  while time.monotonic()<deadline:
   records=(await d.e2e.run_cmd(agent,'job.list',{'jobId':committed_id})).get('result') or []
   record=records[0] if records else {}
   if not record.get('cancellable',True):
    past={k:record.get(k) for k in ('jobId','state','progress','message','cancellable')}
    break
   if len(samples)<10:samples.append({k:record.get(k) for k in ('state','progress','cancellable')})
   await asyncio.sleep(.02)
  if past is None:raise RuntimeError('commit point hang guard')
  control=None
  for _ in range(100):
   control=t.cdp.js("!!document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')"%json.dumps('jobsWindow.cancel.'+committed_id))
   if not control:break
   await asyncio.sleep(.05)
  cancellation=await d.e2e.run_cmd(agent,'job.cancel',{'jobId':committed_id})
  t.run.save('job-commit-partial.json',{'past':past,'cancelControl':control,'cancellation':cancellation})
  final_state=None
  request=json.dumps({'command':'job.list','params':{'jobId':committed_id,'all':True}})
  expression="(async()=>{let r=await window.__TAURI_INTERNALS__.invoke('run_command',{json:%s});if(r instanceof ArrayBuffer||ArrayBuffer.isView(r))r=JSON.parse(new TextDecoder().decode(r));else if(Array.isArray(r))r=JSON.parse(new TextDecoder().decode(Uint8Array.from(r)));else if(typeof r==='string')r=JSON.parse(r);const j=(r.result||[])[0];return j?{state:j.state,cancellable:j.cancellable}:r;})()"%json.dumps(request)
  deadline=time.monotonic()+600
  while time.monotonic()<deadline:
   final_state=t.cdp.js(expression)
   if final_state.get('state') in ('succeeded','failed','cancelled'):break
   await asyncio.sleep(.1)
  final={'result':final_state}
  await asyncio.sleep(.7)
  unchanged=content(before)==content(await t.cmd('doc.export.json',{'scope':'project','binaries':'omit'}))
  closed=t.cdp.js("!document.querySelector('[data-tt-id=jobsWindow]')")
  receipt={'pastCommit':past,'samples':samples,'cancelControl':control,'cancellation':cancellation,'finalState':final.get('result',{}).get('state'),'documentUnchanged':unchanged,'windowClosed':closed}
  t.run.save('job-commit.json',receipt)
  t.run.check(past['state']=='running' and not control and receipt['finalState']=='succeeded' and unchanged and closed,'Job past commit is not cancellable',receipt)
def a3():
 run=d.e2e.Run(OUT);d.e6.dpi_aware();work=Path(tempfile.mkdtemp(prefix='r3-dialogs-'))
 d.build_and_run=lambda t,only:overwrite_only(t) if '--overwrite' in sys.argv else jobs_only(t) if '--jobs' in sys.argv else keys_only(t) if '--keys' in sys.argv else imports_only(t) if '--imports' in sys.argv else modal_extra(t) if '--modal-extra' in sys.argv else a3_body(t)
 try:asyncio.run(d.session_all(APP,run,OUT,work,set()))
 except Exception as ex:
  import traceback;traceback.print_exc();run.check(False,'A3 ran',repr(ex))
 run.save('transcript.json',run.transcript);return int(bool(run.failures))
async def b1_body(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await modal_fixture(t,'List review')
 await asyncio.sleep(1)
 rows=[]
 tree=await d.McpTable.tree(t)
 def lists(n):
  out=[n] if any(c.get('command') for c in n.get('children',[])) and n.get('id','').startswith('mainToolbar.') and n.get('role')=='field' else []
  for c in n.get('children',[]):out+=lists(c)
  return out
 controls=t.cdp.js("[...document.querySelectorAll('select[data-tt-command]')].map(e=>e.dataset.ttId)")
 t.run.save('b1-discovery.json',{'controls':controls,'treeLists':lists(tree)})
 for ident in controls:
  if os.environ.get('REVIEW_LIST') and ident!=os.environ['REVIEW_LIST']:continue
  # Workspace changes may replace visible toolbar controls; restore Drawing for each list.
  await t.cmd('workspace.select',{'name':'Drawing'});await asyncio.sleep(.4)
  node=d.McpTable.node(await d.McpTable.tree(t),ident)
  options=[c for c in node.get('children',[]) if c.get('command')]
  for opt in options:
   for mode in ('mouse','keyboard','current','other-focus'):
    await t.cmd('workspace.select',{'name':'Drawing'});await asyncio.sleep(.25)
    ref=opt['command'];want={'command':ref['commandId'],'params':ref.get('params',{})}
    value=opt['value']
    alternate=next(c for c in options if c['value']!=value)
    setup=opt if mode=='current' else alternate
    await t.cmd(setup['command']['commandId'],setup['command'].get('params',{}));await asyncio.sleep(.3)
    d.key(0x1B);pos=await t.position();t.clear()
    if mode=='other-focus':
     target='drawingView' if t.rect('drawingView') else 'cameraView'
     t.click(target)
    # Open with an OS click. Native popup row coordinates come from screen capture geometry.
    if mode in ('mouse','current','other-focus'):
     r=t.rect(ident);h=d.front();origin=d.wintypes.POINT(0,0);d.u32.ClientToScreen(h,d.ctypes.byref(origin))
     t.click(ident);await asyncio.sleep(.2)
     from PIL import ImageGrab
     ImageGrab.grab().save(OUT/('popup-'+ident+'-'+value+'-'+mode+'.png'))
     # Native WebView popup observed at control bottom, 24 physical pixels per row.
     height=t.cdp.js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']').getBoundingClientRect().height"%json.dumps(ident))
     d.click_screen(origin.x+r['x']*r['dpr'],origin.y+(r['y']+height/2)*r['dpr']+5+24*(options.index(opt)+.5))
    else:
     t.click(ident)
     d.key(0x24)
     for _ in range(options.index(opt)):d.key(0x28)
     d.key(0x0D)
    await asyncio.sleep(.6)
    sent=[c for c in t.recorded() if c['command'] not in SPEC_QUERIES and c['command']!='review.dispatch_key']
    shown=t.cdp.js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')?.value"%json.dumps(ident))
    steps=await t.steps_since(pos)
    expected=[] if mode=='current' else [want]
    row={'list':ident,'option':value,'mode':mode,'declared':want,'sent':sent,'shown':shown,'steps':steps}
    rows.append(row);t.run.save('b1-rows.json',rows)
    t.run.check(sent==expected and shown==value and not steps,'B1 '+ident+' '+value+' '+mode,row)

def bcase(body,name):
 run=d.e2e.Run(OUT);d.e6.dpi_aware();work=Path(tempfile.mkdtemp(prefix='r3-'+name+'-'))
 d.build_and_run=lambda t,only:body(t)
 try:asyncio.run(d.session_all(APP,run,OUT,work,set()))
 except Exception as ex:
  import traceback;traceback.print_exc();run.check(False,name+' ran',repr(ex))
 run.save('transcript.json',run.transcript);return int(bool(run.failures))
def b1():return bcase(b1_body,'b1')

async def b2_body(t):
 import textwrap
 old=(ROOT/'tests/adversarial/c43b-r2-live.py').read_text(encoding='utf-8')
 a=old.index(' async def menus(t,state):');b=old.index(' original_build=m.build_and_run',a)
 scope={'m':d,'ROOT':ROOT,'asyncio':asyncio,'json':json}
 exec(textwrap.dedent(old[a:b]),scope)
 menus=scope['menus']
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record()
 await menus(t,'no-project')
 await modal_fixture(t,'Menu review')
 await t.cmd('project.saveAs',{'path':str(t.work/'Menu.animpkg')})
 await menus(t,'clean')
 await t.cmd('draw.rectangle.add',{'from':{'x':60,'y':60},'to':{'x':100,'y':100}})
 await t.cmd('draw.selection.set',{'by':'all'})
 await menus(t,'dirty-selection')
 paths=[['File','Save'],['Edit','Undo'],['View','Zoom In'],['View','Zoom Out'],['Drawing','Flip Horizontal'],['Drawing','Flip Vertical'],['Window','Extend Side Panels'],['Help','Context Help'],['Window','Workspace','Default'],['File','Open Recent','Menu review']]
 for path in paths:
  if path[0]=='File' and len(path)>2:await t.cmd('project.save',{})
  tree=await d.McpTable.tree(t)
  if path==['Edit','Undo']:
   bar=d.McpTable.node(tree,'menuBar');edit=next(x for x in bar['children'] if x.get('label')=='Edit');path=['Edit',next(x['label'] for x in edit['children'] if x.get('commandId')=='edit.undo')]
  node=d.McpTable.menu_node(tree,path);ref=node.get('command',{})
  t.clear();await t.do(('menu',path));await asyncio.sleep(1)
  sent=[c for c in t.recorded() if c['command']!='review.dispatch_key']
  t.run.check({'command':ref.get('commandId'),'params':ref.get('params',{})} in sent,'B2 native activation '+'/'.join(path),{'tree':node,'sent':sent})
 await t.cmd('workspace.create',{'name':'Review workspace'})
 await t.cmd('workspace.rename',{'name':'Review workspace','newName':'Renamed workspace'})
 await t.cmd('workspace.select',{'name':'Renamed workspace'})
 for name in ('Recent one','Recent two'):
  await preserve_fixture(t);await t.cmd('project.create',{'name':name});await t.cmd('project.saveAs',{'path':str(t.work/(name+'.animpkg'))})
 for name in ('Recent one','Recent two'):await t.cmd('project.open',{'path':str(t.work/(name+'.animpkg'))})
 await menus(t,'workspace-recent')
def b2():return bcase(b2_body,'b2')


def b3():
 step=int(os.environ.get('B3_STEP','1'))
 lc=load('review_lifecycle','scripts/c43-preview-lifecycle.py')
 head=str(ROOT/'target/release/toontown-headless.exe')
 if step==1:
  return subprocess.run([sys.executable,str(ROOT/'scripts/c43-preview-lifecycle.py'),APP,head,str(OUT)],cwd=ROOT).returncode
 class Review(lc.Lifecycle):
  async def fixture(self):
   await super().fixture()
   self.cdp=d.Cdp(d.CDP_PORT)
   self.cdp.js("window.reviewTrace=[];window.reviewInvoke=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=function(cmd,args){const e={cmd,args,asked:performance.now()};if(['canvas_scene','gesture_commit'].includes(cmd))reviewTrace.push(e);return reviewInvoke.apply(this,arguments).then(v=>{e.answered=performance.now();e.value=v;return v})}")
  async def start_row(self,*args):
   await super().start_row(*args)
   self.cdp.js('reviewTrace.length=0')
  async def press(self,*args,**kwargs):
   r=await super().press(*args,**kwargs)
   self.release_page=self.cdp.js('performance.now()')
   return r
  async def settle(self, pressed, extra_s=2.0):
   rel=pressed['times']['release']; before=pressed['before']; samples=[]; answer=None; record=None
   while time.monotonic()-rel < self.commit_ms/1000+14:
    st=await self.state();now=time.monotonic()-rel;raw=st.get('lastGesture')
    if answer is None and raw is not None and json.dumps(raw)!=before:answer=now;record=raw
    samples.append((now,lc.pending(st)))
    import base64
    capture=self.cdp.call('Page.captureScreenshot',{'format':'png','captureBeyondViewport':False})
    data=capture.get('result',{}).get('data')
    if data:(OUT/('frame-'+str(self.n)+'-'+str(len(samples))+'.png')).write_bytes(base64.b64decode(data))
    self.run.save('sample-'+str(self.n)+'.json',{'samples':samples,'latest':st,'record':record})
    if answer is not None and now>=answer+extra_s:break
    await asyncio.sleep(.02)
   trace=self.cdp.js('reviewTrace')
   self.run.save('trace-'+str(self.n)+'.json',trace)
   return {'samples':samples,'record':record,'answeredAt':answer,'trace':trace}
  async def state_b(self):
   await self.start_row('select')
   async def ask():await self.cmd('drawAid.grid.set',{'visible':True})
   r=await self.drag([(-60,40),(-30,50),(0,60)],prepare=ask)
   trace=r['trace'];commits=[e for e in trace if e['cmd']=='gesture_commit'];scenes=[e for e in trace if e['cmd']=='canvas_scene' and e['asked']<self.release_page<e.get('answered',0)]
   commit=commits[-1] if commits else {};scene=next((e for e in scenes if e['answered']<commit.get('answered',0)),{})
   order={'release':self.release_page,'sceneAsked':scene.get('asked'),'sceneAnswer':scene.get('answered'),'commitAnswer':commit.get('answered')}
   ok,detail=self.held_while_pending(r);self.check('measured stale scene order and preview held',bool(scene) and ok,{'orderMs':order,**detail})
   await self.head.run('drawAid.grid.set',{'visible':True})
   same,diff=await self.same_as_direct([r['record']]);self.check('stale scene D-32',same,diff)
  async def state_c(self):
   await self.start_row('brush')
   p=await self.press([(-80,50),(-40,60),(0,55)])
   await asyncio.sleep(.05);d.front();lc.ctrl_z();await asyncio.sleep(.2)
   early=await self.state()
   self.check('undo immediately removes pending preview',not lc.pending(early),early)
   r=await self.settle(p)
   same,diff=await self.same_as_direct([r['record']],extra=[('edit.undo',{})])
   self.check('undo D-32',same and not lc.pending(await self.state()),diff)
  async def state_f(self):
   await self.start_row('brush')
   p=await self.press([(-80,-80),(-40,-70),(0,-75)])
   states=[]
   for view in ('cameraView',lc.VIEW):
    await self.app.run('view.active.set',{'viewId':view});await asyncio.sleep(.2)
    state=self.cdp.js("JSON.parse(document.querySelector('[data-tt-id=\""+view+".canvas\"]').getAttribute('data-tt-value'))")
    states.append({'view':view,'afterRelease':time.monotonic()-p['times']['release'],'state':state})
   r=await self.settle(p)
   self.check('Camera and back while pending have no stale preview',all(not lc.pending(x['state']) and x['afterRelease']<2.5 for x in states),states)
   same,diff=await self.same_as_direct([r['record']]);self.check('view switch D-32',same and not lc.pending(await self.state()),diff)
  async def state_a(self):
   if step!=2:return await super().state_a()
   await self.start_row('brush')
   r=await self.drag([(-80,-40),(-40,-20),(0,-30)])
   self.check('PRODUCT ignores both 3000 ms hooks',r['answeredAt'] is not None and r['answeredAt']<.8, r)
   same,diff=await self.same_as_direct([r['record']]);self.check('PRODUCT D-32',same,diff)
 os.environ['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS']='--remote-debugging-port='+str(d.CDP_PORT)
 os.environ['WEBVIEW2_USER_DATA_FOLDER']=str(OUT/'webview2')
 lc.Lifecycle=Review
 run=lc.e2e.Run(OUT);work=Path(tempfile.mkdtemp(prefix='r4-preview-'))
 lc.e6.dpi_aware()
 states={2:'a',3:'a',4:'b',5:'cd',6:'ef'}[step]
 rows=asyncio.run(lc.session(APP,head,run,work,3000 if step==2 else 2500,3000 if step==2 else 1800 if step==4 else 0,states))
 run.save('rows.json',rows);run.save('transcript.json',run.transcript)
 return int(bool(run.failures))

async def closure_body(t):
 t.cdp=d.Cdp(d.CDP_PORT);t.cdp.record();c=t.cmd
 await modal_fixture(t,'Closure')
 await c('project.saveAs',{'path':str(t.work/'closure.animpkg')})
 step=sys.argv[3]
 async def open_error():
  f=t.work/'invalid.animpkg';f.write_text('This is not a project.');d.choose_menu(['File','Open']);await asyncio.sleep(1.5);d.key(0x4E,0x12);d.key(0x41,0x11);d.type_text(str(f));d.key(0x0D)
  for _ in range(100):
   if t.cdp.js("!!document.querySelector('[role=alertdialog]')"):return
   await asyncio.sleep(.1)
  raise RuntimeError('error box absent')
 async def prefs():
  d.choose_menu(['File','Preferences']);await wait(t,'preferences.category.keyboard');t.click('preferences.category.keyboard');await asyncio.sleep(.3)
 async def focusid():return t.cdp.js('document.activeElement?.dataset.ttId')
 if step=='5':
  before=content(await c('doc.export.json',{'scope':'project','binaries':'omit'}));state=await c('app.state.get');t.clear();await open_error()
  text=t.cdp.js("document.querySelector('[role=alertdialog]').innerText");focus=await focusid();t.click('message.ok');await asyncio.sleep(.4)
  now=await c('app.state.get');after=content(await c('doc.export.json',{'scope':'project','binaries':'omit'})) if now.get('project') else None
  dismissed=t.cdp.js("!document.querySelector('[role=alertdialog]')")
  records='job.wait observed in GUI IPC; all-session listing is GUI-only'
  t.run.check(before==after and state['project']==now['project'] and len(text)>15 and focus=='message.ok' and dismissed,'Invalid project job error',{'text':text,'focus':focus,'dismissed':dismissed,'jobs':records,'beforeProject':state.get('project'),'afterProject':now.get('project'),'sent':t.recorded(),'unchanged':before==after})
 elif step=='3':
  kind=sys.argv[4];vk=int(sys.argv[5]);expected={'unsaved':'message.save','import':'message.merge','reset':'message.cancel','conflict':'keymap.conflict.cancel','error':'message.ok','information':'message.ok'}[kind]
  if kind=='unsaved':
   await c('element.create',{'type':'drawing','name':'Dirty'});d.choose_menu(['File','New Project']);await wait(t,'message.project.unsavedChanges')
  elif kind in ('import','reset','conflict'):
   await prefs()
   if kind=='reset':t.click('keymap.resetAll');await wait(t,'message.keymap.resetAll')
   elif kind=='conflict':
    t.click('keymap.filter');d.type_text('app.info.get');await asyncio.sleep(.3);t.click('keymap.command.app.info.get');t.click('keymap.capture');d.key(0x0D,0x10);await wait(t,'keymap.conflict')
   else:
    path=t.work/'keys.json';await c('keymap.export',{'path':str(path)});t.click('keymap.import');await asyncio.sleep(1.5);d.key(0x4E,0x12);d.key(0x41,0x11);d.type_text(str(path));d.key(0x0D);await wait(t,'message.keymap.importMode')
  elif kind=='error':await open_error()
  else:
   # Open a real configured tip through Preferences, not a synthetic message.
   await prefs();t.click('preferences.category.messages');await asyncio.sleep(.4)
   t.run.save('information-discovery.json',t.cdp.js('document.body.innerText'))
   buttons=t.cdp.js("[...document.querySelectorAll('button[data-tt-id]')].map(e=>({id:e.dataset.ttId,text:e.textContent}))")
   tip=next((b['id'] for b in buttons if 'show' in b['text'].lower()),None)
   if not tip:raise RuntimeError('No information tip Show button: '+json.dumps(buttons))
   t.click(tip);await asyncio.sleep(.4)
  focus=await focusid();t.clear();d.key(vk);await asyncio.sleep(.5)
  still=t.cdp.js("!!document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')"%json.dumps('keymap.conflict')) if kind=='conflict' else t.cdp.js("!!document.querySelector('[role=alertdialog]')")
  t.run.check(focus==expected and not still,'Default '+kind+' '+str(vk),{'focused':focus,'expected':expected,'stillOpen':still,'sent':t.recorded()})
 elif step=='4':
  kind=sys.argv[4]
  if kind=='message':
   await c('element.create',{'type':'drawing','name':'Dirty'});d.choose_menu(['File','New Project']);await wait(t,'message.project.unsavedChanges');ident='message.project.unsavedChanges'
  elif kind=='progress':
   progressBefore=content(await c('doc.export.json',{'scope':'project','binaries':'omit'}))
   import zipfile
   source=t.work/'closure.animpkg';target=t.work/'slow.animpkg';block=bytes(1<<20);digest=hashlib.sha256(block*64).hexdigest()
   if os.environ.get('REVIEW_PROGRESS_PACKAGE'):target=Path(os.environ['REVIEW_PROGRESS_PACKAGE'])
   else:
    with zipfile.ZipFile(source) as z:entries={n:z.read(n) for n in z.namelist()}
    manifest=json.loads(entries['manifest.json'])
    with zipfile.ZipFile(target,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=1) as z:
     for n,data in entries.items():
      if n!='manifest.json':z.writestr(n,data,compress_type=zipfile.ZIP_STORED if n=='mimetype' else zipfile.ZIP_DEFLATED)
     for i in range(256):
      n='review-payload/'+str(i)+'.bin'
      with z.open(n,'w') as f:
       for _ in range(64):f.write(block)
      manifest['entries'].append({'path':n,'size':64<<20,'sha256':digest})
     z.writestr('manifest.json',json.dumps(manifest))
   d.choose_menu(['File','Open']);await asyncio.sleep(1.5);d.key(0x4E,0x12);d.key(0x41,0x11);d.type_text(str(target));d.key(0x0D);await wait(t,'progress');ident='progress'
  else:
   path={'preferences':['File','Preferences'],'new':['File','New Project'],'properties':['File','Project Properties'],'check':['File','Check Project'],'backup':['File','Open Backup']}[kind]
   ident={'preferences':'preferences','new':'newProject','properties':'projectProperties','check':'checkProject','backup':'openBackup'}[kind]
   d.choose_menu(path);await wait(t,ident)
  await asyncio.sleep(.4)
  before=progressBefore if kind=='progress' else content(await c('doc.export.json',{'scope':'project','binaries':'omit'}));t.clear()
  expr="(() => {const r=document.querySelector('[data-tt-id='+JSON.stringify(%s)+']');const a=[...r.querySelectorAll('button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex=\"-1\"])')].filter(e=>e.getClientRects().length&&!e.closest('[inert]'));return {items:a.map(e=>e.dataset.ttId||e.tagName),focus:document.activeElement?.dataset.ttId,inside:r.contains(document.activeElement)}})()"%json.dumps(ident)
  initial=t.cdp.js(expr);route=[]
  for _ in range(len(initial['items'])+2):d.key(0x09);route.append(t.cdp.js(expr))
  for _ in range(len(initial['items'])+2):d.key(0x09,0x10);route.append(t.cdp.js(expr))
  # Place focus on root through real click on its title, so Enter/Space cannot submit a button.
  r=t.rect(ident);h=d.front();origin=d.wintypes.POINT(0,0);d.u32.ClientToScreen(h,d.ctypes.byref(origin));d.click_screen(origin.x+r['x']*r['dpr'],origin.y+(r['y']-t.cdp.js("document.querySelector('[data-tt-id='+JSON.stringify(%s)+']').getBoundingClientRect().height"%json.dumps(ident))/2+15)*r['dpr'])
  keyFocus=t.cdp.js("({id:document.activeElement?.dataset.ttId,tag:document.activeElement?.tagName})")
  for vk,mods in [(0x42,()),(0x5A,(0x11,)),(0x20,()),(0x0D,())]:d.key(vk,*mods)
  d.click_screen(origin.x+10,origin.y+100);await asyncio.sleep(.3)
  after=before if kind=='progress' else content(await c('doc.export.json',{'scope':'project','binaries':'omit'}));sent=t.recorded();bad=[x for x in sent if x.get('command') not in SPEC_QUERIES]
  present=t.cdp.js("!!document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')"%json.dumps(ident));d.key(0x1B);await asyncio.sleep(.4);closed=t.cdp.js("!document.querySelector('[data-tt-id='+JSON.stringify(%s)+']')"%json.dumps(ident))
  if kind=='progress':
   for _ in range(300):
    if t.cdp.js("!document.querySelector('[data-tt-id=progress]')"):break
    await asyncio.sleep(.1)
   closed=t.cdp.js("!document.querySelector('[data-tt-id=progress]')");finalState=await c('app.state.get');after=content(await c('doc.export.json',{'scope':'project','binaries':'omit'})) if finalState.get('project') else None;status=t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent");box=t.cdp.js("!!document.querySelector('[role=alertdialog]')")
   t.run.check('cancelled' in (status or '') and not box,'Cancelled job status line',{'status':status,'errorBox':box})
   bad=[x for x in bad if x.get('command')=='review.dispatch_key' or x.get('command')!='job.cancel']
  rec={'initial':initial,'keyFocus':keyFocus,'route':route,'sent':sent,'unchanged':before==after,'presentBeforeEscape':present,'closed':closed}
  rec['finalState']=finalState if kind=='progress' else None
  rec['beforeDocumentHash']=hashlib.sha256(json.dumps(before,sort_keys=True).encode()).hexdigest()
  rec['afterDocumentHash']=hashlib.sha256(json.dumps(after,sort_keys=True).encode()).hexdigest()
  t.run.check(all(x['inside'] for x in route) and before==after and not bad and present and closed,'Modal '+kind,rec)

def closure():return bcase(closure_body,'closure')

if __name__=='__main__':
 if len(sys.argv)<3:sys.exit(0)
 APP=str(Path(sys.argv[1]).resolve());OUT.mkdir(parents=True,exist_ok=True)
 d=load('r3d39','scripts/c43-d39-bindings.py')
 # Observe key dispatch as well as action IPC; this changes no input or assertion in older probes.
 import inspect,textwrap
 reader=textwrap.dedent(inspect.getsource(d.Cdp._read))
 marker='            if "/run_command" in (req.get("url") or ""):'
 replacement='            if "/dispatch_key" in (req.get("url") or ""):\n                self.calls.append({"command":"review.dispatch_key","params":{"body":req.get("postData")}})\n'+marker
 assert marker in reader
 reader=reader.replace(marker,replacement)
 exec(compile(reader,'<review IPC observer>','exec'),d.__dict__)
 d.Cdp._read=d.__dict__['_read']
 server=subprocess.Popen([sys.executable,'-m','http.server','1420','--bind','127.0.0.1','--directory',str(ROOT/'desktop/dist')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 import threading
 stop=threading.Event();worker=None
 if '--first-click' not in sys.argv:
  worker=threading.Thread(target=activate_new_windows,args=(stop,),daemon=True);worker.start()
 try:sys.exit({'a1':a1,'a2':a2,'a3':a3,'b1':b1,'b2':b2,'b3':b3,'closure':closure}[sys.argv[2]]())
 finally:
  stop.set()
  if worker:worker.join()
  server.terminate();server.wait()