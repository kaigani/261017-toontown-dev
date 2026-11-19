"""Independent round-two live probes: python FILE APP a2|a3|b1|b2|b3. Raw data stays in TEMP."""
import sys
if len(sys.argv)<3:
 print("skipped: no application")
 sys.exit(0)
import asyncio, importlib.util, json, os, subprocess, tempfile, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(os.environ["REVIEW_RAW"]) if os.environ.get("REVIEW_RAW") else Path(tempfile.mkdtemp(prefix="c43b-r2-raw-"))
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,ROOT/path)
 m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
r1=load("r1review","tests/adversarial/c43b-r1-live.py")
r1.OUT=OUT
async def variants(m,s,run,out,ids):
 import ctypes,threading
 from ctypes import wintypes
 rc=m.e2e.run_cmd; e=m.e6; u=ctypes.windll.user32
 async def cmd(n,p=None):
  env=await rc(s,n,p or {})
  if env.get("status")!="ok": raise RuntimeError((n,env))
  return env.get("result") or {}
 results=[]
 for variant in os.environ.get("REVIEW_VARIANTS","triple,rotate,cross-view,escape,refusal,agent-between,played-overlap").split(","):
  for iteration in range(10):
   try:
    await cmd("workspace.select",{"name":"Default"})
    await cmd("window.panel.set",{"panelId":"drawingView","visible":True})
    await cmd("app.context.set",{"elementId":ids["Bob"],"frame":1})
    await cmd("view.active.set",{"viewId":"cameraView"})
    await cmd("view.zoom.set",{"viewId":"cameraView","zoom":.6})
    await cmd("anim.tool.set",{"tool":"select"})
    await cmd("edit.selection.set",{"context":"staging","ids":[ids["Bob"]]})
    await asyncio.sleep(.2)
    start=await cmd("edit.history.get",{"limit":1000}); pos=start["position"]
    checks=[]
    cached=None
    gesture_index=0
    async def gesture(view="cameraView",cancel=False,kind="move"):
     import math
     nonlocal cached,gesture_index
     if cached is not None and variant in ("triple","escape","agent-between","played-overlap") and kind=="move":
      b,place=cached
     else:b=await m.box_of(s,view);place=await e.locate_view(s,rc,view)
     if not b or not place: raise RuntimeError("gesture fixture missing")
     h=e._main_window(m.e2e.APP_PID["pid"]);u.SetForegroundWindow(h)
     origin=wintypes.POINT();u.ClientToScreen(h,ctypes.byref(origin))
     cx,cy=(b[0]+b[2])/2,(b[1]+b[3])/2
     px,py=cx+18,cy+12
     dx,dy=18+gesture_index*5,9+gesture_index*3
     gesture_index+=1
     if kind=="rotate":
      px,py=b[2]+9,b[1]-9
      dx=(px-cx)*math.cos(.2)-(py-cy)*math.sin(.2)+cx-px
      dy=(px-cx)*math.sin(.2)+(py-cy)*math.cos(.2)+cy-py
     if kind=="refusal":px,py=cx,b[1]-12;dx,dy=32,17
     x=int(origin.x+place[0]+px);y=int(origin.y+place[1]+py)
     u.SetCursorPos(x,y)
     if variant=="played-overlap" and gesture_index==1:run.check(not played.done(),"SI-135 real user press overlaps pending played pointer",None)
     e._send_button(True);await asyncio.sleep(.04);u.SetCursorPos(int(x+dx),int(y+dy))
     for capture in range(3):
      await asyncio.sleep(.08)
      summary=await m.canvas_summary(s,view)
      png=m.c42.png_of(await rc(s,"inspect.screenshot",{"target":view}))
      held=bool(summary.get("preview"))
      if kind=="move":
       transform=(summary.get("preview") or {}).get("transform",[])
       held=held and len(transform)==6 and abs(transform[4]-dx)<=1 and abs(transform[5]-dy)<=1
      checks.append(held and bool(png))
      run.check(held and bool(png),"PV5 held gesture preview every capture (preview note section 5) "+variant+str(iteration)+":"+str(capture),summary.get("preview"))
     if cancel:m.c42.os_key(0x1B)
     e._send_button(False)
     cached=([b[0]+dx,b[1]+dy,b[2]+dx,b[3]+dy],place)
    if variant=="cross-view":
     await cmd("view.active.set",{"viewId":"drawingView"});await cmd("tool.active.set",{"toolId":"select"});await cmd("draw.selection.set",{"by":"all"})
    played=None
    if variant=="played-overlap":
     b=await m.box_of(s,"cameraView");cached=(b,await e.locate_view(s,rc,"cameraView"));pts=[((b[0]+b[2])/2+18+i/100,(b[1]+b[3])/2+12) for i in range(1440)]
     played=asyncio.create_task(m.e2e.call(s,"pointer",{"view":"cameraView","events":m.c42.drag(pts)}))
     await asyncio.sleep(.1)
     run.check(not played.done(),"PV5 user begins while played pointer completes",None)
    await gesture("drawingView" if variant=="cross-view" else "cameraView")
    if variant=="agent-between":await cmd("element.rename",{"elementId":ids["Bob"],"name":"Bob"+str(iteration)})
    if variant=="rotate":
     other=next(v for k,v in ids.items() if k!="Bob")
     await cmd("app.context.set",{"elementId":other,"frame":1})
     await cmd("edit.selection.set",{"context":"staging","ids":[other]})
     await cmd("anim.tool.set",{"tool":"transform"})
    if variant=="refusal":
     await cmd("anim.transform.set",{"nodeIds":[ids["Bob"]],"target":"key","frame":1,"skew":88})
     await cmd("view.zoom.set",{"viewId":"cameraView","zoom":.1})
     await cmd("anim.tool.set",{"tool":"transform"})
    if variant=="cross-view":
     await cmd("view.active.set",{"viewId":"cameraView"});await cmd("anim.tool.set",{"tool":"select"})
    if variant in ("rotate","cross-view","refusal"):await asyncio.sleep(.2)
    await gesture("cameraView",variant=="escape",variant if variant in ("rotate","refusal") else "move")
    if played:
     answer=await played
     run.check((answer.get("error") or {}).get("code")=="ui.busy" and ((answer.get("error") or {}).get("details") or {}).get("reason")=="userInput","SI-135 user press cancels played pointer with ui.busy/userInput",answer.get("error"))
    if variant=="triple":await gesture()
    await asyncio.sleep(.6)
    clear=not (await m.canvas_summary(s,"drawingView" if variant=="cross-view" else "cameraView")).get("preview")
    if variant=="refusal":
     rec=(await m.canvas_summary(s)).get("lastGesture") or {}
     run.save("refusal-"+str(iteration)+".json",rec)
     run.check(rec.get("status") not in (None,"ok"),"05 section 19 command table line 1236 skew beyond 89 is refused",rec)
    hist=await cmd("edit.history.get",{"limit":1000});labels=[n.get("label") for n in hist.get("entries",[])[pos:hist["position"]]]
    expected=["Move Element"]*(3 if variant=="triple" else 1 if variant=="escape" else 2)
    if variant=="rotate":expected=["Move Element","Rotate Element"]
    if variant=="cross-view":expected=["Move Object","Move Element"]
    if variant=="refusal":expected=["Move Element","Skew Element"]
    if variant=="agent-between":expected=["Move Element","Rename Drawing Element","Move Element"]
    run.check(labels==expected,"05 section 5.0 line 400; 02 section 7.1.1 line 440; 04 section 4.1 line 284: expected ordered labels "+repr(expected),labels)
    run.check(clear,"PV5 committed/cancelled preview cleared "+variant+str(iteration),None)
    sources=[n.get("source") for n in hist.get("entries",[])[pos:hist["position"]]]
    wanted_sources=["user"]*len(expected)
    if variant=="agent-between":wanted_sources=["user","agent","user"]
    if variant=="refusal":wanted_sources=["user","agent"]
    if variant=="played-overlap":wanted_sources=["user","user"]
    run.check(sources==wanted_sources,"PV1 gesture/agent steps have expected order "+repr(wanted_sources),sources)
    results.append({"sources":sources,"variant":variant,"iteration":iteration,"held":all(checks),"clear":clear,"labels":labels})
    for _ in range(hist["position"]-pos):await cmd("edit.undo")
   except Exception as ex:
    e._send_button(False);run.check(False,"variant completed "+variant+str(iteration),repr(ex))
   run.save("variants.json",results)
