"""Line-addressed review register; run at repository root. Classification is reviewer-authored, extraction only mechanical."""
import pathlib,re,json,collections
R=pathlib.Path('.')
O=R/'reviews/evidence/M2-C2.2-r1'
C={c['id']:c for c in json.loads((O/'command-contracts.json').read_text(encoding='utf-8'))}
rows=[]
def selected(file,sec):
 if file.startswith('02'):return sec in ['3.5','3.11','3.12','8.8','8.9','13.4','13.5','13.6']
 if file.startswith('03'):return sec.split('.')[0] in ['1','7','8','9','10','11','15','16']
 return sec in ['4.8','4.9','4.10','4.11','7.2']
def classify(file,sec,text):
 # Readiness and command defects override general family evidence.
 if ('loading' in text.lower() and ('recover' in text.lower() or 'colour' in text.lower())) or ('fillInconsistent' in text) or ('on load' in text and 'disagreement' in text):return 'divergent','C22-R1-05; crates/persist/src/load.rs; c22_r1_commands::loading_*'
 if ('recovery copy' in text.lower() or 'recovery' in text.lower()) and (sec.startswith('1.6') or sec=='4.11'):return 'divergent','C22-R1-05/06; colour_recovery.rs:109; c22_r1_commands save/load'
 if 'IDs' in text and ('boundary' in text or 'geometry' in text):return 'divergent','C22-R1-03; graph.rs:901; c22_r1_geometry::changing_a_boundarys_geometry_changes_its_zone_id'
 if ('carry over' in text or 'inherit' in text or 'merged' in text) and (sec in ['3.12','7.2','4.9']):return 'divergent','C22-R1-01/03/09; node.rs:247, fill.rs:239; c22_r1_geometry and c22_r1_commands'
 if 'matrix' in text.lower() and file.startswith('03') and sec in ['1.6','7.2']:return 'divergent','C22-R1-09; colour.rs:474; c22_r1_commands pencil/solid mapping tests'
 if file.startswith('02'):
  if sec=='3.11':return 'divergent','C22-R1-05: paste implemented; doc.import.json lacks recovery; bundle.rs import; c22_r1_commands json_import'
  if sec=='3.12' and text.startswith('1.'):return 'divergent','C22-R1-01/02; node.rs:247, fill.rs:239; c22_r1_geometry'
  if sec=='3.12':return 'implemented','cx.rs:687; run.rs drawing events; c22_planar paint undo/revision tests'
  if sec=='8.8':return 'divergent','C22-R1-07/08; colour.rs:961/1743; c22_r1_commands and c22_r1_surface'
  if sec=='8.9':return 'deferred-with-owner','M2 C2.3 operations and M4 C4.2 gestures; plan 4'
  if sec=='13.5' and not any(w in text for w in ['draw.hitTest.get','draw.drawing.get','draw.stroke.add']):return 'deferred-with-owner','Non-zone draw commands: M2 C2.3/C2.4 or M4 C4.2 per registry'
  return 'implemented','graph.rs:628/887; planar.rs:236; drawing.rs readback_schema; c22_planar, c22_settle, c22_determinism'
 if file.startswith('10'):
  if sec=='7.2':return 'divergent','Only fill/colour rows in scope: C22-R1-05; persist/load.rs; c22_r1_commands loading tests'
  return 'implemented','model/entities/drawing.rs; planar/layer.rs:110/184; c22_settle, c22_recovery, c21_r1_surfaces'
 # Colour model and postponed UI/rendering are distinguished from current paint behaviour.
 if sec.startswith('1.'):
  if sec=='1.8' or sec=='1.9':return 'deferred-with-owner','M7 C7.1 default palette/names (new project currently creates palette model); model/presets.rs'
  if any(w in text.lower() for w in ['render','recolour','selector','gui','progress','delete','creating a style','duplicating','export','dropper']):return 'deferred-with-owner','M7 C7.1 palette operations/UI; M2 C2.3/M3 C3.3 rendering'
  return 'implemented','model/entities/palette.rs and drawing.rs; validate.rs; c22_recovery, model schema/roundtrip tests'
 if sec in ['7.1','7.3','7.4','7.5'] or sec=='10':return 'deferred-with-owner','M2 C2.3/M3 C3.3 renderer; M6 C6.3 bones; M7 C7.1 swatch editor (model values preserved now)'
 if sec=='7.2':return 'divergent','C22-R1-09 and tile/native-image-size not implemented by default_matrix; colour.rs:157/474; c22_r1_commands'
 if sec=='8.1' and any(w in text for w in ['Tools in','Views','modifier','Feedback']):return 'deferred-with-owner','M4 C4.2 interactive tools/views; plan 4'
 if sec=='8.2' and ('drag' in text.lower() or 'cross' in text.lower()):return 'divergent','C22-R1-04/08/09; colour.rs:474/638/960; c22_r1_commands'
 if sec=='8.5':return 'deferred-with-owner','M7 C7.1 current colour/UI; pick read-back C2.2, rendered pixel M2 C2.3/M3 C3.3 (SI-065)'
 if sec=='8.7':return 'deferred-with-owner','M2 C2.3 per handoff (registry incorrectly M7 C7.1): C22-R1-10; M4 C4.2 manipulators'
 if sec=='8.8' and 'persisted user setting' in text:return 'divergent','C22-R1-07; colour.rs:1743; c22_r1_surface scope regression'
 if sec.startswith('8.'):return 'implemented','colour.rs:891/1140/1250/1650, planar/gap.rs:101; c22_planar, c22_replay (exceptions in report)'
 if sec.startswith('9.'):
  return 'deferred-with-owner','M2 C2.3 selection/recolour per handoff, registry M7 C7.1 (C22-R1-10); M7 palette editor; M7 C7.2 effects'
 if sec.startswith('11.'):
  if sec=='11.2':return 'divergent','C22-R1-05; paste/on-demand implemented colour_recovery.rs:245; c22_recovery; load tests fail'
  if sec=='11.1':return 'implemented','draw.rs content_add; colour_recovery.rs:245/397; c22_recovery, c22_planar content roundtrip'
  return 'deferred-with-owner','M9 C9.1 templates; M10 C10.2 vector import; M2 C2.4 vectorization; SWF excluded by plan 6'
 if sec=='15.1' and any(w in text for w in ['nameEmpty','nameExists','lastPalette','lastStyle','paletteHidden','invalidValue','inUse','needsConfirmation','imageMissing']):return 'deferred-with-owner','M7 C7.1 palette/style/editor errors; strict generated declarations'
 if sec=='15.2':return 'deferred-with-owner','Paint/Unpaint/Paint Unpainted/Close Gap/Colour Recovery labels implemented and verified in command-contracts.json; remaining listed actions M2 C2.3 or M7 C7.1'
 if sec.startswith('15.'):
  return 'implemented','colour.rs error/refusal paths, command undo labels; colour_recovery.rs; c22_planar, c22_recovery; unused palette codes M7 C7.1'
 if sec.startswith('16.'):
  if any(w in text for w in ['scene`','view`','rendered','composit']):return 'deferred-with-owner','SI-065: M2 C2.3/M3 C3.2/C3.3/M4 C4.2; colour.rs:113,1444'
  if any(w in text for w in ['palette.get','swatch.get','usage.list','current.set','palette view']):return 'deferred-with-owner','M7 C7.1; registry declarations have strict schemas and owners; command-contracts.json'
  if text.startswith('4.'):return 'divergent','C22-R1-04/07/08/09/10; remaining surfaces implemented; c22_r1_surface/c22_r1_commands'
  return 'implemented','colour.rs, colour_recovery.rs, run.rs; c22_replay; c22_r1_surface atomic/sequential/typed/script tests'
 return 'deferred-with-owner','M7 C7.1; plan 4'
