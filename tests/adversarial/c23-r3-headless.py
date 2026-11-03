"""Official SDK full C2.3 command-route contract replay; outputs default to a temp directory."""
import argparse,asyncio,hashlib,json,pathlib,shutil,subprocess,tempfile,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2];D='dr_01K742SG0KKMCAP9X1E6Z5P4SW';P='st_01K742SG0R61SYK9GG0HGYVNBT'
def sc(r):return getattr(r,'structured_content',None) or getattr(r,'structuredContent',None)
def normalize(v):
 if isinstance(v,dict):return {k:normalize(x) for k,x in v.items() if k!='rev'}
 if isinstance(v,list):return [normalize(x) for x in v]
 return v
async def exercise(binary,inputs,out,work):
 from mcp import ClientSession,StdioServerParameters,types
 from mcp.client.stdio import stdio_client
 async def elicit(context,params):return types.ElicitResult(action='accept',content={'confirm':True})
 async def roots(context):return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(),name='review')])
 cases=json.loads(inputs.read_bytes());observations=[]
 for case in cases:
  if case['params'].get('elementId')=='scene:#1/element:"Bob"':case['params']['elementId']='el_01K742SG11W3SGB8Q1H50W74VB'
 extras=[{'id':id,'params':p,'prelude':[]} for id,p in [
 ('draw.selection.set',{'drawingId':D,'ids':[P]}),('draw.selection.get',{'drawingId':D}),
 ('draw.pointSelection.set',{'drawingId':D,'points':[{'strokeId':P,'anchor':0}]}),
 ('draw.bounds.get',{'drawingId':D}),('draw.viewMapping.get',{}),('draw.inspectorPreview.set',{'visible':False}),
 ('colour.fillMapping.get',{'drawingId':D,'objectId':P,'toClipboard':True}),('draw.drawing.render',{'drawingId':D,'width':64})]]
 extras[-2]['prelude']=[{'id':'colour.paint.apply','params':{'drawingId':D,'objectIds':[P],'colourId':'col_01K742SG069WMGEFHP887CGF7T'}}]
 # Gradient ID is taken from the original contract corpus rather than guessed.
 mapping_case=next(c for c in cases if c['id']=='colour.fillMapping.set');extras[-2]['prelude']=mapping_case['prelude']
 async def session_args(base,user):return StdioServerParameters(command=str(binary),args=['--mcp-stdio','--project',str(base),'--policy','autonomous','--allow-path',str(work),'--user-dir',str(user)])
 try:
  for index,case in enumerate(cases+extras):
   folder=work/str(index);folder.mkdir();base=folder/'base.animpkg';shutil.copy2(ROOT/'reviews/evidence/M2-C2.2-r3/headless-base.animpkg',base)
   obs={'index':index,'command':case['id'],'calls':[]};observations.append(obs);print(index,case['id'],flush=True)
   async with stdio_client(await session_args(base,folder/'user')) as (read,write):
    async with ClientSession(read,write,elicitation_callback=elicit,list_roots_callback=roots) as s:
     await s.initialize()
     if index==0:
      desc=sc(await s.call_tool('describe_command',{'id':'draw.selection.changed'}));obs['selectionEventDescription']=desc
      ev=sc(await s.call_tool('run_command',{'id':'draw.selection.changed','params':{},'dryRun':True}));obs['selectionEventInvocation']=ev;assert ev['status']=='error',ev
     async def run(id,p,**options):
      e=sc(await s.call_tool('run_command',{'id':id,'params':p,**options}));obs['calls'].append({'id':id,'params':p,'options':options,'envelope':e});assert e['status']=='ok',e;return e
     async def drawing():return (await run('draw.drawing.get',{'drawingId':D}))['result']
     async def save(path):
      e=await run('project.saveAs',{'path':str(path),'onExists':'overwrite'});job=e['result'].get('jobId')
      if job:
       done=await run('job.wait',{'jobId':job,'timeoutMs':60000});assert done['result']['state']=='succeeded',done
     for pre in case['prelude']:await run(pre['id'],pre['params'])
     before=await drawing();rev=(await run('doc.state.get',{}))['documentRev']
     await run(case['id'],case['params'],dryRun=True);assert await drawing()==before
     e=await run(case['id'],case['params']);after=await drawing();changed=e['documentRev']!=rev
     obs['changedDocument']=changed
     if changed:
      assert e['documentRev']==rev+1,(case,e)
      for attempt in range(20):
       resource=await s.read_resource('app://events');events=json.loads(resource.contents[0].text)['events']
       own=[x for x in events if x['type']=='draw.drawing.changed' and x['payload'].get('drawingId')==D and x['payload'].get('rev')==e['documentRev']]
       if own:break
       await asyncio.sleep(0.025)
      obs['revisionEvents']=own;assert len(own)==1,(case,events)
      await run('edit.undo',{});assert normalize(await drawing())==normalize(before)
      await run('edit.redo',{});assert normalize(await drawing())==normalize(after)
      after=await drawing()
     saved=folder/'after.animpkg';await save(saved)
     obs['preSaveReadback']=after;after=await drawing();obs['postSaveReadback']=after
     # Export committed commands with their transaction boundaries and deterministic seeds for --run.
     replay=folder/'replay.json';await run('agent.activity.export',{'path':str(replay),'format':'batch'})
     exported=json.loads(replay.read_bytes());obs['replayBatch']=exported
     # Explicit query validates the replay's geometry and mappings, independent of exported summaries.
     exported['steps'].append({'id':'draw.drawing.get','params':{'drawingId':D}})
     replay.write_text(json.dumps(exported),encoding='utf-8')
     rr=subprocess.run([str(binary),'--run',str(replay),'--project',str(base),'--policy','autonomous','--allow-path',str(work),'--user-dir',str(folder/'replay-user')],capture_output=True,timeout=120)
     obs['replayExit']=rr.returncode;obs['replayStdout']=rr.stdout.decode(errors='replace');obs['replayStderr']=rr.stderr.decode(errors='replace');assert rr.returncode==0,obs
     re=json.loads(rr.stdout);assert normalize(re['steps'][-1]['result'])==normalize(after),(case,re)
   async with stdio_client(await session_args(saved,folder/'open-user')) as (read,write):
    async with ClientSession(read,write,elicitation_callback=elicit,list_roots_callback=roots) as s:
     await s.initialize();opened=sc(await s.call_tool('run_command',{'id':'draw.drawing.get','params':{'drawingId':D}}));obs['savedOpenReadback']=opened;obs['expectedReadback']=after;assert opened['status']=='ok';assert opened['result']==after
     obs['savedOpenIdentical']=True
 finally:
  out.mkdir(parents=True,exist_ok=True);(out/'sdk-contracts.json').write_text(json.dumps({'binary':str(binary),'observations':observations},indent=2),encoding='utf-8')
class Format(unittest.TestCase):
 def test_normalization_removes_only_revisions(self):self.assertEqual(normalize({'rev':2,'nodes':[{'id':'a','rev':3}]}),{'nodes':[{'id':'a'}]})
if __name__=='__main__':
 import sys
 if '--binary' not in sys.argv:unittest.main(verbosity=2)
 else:
  p=argparse.ArgumentParser();p.add_argument('--binary',type=pathlib.Path,required=True);p.add_argument('--inputs',type=pathlib.Path,required=True);p.add_argument('--out',type=pathlib.Path);a=p.parse_args()
  with tempfile.TemporaryDirectory(prefix='tt-c23-r3-sdk-') as tmp:
   work=pathlib.Path(tmp).resolve();asyncio.run(exercise(a.binary.resolve(),a.inputs.resolve(),a.out.resolve() if a.out else work/'receipts',work))
