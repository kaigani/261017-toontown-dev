"""Rebuild the clause inventory from frozen sources; review dispositions are explicit below.

Run from the repository root. This does not edit spec or product files.
"""
from pathlib import Path
from collections import Counter
import json
import re

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
IMPLEMENTED = {
    'draw.drawing.get': ('draw.rs', 'draw_drawing_get'),
    'draw.current.get': ('draw.rs', 'draw_current_get'),
    'draw.pivot.set': ('draw.rs', 'draw_pivot_set'),
    'draw.object.delete': ('draw.rs', 'draw_object_delete'),
    'draw.group.create': ('draw.rs', 'draw_group_create'),
    'draw.group.delete': ('draw.rs', 'draw_group_delete'),
    'draw.stroke.add': ('draw_stroke.rs', 'draw_stroke_add'),
    'draw.stroke.set': ('draw_stroke.rs', 'draw_stroke_set'),
    'draw.content.add': ('draw_stroke.rs', 'draw_content_add'),
    'draw.text.add': ('draw_text.rs', 'draw_text_add'),
    'draw.text.set': ('draw_text.rs', 'draw_text_set'),
    'draw.textFormat.set': ('draw_text.rs', 'draw_text_format_set'),
}
def location(file, token):
    lines = (ROOT/file).read_text(encoding='utf8').splitlines()
    return f'{file}:{next(i for i,s in enumerate(lines,1) if token in s)}'

def command_disposition(cmd):
    if cmd in IMPLEMENTED:
        f, token = IMPLEMENTED[cmd]
        loc = location('crates/executor/src/commands/'+f, 'fn '+token+'(')
        findings = {'draw.stroke.add':'01, 02, 06', 'draw.stroke.set':'03, 04, 08',
                    'draw.text.set':'03, 08', 'draw.textFormat.set':'03, 08'}
        note = ('Divergent C21-R1-'+findings[cmd] if cmd in findings else 'Implemented subset; event/result omissions C21-R1-07/08 apply to edits')
        return ('divergent' if cmd in findings else 'implemented', f'{loc}; c21_draw / c21_example_results; {note}')
    if cmd == 'draw.drawing.changed':
        return 'missing', 'C21-R1-07; only doc.changed emitted by crates/executor/src/run.rs; c21_r1_contracts::content_changes_emit_drawing_event_including_undo_redo'
    if cmd in ('draw.drawing.rename','draw.lock.set'):
        return 'deferred-with-owner','M3 C3.1; delegation to element.drawing.rename / element.drawingLock.set per ADR-0013 section 10'
    if cmd.startswith('tool.') or cmd.startswith('drawAid.') or cmd in ('draw.selection.changed','draw.inspectorPreview.set','draw.viewMapping.get'):
        return 'deferred-with-owner','M4 C4.2 (M7 C7.1 current colour); generated closed SI-tagged declarations in crates/registry/src/declared.json; plan section 4'
    if cmd.startswith('draw.vectorFile.'):
        return 'deferred-with-owner','M10 C10.2; generated declaration / SI-026; plan section 4'
    if cmd.startswith('draw.vectorize.'):
        return 'deferred-with-owner','M2 C2.4; generated declaration / SI-026; plan section 4'
    return 'deferred-with-owner','M2 C2.3; generated declaration / SI-026; plan section 4'

