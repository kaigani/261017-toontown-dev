"""History-pinned C3.1 requirement register. Default validates extraction without writing.

--output DIRECTORY emits M3-C3.1-r1-register.json and .md. Assessments are the reviewer's rules below;
source paragraphs, list items, table rows and example blocks remain separately addressable.
"""
import collections
import json
import pathlib
import re
import subprocess
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
HEAD = '8f44969'


def source(path, sha=HEAD):
    return subprocess.check_output(['git','show',f'{sha}:{path}'],cwd=ROOT).decode('utf-8')


def units(path, wanted):
    section = ''; pending=[]; start=0; code=False
    out=[]
    def flush():
        nonlocal pending
        if pending and section in wanted:
            text=' '.join(s.strip() for s in pending)
            if not text.startswith('|---') and text!='---':
                out.append({'source':path,'clause':section,'line':start,'text':text})
        pending=[]
    for n,line in enumerate(source(path).splitlines(),1):
        heading=re.match(r'^#{2,3} (\d+(?:\.\d+)?)\.?\s',line)
        if heading:
            flush(); section=heading[1]; continue
        if line.startswith('```'):
            if code:
                pending.append(line); code=False; flush()
            else:
                flush(); start=n; pending=[line]; code=True
            continue
        if code:
            pending.append(line); continue
        if not line.strip():
            flush(); continue
        if line.startswith('|') or re.match(r'^(?:- |\d+\. )',line):
            flush(); start=n; pending=[line]
            if line.startswith('|'): flush()
        else:
            if not pending: start=n
            pending.append(line)
    flush()
    # Header rows describe columns, not requirements.
    return [u for u in out if not (u['text'].startswith('|') and any(u['text'].startswith('| '+h+' |') for h in ['Term','State','Topic','Type','Parameter','Field','Command','Command ID','Code','Case','Item','How','Operation','Command / gesture','Toggle / command']))]


REPRO = {
 '01':'cargo test -p tt-anim --locked -j 4 --test c31_r1_oracle columnate_total_frames_short_last_reaches_requested_length',
 '02':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract timeline_colour',
 '03':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract delegated_; cargo test -p tt-executor --locked -j 4 --test c31_r1_contract patch_unlock_ops_matches_command',
 '04':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract clipboard_',
 '05':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract copied_cells_keep_notes; cargo test -p tt-executor --locked -j 4 --test c31_r1_contract inserted_cells_shift_existing_notes',
 '06':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract duplicate_clipping_preserves_required_parameter_row',
 '07':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract multi_column_overlap_copies_original_source_values',
 '08':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract xsheet_missing_name_on_two_clones_is_created_once',
 '09':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract fill_target_above_spec_maximum_refuses',
 '10':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract extreme_timing_parameters_do_not_stop_document_worker',
 '11':'cargo test -p tt-anim --locked -j 4 --test c31_r1_oracle insert_remove_4096',
 '12':'python tests/adversarial/c31-r1-audit.py --collect <new-directory-containing-downloaded-ci-inputs>',
 '13':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract generic_paste_bridge_overwrites_opaque_drawing_properties',
 '14':'cargo test -p tt-executor --locked -j 4 --test c31_r1_selection_budget',
 '15':'cargo test -p tt-executor --locked -j 4 --test c31_r1_contract cycle_selection_applies_to_every_selected_column',
}

COMMAND_FINDINGS = {
 'cell.columnate.apply':['01'], 'element.solo.set':['02'], 'element.typeColour.set':['02'],
 'element.visibility.set':['03'], 'element.create':['03'],
 'cell.clipboard.set':['04','05'], 'cell.clipboard.apply':['04','05','13'],
 'cell.range.move':['05','07'], 'cell.blank.add':['05'], 'cell.blank.remove':['05'],
 'cell.exposure.add':['05'], 'cell.exposure.remove':['05'], 'cell.exposure.set':['05'],
 'cell.range.add':['05'], 'cell.range.delete':['05'], 'cell.cycle.create':['05'],
 'element.duplicate':['06'], 'xsheet.exposures.set':['08'], 'cell.fill.apply':['09'],
 'element.timing.set':['10'], 'element.state.changed':['03'], 'element.drawing.changed':['03'],
 'cell.selection.changed':['03'], 'cell.select':['14'],
}
COMMAND_FINDINGS['cell.cycle.create'] += ['15']
REPRO['03'] += '; cargo test -p tt-executor --locked -j 4 --test c31_r1_contract automatic_position_changes_emit_selection_event'
COMMAND_FINDINGS['element.clone']=['08']
COMMAND_FINDINGS['element.duplicate']+=['08']
REPRO['08']+='; cargo test -p tt-executor --locked -j 4 --test c31_r1_contract default_copy_name_works_for_maximum_length_source_name'


