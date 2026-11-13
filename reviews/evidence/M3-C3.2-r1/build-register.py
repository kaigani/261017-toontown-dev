"""Build a clause/paragraph register with explicit evidence links and adjudication notes."""
import json,re,collections,importlib.util
from pathlib import Path
R=Path(__file__).resolve().parents[3]; E=Path(__file__).parent
spec=importlib.util.spec_from_file_location('gen',R/'scripts/gen-command-declarations.py'); gen=importlib.util.module_from_spec(spec);spec.loader.exec_module(gen)
matrix=json.loads((E/'sdk-matrix/results.json').read_text())
commands={r['id']:r for r in matrix if r['id'].startswith('anim.')}
assert len(commands)==34 and all(x['ok'] for x in commands.values())
registry=[]
for cid in sorted(commands):
 issue='C32-R1-01: first passage frame; reproduce c32_r1_passage/c32_r1_corpus.' if cid=='anim.path.get' else 'C32-R1-03: exact default camera width; reproduce c32_r1_default_frame.' if cid in ('anim.camera.create','anim.camera.get','anim.projection.get','anim.evaluate') else None
 status='divergent' if issue else 'implemented'
 registry.append(dict(id=cid,status=status,evidence='sdk-matrix/results.json; '+('independent-maths.txt' if issue and '03' in issue else 'passage.txt' if issue else 'executor.txt'),note='Release SDK: '+', '.join(commands[cid]['checks'])+('. '+issue+(' C32-R1-06 also affects this row; see finite-decomposition/result.json.' if cid=='anim.evaluate' else '') if issue else '')))
for event in ['anim.changed','anim.camera.currentChanged']:
 registry.append(dict(id=event,status='divergent' if event=='anim.changed' else 'implemented',evidence='sdk-matrix/results.json; independent-queries.txt; executor.txt' if event=='anim.changed' else 'executor.txt: c32_semantics camera current-event assertions',note='C32-R1-05: mixed static/keyed range union fails; sdk-matrix.py reproduces both orders.' if event=='anim.changed' else 'Executed current-camera event coverage; coalescing is scoped to one apply_effects call.'))