MAPPING = {
 '2.1':('implemented','crates/vector/src/geom.rs:1; drawing.rs::read-back space; c21_draw::read_back_options_flatten_ids_and_include; field guide/camera projection M3 C3.2'),
 '2.2':('deferred-with-owner','M3 C3.2 transforms / M4 C4.2 view mapping and status display'),
 '3.1':('implemented','crates/vector/src/drawing.rs:191 layer_target; c21_draw layers; planar-map subrules C2.2, Draw on Top C2.3/M4 C4.2'),
 '3.2':('divergent','crates/vector/src/stroke.rs:109; draw_stroke.rs:789; C21-R1-04; sides/intersections deferred C2.2'),
 '3.3':('divergent','crates/vector/src/fit.rs:318; C21-R1-01/06; c21_r1_geometry independent max-polyline-distance and bounded dense input'),
 '3.4':('divergent','crates/vector/src/brush.rs:127; C21-R1-02; c21_r1_geometry pressure-peak containment; square-loop hole control passes'),
 '3.5':('deferred-with-owner','M2 C2.2 zones/colour recovery; existing colour IDs and sides retained; c21_draw::content_round_trips_through_read_back_with_new_ids'),
 '3.6':('implemented','crates/executor/src/commands/draw.rs:714; c21_draw::groups_delete_and_pivot; c21_r1_surfaces'),
 '3.7':('divergent','crates/vector/src/text.rs:1; C21-R1-05 generic patch; c21_draw text/font lint; c21_r1_geometry text edge cases; glyph rendering C2.3/M3 C3.3'),
 '3.8':('implemented','crates/executor/src/commands/draw.rs:520; c21_draw::groups_delete_and_pivot'),
 '3.9':('implemented','crates/vector/src/content.rs:548; c21_r1_surfaces bitmap insertion and both forms; transform/arrange C2.3'),
 '3.10':('deferred-with-owner','M2 C2.3 / M3 C3.3 rendering; persisted ordered arrays are available now'),
 '3.11':('deferred-with-owner','M2 C2.2 recovery; colour.unresolved retained and tested in c21_draw'),
 '3.12':('implemented','crates/model/src/document/; executor run.rs; c21_obligations rollback/revisions/inverse/redo; geometry/paragraph divergences recorded separately; noding C2.2'),
 '6.1':('deferred-with-owner','M4 C4.2 pen-panel operations; C2.1 consumes explicit pen/penId in draw_stroke.rs:224; c21_draw / properties'),
 '6.2':('deferred-with-owner','M4 C4.2 setters; crates/vector/src/symmetry.rs:37 maps and tests; command setting currently fixed None/false at draw_stroke.rs:319; coverage risk P2-09'),
 '12.1':('implemented','crates/executor/src/commands/draw*.rs command undo annotations; c21_draw labels and c21_obligations atomic steps; unimplemented tools keep later owners'),
 '12.2':('implemented','crates/executor/src/commands/draw.rs:61, invalid_geometry; c21_draw errors; table rows outside implemented scope retain C2.2/C2.3/M4/M10 owners'),
 '12.3':('deferred-with-owner','M4 C4.2 user tool settings / M7 C7.1 colour; persisted model pen list M1 and c21_r1_surfaces package/folder content'),
 '13.1':('implemented','crates/executor/src/commands/draw*.rs; c21_obligations / c21_replay / c21_r1_surfaces; specific numbered divergences below'),
 '13.2':('deferred-with-owner','M4 C4.2 UI/panels; M2 C2.3 operations, M2 C2.4 vectorize, M10 C10.2 import; draw.textFormat.set available now'),
 '13.3':('deferred-with-owner','M2 C2.3 / M4 C4.2 gesture equivalence, M3 C3.2 mappings; no fake gesture success claimed'),
 '13.4':('implemented','crates/vector/src/readback.schema.json:1 / drawing.rs / content.rs; c21_draw read-back and content round-trip; SI-053/054/055/057 qualifications in report'),
 '13.6':('divergent','C21-R1-04/05 generic validation; C21-R1-07 drawing events; c21_r1_contracts; render/hit queries C2.3'),
 '4.8':('implemented','crates/model/src/entities/drawing.rs:435 and validate.rs; c21_obligations saved/reopened; c21_r1_surfaces both forms; zones C2.2'),
 '4.9':('divergent','crates/model/src/entities/drawing.rs:1; C21-R1-04/05; c21_r1_surfaces types round-trip; brush contours SI-053 and text outline SI-055 qualified'),
 '4.10':('implemented','crates/model/src/validate.rs and document/; retained c11_r1_adversarial::image_element_bank_requires_exactly_one_bitmap_per_drawing; image import M10 C10.2'),
 '4.11':('deferred-with-owner','M2 C2.2 colour recovery/save population; colour.unresolved and opaque extension preservation retained, c21_obligations'),
}

