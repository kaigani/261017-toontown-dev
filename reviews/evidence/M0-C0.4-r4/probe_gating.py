"""Independent literal-name and launch probes, deliberately not parsing the host registry."""
import hashlib
import json
import os
import pathlib
import subprocess
import sys

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]
MODE = sys.argv[1]
EXE = ROOT/'target/release/toontown.exe'
NAMES = ['bench_plan','bench_report','spike_mode','spike_tablet_native_start',
         'spike_tablet_native_stop','spike_pen_simulate','spike_save_log','spike_environment',
         'spike_probe_native_window','spike_raw_input_sink','spike_canvas_scene','spike_canvas_pixels']
data = EXE.read_bytes()
result = {'mode':MODE, 'sha256':hashlib.sha256(data).hexdigest(), 'bytes':len(data),
          'binary_counts':{n:data.count(n.encode()) for n in NAMES+['--ipc-bench','--spike','app_info']},
          'dist':[], 'launches':[]}
for path in sorted((ROOT/'desktop/dist').rglob('*')):
    if not path.is_file():
        continue
    content = path.read_bytes()
    result['dist'].append({'file':str(path.relative_to(ROOT/'desktop/dist')),
                           'sha256':hashlib.sha256(content).hexdigest(),
                           'names':[n for n in NAMES+['TabletSpike','CanvasSpike'] if n.encode() in content]})
assert all(bool(result['binary_counts'][n]) == (MODE == 'diagnostics') for n in NAMES)
if MODE == 'default':
    for file in result['dist']:
        assert not set(file['names']).intersection(NAMES), file
        if not file['file'].endswith('.map'):
            assert not file['names'], file
    variants = [({}, 'ordinary'), ({'CARGO_FEATURE_M0_DIAGNOSTICS':'1','TT_M0_DIAGNOSTICS':'1',
        '__TT_M0_DIAGNOSTICS__':'true','VITE_M0_DIAGNOSTICS':'true',
        'TAURI_CONFIG':'{"build":{"features":["m0-diagnostics"]}}'}, 'runtime-environment')]
    for env_extra, label in variants:
        for args in [['--spike','tablet'], ['--ipc-bench',str(OUT/'must-not-exist.json')], ['--not-a-real-flag']]:
            p = subprocess.run([str(EXE)]+args,cwd=ROOT,env=os.environ|env_extra,capture_output=True,timeout=20)
            row = {'variant':label,'args':args,'exit':p.returncode,'stdout':p.stdout.decode(errors='replace'),
                   'stderr':p.stderr.decode(errors='replace')}
            result['launches'].append(row)
            assert p.returncode == 1, row
            assert ('unknown argument' if args[0]=='--not-a-real-flag' else 'is an M0 diagnostic and this build has none') in row['stderr'], row
    assert not (OUT/'must-not-exist.json').exists()
else:
    p = subprocess.run([str(EXE),'--ipc-bench',str(OUT/'ipc-bench.json'),'--ipc-bench-count','50'],
                       cwd=ROOT,capture_output=True,timeout=150)
    result['launches'].append({'args':['--ipc-bench','ipc-bench.json','--ipc-bench-count','50'],
        'exit':p.returncode,'stdout':p.stdout.decode(errors='replace'),'stderr':p.stderr.decode(errors='replace')})
    assert p.returncode == 0
    assert json.loads((OUT/'ipc-bench.json').read_text())['frontend']['ok']
(OUT/f'independent-gating-{MODE}.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(MODE, 'independent binary/dist/runtime probes passed')