def assessment(u):
    c,t=u['clause'],u['text']; low=t.lower(); model=u['source'].startswith('spec/10')
    findings=[]; owner='M3 C3.1'; status='implemented'
    basis='Model/handler inspection, builder c31 suites, independent c31_r1_mutations common-contract matrix and candidate CI.'
    def defer(o,why): return 'deferred-with-owner',o,why,[]
    if model:
        if c=='4.4': return defer('M9 C9.2','Transition model is inherited; transition execution and storyline are outside C3.1 (plan §4).')
        if c=='4.6' and any(s in t for s in ['Static transform block','`offset`','`rotation`','`skew`','`scale`','`keepProportions`','Pivot','`pivot`','`applyPivotTo`','`useChildDrawingPivot`','`lockDepth`','`orientToPath`','`camera` |','`exportGroup`']):
            return defer('M3 C3.2 / M6 / M10','Persisted shape inherited; transform evaluation and type-specific behavior belong to named plan checkpoints.')
        if c=='4.6' and any(s in t for s in ['`skeleton` |','`bone` |','`params` |','`blocks` |','`sound` |','`media` |']):
            return defer('M6 C6.3 / M7 / M8','Type-specific evaluation, sound and media model owners; row/type creation is separately covered.')
        if c=='4.6' and '`effectParameter`' in t: findings=['06']
        if c=='4.7' and '`cellNotes`' in t: findings=['05']
    else:
        if c=='6': return defer('M5 C5.2','Properties/Cells windows and panel selection are GUI work.')
        if c=='10.5': return defer('M5 C5.1 / M6 C6.1','View display toggles; annotation type/visibility flags are covered by dedicated command units.')
        if c=='9.4' and ('mini-bar' in low or 'adding a keyframe' in low): return defer('M5 C5.1 / M3 C3.2 / M6 C6.2','Collapsed child summary rendering and group keying require later view/key implementations.')
        if c=='2.9' and ('keyframe symbol' in low or 'global display toggle' in low): return defer('M5 C5.1 / M7 C7.2','Effect keyframe display and global effect visibility UI.')
        if c=='2.3' and 'gui refers' in low: return defer('M5 C5.2','Image terminology in GUI contexts.')
        if c=='2.2' and 'drawing in the drawing or camera view' in low: return defer('M4 C4.2 / spec owner SI-091','M2 blank-cell drawing path remains inherited; highest-name alignment awaits SI-091. C3.1 named-cell creation tested separately.')
        if c=='5.1' and 'window background colour' in low: return defer('M4 C4.1 / M5 C5.1','Window background preferences and their views.')
        if c=='10.1' and 'selection always visible' in low: return defer('M3 C3.3 / M6 C6.1','Selection visibility override in composed scene views.')
        if c=='17.3' and ('omitted fields' in low or t.startswith('```')): return defer('M5 C5.1 / M3 C3.2','Inline cell transfer options are accepted; stored option sets, automatic extension and key transfer remain later work. Current non-default unsupported fields warn explicitly.')
        if c=='17.4':
            m=re.search(r'`([^`]+)`',t)
            if m:
                command=m[1]
                if command.startswith('timeline.') or command.startswith('xsheet.') and command not in ['xsheet.exposures.get','xsheet.exposures.set','xsheet.lightTable.list','xsheet.lightTable.changed']:
                    return defer('M5 C5.1/C5.2','Generated declaration still owned by Timeline/Exposure Sheet view checkpoint.')
                findings=COMMAND_FINDINGS.get(command,[])
        if c.startswith('15') and c!='15.3': return defer('M4 C4.2 / M5 C5.2','Static-light-table rendering and UI commands are later owners; current list query is covered separately.')
        if c=='15.3': return defer('M5 C5.2 (SI-085)','Interim per-scene session list and invalid-source filtering exist; population, persistence and UI await M5.')
        if c=='3.1': return defer('M4 C4.1 / M5 / M7 / M8','GUI creation contexts and import affordances are later checkpoints; headless creation separately covered.')
        if c=='6.3': return defer('M5 / M6 / M8','Properties panels and animation/sound UI are outside C3.1.')
        if c=='6.1' and any(s in low for s in ['transform','velocity','rotation','skew','offset','scale','pivot','orient to path','numeric input']):
            return defer('M3 C3.2 / M5 C5.2','Static persisted shapes inherited; transform/key editing and panel implementation are later scope.')
        if any(s in low for s in ['reachable from','double-click','hovering','toolbar','main menu','disclosure control','panel is disabled','panel refresh','show preview','preview |','column header must','visually distinguishable','row and column must show','indicator is shown','text fields commit']):
            return defer('M4 / M5 / M7','GUI context, display or interaction requirement; corresponding model behavior separately assessed.')
        if c=='2.3' and any(s in low for s in ['import','capture','scanner']): return defer('M8 C8.3 / M10 C10.3','Image acquisition/import contexts; model/image-bank preservation inherited.')
        if c in ['2.4','2.5'] and any(s in low for s in ['sound','linked','media','reset','file location']): return defer('M8 C8.1/C8.3 (SI-088)','Media/sound placements and natural-length discovery; explicit media length supported now.')
        if c=='2.10' and 'bone' in low: return defer('M6 C6.3 / M3 C3.2','Skeleton row scaffolding exists; bone tools and evaluated keys are later scope.')
        if c=='13.18' or c=='13.19': return defer('M5 C5.2 / M8 C8.3','Static light table UI and media reset natural length are declared later owners.')
        if c=='14.7': return defer('M9 C9.1','Library templates, transfer and update-drawings workflow.')
        if c=='14.3' and '| Import ' in t: return defer('M8 / M10','Acquisition/import pipeline owners in plan §4; vectorize remains inherited M2.')
        if c=='17.5' and ('timeline.rows.list' in t or 'where each row' in low): return defer('M5 C5.1','Timeline view read model remains generated; C3.1 element and exposure queries are implemented.')
        if c=='17.6' and any(s in t for s in ['element.templateEditRestricted','cell.busy']): return defer('M9 C9.1 / M6 C6.3','Library template editing and live bone deformation contexts do not exist in C3.1.')
        if c=='17.6' and any(s in t for s in ['element.notDrawing','drawing.fileMissing']): return defer('M8 C8.3 / owning drawing command','Full media-missing discovery and future command-specific applicability; loaded placeholder missing flag is supported.')
        if c=='6.1' and 'background elements automatically extend' in low:
            return defer('M3 C3.3','Background role is persisted now; frame rendering and automatic background hold evaluation are C3.3.')
        if c=='2.6' and 'final render' in low: return defer('M3 C3.3','Annotation type conversion implemented; renderer/export exclusion is next renderer checkpoint.')
        if c=='13.10': findings=['01'] if any(s in low for s in ['total frames','requested frame count']) else []
        if c=='13.5' and ('maximum 9999' in low): findings=['09']
        if c in ['13','13.16'] and any(s in low for s in ['clipboard','copy','paste','properties','cut']): findings=['04','05','13']
        if c=='7.2' and ('stored per element per frame' in low): findings=['05']
        if c=='8.2' and 'copy' in low: findings=['06']
        if c=='2.9' and 'parameter' in low: findings=['06']
        if c=='17.1' and 'atomic' in low: findings=['02']
        if c=='13' and 'selection spanning several elements' in low: findings=['15']
        if c=='8.1' and 'unique suffix' in low: findings=['08']
        if c=='17.2' and ('createMissingDrawings' in t or 'unknown names' in low): findings=['08']
        if c=='17.6' and 'cell.frameOutOfRange' in t: findings=['09']
        if c=='10.3': findings=['02']
        if c=='17.4' and 'xsheet.lightTable.changed' in t: return defer('M5 C5.2','Event is declared with schema; its producers are the later light-table mutation commands.')
    if findings:
        status='divergent'; basis='Implemented normal path, but independent reproductions fail; see findings. Other aspects remain covered by the common matrix.'
    return status,owner,basis,findings


