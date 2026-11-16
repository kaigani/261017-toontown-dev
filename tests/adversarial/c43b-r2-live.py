"""Independent round-two live probes: python FILE APP a2|a3. Raw data stays in TEMP."""
import sys
if len(sys.argv)<3:
 print("skipped: no application")
 sys.exit(0)
import asyncio, importlib.util, json, os, subprocess, tempfile, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(tempfile.mkdtemp(prefix="c43b-r2-raw-"))
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
if __name__=="__main__":
 import socket
 sock=socket.socket();listening=sock.connect_ex(("127.0.0.1",1420))==0;sock.close()
 server=None
 if not listening:
  server=subprocess.Popen([sys.executable,"-m","http.server","1420","--bind","127.0.0.1","--directory",str(ROOT/"desktop/dist")],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  time.sleep(.5)
 print("Raw evidence:",OUT,flush=True)
 try:{"a2":a2,"a3":a3}[sys.argv[2]]()
 finally:
  if server:server.terminate();server.wait()
