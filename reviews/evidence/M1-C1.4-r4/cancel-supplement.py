import importlib.util,os,pathlib,json,time,queue
R=pathlib.Path.cwd(); os.environ['TT_REVIEW_BINARY']=str(R/'target/debug/toontown-headless.exe')
s=importlib.util.spec_from_file_location('r1',R/'tests/adversarial/c14-r1-mcp.py'); m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
for phase in ['before-send','waiting','late-answer','answer-first']:
 c=m.Client(caps={'elicitation':{}})
 try:
  c.project(); target=c.root/'cancel.jsonl'; target.write_bytes(b'original'); rid=9000
  call={'jsonrpc':'2.0','id':rid,'method':'tools/call','params':{'name':'run_command','arguments':{'id':'agent.activity.export','params':{'path':str(target),'onExists':'overwrite'}}}}
  cancel={'jsonrpc':'2.0','method':'notifications/cancelled','params':{'requestId':rid}}
  if phase=='before-send': frames=[call,cancel]
  else:
   c.send(call)
   while True:
    ask=c.q.get(timeout=5)
    if ask.get('method')=='elicitation/create':break
    c.saved.append(ask)
   accept={'jsonrpc':'2.0','id':ask['id'],'result':{'action':'accept','content':{'confirm':True}}}
   frames=[cancel] if phase=='waiting' else ([cancel,accept] if phase=='late-answer' else [accept,cancel])
  c.p.stdin.write(('\n'.join(json.dumps(x) for x in frames)+'\n').encode());c.p.stdin.flush()
  assert 'result' in c.request('ping',{},timeout=5)
  state=c.run('agent.server.status',{}); assert state['status']=='ok',state
  assert c.run('app.state.get',{})['status']=='ok'
  time.sleep(.1)
  received=list(c.saved)
  while True:
   try:received.append(c.q.get_nowait())
   except queue.Empty:break
  answers=[x for x in received if x.get('id')==rid and 'method' not in x]
  changed=target.read_bytes()!=b'original'
  if phase=='answer-first' and answers:
   assert changed and answers[0]['result']['structuredContent']['status']=='ok'
   print(phase,': effect committed and answered before cancellation won')
  else:
   assert not answers and not changed,(phase,received)
   pending=state['result']['pendingConfirmations']
   if phase!='before-send': assert pending and all(x['decision'] is None for x in pending),pending
   print(phase,': no overwrite, no dropped-request response, pending undecided =',len(pending),'; ping and worker answer within 5s')
 finally:c.close()
