"""Literal decision-table counterexamples, not hardware results or product code.

Tablet aggregation implements the six published Step 3 predicates explicitly.
Canvas applies validation/availability guards before the scene rows.
"""
import importlib.util
import copy
import itertools
import json
import pathlib
import subprocess
import sys

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]


def step3(attributes, candidates, pending=False, limitations=False, no_go=False):
    rows = []
    if not attributes and not limitations and not no_go and not pending:
        rows.append('W')
    passing_all = [name for name, passed in candidates.items() if set(attributes) <= set(passed)]
    if attributes and passing_all:
        rows.append('W + candidate: ' + ','.join(passing_all))
    if attributes and pending:
        rows.append('run pending E-H1/E-D1')
    if limitations:
        rows.append('path with limitations')
    if no_go:
        rows.append('platform no-go')
    return rows


def canvas(gpu, gl, valid=True):
    if not valid:
        return dict(decision='rerun; not evidence')
    if gl is None:
        return dict(decision='investigate mandatory WebGL2' if gpu else 'no-go until investigated')
    selected = 'webgpu' if gpu is not None and min(gpu) >= min(gl) else 'webgl2'
    p, g, b = gpu if selected == 'webgpu' else gl
    decision = ('native-wgpu prototype required' if g < 57 else
                'fill LOD / E-C2, native prototype if still missed' if b < 57 else
                'go with path culling / LOD' if p < 57 else 'go')
    return dict(selected=selected, decision=decision)


result = dict(
    interpretation='Step 3 rows evaluated literally; no added mixed-native or partial-native rule.',
    tablet=[
        dict(case='pressure512 / supported tilt absent /150Hz / stationary missing; R,H,D all measured fail relevant rows',
             classification={'identity':'pass','pressure256':'pass','pressure1024':'partial','tilt':'fail',
                             'twist':'n/a','coalesced':'pass','rate120':'pass','rate200':'partial','stationary':'fail'},
             resolution='W, tilt options disabled; stationary pressure ignored; both documented limitations'),
        dict(case='same, R fails and H,D not measured', resolution='E-H1, then E-D1 as needed; no hardware decision'),
        dict(case='all candidates measured; W pressure partial and tilt fail; R passes pressure only; H passes tilt only; D fails both',
             step2={'pressure':'R','tilt':'H'}, step3=step3(['pressure','tilt'],{'R':['pressure'],'H':['tilt'],'D':[]})),
        dict(case='Mac W50Hz, A90Hz; other rows W pass/n/a; row7 accepts native partial',
             step2={'rate120':'A (partial accepted by row7)'},step3=step3(['rate120'],{'A':[]})),
        dict(case='Mac W has no pen identity, A pen identity passes but A pressure binary; W pressure passes',
             conflict='row1 native for EVERY pen attribute vs row2 W pressure; Step3 says only candidate passes all rows. '
                      'No check requires native pressure to pass when row1 alone switches every attribute.'),
    ],
    canvas=[])
for name, gpu, gl, valid in [
    ('requested 45/58 paths; brush40 both', [45,60,40], [58,60,40], True),
    ('R1 available slower WebGPU', [60,40,60], [60,60,60], True),
    ('Windows software runner', None, [0.8,16.9,8.9], True),
    ('Mac virtual runner / no WebGPU adapter', None, [22.1,60,60], True),
    ('physical Mac no adapter but GL passes', None, [60,60,60], True),
    ('mandatory GL fails but GPU works', [60,60,60], None, True),
    ('both unavailable', None, None, True),
    ('malformed log', None, None, False),
]:
    result['canvas'].append(dict(case=name, webgpu=gpu, webgl2=gl, **canvas(gpu,gl,valid)))
result['canvas_scene_combinations'] = [dict(fps=values, **canvas(None,values))
                                       for values in itertools.product([40,60], repeat=3)]
canvas_spec = importlib.util.spec_from_file_location('canvas_analyzer', ROOT/'scripts/analyse-canvas-log.py')
ca = importlib.util.module_from_spec(canvas_spec)
canvas_spec.loader.exec_module(ca)
original = json.loads((ROOT/'reviews/evidence/M0-C0.3-r4/page-r3-one-frame.json').read_bytes())
partial = copy.deepcopy(original)
partial['runs'] = partial['runs'][:1]
both = copy.deepcopy(original)
both['environment'].update(webgpuApi=True, forcedRenderer=None)
both['environment']['mode']['renderer'] = None
gpu_runs = copy.deepcopy(both['runs'])
for row in gpu_runs:
    row['backend'] = 'webgpu'
    for frame in row['frames']:
        frame['gpuMs'] = 0
both['runs'] = gpu_runs + both['runs']
for name, fixture in [('partial-canvas',partial),('null-cadence-both',both)]:
    ca.validate(fixture)
    (OUT/f'{name}.json').write_text(json.dumps(fixture,indent=2)+'\n')
    rows = [ca.analyse_run(r) for r in fixture['runs']]
    (OUT/f'{name}.analysis.json').write_text(json.dumps(rows,indent=2)+'\n')
    result['canvas'].append(dict(case=name, analyzerAccepted=True,
                                scenes=[dict(backend=r['backend'],scene=r['scene'],fps=r['fps']) for r in rows],
                                decision='missing-scene guard absent' if name=='partial-canvas' else
                                'worst-fps comparison has no score for complete null cadence'))
# Permanent tests run independently of the product gate (failure expected and retained).
proc = subprocess.run([sys.executable,'-X','utf8','tests/adversarial/c04-r2-decisions.py'], cwd=ROOT,
                      stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
(OUT/'decision-regressions.log').write_bytes(proc.stdout)
result['regressions_exit'] = proc.returncode
spec = importlib.util.spec_from_file_location('regressions', ROOT/'tests/adversarial/c04-r2-decisions.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
for rotation in (False,True):
    fixture = module.manual_fixture(rotation)
    suffix = 'rotation' if rotation else 'no-rotation'
    (OUT/f'manual-fixture-{suffix}.json.gz').write_bytes(
        __import__('gzip').compress(json.dumps(fixture).encode(),mtime=0))
    analysis = module.analyzer.web_analysis(fixture)
    (OUT/f'manual-fixture-{suffix}.analysis.json').write_text(json.dumps(analysis,indent=2)+'\n')
(OUT/'decision-counterexamples.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print('Tablet holes:', [row['case'] for row in result['tablet'] if row.get('step3') == []])
print('Permanent regression exit', proc.returncode)