units=[]
for filename in ['05_scene_planning.md','10_data_model_project_format.md']:
 text=(R/'spec'/filename).read_text(encoding='utf-8');lines=text.splitlines();section='';top=0;buffer=[];start=0;serial=collections.Counter()
 def emit():
  global buffer
  if not buffer:return
  content=' '.join(buffer);buffer=[]
  if set(content)<=set('| -:'):return
  if content.startswith('|') and start<len(lines) and set(lines[start].strip())<=set('| -:'):return
  serial[section]+=1
  status='implemented';owner=None;note='Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness.'
  evidence=['sdk-matrix/results.json','independent-maths.txt','independent-queries.txt','executor.txt']
  if filename.startswith('05'):
   if section.startswith(('2.8','12.4')) or ('native size' in content.lower() and 'bitmap' in content.lower()):status='deferred-with-owner';owner='M3 C3.3';note='Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance.'
   if section.startswith('12.4'):owner='M6 C6.1 (views), M3 C3.3 (camera rendering)'
   if 'templateEditMode' in content:status='deferred-with-owner';owner='M9';note='Template editing is not implemented in this checkpoint.'
   if re.search(r'\bbone[sA-Z.]?\b',content,re.I) and ('bone.' in content or section=='1.4'):status='deferred-with-owner';owner='M6 C6.3';note='SI-097: skeleton transform only; bone kinematics excluded.'
   if section=='2.7' and 'exactly' in content or section=='1.2' and 'exactly' in content and '12 fields' in content:
    status='divergent';owner=None;note='C32-R1-03: width 12.000003045341391, versus exact 12; reproduce cargo test -p tt-anim --test c32_r1_default_frame --locked.';evidence=['independent-maths.txt']
   if section=='10.5' and ('smallest' in content or 'non-monotonic' in content):
    status='divergent';owner=None;note='C32-R1-01: first subframe crossing missed; cargo test -p tt-anim --test c32_r1_passage --test c32_r1_corpus --locked --no-fail-fast.';evidence=['passage.txt','passage-corpus.txt']
   if section=='19.3':
    ids=re.findall(r'`(anim\.[\w.]+)`',content)
    if ids and ids[0] not in commands:
     status='deferred-with-owner';owner=gen.owner_of(ids[0]);note='Outside the 34-command C3.2 allocation; registry owner retained.'
   if section=='19.4' and ('render_frame' in content or 'describe_frame' in content):status='deferred-with-owner';owner='M3 C3.3';note='Render/description contract allocated downstream.'
  if filename.startswith('05') and section=='2.9':
   if content.startswith('3.'):
    status='deferred-with-owner';owner='M3 C3.3';note='Render-list exposure/hidden filtering; numeric hidden-node queries follow SI-100.'
   elif content.startswith('4.'):
    status='deferred-with-owner';owner='M6 C6.3';note='Bone kinematics and skin deformation deferred under SI-097.'
   elif content.startswith('5.'):
    status='deferred-with-owner';owner='M3 C3.3 / M7';note='Projection is numeric C3.2; effect execution and compositing retain downstream owners.'
   elif content.startswith('6.'):
    status='deferred-with-owner';owner='M6 C6.1';note='Scene-view overlays are future UI, not numeric evaluation.'
  if filename.startswith('05') and section=='12.2' and 'MUST be reachable' in content:
   status='deferred-with-owner';owner='M6 C6.2';note='Camera list/current commands execute; scene-view reachability is the allocated future UI.'
  if filename.startswith('05') and section=='2.5':
   status='divergent';owner=None;note='C32-R1-06: 26 legal 1e6-scale ancestors produce finite matrix entries but internal result-encoding failure. Reproduce finite-decomposition.py against the release binary.';evidence=['finite-decomposition/result.json']
  if filename.startswith('05') and section=='18':
   future={'anim.noAttachTarget':'M6 C6.1','anim.hierarchyCycle':'M6 C6.1','anim.pasteIncompatible':'M6 C6.2','anim.busyDeforming':'M6 C6.3','anim.pegOnlyNoPeg':'M6 C6.1','anim.samplesInvalid':'M6 C6.2'}
   for code,allocated in future.items():
    if code in content:status='deferred-with-owner';owner=allocated;note='Error belongs to the named future operation, not one of the C3.2 commands.'
  if filename.startswith('05') and section=='1.3' and 'exactly' in content and '12-field' in content:
   status='divergent';owner=None;note='C32-R1-03: exact drawing-guide alignment inherits rounded default-FOV discrepancy.';evidence=['independent-maths.txt']
  if filename.startswith('05') and section=='19.4' and 'union of affected' in content:
   status='divergent';owner=None;note='C32-R1-05: mixed static/keyed batch narrows all-frame invalidation to frame 7. Reproduce sdk-matrix.py against the release binary.';evidence=['sdk-matrix/results.json']
  if filename.startswith('05') and section=='18' and 'anim.valueOutOfRange' in content:
   status='divergent';owner=None;note='C32-R1-06: overflowing decomposition reaches internal encoding error instead of typed anim.valueOutOfRange; reproduce finite-decomposition.py.';evidence=['finite-decomposition/result.json']
  units.append(dict(id=f'{filename}:{section}:{serial[section]}',clause=section,line=start,text=content,status=status,owner=owner,note=note,evidence=evidence))
 for n,line in enumerate(lines,1):
  h=re.match(r'^(#{2,3}) (\d+(?:\.\d+)?)[. ]',line)
  if h:
   emit();section=h.group(2);top=int(section.split('.')[0]);continue
  selected=(top in [1,2,8,9,10,12,18,19]) if filename.startswith('05') else section in ['4.6','4.12']
  if not selected:continue
  if line.startswith('#') or line.strip()=='---':emit();continue
  if not line.strip():emit();continue
  if line.startswith('|'):
   emit();start=n;buffer=[line.strip()];emit()
  elif re.match(r'^[-*] |^\d+\. ',line):
   emit();start=n;buffer=[line.strip()]
  else:
   if not buffer:start=n
   buffer.append(line.strip())
 emit()
units.append(dict(id='10_data_model_project_format.md:AG-02:extra',clause='AG-02 / RFC 6902 4.6',line=None,text='RFC numeric equality on newly delegated transform properties',status='divergent',owner=None,note='C32-R1-04: integer 0 fails test against stored floating zero; floating 0.0 passes. Reproduce sdk-matrix.py.',evidence=['sdk-matrix/results.json']))
counts=lambda rows:dict(collections.Counter(x['status'] for x in rows))
result=dict(reviewed_code_head='142226d8da9139c685f576a079c78865022dc70b',registry_counts=counts(registry),source_unit_counts=counts(units),registry=registry,source_units=units,findings=['C32-R1-01','C32-R1-03','C32-R1-04','C32-R1-05','C32-R1-06'],withdrawn_findings={'C32-R1-02':'charter glob error; ruling'},coverage_note='Implementation mapping and recorded probe results, not an assertion of exhaustive input-space coverage. See report limitations.')
(E/'register.json').write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
md=['# C3.2 round-1 register','',f'Registry: {counts(registry)}. Source units: {counts(units)}.','', '| Row | Adjudication | Evidence |','|---|---|---|']
md += [f"| {x['id']} | {x['status']} | {x['evidence']} |" for x in registry]
md+=['','## Source requirement units','']
for x in units:md += [f"- **{x['id']}** (line {x['line']}): **{x['status']}**"+(f"; owner {x['owner']}" if x['owner'] else '')+f". {x['note']} Source: {x['text'].replace('|',' / ')}"]
(E/'register.md').write_text('\n'.join(line.rstrip() for line in md)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'registry':counts(registry),'source':counts(units),'units':len(units)}))