def a2():
 r1.live_preview=variants
 r1.case5()
def a3():
 m=load("d39review","scripts/c43-d39-bindings.py")
 original_do=m.Table.do
 async def do(t,action):
  tree=await m.McpTable.tree(t)
  node=m.McpTable.node(tree,action[1]) if action[0] in ("dom","select") else None
  if action[0]=="select" and node is not None:node=next((c for c in node.get("children") or [] if c.get("value")==action[2]),None)
  t.run.save("declared-"+str(action[1])+".json",node)
  ref=(node or {}).get("command") or {}
  t.review_declared={"command":ref.get("commandId"),"params":ref.get("params") or {}}
  return await original_do(t,action)
 m.Table.do=do
 original_observed=m.Table.observed
 async def observed(t,want):
  sent=await original_observed(t,want)
  t.run.check(sent==[t.review_declared],"D-39 tree declaration equals OS input command/params; "+m.spec_line(want[0]),{"declared":t.review_declared,"sent":sent})
  return sent
 m.Table.observed=observed
 original_row=m.Table.row
 async def menus(t,state):
  import ctypes
  from ctypes import wintypes
  u=ctypes.windll.user32;u.GetMenu.restype=wintypes.HMENU;u.GetSubMenu.restype=wintypes.HMENU
  u.GetMenu.argtypes=[wintypes.HWND];u.GetSubMenu.argtypes=[wintypes.HMENU,ctypes.c_int]
  u.GetMenuItemCount.argtypes=[wintypes.HMENU];u.GetMenuState.argtypes=[wintypes.HMENU,ctypes.c_uint,ctypes.c_uint]
  u.GetMenuStringW.argtypes=[wintypes.HMENU,ctypes.c_uint,wintypes.LPWSTR,ctypes.c_int,ctypes.c_uint]
  h=m.window_handle()
  def native(handle):
   rows=[]
   for i in range(u.GetMenuItemCount(handle)):
    b=ctypes.create_unicode_buffer(1024);u.GetMenuStringW(handle,i,b,1024,0x400)
    flags=u.GetMenuState(handle,i,0x400);sub=u.GetSubMenu(handle,i)
    text=b.value.replace('&','');parts=text.split('\t')
    rows.append({"role":"menu" if sub else "separator" if flags&0x800 else "menuitem","label":parts[0].removeprefix(chr(0x2713)+" "),"enabled":not bool(flags&3),"checked":bool(flags&8) or parts[0].startswith(chr(0x2713)+" "),"shortcut":parts[1] if len(parts)>1 else "","children":native(sub) if sub else []})
   return rows
  await asyncio.sleep(.5)
  bindings={}
  def collect(items,path):
   for item in items:
    if item.get("platforms") and "windows" not in item["platforms"]:continue
    label=item.get("label","");at=path+"/"+label
    if item.get("kind")=="command":bindings[at]=item
    collect(item.get("items",[]),at)
  for menu in json.loads((ROOT/"desktop/src/ui-bindings/menu.json").read_text(encoding="utf-8"))["menus"]:
   if not menu.get("platforms") or "windows" in menu["platforms"]:collect(menu.get("items",[]),"menu:"+menu["label"])
  tree=await m.McpTable.tree(t);bar=m.McpTable.node(tree,"menuBar") or {}
  raw=native(u.GetMenu(h));t.run.save("menus-"+state+".json",{"native":raw,"tree":bar})
  def walk(a,b,path):
   t.run.check(len(a)==len(b),"D-38 every native/menu tree item "+state+path,{"native":len(a),"tree":len(b)})
   for n,q in zip(a,b):
    label=q.get("label","")
    equal=n["role"]==q.get("role") and (n["role"]=="separator" or n["label"]==label)
    if n["role"]!="separator":equal=equal and n["enabled"]==q.get("enabled")
    if n["role"]=="menuitem":
     equal=equal and n["enabled"]==q.get("enabled") and n["checked"]==bool(q.get("checked"))
     def norm(text):
      value=(text or "").lower().replace("primary","ctrl").replace("control","ctrl").replace(" ","")
      for word,symbol in (("equal","="),("minus","-"),("bracketright","]"),("bracketleft","["),("backslash",chr(92)),("comma",","),("period",".")):
       value=value.replace(word,symbol)
      return value
     equal=equal and norm(n["shortcut"])==norm(q.get("shortcut"))
     ref=q.get("command") or {}
     equal=equal and ref.get("commandId")==q.get("commandId") and isinstance(ref.get("params",{}),dict)
     binding=bindings.get(q.get("id"))
     if binding:
      equal=equal and ref.get("commandId")==binding["command"] and all(ref.get("params",{}).get(k)==v for k,v in binding.get("params",{}).items())
    t.run.check(equal,"D-38 role/label/enabled/checked/reference "+state+path+"/"+label,{"native":n,"tree":q})
    if n["children"]:walk(n["children"],q.get("children",[]),path+"/"+label)
  walk(raw,bar.get("children",[]),"")
  class Info(ctypes.Structure):
   _fields_=[("cbSize",wintypes.UINT),("fMask",wintypes.UINT),("fType",wintypes.UINT),("fState",wintypes.UINT),("wID",wintypes.UINT),("hSubMenu",wintypes.HMENU),("hbmpChecked",wintypes.HBITMAP),("hbmpUnchecked",wintypes.HBITMAP),("dwItemData",ctypes.c_size_t),("dwTypeData",wintypes.LPWSTR),("cch",wintypes.UINT),("hbmpItem",wintypes.HBITMAP)]
  u.GetMenuItemInfoW.argtypes=[wintypes.HMENU,wintypes.UINT,wintypes.BOOL,ctypes.POINTER(Info)]
  help_index=next(i for i,n in enumerate(raw) if n["label"]=="Help")
  help_handle=u.GetSubMenu(u.GetMenu(h),help_index)
  text=ctypes.create_unicode_buffer(256);info=Info();info.cbSize=ctypes.sizeof(Info);info.fMask=0x40;info.dwTypeData=ctypes.cast(text,wintypes.LPWSTR);info.cch=255
  about_index=next(i for i,n in enumerate(raw[help_index]["children"]) if n["label"]=="About")
  if not u.GetMenuItemInfoW(help_handle,about_index,True,ctypes.byref(info)):raise RuntimeError("secondary menu label API failed")
  help_tree=bar["children"][help_index]["children"][about_index]
  t.run.check(text.value.replace("&","")==help_tree.get("label"),"D-38 second native API GetMenuItemInfoW About label "+state,{"native":text.value.replace("&",""),"tree":help_tree.get("label")})
  t.run.save("about-second-api-"+state+".json",{"native":text.value,"tree":help_tree})
 original_build=m.build_and_run
 async def build(t,only):
  await menus(t,"no-project")
  await original_build(t,only)
 m.build_and_run=build
 original_cmd=m.Table.cmd
 menu_states=set()
 async def cmd(t,n,p=None):
  result=await original_cmd(t,n,p)
  if n=="element.drawing.create" and "empty" not in menu_states:
   menu_states.add("empty");await asyncio.sleep(.3);await menus(t,"empty-selection")
  return result
 m.Table.cmd=cmd

 chosen=[]
 async def row(t,r):
  action=r["action"]
  if callable(action):
   try:action=action()
   except (LookupError,TypeError):return
  wanted=action[0] in ("dom","select") and (action[1].startswith(("mainToolbar.","drawingTools.")) or action[1] in [v+x for v in ("drawingView","cameraView") for x in (".aid.grid",".aid.onionSkin",".symmetry",".type.camera",".type.drawing")])
  if not wanted:return
  prefix=action[1].split(".")[0]
  quota={"mainToolbar":6,"drawingView":5,"cameraView":5,"drawingTools":3,"status":1}
  if sum(x==prefix for x in chosen)>=quota[prefix]:return
  chosen.append(prefix)
  if "selection" not in menu_states:
   menu_states.add("selection")
   await t.cmd("draw.rectangle.add",{"from":{"x":-20,"y":-20},"to":{"x":20,"y":20}})
   await t.cmd("view.active.set",{"viewId":"drawingView"});await t.cmd("draw.selection.set",{"by":"all"})
   await asyncio.sleep(.4);await menus(t,"drawing-selection")
   await t.cmd("draw.lock.set",{"locked":True});await asyncio.sleep(.4);await menus(t,"locked")
   await t.cmd("draw.lock.set",{"locked":False})
  await original_row(t,r)
  if r.get("shows"):
   before=await r["state"]();want=r["expect"](before) if callable(r["expect"]) else r["expect"]
   if want[0]=="tool.active.set":await t.cmd("tool.active.set",{"toolId":"brush" if want[1]["toolId"]!="brush" else "select"})
   if want[0]=="view.type.set":await t.cmd("view.type.set",{**want[1],"type":"drawing" if want[1]["type"]!="drawing" else "camera"})
   await t.cmd(want[0],want[1]);after=await r["state"]()
   shown=await m.McpTable.shows(t,r,after)
   t.run.check(shown is not False,"D-39 agent state reflected in tree; "+r["spec"],shown)
 m.Table.row=row
 original_jobs=m.jobs_row
 async def jobs(t):
  await original_jobs(t)
  tree=await t.tree();n=t.node(tree,"agent.toggle")
  t.run.check(n is not None,"01 section 9.1 status agent control is in tree",n)
  if n:
   ref=n.get("command");t.clear();t.click("agent.toggle");await asyncio.sleep(.3)
   expected=[] if not ref else [{"command":ref["commandId"],"params":ref.get("params",{})}]
   t.run.check([c for c in t.recorded() if t.kinds.get(c.get("command"))!="query"]==expected,"D-39 status bar agent toggle GUI-only declares no command (01 section 9.1; parity note section 5)",t.recorded())
 m.jobs_row=jobs
 async def no_custom(*args,**kwargs):return None
 m.custom_row=no_custom
 m.Table.tree=m.McpTable.tree;m.Table.node=staticmethod(m.McpTable.node)
 sys.argv=["d39",str(Path(sys.argv[1]).resolve()),str(OUT/"a3"),"--mode","os"]
 raise SystemExit(m.main())
