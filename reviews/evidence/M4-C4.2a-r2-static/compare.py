import json,re,pathlib,importlib.util
R=pathlib.Path.cwd()
p=R/'tests/adversarial/c42a-r1s-declarations.py'
s=importlib.util.spec_from_file_location('old',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
t=(R/'desktop/src/ipc/generated/commands.ts').read_text(encoding='utf-8');defs=m.ts_object(t,'export const COMMAND_DEFS: Readonly<Record<string, Schema>> = ')
def expand(x):
 if isinstance(x,dict):
  if '$ref' in x:return expand(defs[x['$ref'].split('/')[-1]])
  return {k:expand(v) for k,v in x.items()}
 if isinstance(x,list):return [expand(v) for v in x]
 return x
rows=[]
for n,line in enumerate((R/'spec/A_command_registry.md').read_text(encoding='utf-8').splitlines(),1):
 cells=re.split(r'(?<!\\)\|',line)
 if len(cells)<8:continue
 name=cells[1].strip().strip('`')
 if not (name.startswith(('tool.','view.','drawAid.')) or name in ('inspect.screenshot','colour.current.set','colour.current.changed')):continue
 cells=[v.strip().strip('`').replace('\\|','|') for v in cells]
 p,r=expand(defs[name+'.params']),expand(defs[name+'.result'])
 fields=re.findall(r'(?:^|;)\s*(\w+)(\?)?:\s*([^;]+)',cells[4])
 result=cells[5]; outer=result.strip('{}').split(':')[0] if False else result
 # Preserve every expanded contract for manual nested-result and enum comparison.
 rows.append({'id':name,'line':n,'parameters':cells[4],'result':result,'kind':cells[2],'undo':cells[6],'schemaParams':p,'schemaResult':r})
(R/'reviews/evidence/M4-C4.2a-r2-static/contracts.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8',newline='\n')
print(len(rows),'expanded contracts')
