"""Independent positive cases, old/new regression comparison and context-isolation probes.

Run after select-sample.py. Does not regenerate or write any product/spec file.
"""
import copy
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

from jsonschema import Draft202012Validator as Validator

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
rows = {r['id']: r for r in json.loads((ROOT / 'crates/registry/src/declared.json').read_text(encoding='utf-8'))}
old = {r['id']: r for r in json.loads(subprocess.check_output(['git', 'show', 'be5ef32:crates/registry/src/declared.json'], cwd=ROOT))}
out = []

def check(cid, part, value, source, group):
    before = list(Validator(old[cid][part]).iter_errors(value))
    after = list(Validator(rows[cid][part]).iter_errors(value))
    out.append({'id': cid, 'part': part, 'value': value, 'source': source, 'group': group,
                'acceptedBefore': not before, 'acceptedNow': not after,
                'errors': [{'path': list(e.path), 'message': e.message} for e in after]})

original = json.loads((ROOT / 'reviews/evidence/M1-C1.2-r3/semantic-schema-cases.json').read_text(encoding='utf-8'))
for c in original:
    check(c['id'], c['part'], c['value'], c['source'], 'original14')
    v = copy.deepcopy(c['value'])
    cid = c['id']
    if cid == 'anim.path.get':
        v['length'] = 22.875
        v['points'][1]['frame'] = 8.625
    elif cid == 'anim.function.get':
        v['pins'] = [{'frame': 3, 'value': 1.25}, {'frame': 17, 'value': 9.125}]
    elif cid == 'draw.pointSelection.set':
        v['selectedPoints'] = [{'strokeId': 'closure-stroke', 'anchor': 5}]
    elif cid == 'xsheet.panel.set':
        v = {'elementList': False, 'lightTable': True, 'toolbar': False}
    elif cid == 'prefs.list':
        v[0]['default'] = False
        v[0]['value'] = False
    elif cid == 'keymap.list':
        v[0].update(commandId='doc.object.find', params={'type': 'stroke', 'limit': 17})
    elif cid == 'tool.polyline.options.set':
        v = {'options': {'snap': False, 'snapRadius': 5.75}}
    elif cid == 'print.settings.set':
        v = {'values': {'copies': 7}}
    elif cid == 'export.settings.get':
        v['preset'] = 'custom'
    elif cid == 'drawAid.staticLightTable.set':
        v['entries'][0].update(entryId='closure-entry', sourceId='closure-drawing', visible=False, placement='overlay')
    elif cid == 'tool.pen.set':
        v['pen'].update(id='closure-pen', name='Closure Pen', minSize=1.25, maxSize=8.75)
    elif cid.startswith('sound.envelope.'):
        v = {'points': [{'time': 0.375, 'left': 0.125, 'right': 0.875}, {'time': 0.875, 'left': 1, 'right': 0}]}
    else:
        raise AssertionError(cid)
    check(cid, c['part'], v, c['source'], 'fresh14')

