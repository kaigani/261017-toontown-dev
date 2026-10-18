import json, pathlib, subprocess
root=pathlib.Path(__file__).resolve().parents[3]
rows=[]
for ext in ['ts','tsx','mts','cts','js','jsx','mjs','cjs']:
 command=f'pnpm --dir desktop exec eslint --print-config src/review-config-probe.{ext}'
 p=subprocess.run(['cmd','/c',command],cwd=root,capture_output=True,text=True)
 assert p.returncode==0,p.stderr
 c=json.loads(p.stdout)
 assert c['linterOptions']['noInlineConfig'] is True
 assert c['rules']['compat/compat']==[2]
 rows.append({'command':command,'exit':p.returncode,'linterOptions':c['linterOptions'],'compat/compat':c['rules']['compat/compat']})
pathlib.Path(__file__).with_name('print-configs.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows,indent=2))