def baseline(case, check=None):
 if os.environ.get("REVIEW_RESUME"):
  paths={"case3":OUT/"case3/webgpu/manifest.json","case4":OUT/"case4/manifest.json","case6":OUT/"case6-drawing/manifest.json"}
  if case in paths and paths[case].exists():
   return int(bool(json.loads(paths[case].read_text()).get("failures")))
  if check and (OUT/"case7"/(check+"-result.json")).exists():
   return json.loads((OUT/"case7"/(check+"-result.json")).read_text())["exit"]
 old=sys.argv[:]
 sys.argv=[old[0],str(Path(old[1]).resolve()),case]+([check] if check else ([str(ROOT/"target/release/toontown-headless.exe")] if case=="case4" else []))
 original_run=subprocess.run
 def redirected_run(args,**kwargs):
  if len(args)>1 and str(args[1]).endswith("c43b-r1-regression-runner.py"):
   runner=OUT/"regression-runner.py"
   source=(ROOT/"tests/adversarial/c43b-r1-regression-runner.py").read_text()
   source=source.replace("ROOT=Path(__file__).resolve().parents[2]","ROOT=Path("+repr(str(ROOT))+")")
   source=source.replace("NEW='reviews/evidence/M4-C4.3b-r1/case7/legacy'","NEW="+repr((OUT/"legacy").as_posix()))
   runner.write_text(source,encoding="utf-8",newline="\n")
   args=[args[0],str(runner)]+list(args[2:])
  return original_run(args,**kwargs)
 subprocess.run=redirected_run
 try:
  getattr(r1,case)()
  return 0
 except SystemExit as ex:return int(ex.code or 0)
 finally:sys.argv=old;subprocess.run=original_run

