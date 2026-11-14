"""Independent round-three global visibility sequences; private settings only."""
import asyncio, importlib.util, json, sys, tempfile, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sp=importlib.util.spec_from_file_location('floats',ROOT/'tests/adversarial/c41-r2-desktop-floats.py'); f=importlib.util.module_from_spec(sp); sp.loader.exec_module(f)
e=f.e
async def main(app,out):
 receipt=[]
 with tempfile.TemporaryDirectory(prefix='tt-r3-c2-') as temp:
  user=Path(temp); (user/'Settings').mkdir(); (user/'Settings/preferences.json').write_text(json.dumps(dict(format='toontown-settings',kind='preferences',formatVersion='1.0',data={'agent.policy.defaults':{'policy':'autonomous'}})),encoding='utf-8')
  disc=user/'discovery.json'; proc=e.launch(app,user,disc)
  async def phase(restart=False):
   nonlocal proc
   async with f.session(e.wait_discovery(disc,proc)) as s:
    async def cmd(cid,p=None):
     v=await e.run_cmd(s,cid,p); assert v.get('status')=='ok',v
     return v.get('result')
    async def verify(name,minimized=False,geometry=None):
     start=time.monotonic(); end=start+10
     while True:
      ps=await cmd('window.panel.list'); wanted={p['title'] if 'title' in p else {'history':'History','colour':'Colour','properties':'Properties','timeline':'Timeline'}.get(p['panelId'],p['panelId']) for p in ps if p.get('floating') and p.get('visible') and not minimized}
      rows=f.windows(proc.pid); actual={r['title'] for r in rows if r['visible'] and not r['title'].startswith('Toontown')}
      ok=actual==wanted
      if geometry: ok=ok and all(all(abs(r[k]-g[k])<=2 for k in g) for title,g in geometry.items() for r in rows if r['title']==title)
      if ok or time.monotonic()>=end: break
     receipt.append(dict(check=name,ok=ok,durationS=time.monotonic()-start,kernel=ps,wanted=sorted(wanted),windows=rows)); print(('PASS ' if ok else 'FAIL ')+name,flush=True)
    async def hide(): await cmd('window.floatingPanels.set',{'visible':False})
    async def show(): await cmd('window.floatingPanels.set',{'visible':True})
    if restart:
     await verify('restart retains hidden'); await show(); await verify('restart show restores geometry',geometry={'History':dict(x=140,y=120,width=420,height=300)})
     await hide(); await cmd('window.panel.set',dict(panelId='timeline',floating=True,visible=True)); await verify('fourth float while globally hidden')
     await cmd('window.panel.set',dict(panelId='timeline',floating=False)); await verify('remove fourth while hidden')
     for final in [True,False]:
      for i in range(20): await cmd('window.floatingPanels.set',{'visible':bool(i%2)==final})
      await verify('twenty alternating ending '+str(final))
     await cmd('window.panel.set',dict(panelId='history',floating=False)); await show(); await verify('dock hidden then show two')
     return
    await e.tree(s); await cmd('workspace.create',{'name':'R3Floats'}); await cmd('workspace.select',{'name':'R3Floats'})
    for p in ['history','colour','properties']: await cmd('window.panel.set',dict(panelId=p,floating=True,visible=True,geometry=dict(x=140,y=120,width=420,height=300)))
    await verify('three floats'); await hide(); await verify('hide all'); await show(); await verify('show all')
    await hide(); await cmd('window.minimized.set',{'minimized':True}); await verify('hidden minimized',True); await cmd('window.minimized.set',{'minimized':False}); await verify('hidden restored')
    await show(); await cmd('window.minimized.set',{'minimized':True}); await hide(); await verify('minimize then hide',True); await cmd('window.minimized.set',{'minimized':False}); await verify('minimize hide restore')
    await cmd('workspace.save'); await cmd('workspace.select',{'name':'Drawing'}); await verify('hidden switch workspace'); await cmd('workspace.select',{'name':'R3Floats'}); await verify('hidden switch back'); await hide(); await cmd('workspace.save')
  try:
   await phase(); e.stop(proc); proc=e.launch(app,user,disc); await phase(True)
  finally: e.stop(proc)
 out.write_text(json.dumps(receipt,indent=2),encoding='utf-8',newline='\n'); return int(any(not r['ok'] for r in receipt))
if __name__=='__main__':
 if len(sys.argv)<2 or not Path(sys.argv[1]).is_file(): print('skipped: no application'); sys.exit(0)
 sys.exit(asyncio.run(main(str(Path(sys.argv[1]).resolve()),Path(sys.argv[2]))))
