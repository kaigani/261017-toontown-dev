"""Independent release-SDK command matrix, fresh saved base per mutation."""
import asyncio,importlib.util,json,tempfile,sys,hashlib,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('e4',ROOT/'scripts/e4-cutout-numeric.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
OUT=Path(sys.argv[2]);OUT.mkdir(parents=True,exist_ok=True)
RESULTS=[]
def clean(v):
 if isinstance(v,dict):return {k:clean(x) for k,x in v.items() if k not in ('rev','modifiedAt','createdAt')}
 if isinstance(v,list):return [clean(x) for x in v]
 return v
async def main():
 binary=str(Path(sys.argv[1]).resolve());work=Path(tempfile.mkdtemp(prefix='tt-c32-review-'))
 pkg,ids,request,_,_=await m.build(binary,work)
 async def roots(ctx):return m.types.ListRootsResult(roots=[m.types.Root(uri=work.as_uri(),name='work')])
 async with m.stdio_client(m.session_params(binary,work)) as (read,write):
  async with m.ClientSession(read,write,list_roots_callback=roots) as s:
   await s.initialize();await asyncio.sleep(.5)
   async def invoke(cid,p=None,**opts):return m.sc(await s.call_tool('run_command',dict(id=cid,params=p or {},**opts)))
   async def cmd(cid,p=None,**opts):
    r=await invoke(cid,p,**opts);assert r['status']=='ok',(cid,r);return r['result']
   copies=[]
   async def openbase():
    fresh=work/f'base-{len(copies)}.animpkg';copies.append(fresh);shutil.copyfile(pkg,fresh)
    r=await cmd('project.open',{'path':str(fresh),'onUnsaved':'save'})
    if r.get('jobId'):await s.call_tool('job_wait',{'jobId':r['jobId'],'timeoutMs':60000})
   async def state():return clean(await cmd('doc.object.get',{'ref':'scene:#1','depth':-1}))
   await openbase();p=ids['Master'];camera=(await cmd('anim.camera.current.get'))['cameraId']
   point=(await cmd('anim.path.get',{'nodeId':p}))['points'][1]['id']
   channel={'nodeId':p,'channel':'rotation'}
   actions=[
    ('anim.transform.set',{'nodeIds':[p],'target':'static','rotation':23}),
    ('anim.loop.set',{'nodeId':p,'loops':2}),
    ('anim.path.set',{'nodeId':p,'points':[{'kind':'key','frame':1,'x':0,'y':0,'z':0},{'kind':'key','frame':24,'x':5,'y':2,'z':1}]}),
    ('anim.path.point.add',{'nodeId':p,'kind':'key','frame':7}),
    ('anim.path.point.set',{'pointIds':[point],'x':7,'bias':.25}),
    ('anim.path.point.remove',{'pointIds':[point]}),
    ('anim.path.lockDepth.set',{'nodeId':p,'frame':1,'enabled':True}),
    ('anim.function.set',dict(channel,keys=[{'frame':1,'value':5},{'frame':24,'value':9}])),
    ('anim.function.curveType.set',dict(channel,curveType='spline')),
    ('anim.function.preset.apply',dict(channel,preset='easeInOut',scope='all')),
    ('anim.function.key.add',dict(channel,frame=7,value=33)),
    ('anim.function.key.set',dict(channel,frame=12,value=33)),
    ('anim.function.key.remove',dict(channel,frames=[12])),
    ('anim.function.key.duplicate',dict(channel,frame=12,toFrame=13)),
    ('anim.key.add',{'nodeIds':[p],'frame':7}),
    ('anim.key.move',{'nodeId':p,'fromFrame':12,'toFrame':13}),
    ('anim.key.remove',{'nodeIds':[p],'frame':12}),
    ('anim.segment.set',{'nodeIds':[p],'frame':1,'constant':True}),
    ('anim.segment.uniform.apply',{'nodeId':p,'frame':1,'side':'right'}),
    ('anim.camera.create',{'name':'Independent','makeCurrent':True}),
    ('anim.camera.set',{'cameraId':camera,'fov':72}),
   ]
   for cid,params in actions:
    try:
     await openbase()
     if cid=='anim.segment.uniform.apply':await cmd('anim.function.preset.apply',dict(channel,preset='easeInOut',scope='all'))
     before=await state();app=await cmd('app.state.get');hist=await cmd('edit.history.get')
     obj=(await cmd('doc.object.get',{'ref':p}))['object']
     stale=await invoke(cid,params,expectRev={p:obj['rev']-1});assert stale['status']=='error' and stale['error']['code']=='rev.conflict',stale;assert await state()==before
     failed=m.sc(await s.call_tool('batch',{'label':'Independent rollback','atomic':True,'steps':[{'id':cid,'params':params},{'id':'anim.evaluate','params':{'nodeIds':[p],'frames':[0]}}]}));assert failed['status']=='error',failed;assert await state()==before,'failed second step committed'
     dry=await invoke(cid,params,dryRun=True);assert dry['status']=='ok',dry
     assert await state()==before,'dry-run changed document';assert await cmd('app.state.get')==app;assert await cmd('edit.history.get')==hist
     rev0=(await invoke('doc.object.get',{'ref':'scene:#1'}))['documentRev']
     r=await invoke(cid,params);assert r['status']=='ok',r;assert r['documentRev']==rev0+1,(cid,rev0,r);after=await state()
     await cmd('edit.undo');assert await state()==before,'undo changed persisted content'
     await cmd('edit.redo');assert await state()==after,'redo differs'
     saved=await cmd('project.save')
     if saved.get('jobId'):await s.call_tool('job_wait',{'jobId':saved['jobId'],'timeoutMs':60000})
     opened=await cmd('project.open',{'path':str(copies[-1]),'onUnsaved':'save'})
     if opened.get('jobId'):await s.call_tool('job_wait',{'jobId':opened['jobId'],'timeoutMs':60000})
     assert await state()==after,'save/open differs'
     RESULTS.append({'id':cid,'ok':True,'checks':['dry document/session/history','stale guard','failing second step rollback','one revision','execution','undo','redo','save/open']})
    except Exception as e:RESULTS.append({'id':cid,'ok':False,'error':repr(e)})
   await openbase()
   queries=[('anim.evaluate',request),('anim.projection.get',{'points':[{'x':3,'y':2,'z':0}]}),('anim.path.sample',{'nodeId':p,'from':1,'to':24,'step':.5}),('anim.function.sample',dict(channel,**{'from':1,'to':24,'step':.5})),('anim.path.get',{'nodeId':p}),('anim.function.list',{'nodeId':p}),('anim.function.get',channel),('anim.key.list',{'nodeId':p}),('anim.camera.list',{}),('anim.camera.get',{}),('anim.camera.current.get',{})]
   for cid,params in queries:
    try:
     before=await state();a=await cmd(cid,params);b=await cmd(cid,params);assert a==b;assert await state()==before
     RESULTS.append({'id':cid,'ok':True,'checks':['repeat bytes','read purity']})
    except Exception as e:RESULTS.append({'id':cid,'ok':False,'error':repr(e)})
   # Per 05 section 6.6, element locks affect picking, not commands addressed by ID.
   await openbase();await cmd('doc.object.patch',{'targets':[p],'props':{'locked':True}})
   await cmd('anim.transform.set',{'nodeIds':[p],'target':'static','rotation':17})
   RESULTS.append({'id':'element lock semantics','ok':True,'clause':'05 6.6: explicit ID acts'})
   # Compare all three props owners and RFC operations against the owning commands.
   for props,target,owner,params in [({'transform':{'rotation':17}},p,'anim.transform.set',{'nodeIds':[p],'target':'static','rotation':17}),({'camera':{'fov':65}},camera,'anim.camera.set',{'cameraId':camera,'fov':65})]:
    await openbase();await cmd('doc.object.patch',{'targets':[target],'props':props});patched=await state()
    await openbase();await cmd(owner,params);assert await state()==patched
   for op in ['add','replace','remove','copy','move','test']:
    await openbase();await cmd('anim.transform.set',{'nodeIds':[p],'target':'static','rotation':17,'skew':0})
    operation={'op':op,'path':'/transform/skew'}
    owner={'nodeIds':[p],'target':'static','skew':17}
    if op in ('copy','move'):operation['from']='/transform/rotation'
    elif op in ('add','replace'):operation['value']=17
    elif op=='test':operation['value']=0;owner['skew']=0
    else:owner['skew']=0
    if op=='move':owner['rotation']=0
    if op in ('remove','move'):
     before=await state();refused=await invoke('doc.object.patch',{'targets':[p],'ops':[operation]})
     assert refused['status']=='error' and refused['error']['code']=='doc.schemaViolation',refused
     assert await state()==before
     continue
    if op=='test':
     tested=await invoke('doc.object.patch',{'targets':[p],'ops':[operation]})
     RESULTS.append({'id':'C32-R1-04 RFC numeric equality','ok':tested['status']=='ok','request':dict(operation),'observed':tested})
     operation['value']=0.0
     await cmd('doc.object.patch',{'targets':[p],'ops':[operation]})
    else:await cmd('doc.object.patch',{'targets':[p],'ops':[operation]})
    patched=await state()
    await openbase();await cmd('anim.transform.set',{'nodeIds':[p],'target':'static','rotation':17,'skew':0});await cmd('anim.transform.set',owner)
    assert await state()==patched,op
   RESULTS.append({'id':'AG-02 props and RFC operations','ok':True,'operations':['add','replace','remove','copy','move','test']})
   await openbase()
   samples=(await cmd('anim.path.sample',{'nodeId':p,'from':1,'to':24,'step':.5,'space':'world'}))['samples']
   functions=(await cmd('anim.function.sample',dict(channel,**{'from':1,'to':24,'step':.5})))['samples']
   evaluations=(await cmd('anim.evaluate',{'nodeIds':[p],'frames':[x['frame'] for x in samples]}))['frames']
   for sample,fun,ev in zip(samples,functions,evaluations):
    matrix=ev['nodes'][0]['world']['matrix']
    assert all(abs(sample[a]-matrix[i])<1e-9 for a,i in [('x',3),('y',7),('z',11)])
    assert abs(fun['value']-ev['nodes'][0]['animated']['rotation'])<1e-9
   RESULTS.append({'id':'sample/evaluate consistency','ok':True,'samples':len(samples)})
   await openbase();second=(await cmd('anim.camera.create',{'name':'Second'}))['cameraId']
   for cid,params in [('anim.camera.current.set',{'cameraId':second}),('anim.camera.delete',{'cameraId':second})]:
    try:
     before=await state();await cmd(cid,params,dryRun=True);assert await state()==before
     await cmd(cid,params);after=await state();await cmd('edit.undo');assert await state()==before;await cmd('edit.redo');assert await state()==after
     RESULTS.append({'id':cid,'ok':True,'checks':['dry document','execution','undo','redo']})
    except Exception as e:RESULTS.append({'id':cid,'ok':False,'error':repr(e)})
   async def events(after=0):
    resource=await s.read_resource(f'app://events?after={after}')
    return json.loads(resource.contents[0].text)
   for reverse in [False,True]:
    await openbase();await asyncio.sleep(.1);seq=(await events())['lastSeq']
    steps=[{'id':'anim.transform.set','params':{'nodeIds':[p],'target':'static','rotation':13}},{'id':'anim.transform.set','params':{'nodeIds':[p],'frame':7,'skew':17}}]
    if reverse:steps.reverse()
    result=m.sc(await s.call_tool('batch',{'label':'Mixed static/keyed','atomic':True,'steps':steps}));assert result['status']=='ok',result
    await asyncio.sleep(.1);changed=[x for x in (await events(seq))['events'] if x['type']=='anim.changed']
    RESULTS.append({'id':'C32-R1-05 mixed event frame union','ok':len(changed)==1 and ('frames' not in changed[0]['payload'] or (changed[0]['payload']['frames']['from']<=1 and changed[0]['payload']['frames']['to']>=100000)),'reverse':reverse,'observed':changed})
 (OUT/'results.json').write_text(json.dumps(RESULTS,indent=2))
 print(json.dumps(RESULTS,indent=2));return int(any(not r['ok'] for r in RESULTS))
sys.exit(asyncio.run(main()))
