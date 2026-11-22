"""Source-only spec A/declaration audit. Run from repository root; no build."""
import json,re,pathlib,sys
R=pathlib.Path(__file__).resolve().parents[2]
def read(p): return (R/p).read_text(encoding='utf-8')
def ts_object(text,start):
 s=text.split(start,1)[1].split('\n};',1)[0]+'\n}'
 tok=re.compile(r'"(?:\\.|[^"\\])*"|\b\d+n\b|(?<![\w$])[A-Za-z_$][\w$]*(?=\s*:)|,(?=\s*[}\]])')
 return json.loads(tok.sub(lambda m:m[0] if m[0].startswith('"') else m[0][:-1] if m[0].endswith('n') and m[0][0].isdigit() else '' if m[0]==',' else json.dumps(m[0]),s))
def main():
 ts=read('desktop/src/ipc/generated/commands.ts');defs=ts_object(ts,'export const COMMAND_DEFS: Readonly<Record<string, Schema>> = ');idx=ts_object(ts,'export const COMMAND_INDEX = ')
 examples=json.loads(read('crates/registry/src/example-results.json'))
 out=[]; differences=[]
 for n,line in enumerate(read('spec/A_command_registry.md').splitlines(),1):
  m=re.match(r'^\| `((?:view|tool|drawAid)\.[^`]+|inspect.screenshot)` \| (\w+) \| (.*)',line)
  if not m or m[1]=='tool.symmetry.set':continue
  k,kind=m[1],m[2];cells=re.split(r'(?<!\\)\|',m[3]);params,result,undo=[x.strip().replace('\\|','|').strip('`') for x in cells[1:4]]
  p=defs[k+'.params'];r=defs[k+'.result'];diff=[]
  def expand(v):
   if isinstance(v,dict):
    if '$ref' in v and v['$ref'].startswith('#/$defs/'):
     return expand(defs[v['$ref'][8:]])
    return {a:expand(b) for a,b in v.items()}
   return [expand(x) for x in v] if isinstance(v,list) else v
  p=expand(p);r=expand(r)
  if idx[k]['kind']!=kind:diff.append('kind')
  for entry in params.split(';'):
   a=re.match(r'\s*(\w+)(\?)?:\s*(.*)',entry)
   if not a:continue
   name,opt,typ=a.groups();actual=p.get('properties',{}).get(name)
   if actual is None:diff.append(name+': missing');continue
   required=not opt and '=' not in typ
   if required!=(name in p.get('required',[])):diff.append(name+': optionality')
   encoded=json.dumps(actual)
   enums=re.findall(r'"([^"]+)"',typ.split('=')[0])
   if 'enum' in typ and any(json.dumps(v) not in encoded for v in enums):diff.append(name+': enum not declared')
   primitive=next((v for t,v in [('bool','boolean'),('int','integer'),('number','number'),('string','string'),('id','string')] if re.match(t+r'\b',typ)),None)
   if primitive and primitive not in encoded:diff.append(name+': type')
   default=re.search(r'=\s*(true|false|"[^"]*"|-?\d+(?:\.\d+)?)',typ)
   if default:
    wanted=json.loads(default[1])
    if actual.get('default','<absent>')!=wanted:diff.append(name+': default '+repr(wanted)+' != '+repr(actual.get('default','<absent>')))
  if result.startswith('{') and not r.get('properties') and '$ref' not in r:diff.append('result: named shape absent')
  rec={'command':k,'specLine':n,'specParams':params,'specResult':result,'specKind':kind,'specUndo':undo,'schemaParams':p,'schemaResult':r,'differences':diff,'examplePresent':k in json.dumps(examples)}
  out.append(rec);differences.extend(k+': '+x for x in diff)
 print(json.dumps({'commands':len(out),'differences':differences,'rows':out},ensure_ascii=True,indent=2))
 return bool(differences)
if __name__=='__main__':sys.exit(main())