files=['02_drawing.md','03_colour.md','10_data_model_project_format.md']
for file in files:
 lines=(R/'spec'/file).read_text(encoding='utf-8').splitlines();sec='';idx=0
 while idx<len(lines):
  line=lines[idx];m=re.match(r'^#{2,4} (\d+(?:\.\d+)*)\b',line)
  if m:sec=m[1]
  if not selected(file,sec):idx+=1;continue
  # Every numbered/bulleted requirement and every non-header table row, retaining wrapped prose.
  isrow=line.startswith('|') and not re.match(r'^\|[ :|-]+\|$',line)
  isrule=bool(re.match(r'^\s*(?:[-*]|\d+\.) ',line))
  if isrow or isrule:
   if isrow and any(line.startswith('| '+h) for h in ['Rule |','Property |','Field |','Kind |','Identity |','Object |','Situation |','Mode |','Target |','Command ID |','Command |','Space |','Code |','Problem |','Condition |','Fill |','Method |']):idx+=1;continue
   if file.startswith('10') and sec=='7.2' and not any(w in line for w in ['zone fills','colour ID','fillInconsistent','colour.recovered']):idx+=1;continue
   if file.startswith('02') and sec=='13.5' and not any(w in line for w in ['zone','draw.hitTest']):idx+=1;continue
   text=line.strip();j=idx+1
   if isrule:
    while j<len(lines) and lines[j].strip() and not re.match(r'^\s*(?:[-*]|\d+\.) |^#|^\|',lines[j]):text+=' '+lines[j].strip();j+=1
   status,evidence=classify(file,sec,text)
   cmd=re.match(r'^\| `(colour\.[^`]+|draw.hitTest.get)`',text)
   if cmd and cmd[1] in C:
    c=C[cmd[1]];status='implemented' if c['status']=='implemented' else 'deferred-with-owner';evidence=f"command-contracts.json → {cmd[1]}; "+(c.get('milestone') or 'colour.rs / colour_recovery.rs, c22_planar/c22_recovery')
    if cmd[1] in ['colour.paint.apply','colour.paintUnpainted.apply','colour.paint.remove','colour.gapClosing.set']:status='divergent';evidence+='; C22-R1-04/07/08/09 as applicable'
   rows.append((file,sec,idx+1,text,status,evidence));idx=j
  else:idx+=1