def build():
    wanted04={str(n) for n in list(range(1,11))+[13,14,15,17]}
    wanted04|={m[1] for m in re.finditer(r'^### (\d+\.\d+)',source('spec/04_timeline_xsheet.md'),re.M) if m[1].split('.')[0] in wanted04}
    all_units=units('spec/04_timeline_xsheet.md',wanted04)+units('spec/10_data_model_project_format.md',{f'4.{n}' for n in range(3,9)})
    records=[]
    for i,u in enumerate(all_units,1):
        state,owner,basis,findings=assessment(u)
        records.append({'id':f'C31-U{i:03}',**u,'status':state,'owner':owner,'basis':basis,
                        'findings':[f'C31-R1-{n}' for n in findings], 'reproduction':[REPRO[n] for n in findings]})
        # Mixed model/view sentences are split: the original row's implemented assessment covers its data/API
        # projection only; a separate unit explicitly owns the visible/evaluated behavior.
        if state=='implemented' and any(s in u['text'].lower() for s in ['panel','render','composite','preview','view','keyframe','transformation','indicator','display','shown','hover']):
            records[-1]['basis']='C3.1 model/API projection only. '+basis
            records.append({'id':f'C31-U{i:03}-V',**u,'status':'deferred-with-owner','owner':'M3 C3.2/C3.3 / M4 / M5 / M6 / M7 / M8',
                'basis':'Separate evaluated/rendered/GUI aspect of this mixed source unit; persisted data and C3.1 commands are assessed in the paired unit. Plan §4 assigns the relevant view, animation, renderer or type-specific checkpoint.',
                'findings':[],'reproduction':[]})
    extra=[
      ('10 AG-02','Delegated property patches apply all owning command semantics, rules, UI changes and events.','divergent','M3 C3.1',['03']),
      ('M2 handoff 1','Symmetry and TimelineState stay outside document revisions, history and saved defaults; session acceptance covers all new fields.','divergent','M3 C3.1',['02']),
      ('M2 handoff 2','Compose drawing/element/peg transforms, det +/-1, nesting, inverse targeting and retained 1200-du vectorization coordinates.','deferred-with-owner','M3 C3.2',[]),
      ('M2 handoff 3','Render lists derive from exposures, hierarchy, visibility and order without changing drawing/zone IDs.','deferred-with-owner','M3 C3.3',[]),
      ('M2 handoff 4','Bitmap/image scene renders preserve transforms, alpha, assets and mixed fills.','deferred-with-owner','M3 C3.3',[]),
      ('M2 handoff 5','Unrelated element/exposure edits preserve drawing revisions, zone IDs and selections.','implemented','M3 C3.1',[]),
      ('M2 handoff 6','All copy/clipboard bridges preserve owner records, stretch fills and opaque namespaces through save/open and undo.','divergent','M3 C3.1',['04','13']),
      ('M2 handoff 7','Common staged contract and request-owned cancellation including session/UI acceptance boundary.','divergent','M3 C3.1',['02','10']),
      ('M2 handoff 8','Compare candidate timing/allocation/RSS/queue/maintenance raw records with M2 round-3; keep scale gate owned by M11.','implemented','M3 C3.1 / M11 C11.1',[]),
      ('M2 handoff 9','Release E2, cross-OS PNGs, image conversion and inherited oracles remain unchanged and passing.','implemented','M3 C3.1',[]),
      ('M2 handoff 10','Carry SI-077 Feather deferral and SI-084 adopted colour precedence without inferring a model amendment.','implemented','M7 C7.2 / spec owner',[]),
      ('engineering M1 edit','Interned-index exposure diffs and range patches are output-identical to the algorithm at 0b8fd0b.','implemented','M3 C3.1',[]),
      ('engineering helpers','Insertion helpers return trimmed arrays even on the beyond-end early-return path.','divergent','M3 C3.1',['11']),
      ('engineering evidence','Candidate whole-sheet under-1-second and <=4-ms refusal claims agree with candidate raw records.','divergent','M3 C3.1',['12']),
      ('SI-089','Held value at truncated bar new end needs evaluated curve value and explicit end-key behavior.','deferred-with-owner','M3 C3.2',[]),
      ('work budgets','Every selection generation/deduplication scan observes cancellation and releases the worker.','divergent','M3 C3.1',['14']),
    ]
    for i,(clause,text,status,owner,findings) in enumerate(extra,1):
        records.append({'id':f'C31-X{i:02}','source':'review charter / binding rulings','clause':clause,'line':0,'text':text,'status':status,'owner':owner,
          'basis':'Independent review adjudication; see report and retained local/CI receipts.','findings':[f'C31-R1-{n}' for n in findings],'reproduction':[REPRO[n] for n in findings]})
    old=json.loads(source('crates/registry/src/declared.json','0b8fd0b'))
    new={r['id'] for r in json.loads(source('crates/registry/src/declared.json'))}
    inventory={m[1]:m[2] for m in re.finditer(r'^\| `([^`]+)` \| (\w+) \|',source('spec/A_command_registry.md'),re.M)}
    commands=[]
    for row in old:
        name=row['id']
        if name in new: continue
        findings=COMMAND_FINDINGS.get(name,[])
        deferred=name=='xsheet.lightTable.changed'
        commands.append({'id':name,'kind':inventory[name],'status':'divergent' if findings else 'deferred-with-owner' if deferred else 'implemented',
            'owner':'M5 C5.2' if deferred else 'M3 C3.1','findings':[f'C31-R1-{n}' for n in findings],
            'reproduction':[REPRO[n] for n in findings],
            'basis':'Declaration/schema/example and handler inspection; c31_r1_mutations (44 action/toggle examples), c31 builder suites and candidate never-block/example-results CI.',
            'deferredAspects':(['natural media length discovery: M8 C8.3 (SI-088)'] if name=='element.mediaLength.set' else ['allowed sound/media cell operations: M8 C8.1/C8.3 (SI-088)'] if name.startswith('cell.') else [])})
    states=['implemented','divergent','missing','deferred-with-owner']
    count=lambda rows:{status:sum(r['status']==status for r in rows) for status in states}
    return {'candidate':HEAD,'granularity':'One source paragraph, list item, table requirement row or fenced shape/example. A divergent unit can contain passing aspects; the attached reproduction identifies the failing aspect. Inherited data shapes are not claims of later UI/render behavior.',
            'requirements':records,'commandsAndEvents':commands,
            'counts':{'requirements':count(records),'commandsAndEvents':count(commands)}}