cases = {
    'sound.asset.probe': ('params', {'path': 'C:/closure/audio dialog.wav'}, '07 §15.2 asset commands / §3.1 audio formats; explicit path input'),
    'playback.start': ('params', {'sceneId':'closure-scene','fromFrame':17}, '09 §12 playback commands; current scene and integer fromFrame'),
    'tool.placeBehind.set': ('result', {'enabled':False}, '02 §7 drawing behind; §13.3 paired boolean enabled'),
    'fx.preview.open': ('result', {'panelId':'closure-preview'}, '06 effect preview / fx.preview.open command row'),
    'colour.fillMapping.set': ('params', {'drawingId':'closure-drawing','objectIds':['closure-zone'],'mapping':'clipboard'}, '03 fill mapping / colour.fillMapping.set; SI-025 explicit object shape remains deferred'),
    'export.report.get': ('result', {'file':'closure.svg','totalBytes':1189,'cameraSize':'1920x1080','fps':23.976,'groups':[{'kind':'paths','items':[{'name':'line','bytes':127}]}]}, '09 §4.10 size report / export.report.get; cameraSize scalar uses accepted SI-029 placeholder'),
    'lipsync.track.get': ('result', {'tracks':[{'placementId':'closure-placement','fps':24,'startFrame':17,'source':'transcript','stale':False,'frames':[{'frame':17,'mouth':'X','manual':False,'label':None}],'segments':[{'start':0.125,'end':0.375,'mouth':'X','phoneme':'sil'}]}]}, '07 §15.4 literal LipTrack shape with fresh values and fractional segment times'),
    'library.closed': ('result', {'libraryId':'closure-library'}, '08 §13 event command row: libraryId'),
    'draw.polyline.add': ('params', {'points':[{'x':1.25,'y':-2.5,'inHandle':{'x':0.125,'y':-0.375},'outHandle':{'x':2.25,'y':1.5}}],'closed':False,'snap':True,'snapRadius':4.75,'layer':'behind','symmetry':False}, '02 §7.3 Polyline / §13.2 command input; geometric points'),
    'anim.point.paste': ('params', {'targetPointIds':['closure-point'],'properties':['rotationPivot','scaleX'],'again':False}, '05 point Copy/Paste Special / anim.point.paste command row'),
    'anim.camera.current.get': ('result', {'cameraId':'closure-camera'}, '05 current camera / anim.camera.current.get command row'),
    'anim.skeleton.skin.remove': ('result', {'elementIds':['closure-element-1','closure-element-2']}, '05 skin group / anim.skeleton.skin.remove command row and typed parameters'),
    'draw.linesToBrush.apply': ('params', {'drawingId':'closure-drawing','ids':['closure-line'],'penId':'closure-brush'}, '02 Convert Lines to Brush / §13.2 command row'),
    'colour.style.duplicate': ('result', {'styleId':'closure-style','name':'Night palette'}, '03 Duplicate Style / colour.style.duplicate command row'),
    'ui.tips.reset': ('result', {'reenabled':17}, '01 tips / ui.tips.reset command row, explicit int'),
    'draw.object.delete': ('result', {'removedIds':['closure-zone','closure-stroke'],'rev':73}, '02 Delete Object / §13.2 command row'),
    'tool.tablet.set': ('params', {'ignoreTablet':False,'portraitCorrection':True,'compatibilityMode':False,'inputInterface':'Windows Ink'}, '02 §6.3 tablet settings and §13.3 explicit parameter fields'),
    'tool.active.set': ('result', {'toolId':'contourEditor'}, '02 tools and tool.active.set enum input; result returns selected toolId'),
    'keymap.remove': ('result', {'removed':[{'commandId':'doc.object.find','params':{'type':'stroke','limit':17},'keys':'Primary+7','context':'global','isDefault':False}]}, '01 §12.4 Binding; arbitrary bound command parameters'),
    'export.publish.service.list': ('result', {'services':[{'id':'closure-service','name':'Review Service','formats':['mp4'],'limits':{'minSec':0.5,'maxSec':90,'maxBytes':1000000},'fields':[{'name':'title','required':True}],'visibilities':['private','public'],'connected':True}]}, '09 §4.9 and §12.3 service.list/account.get/account.close; a configured service can be signed in'),
}
sample = json.loads((HERE / 'sample20.json').read_text(encoding='utf-8'))
assert set(sample['ids']) == set(cases)
for cid in sample['ids']:
    part, value, source = cases[cid]
    check(cid, part, value, source, 'random20')

check('storyline.play.changed', 'result', {'playing':False,'globalFrame':17,'sceneId':'closure-scene'},
      '08 storyline playback state event, emitted when playback stops; start has playing:true, stop has playing:false', 'constant-regression-sweep')