rows=[]
for source, allowed in [('02_drawing.md',{'2','3','6.1','6.2','12','13'}),('10_data_model_project_format.md',{'4.8','4.9','4.10','4.11'})]:
    lines=(ROOT/'spec'/source).read_text(encoding='utf8').splitlines()
    section=''; current=None; fence=False
    for i,line in enumerate(lines,1):
        h=re.match(r'^#{2,4} (\d+(?:\.\d+)*)\b',line)
        if h: section=h.group(1); current=None
        active=any(section==x or section.startswith(x+'.') for x in allowed)
        if not active: continue
        if line.startswith('```'): fence=not fence; current=None; continue
        if fence: continue
        starts=bool(re.match(r'^(?:- |\d+\. |\| )',line))
        if starts and not re.match(r'^\|[- :|]+\|$',line) and not re.match(r'^\| (?:Command|Property|Setting|Code|Field|kind|GUI surface|Tool|Option) \|',line):
            current=[source,i,section,line.strip()]; rows.append(current)
        elif current and line.startswith('  ') and line.strip(): current[3]+=' '+line.strip()
        elif not line.strip(): current=None
    # JSON-shape blocks and structural ASCII tree remain explicit inventory rows.
    if source.startswith('02'):
        for sec,line,label in [('3.1',115,'Drawing node tree, layer/group/text/bitmap, pivot and metadata'),('13.4',1239,'Full draw.drawing.get JSON example, including all object fields'),('13.4',1299,'Hit-test and render JSON shapes')]:
            rows.append([source,line,sec,label])

out=[]
for source,line,sec,requirement in rows:
    status,note=MAPPING.get(sec,('deferred-with-owner','M2 C2.3 / M4 C4.2; plan section 4'))
    cmd=re.match(r'^\| `(draw\.[^`]+)`',requirement)
    if cmd: status,note=command_disposition(cmd.group(1))
    if sec=='13.1':
        n=re.match(r'^(\d+)\.',requirement)
        if n and int(n.group(1)) in (4,11): status,note='divergent','C21-R1-03 addressing / C21-R1-08 result drawingId; c21_r1_contracts'
        elif n and int(n.group(1)) in (1,3,5,7,10,12): status,note='deferred-with-owner','C2.2 paint / C2.3 operations/render / C2.4 vectorize / M3 C3.2 mappings / M4 C4.2 UI; implemented read-back exists'
    if sec=='3.12' and requirement.startswith(('1.','2.')): status,note='deferred-with-owner','M2 C2.2 planar map and C2.3/M3 C3.3 rendering; SI-057 warning'
    if sec=='2.1' and ('Field guide' in requirement or 'Relation to camera' in requirement): status,note='deferred-with-owner','M3 C3.2 camera / M4 C4.2 guide rendering'
    out.append({'source':f'spec/{source}:{line}','section':sec,'requirement':requirement,'status':status,'evidence':note})

for i,line in enumerate((ROOT/'spec/A_command_registry.md').read_text(encoding='utf8').splitlines(),1):
    m=re.match(r'^\| `(draw\.[^`]+)`',line)
    if m:
        status,note=command_disposition(m.group(1))
        out.append({'source':f'spec/A_command_registry.md:{i}','section':'A','requirement':line,'status':status,'evidence':note})
counts=dict(Counter(r['status'] for r in out))
(HERE/'conformance.json').write_text(json.dumps({'counts':counts,'rows':out},ensure_ascii=False,indent=2)+'\n',encoding='utf8')
md=['# C2.1 round 1 conformance register','',
    'Inventory unit: one numbered rule, bullet, or non-header table row in the requested sections; multiline prose is joined. Three additional rows inventory the structural tree and JSON examples. Spec A repeats command rows intentionally: its declarations are a separate requested source. Counts are clause dispositions, not numbers of defects or passing tests.',
    '', 'A mixed-scope row records its divergence when present; otherwise the implemented subset or deferred computation is named. “Implemented” is code/test correspondence, not a claim that every future UI behavior ships in C2.1. Deferred owners must complete the entire rule before the owning milestone closes.',
    '',f'Counts: {counts}; total {len(out)}.','',
    '| # | Requirement / source | Disposition | Implementation, test, finding or owner |','|---|---|---|---|']
for n,r in enumerate(out,1):
    req=r['requirement'].replace('|',' / ').replace('\\ / ',' / ')
    md.append(f"| {n} | {r['source']} section {r['section']}: {req} | {r['status']} | {r['evidence']} |")
(HERE/'conformance.md').write_text('\n'.join(md)+'\n',encoding='utf8')
print(counts,len(out))
