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

issues=[]
for row in rows:
 p=row['schemaParams']; props=p.get('properties',{}); required=p.get('required',[])
 for name,optional,typ in re.findall(r'(?:^|;)\s*(\w+)(\?)?:\s*([^;]+)',row['parameters']):
  schema=props.get(name)
  if schema is None:issues.append(row['id']+': missing '+name);continue
  if (name in required) != (not optional and '=' not in typ):issues.append(row['id']+': optionality '+name)
  enums=re.findall(r'"([^"]+)"',typ.split('=')[0])
  if 'enum' in typ and any(json.dumps(v) not in json.dumps(schema) for v in enums):issues.append(row['id']+': enum '+name)
  primitive=next((j for a,j in [('bool','boolean'),('int','integer'),('number','number'),('string','string'),('id','string')] if re.match(a+r'\b',typ)),None)
  if primitive and primitive not in json.dumps(schema):issues.append(row['id']+': primitive '+name)
  default=re.search(r'=\s*(true|false|"[^"]*"|-?\d+(?:\.\d+)?)',typ)
  if default and schema.get('default') != json.loads(default[1]):issues.append(row['id']+': default '+name)
 # Compare top-level named result fields without confusing nested object/array commas.
 result=row['result']
 if result.startswith('{'):
  depth=0;outer=''
  for c in result[1:-1]:
   if c in '{[':depth+=1
   if depth==0:outer+=c
   if c in '}]':depth-=1
  names=[re.split(r'[:|]',v.strip())[0] for v in outer.split(',')]
  for n in names:
   if n and n not in row['schemaResult'].get('properties',{}):issues.append(row['id']+': result '+n)
print(json.dumps({'differences':issues},indent=2))
raise SystemExit(bool(issues))