# Exercise the actual new context-selection function with independently constructed conflicting families.
spec = importlib.util.spec_from_file_location('closure_generator', ROOT / 'scripts/gen-command-declarations.py')
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)
def obj(**props):
    return {'type':'object','properties':props}
raw = {
    'sound.envelope.get': ({},obj(points={'type':'array','items':obj(time={'type':'number'},left={'type':'number'},right={'type':'number'})})),
    'draw.polyline.get': ({},obj(points={'type':'array','items':obj(x={'type':'number'},y={'type':'number'})})),
    'sound.envelope.set': (obj(points={}),obj(points={})),
}
index = gen.field_index({'id':'sound.envelope.set'}, raw)
foreign_isolated = 'time' in index['points']['items']['properties'] and 'x' not in index['points']['items']['properties']
assert foreign_isolated
raw2 = {
    'export.publish.account.close': ({},obj(connected={'const':False})),
    'export.publish.account.get': ({},obj(connected={'type':'boolean'})),
    'export.publish.service.list': ({},obj(services={'type':'array','items':obj(connected={})})),
}
collision = gen.field_index({'id':'export.publish.service.list'}, raw2)['connected']

# Check all attached spec examples with an independent validator, all placeholder SI markers, and literal origins.
examples, placeholders, literal_origins = [], [], []
decoder = json.JSONDecoder()
for row in rows.values():
    for ex in row.get('specExamples', []):
        schema = row[ex['part']]
        target = {'$defs':schema.get('$defs',{}),'$ref':'#/$defs/'+ex['type']} if ex['type'] else schema
        errors = list(Validator(target).iter_errors(ex['value']))
        examples.append({'id':row['id'],'section':ex['section'],'type':ex['type'],'literal':ex['literal'], 'valid':not errors})
        if ex['literal']:
            source = next((ROOT/'spec').glob(ex['section']+'_*.md')).read_text(encoding='utf-8')
            candidates = []
            for pos, ch in enumerate(source):
                if ch in '{[':
                    try: candidates.append(decoder.raw_decode(source[pos:])[0])
                    except ValueError: pass
            literal_origins.append({'id':row['id'],'type':ex['type'],'foundVerbatimParsedValue':ex['value'] in candidates})
    def walk(v, path):
        if isinstance(v, dict):
            if v.get('x-placeholder'):
                issue = v.get('x-specIssue')
                heading = f"## {issue} — `{row['id'].split('.')[0]}.*`"
                placeholders.append({'id':row['id'],'path':path,'issue':issue,'documented':heading in (ROOT/'plan/spec-issues.md').read_text(encoding='utf-8')})
            for k,x in v.items(): walk(x, path+'/'+k)
        elif isinstance(v,list):
            for i,x in enumerate(v): walk(x,path+'/'+str(i))
    for part in ['params','result']: walk(row[part],part)

summary = {'seed':sample['seed'],'cases':out,'context':{'foreignTwoSegmentFamilyIsolated':foreign_isolated,
           'accountToServiceCollision':collision,'rootCause':'first two ID segments group distinct subfamilies; first typed field wins, including command-specific constants'},
           'specExamples':examples,'literalOrigins':literal_origins,'placeholders':placeholders}
(HERE/'schema-closure.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'groups':{g:{'cases':sum(c['group']==g for c in out),'accepted':sum(c['group']==g and c['acceptedNow'] for c in out)} for g in sorted({c['group'] for c in out})},
                  'regressions':[c['id'] for c in out if c['acceptedBefore'] and not c['acceptedNow']],
                  'examples':len(examples),'literalOrigins':len(literal_origins),'unmatchedLiterals':[x for x in literal_origins if not x['foundVerbatimParsedValue']],
                  'placeholders':len(placeholders),'context':summary['context']},indent=2))
assert all(x['valid'] for x in examples) and all(x['documented'] for x in placeholders)
sys.exit(1 if any(not c['acceptedNow'] for c in out) else 0)