def summarize(case, results):
 import hashlib
 files=[{"file":str(p.relative_to(OUT)),"sha256":hashlib.sha256(p.read_bytes()).hexdigest()} for p in OUT.rglob("*") if p.is_file()]
 dest=ROOT/"reviews/evidence/M4-C4.3b-r2"/(case+"-summary.json")
 receipts=[]
 for p in OUT.rglob("*.json"):
  if p.name not in ("manifest.json","rows.json","measurements.json") and not (case=="b3" and p.name=="transcript.json"):continue
  try:
   data=json.loads(p.read_text(encoding="utf-8"))
   if isinstance(data,list):
    if p.name=="transcript.json":receipts.append({"file":str(p.relative_to(OUT)),"checks":[{"check":r.get("check"),"ok":r.get("ok")} for r in data if isinstance(r,dict)]})
    if p.name=="rows.json":receipts.append({"file":str(p.relative_to(OUT)),"rows":[{"name":r.get("name"),"ok":r.get("ok")} for r in data if isinstance(r,dict)]})
   elif isinstance(data,dict):receipts.append({"file":str(p.relative_to(OUT)),**{k:data[k] for k in ("checks","rows","failed","failures","seconds") if k in data}})
  except (ValueError,UnicodeError):pass
 dest.write_text(json.dumps({"receipts":receipts,"results":results,"raw":str(OUT),"hashes":files,"regenerate":"python tests/adversarial/c43b-r2-live.py target/release/toontown.exe "+case},indent=1)+"\n",encoding="utf-8",newline="\n")
 print(json.dumps(results),flush=True)
 return any(r.get("exit",0) for r in results)