for file in files:
 lines=(R/'spec'/file).read_text(encoding='utf-8').splitlines();sec='';fenced=False;i=0
 while i<len(lines):
  line=lines[i];m=re.match(r'^#{2,4} (\d+(?:\.\d+)*)\b',line)
  if m:sec=m[1]
  if line.startswith('```'):fenced=not fenced;i+=1;continue
  if not selected(file,sec) or fenced or not line.strip() or re.match(r'^#|^\||^---|^\s*(?:[-*]|\d+\.) ',line):i+=1;continue
  # A continuation directly following a list/table is already represented above.
  if i>0 and lines[i-1].strip() and not lines[i-1].startswith('#'):i+=1;continue
  start=i;text=line.strip();i+=1
  while i<len(lines) and lines[i].strip() and not re.match(r'^#|^\||^```|^\s*(?:[-*]|\d+\.) ',lines[i]):text+=' '+lines[i].strip();i+=1
  if file.startswith('10') and sec=='7.2' and not any(w in text.lower() for w in ['fill','colour']):continue
  status,evidence=classify(file,sec,text)
  rows.append((file,sec,start+1,text,status,evidence))
rows.sort(key=lambda r:(r[0],r[2]))

# Model/code shapes are not bullet tables: retain each zone/side/recovery JSON field as a requirement row.
for file,sec,fields in [('02_drawing.md','13.4',['leftZoneId','rightZoneId','textureMatrix','boundaryStrokeIds','holes','parentZoneId','area','interiorPoint']),('10_data_model_project_format.md','4.11',['colourId','paletteName','styleName','swatch'])]:
 text=(R/'spec'/file).read_text(encoding='utf-8').splitlines();start=next(i for i,l in enumerate(text) if l.startswith('### '+sec+' '));end=next((i for i in range(start+1,len(text)) if text[i].startswith('### ')),len(text))
 for field in fields:
  n=next((i for i in range(start,end) if field in text[i]),start)
  rows.append((file,sec,n+1,field,'implemented' if sec=='13.4' else 'divergent','planar.rs:236/264; c22_planar/readback schema' if sec=='13.4' else 'colour_recovery.rs:109; C22-R1-05/06; save/load regression tests'))
counts=collections.Counter(r[4] for r in rows)
out=['# M2 C2.2 round 1 conformance register','', 'Target 648e22d; base 812554b. Each row is a numbered/bulleted rule, table row, or explicit zone/side JSON field. Mixed clauses are divergent when any current-checkpoint obligation fails. Deferred means a named real checkpoint owns the later behaviour; it is not a pass claim for that behaviour. Model-only implementation is distinguished from future rendering/UI. Cross-cutting exceptions in the report apply to every affected family.', '', '**Rule register counts:** '+', '.join(f'{k} {v}' for k,v in sorted(counts.items()))+f'; total {len(rows)}.', '', '| Requirement | Text / field | Disposition | Implementation / test / owner |','|---|---|---|---|']
for f,s,n,t,st,e in rows:out.append(f'| {f} §{s}, L{n} | {t.replace(chr(124)," / ")} | {st} | {e} |')
out+=['','## Spec A: every colour command and draw.hitTest.get','', 'Exact parameter/result schemas, enablement, scopes, undo labels, retained examples and placeholder owners are captured from the live assembled registry in [command-contracts.json](command-contracts.json). The following table compares the command-level dimensions; absence of runtime enablement metadata is noted even where handlers enforce checks.', '', '| Command | Status / owner | Params and result | Undo label | Scopes | Enablement / errors / examples |','|---|---|---|---|---|---|']
for id,c in sorted(C.items()):
 status=c['status'];owner=c.get('milestone','M2 C2.2');dim='Schema + retained executed example; SI-061 extensions where applicable' if status=='implemented' else 'Strict generated spec A schema; runtime placeholder'
 if id=='colour.pick':dim+='; SI-065 RGBA/space restriction'
 if id in ['colour.fillMapping.get','colour.fillMapping.set','colour.selection.apply','colour.recolour.apply']:owner+='; handoff says M2 C2.3 → C22-R1-10'
 notes='handler checks project/targets/editability; metadata says always enabled' if status=='implemented' else c['enablement']
 if id=='colour.gapClosing.set':notes+='; app prevents every MCP agent (C22-R1-07)'
 if status=='implemented':notes+='; errors c22_planar/c22_recovery; examples retained '+str(len(c['examples']))
 out.append(f"| `{id}` | {status}; {owner} | {dim} | {c['undoLabel'] or 'n/a'} | {', '.join(c['scopes'])} | {notes} |")
(O/'conformance.md').write_text('\n'.join(out)+'\n',encoding='utf-8')
(O/'register-counts.json').write_text(json.dumps({'rules':dict(counts),'totalRules':len(rows),'commands':len(C),'implementedCommands':sum(c['status']=='implemented' for c in C.values()),'placeholderCommands':sum(c['status']!='implemented' for c in C.values())},indent=2)+'\n',encoding='utf-8')
print((O/'register-counts.json').read_text())