class Register(unittest.TestCase):
    def test_complete_pinned_command_inventory(self):
        r=build(); self.assertEqual(len(r['commandsAndEvents']),64)
        self.assertEqual(len({u['id'] for u in r['commandsAndEvents']}),64)
        self.assertEqual(sum(u['kind']=='event' for u in r['commandsAndEvents']),5)
        self.assertGreater(len(r['requirements']),250)
        for u in r['requirements']+r['commandsAndEvents']:
            self.assertIn(u['status'],['implemented','divergent','missing','deferred-with-owner'])
            if u['status']=='divergent': self.assertTrue(u['reproduction'])
            if u['status']=='deferred-with-owner': self.assertTrue(u['owner'])


def emit(dest):
    dest.mkdir(parents=True,exist_ok=True); r=build()
    p=dest/'M3-C3.1-r1-register.json'
    if p.exists(): raise FileExistsError('register output exists; preserve evidence and choose a new output directory')
    p.write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    lines=['# C3.1 round-1 requirement register','',f'Candidate: `{HEAD}`. Counts: `{json.dumps(r["counts"])}`. Missing: 0.','',r['granularity'],'','## Commands and events','']
    for u in r['commandsAndEvents']:
        lines += [f'- **{u["id"]}** — {u["status"]}; owner {u["owner"]}. '+', '.join(u['findings'])]
    lines+=['','## Source requirement units','']
    for u in r['requirements']:
        lines += [f'### {u["id"]} — {u["source"]}:{u["line"]} (§{u["clause"]})','',u['text'],'',f'**{u["status"]}**, owner {u["owner"]}. {u["basis"]}']
        if u['findings']: lines += ['',', '.join(u['findings']),*['- `'+s+'`' for s in u['reproduction']]]
        lines+=['']
    (dest/'M3-C3.1-r1-register.md').write_text('\n'.join(lines),encoding='utf-8')
    print(json.dumps(r['counts']))


if __name__=='__main__':
    if len(sys.argv)==3 and sys.argv[1]=='--output': emit(pathlib.Path(sys.argv[2]).resolve())
    else: unittest.main()