def b1():
 results=[]
 for name in ("measure-webgl2","measure-webgpu","one-drag-gate"):
  results.append({"name":name,"exit":baseline("case7",name)})
 m=load("assessmentFixtures","scripts/test_c43_measure_assess.py")
 import copy
 mutations={
 "wrong-backend":lambda r:r.update(backend="webgl2"),
 "fallback":lambda r:r.update(fallback="adapter unavailable"),
 "1001-resident":lambda r:r["groups1k"].update(resident=1001),
 "29-samples":lambda r:r["sample"]["commits"].update(n=29),
 "uncommitted":lambda r:r["sample"]["commits"].update(n=0),
 "short-ring":lambda r:r["sample"].update(dragIntervals=[16.7]),
 "missing-control":lambda r:r.pop("control"),
 "idle-only-work":lambda r:r["sample"].pop("dragWork"),
 "zero-control":lambda r:r["control"]["frameMs"].update(p50=0,p95=0,max=0)}
 for name,mutate in mutations.items():
  record=m.complete();mutate(record)
  path=OUT/(name+".json");path.write_text(json.dumps(record),encoding="utf-8")
  failures=m.failed(json.loads(path.read_text()))
  results.append({"name":name,"exit":0 if failures else 1,"rejectedBy":failures})
  if not failures:
   independent=load("assessmentIndependent"+name.replace("-",""),"scripts/c43_measure_assess.py")
   results[-1]["secondAssessment"]=independent.assess(json.loads(path.read_text()),m.BOUNDS,30)
 for backend in ("webgl2","webgpu"):
  path=OUT/"case7"/("measure-"+backend)/"measurements.json"
  if path.exists():
   rec=json.loads(path.read_text())["record"]
   figures={"control":rec.get("control"),"backend":rec.get("backend"),"fallback":rec.get("fallback")}
   for scene in ("sample","groups1k"):
    data=rec.get(scene,{})
    figures[scene]={k:data.get(k) for k in ("resident","idleWork","dragWork","feedback","d33")}
    figures[scene]["commits"]={k:v for k,v in data.get("commits",{}).items() if k!="samples"}
   results.append({"name":backend+"-figures","record":figures})
 raise SystemExit(summarize("b1",results))

def b2():
 results=[]
 for case in ("case3","case4","case6"):
  results.append({"name":case,"exit":baseline(case)})
 names=["c41","c42","byte","r1","r2","r3","r4","c42b-r1s-selection","c42b-r1s-generation","c42b-r1s-experiments","c42b-r1-conformance"]
 names += ["r1-"+x for x in ("c3","d2","d2-exact-attached","d2-exact-headless","d3-webgl2")]
 names += ["r2-"+x for x in ("c2","d3-webgl2","gui","regression","byte","exact","exact-select","exact-tools")]+["r3-labels"]
 if len(sys.argv)>3:names=[sys.argv[3]]
 for name in names:results.append({"name":name,"exit":baseline("case7",name)})
 app=str(Path(sys.argv[1]).resolve());head=str(ROOT/"target/release/toontown-headless.exe")
 jobs=[("builder-e6",["scripts/c43-e6-coexistence.py",app,str(OUT/"builder-e6")]),("builder-equivalence",["scripts/c43-gesture-equivalence.py",app,head,str(OUT/"builder-equivalence")]),("builder-d39",["scripts/c43-d39-bindings.py",app,str(OUT/"builder-d39"),"--mode","os"])]
 jobs += [("builder-camera-"+b,["scripts/c43-camera-e2e.py",app,str(OUT/("builder-camera-"+b)),"--backend",b]) for b in ("webgl2","webgpu")]
 for name,args in jobs:
  cached=OUT/(name+"-result.json")
  if os.environ.get("REVIEW_RESUME") and cached.exists():
   results.append(json.loads(cached.read_text()));continue
  try:
   proc=subprocess.run([sys.executable]+args,cwd=ROOT,capture_output=True,timeout=1800 if name=="builder-d39" else 530)
   (OUT/(name+".txt")).write_bytes(proc.stdout+proc.stderr)
   results.append({"name":name,"exit":proc.returncode})
  except subprocess.TimeoutExpired:results.append({"name":name,"exit":1,"error":("1800" if name=="builder-d39" else "530")+" second timeout"})
  cached.write_text(json.dumps(results[-1]),encoding="utf-8")
 raise SystemExit(summarize("b2",results))

