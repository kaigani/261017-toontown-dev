import pathlib,json,datetime,re
out=pathlib.Path('reviews/evidence/M3-C3.2-r4')
log=out/'workspace.txt'; sidecar=out/'workspace.exit.txt'
data=log.read_text(encoding='utf-8',errors='replace')
record={'failed_result_summaries':len(re.findall(r'test result: FAILED',data)), 'observed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'log_bytes':log.stat().st_size,'native_exit':int(sidecar.read_text()) if sidecar.exists() else None,'result_summaries':data.count('test result:'),'tail':data.splitlines()[-6:]}
with (out/'workspace-polls.jsonl').open('a',encoding='utf-8') as f:f.write(json.dumps(record)+'\n')
print(json.dumps(record,indent=2))