def b3():
 m=load("b3table","scripts/c43-d39-bindings.py")
 m.Table.tree=m.McpTable.tree;m.Table.node=staticmethod(m.McpTable.node)
 def merge_confirm(t):
  t.click("message.merge");time.sleep(.6)
  state=t.cdp.js("({confirm:String(window.confirm),notice:document.querySelector('[data-tt-id=\"keymap.notice\"]')?.textContent})")
  t.run.save("b3-merge-confirm.json",state)
  t.run.check("(merge)" in (state.get("notice") or ""),"01 section 8.3 line 857 Import offers merge rather than always replacing",state)
 original_row=m.Table.row
 async def reflecting_row(t,r):
  await original_row(t,r)
  if r.get("shows"):
   before=await r["state"]();want=r["expect"](before) if callable(r["expect"]) else r["expect"]
   await t.cmd(want[0],want[1]);after=await r["state"]()
   shown=await m.McpTable.shows(t,r,after)
   t.run.check(shown is not False,"D-39 agent changes reflected; "+r["spec"],shown)
 m.Table.row=reflecting_row
 original_jobs=m.jobs_row
 async def extra(t):
  await original_jobs(t)
  discovery=json.loads((t.work/"discovery.json").read_text())
  async with m.contextlib.AsyncExitStack() as stack:
   client=await stack.enter_async_context(m.e2e.httpx2.AsyncClient(headers={"Authorization":"Bearer "+discovery["token"]},timeout=300))
   streams=await stack.enter_async_context(m.e2e.streamable_http_client(discovery["endpoint"],http_client=client))
   from mcp import types
   async def roots(_ctx):return types.ListRootsResult(roots=[types.Root(uri=t.work.resolve().as_uri(),name="work")])
   agent=await stack.enter_async_context(m.e2e.ClientSession(streams[0],streams[1],list_roots_callback=roots))
   await agent.initialize()
   env=await m.e2e.run_cmd(agent,"render.range.run",{"from":1,"to":2400,"scale":2,"directory":str(t.work/"cancel-frames"),"createDirs":True,"onConflict":"overwrite"})
   jid=(env.get("result") or {}).get("jobId")
   await asyncio.sleep(.5)
   listed=await m.e2e.run_cmd(agent,"job.list",{})
   status_text=t.cdp.js("document.querySelector('[data-tt-id=\"status.jobs\"]')?.textContent")
   t.run.check(bool(status_text) and "running" in status_text,"01 section 9.1 second agent job shown while running",status_text)
   dom=t.cdp.js("Array.from(document.querySelectorAll('[data-tt-command=\"job.cancel\"]')).map(e=>({id:e.dataset.ttId,params:e.dataset.ttParams}))")
   tree=(await m.e2e.tree(t.s)).get("result")
   t.run.save("b3-job-cancel.json",{"jobId":jid,"jobs":listed,"dom":dom,"tree":tree})
   t.run.check(bool(dom),"01 section 9.5 lines 925-927 cancellable running operation has window Cancel",dom)
   if dom:
    t.clear();t.click(dom[0]["id"]);await asyncio.sleep(.5)
    t.run.check(any(c.get("command")=="job.cancel" and c.get("params",{}).get("jobId")==jid for c in t.recorded()),"01 section 9.5 lines 925-927 window sends job.cancel for displayed job",t.recorded())
   if jid:
    payload={"command":"job.cancel","params":{"jobId":jid}}
    direct=t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json:"+json.dumps(json.dumps(payload))+"})")
    t.run.save("b3-job-cancel-direct.json",{"jobId":jid,"response":direct})
    await m.e2e.call(agent,"job_wait",{"jobId":jid,"timeoutMs":120000})
    for _ in range(30):
     end=t.cdp.js("document.querySelector('[data-tt-id=\"status.jobs\"]')?.textContent")
     if not end:break
     await asyncio.sleep(.1)
    t.run.check(end=="","01 section 9.1 second agent job disappears when ended",end)
  async def cmd(n,p=None):return await t.cmd(n,p or {})
  async def document():return r1.canonical((await cmd("doc.export.json",{"scope":"project","binaries":"omit"})).get("bundle"))
  async def check(name,fn):
   try:await fn()
   except Exception as ex:t.run.check(False,name+" ran",repr(ex))
  async def backups():
   import hashlib
   await cmd("project.save",{})
   path=t.work/"B3.animpkg"
   await cmd("project.saveAs",{"path":str(path),"onExists":"overwrite"})
   snapshots=[]
   # D-60 item 3d: the current element set to its drawing (an earlier row may have made a camera current).
   els=(await cmd("element.list",{})).get("elements") or []
   bob=next(x["id"] for x in els if x.get("name")=="Bob")
   await cmd("app.context.set",{"elementId":bob,"frame":1})
   for i in range(3):
    await asyncio.sleep(1.1)
    await cmd("draw.rectangle.add",{"from":{"x":i*20,"y":0},"to":{"x":i*20+10,"y":10}})
    await cmd("project.save",{})
    snapshots.append(await document())
   listed=await cmd("project.backup.list",{})
   before=hashlib.sha256(path.read_bytes()).hexdigest()
   m.choose_menu(["File","Open Backup"]);await asyncio.sleep(1)
   text=t.cdp.js("document.querySelector('[data-tt-id=\"openBackup.list\"]')?.textContent")
   t.run.check(len(listed)>=2 and all(x["savedAt"] in text for x in listed),"01 section 3.7 lines 259-262 backup save times listed",text)
   t.clear();t.click("openBackup.open.1");await asyncio.sleep(2)
   state=await cmd("app.state.get",{})
   doc=await document()
   t.run.save("b3-backup-state.json",{"listed":listed,"snapshots":snapshots,"opened":doc,"state":state,"sent":t.recorded()})
   t.run.check(not (state.get("project") or {}).get("path"),"01 section 3.7 lines 259-262 older backup is untitled",state)
   t.run.check(hashlib.sha256(path.read_bytes()).hexdigest()==before,"01 section 3.7 lines 259-262 original file untouched",None)
   t.run.check(doc==snapshots[0],"01 section 3.7 lines 259-262 older selected backup equals first content save",None)
   found=await cmd("doc.object.find",{"type":"element","name":"Bob"})
   bob=(found.get("items") or found.get("matches") or [{}])[0].get("id")
   await cmd("app.context.set",{"elementId":bob,"frame":1})
   await cmd("draw.rectangle.add",{"from":{"x":200,"y":0},"to":{"x":220,"y":10}})
   dirty=await document()
   m.choose_menu(["File","Open Backup"]);await asyncio.sleep(.8)
   message=t.cdp.js("document.querySelector('[data-tt-id=\"message.project.unsavedChanges\"]')?.textContent")
   t.run.check(bool(message),"01 section 3.5 unsaved backup opening prompts Save/Discard/Cancel",message)
   if message:
    t.click("message.cancel");await asyncio.sleep(.5)
    t.run.check(await document()==dirty,"01 section 3.5 Cancel preserves dirty current document",None)
    m.choose_menu(["File","Open Backup"]);await asyncio.sleep(.5);t.click("message.discard");await asyncio.sleep(.7)
    t.clear();t.click("openBackup.open.1");await asyncio.sleep(2)
    after_discard=await document();sent=t.recorded()
    t.run.check(after_discard==snapshots[0],"01 section 3.5 Discard then Open restores chosen backup",sent)
    payload={"command":"project.backup.open","params":{"backupId":listed[1]["backupId"],"onUnsaved":"discard"}}
    direct=t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json:"+json.dumps(json.dumps(payload))+"})")
    await asyncio.sleep(.7);after_direct=await document()
    t.run.save("b3-backup-unsaved.json",{"dirty":dirty,"afterDiscardOpen":after_discard,"expected":snapshots[0],"sent":sent,"direct":direct,"afterDirect":after_direct})
    t.run.check(after_direct==snapshots[0],"01 section 3.5 independent explicit discard control restores backup",None)
  await check("backup",backups)
  async def empty_backup():
   await cmd("project.saveAs",{"path":str(t.work/"BeforeEmpty.animpkg"),"onExists":"overwrite"})
   await cmd("project.create",{"name":"EmptyB3","onUnsaved":"fail"})
   await cmd("project.saveAs",{"path":str(t.work/"EmptyB3.animpkg"),"onExists":"overwrite"})
   m.choose_menu(["File","Open Backup"]);await asyncio.sleep(1)
   text=t.cdp.js("document.querySelector('[data-tt-id=\"openBackup.list\"]')?.textContent")
   buttons=t.cdp.js("document.querySelectorAll('[data-tt-command=\"project.backup.open\"]').length")
   t.run.check(text and "No backups" in text and buttons==0,"01 section 3.7 lines 259-262 empty backup notice and no Open",text)
   m.key(0x1B);await asyncio.sleep(.5)
  await check("empty backup",empty_backup)
  async def keymaps():
   await cmd("keymap.set",{"commandId":"project.save","keys":"Primary+Shift+F11","replaceExisting":True})
   await cmd("keymap.set",{"commandId":"edit.undo","keys":"Primary+Shift+F12","replaceExisting":True})
   expected=await cmd("keymap.list",{})
   m.choose_menu(["File","Preferences"]);await asyncio.sleep(.8);t.click("preferences.category.keyboard");await asyncio.sleep(.5)
   exported=t.work/"B3-keymap.json"
   t.click("keymap.export");await asyncio.sleep(1.5);m.type_text(str(exported));m.key(0x0D);await asyncio.sleep(1)
   t.run.check(exported.exists(),"01 section 8.3 line 857 Export real save dialog creates file",None)
   m.key(0x1B);await asyncio.sleep(.4)
   for mode in ("replace","merge"):
    await cmd("keymap.reset",{"all":True})
    defaults=await cmd("keymap.list",{})
    m.choose_menu(["File","Preferences"]);await asyncio.sleep(.8);t.click("preferences.category.keyboard");await asyncio.sleep(.5)
    t.click("keymap.import");await asyncio.sleep(1.5);m.type_text(str(exported));m.key(0x0D);await asyncio.sleep(1)

    if mode=="replace":t.click("message.replace")
    else:merge_confirm(t)
    await asyncio.sleep(.5);t.click("preferences.submit");await asyncio.sleep(1)
    after=await cmd("keymap.list",{})
    canonical=lambda rows:sorted(json.dumps(x,sort_keys=True) for x in rows)
    wanted=expected if mode=="replace" else defaults+[x for x in expected if x not in defaults]
    t.run.check(canonical(after)==canonical(wanted),"01 section 8.3 lines 857-858 imported "+mode+" keymap equals replacement or union",None)
    tree=(await m.e2e.tree(t.s)).get("result") or {}
    encoded=json.dumps(tree)
    t.run.save("b3-keymap-"+mode+".json",{"expected":wanted,"actual":after,"tree":tree,"sent":t.recorded()})
    if mode=="merge":
     await cmd("keymap.reset",{"all":True})
     await cmd("keymap.import",{"path":str(exported),"mode":"merge"})
     direct=await cmd("keymap.list",{})
     after=direct
     t.run.save("b3-merge-direct.json",{"expected":wanted,"actual":direct})
     t.run.check(canonical(direct)==canonical(wanted),"01 section 8.3 line 857 independent explicit merge preserves union",None)
    if mode=="replace":t.run.check("Primary+Shift+F11" in encoded and "Primary+Shift+F12" in encoded,"D-38 replaced keymap menu shortcuts reflect imported bindings",None)
    import ctypes
    u=ctypes.windll.user32;u.GetMenu.restype=ctypes.c_void_p;u.GetMenu.argtypes=[ctypes.c_void_p];u.GetSubMenu.restype=ctypes.c_void_p
    u.GetMenuItemCount.argtypes=[ctypes.c_void_p];u.GetSubMenu.argtypes=[ctypes.c_void_p,ctypes.c_int]
    u.GetMenuStringW.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_wchar_p,ctypes.c_int,ctypes.c_uint]
    def labels(menu):
     found=[]
     for i in range(u.GetMenuItemCount(menu)):
      b=ctypes.create_unicode_buffer(1024);u.GetMenuStringW(menu,i,b,1024,0x400)
      found.append(b.value.replace("&",""))
      sub=u.GetSubMenu(menu,i)
      if sub:found.extend(labels(sub))
     return found
    native=labels(u.GetMenu(m.front()))
    for command,label in (("project.save","Save"),("edit.undo","Undo")):
     allowed=[b["keys"] for b in after if b["commandId"]==command]
     normalize=lambda x:sorted(x.replace("Primary","Ctrl").split("+"))
     visible=[x.split("\t",1)[1] for x in native if "\t" in x and (x.split("\t",1)[0]==label or label=="Undo" and x.split("\t",1)[0].startswith("Undo "))]
     t.run.check(bool(visible) and all(any(normalize(k)==normalize(v) for k in allowed) for v in visible),"D-38 native "+label+" accelerator belongs to imported "+mode+" keymap",{"shown":visible,"allowed":allowed})
   await cmd("keymap.reset",{"all":True})
   for kind in ("malformed","conflict"):
    before=await cmd("keymap.list",{})
    bad=t.work/(kind+".json")
    data=json.loads(exported.read_text())
    if kind=="conflict":
     bindings=data["bindings"]
     bindings[1]["keys"]=bindings[0]["keys"];bindings[1]["context"]=bindings[0]["context"]
    bad.write_text("{oops" if kind=="malformed" else json.dumps(data),encoding="utf-8")
    m.choose_menu(["File","Preferences"]);await asyncio.sleep(.8);t.click("preferences.category.keyboard");await asyncio.sleep(.5)
    t.click("keymap.import");await asyncio.sleep(1.5);m.type_text(str(bad));m.key(0x0D);await asyncio.sleep(1);m.key(0x0D);await asyncio.sleep(.5);t.click("preferences.submit");await asyncio.sleep(1)
    after=await cmd("keymap.list",{})
    notice=t.cdp.js("document.querySelector('[data-tt-id=\"preferences.refusal\"]')?.textContent")
    t.run.check(bool(notice),"01 section 8.3 lines 850-858 "+kind+" import reports refusal",notice)
    t.run.save("b3-"+kind+".json",{"before":before,"after":after,"notice":notice})
    t.run.check(before==after,"01 section 8.3 lines 850-858 "+kind+" import no partial change",None)
    m.key(0x1B);await asyncio.sleep(.3);m.key(0x1B);await asyncio.sleep(.3)
  await check("keymaps",keymaps)
 m.jobs_row=extra
 original_build=m.build_and_run
 async def build(t,only):
  await original_build(t,only)
  env=await m.e2e.run_cmd(t.s,"tool.symmetry.set",{"mode":"mirror"})
  t.run.check(env.get("status")!="ok" and (env.get("error") or {}).get("code")=="project.none","D-39 and 01 section 1.3 lines 71-75 symmetry without project refused project.none",env)
 m.build_and_run=build
 old=sys.argv[:];sys.argv=["d39",str(Path(old[1]).resolve()),str(OUT/"b3"),"--mode","os","--only","main toolbar,type camera,type drawing,symmetry"]
 code=m.main();sys.argv=old
 raise SystemExit(summarize("b3",[{"name":"B3-window","exit":code}]))

if __name__=="__main__":
 import socket
 sock=socket.socket();listening=sock.connect_ex(("127.0.0.1",1420))==0;sock.close()
 server=None
 if not listening:
  server=subprocess.Popen([sys.executable,"-m","http.server","1420","--bind","127.0.0.1","--directory",str(ROOT/"desktop/dist")],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  time.sleep(.5)
 print("Raw evidence:",OUT,flush=True)
 try:{"a2":a2,"a3":a3,"b1":b1,"b2":b2,"b3":b3}[sys.argv[2]]()
 finally:
  if server:server.terminate();server.wait()
