"""R3 independent analyzer probes. Synthetic fixtures are never hardware evidence."""
import copy
import gzip
import importlib.util
import json
import pathlib
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

t = load('tablet', 'scripts/analyse-tablet-log.py')
c = load('canvas', 'scripts/analyse-canvas-log.py')
f = load('fixtures', 'scripts/test_decision_rules.py')
results = []

def cli(name, log, kind='tablet', expected=0):
    if kind=='canvas':
        log=copy.deepcopy(log)
        # Decision probes need cadence/geometry records, not duplicate embedded PNG previews.
        for run in log['runs']: run.pop('snapshot',None)
    path = OUT / (name + '.json.gz')
    path.write_bytes(gzip.compress(json.dumps(log).encode(), mtime=0))
    target = OUT / (name + '.analysis.json')
    p = subprocess.run(['python', '-X', 'utf8', 'scripts/analyse-'+kind+'-log.py', str(path), '--json', str(target)],
                       cwd=ROOT, capture_output=True)
    (OUT / (name + '.log.gz')).write_bytes(gzip.compress(p.stdout + p.stderr, mtime=0))
    assert p.returncode == expected, (name, p.returncode, p.stderr)
    d = json.loads(target.read_bytes()) if p.returncode == 0 else None
    if d:
        assert isinstance(d['decision']['outcome'], str) and d['decision']['outcome']
        assert p.stdout.count(b'### Decision (') == 1
    results.append(dict(case=name, exit=p.returncode, decision=d['decision'] if d else None))
    return d

def computed(log):
    w, n = t.web_analysis(log), t.native_analysis(log)
    a = t.action_analysis(log, w, n)
    return w, n, a

def native_copy(log, prefix=0):
    packets = []
    for i in range(prefix):
        packets.extend([dict(kind='down', tool='pen', pressure=.5, hostEpochMs=1e12-1000+i*100),
                        dict(kind='up', tool='pen', pressure=0, hostEpochMs=1e12-950+i*100)])
    for s in log['web']['samples']:
        packets.append(dict(kind={'move':'pose'}.get(s['kind'], s['kind']), tool='pen',
                            hostEpochMs=1e12+s['t'], pressure=s['pressure'],
                            tiltX=s['tiltX'], tiltY=s['tiltY'], twist=s['twist']))
    log['native']['packets'] = packets
    log['nativeInfo'] = {'backend': 'test-native'}

def main():
    for rotation in (False, True):
        log = f.manual_log(rotation)
        native_copy(log, 3)
        d = cli('mapping-'+str(rotation), log)
        a = {a['id']:a for a in d['actions']['actions']}
        assert a['A6']['web']['measured'] == (5 if rotation else 4)
        assert a['A6']['native']['measured'] == (8 if rotation else 7)
        assert d['actions']['nativeOutside'] == [1,2,3]
    for hz, want in [(119.95, ('partial','fail')), (120., ('pass','partial')),
                     (199.99, ('pass','partial')), (200., ('pass','pass')), (None, ('insufficient','insufficient')),
                     ('single', ('insufficient','insufficient'))]:
        log = f.manual_log(False)
        a2 = [s for s in log['web']['samples'] if 6000 <= s['t'] < 11100]
        # 120 contact intervals allow exactly representable duration at 120/200 Hz.
        keep = a2[:121] + [a2[-1]]
        log['web']['samples'] = [s for s in log['web']['samples'] if s not in a2 or s in keep]
        a2 = keep
        if hz == 'single':
            log['web']['samples'] = [s for s in log['web']['samples'] if s not in a2 or s['kind'] != 'move']
        else:
            for i, s in enumerate(a2):
                s['t'] = 6000 if hz is None else 6000 + i*1000/hz
                s['handlerT'] = s['t']+1
        d = cli('rate-'+str(hz), log)['decision']
        assert tuple(d['rows']['W'][str(r)]['result'] for r in (7,8)) == want
    # pressure_levels() has always rounded its implied grid to integers. Exercise fractional
    # classification at the real classify boundary, without falsely claiming raw logs emit fractions.
    log = f.manual_log(False)
    w, n, a = computed(log)
    for levels, want in [(255.5, ('partial','fail')), (256, ('pass','partial')),
                         (1023.9, ('pass','partial')), (1024, ('pass','pass'))]:
        w['strokes'][0]['pressureDistinct'] = 100
        w['strokes'][0]['pressureLevelsImplied'] = levels
        rows = t.classify('web', w['strokes'], a, log['metadata'], web=w)
        assert (rows[2][0],rows[3][0]) == want
        decision = t.aggregate('windows', {'W':{r:v[0] for r,v in rows.items()}})
        results.append(dict(case='fractional-pressure-'+str(levels), boundary='real classify/aggregate; injected measurement', decision=decision))
    ok = {r:'pass' for r in range(1,10)}
    cases = [
        ('r2-windows', 'windows', {'W':{**ok,2:'partial',3:'fail',4:'fail'}, 'R':{**ok,4:'fail',6:'n/a'},
          'H':{**ok,2:'fail',3:'fail',6:'n/a'}, 'D':{**ok,2:'fail',3:'fail',4:'fail',6:'n/a'}}, 'R'),
        ('r2-macos', 'macos', {'W':{**ok,7:'fail',8:'fail'}, 'A':{**ok,6:'n/a',7:'partial',8:'fail'}}, 'A'),
        ('tie-W-R', 'windows', {'W':{**ok,6:'partial'}, 'R':{**ok,6:'n/a'}}, 'W'),
        ('tie-R-H-D', 'windows', {k:{**ok,6:'n/a',4:'fail'} for k in ('R','H','D')}, 'R'),
        ('D-wins-under-added-filter', 'windows', {'W':{**ok,2:'fail'},'R':{**ok,4:'fail',5:'fail',6:'n/a'},
                              'H':{**ok,7:'fail',6:'n/a'},'D':{**ok,6:'n/a'}}, 'D'),
    ]
    for name, platform, rows, winner in cases:
        d = t.aggregate(platform, rows)
        assert d['selected'] == winner
        assert isinstance(d['outcome'], str)
        results.append(dict(case=name, boundary='real aggregate; R/D unimplemented capture paths', inputs=rows, decision=d))
    # Binding brief says rank measured candidates by pass count, then priority. The builder
    # explicitly added an eligibility filter; demonstrate the semantic difference, not endorse it.
    rows={'W':{**ok,2:'fail',3:'fail'},'R':{**ok,4:'fail',5:'fail',6:'n/a'}}
    d=t.aggregate('windows',rows)
    assert d['scores']=={'W':7,'R':6} and d['selected']=='R'
    results.append(dict(case='unruled-core-filter',inputs=rows,ruledWinner='W',decision=d))
    log = f.manual_log(False)
    native_copy(log)
    for p in log['native']['packets']:
        if p['kind'] != 'up':
            p.update(pressure=.5, tiltX=None, tiltY=None, twist=None)
    # One contact sample per native action: identity survives, rate is null, other attributes fail.
    log['native']['packets'] = [p for p in log['native']['packets'] if p['kind'] in ('down','up')]
    d = cli('identity-only-native', log)['decision']
    assert d['rows']['H']['1']['result'] == 'pass'
    assert d['outcome'].startswith('insufficient evidence') and d['selected'] is None
    # Wrong-order, duplicates, reverse and perf overlap reject; epoch overlap currently does not.
    for name in ('wrong-order','duplicate','reversed','perf-overlap'):
        log = f.manual_log(False)
        if name == 'wrong-order': log['actions'][0]['id'] = 'A2'
        if name == 'duplicate': log['actions'][1]['id'] = 'A1'
        if name == 'reversed': log['actions'][0]['endPerf'] = -200
        if name == 'perf-overlap': log['actions'][1]['startPerf'] = 100
        cli(name, log, expected=2)
    log = f.manual_log(False)
    native_copy(log)
    log['actions'][1]['startEpochMs'] = log['actions'][0]['startEpochMs']
    d = cli('epoch-overlap-accepted', log)
    assert 1 in d['actions']['actions'][0]['native']['episodes']
    assert 1 in d['actions']['actions'][1]['native']['episodes']
    # A coalesced contact outside every action changes the supposedly A2-only row.
    log = f.manual_log(False)
    native_copy(log)
    for i,p in enumerate(log['native']['packets']):
        p['tiltX']=p['tiltY']=45 if i%2 else -45
    baseline = cli('coalescing-baseline', log)
    s = copy.deepcopy(log['web']['samples'][0])
    s.update(t=-200, handlerT=-199, coalescedCount=8, dispatch=-1)
    up = {**s, 'kind':'up', 'pressure':0, 't':-150, 'handlerT':-149}
    log['web']['samples'][0:0] = [s, up]
    altered = cli('coalescing-outside', log)
    assert altered['actions']['webOutside'] == [1]
    assert baseline['decision']['rows']['W']['6']['result'] == 'partial'
    assert altered['decision']['rows']['W']['6']['result'] == 'pass'
    assert baseline['decision']['selected']=='H' and altered['decision']['selected']=='W'
    # Split A2: a long slow episode followed by a short high-sample burst. The documented
    # largest-episode rule deliberately measures the latter, not full-action sustained rate.
    log = f.manual_log(False)
    original = [s for s in log['web']['samples'] if 6000 <= s['t'] < 11100]
    def episode(start, count, dt):
        out=[]
        for i in range(count+1):
            s=copy.deepcopy(original[i % len(original)])
            s.update(kind='down' if i==0 else 'up' if i==count else 'move', t=start+i*dt,
                     handlerT=start+i*dt+1, pressure=.5 if i<count else 0)
            out.append(s)
        return out
    log['web']['samples'] = [s for s in log['web']['samples'] if s not in original] + episode(6000, 50, 70) + episode(10000, 100, 5)
    log['web']['samples'].sort(key=lambda s:s['t'])
    d=cli('split-rate',log)
    a2=d['actions']['actions'][1]['web']
    assert a2['outcome']=='split: 2 episodes'
    assert d['web']['strokes'][a2['measured']-1]['rateHz']==200
    # Real accepted canvas fixtures: preserve all validation fields, modify only the matrix/cadence.
    base=json.loads(gzip.decompress((ROOT/'reviews/evidence/M0-C0.3-r3/canvas-auto.json.gz').read_bytes()))
    base['source']='interactive'
    base['environment']['mode']['auto']=False
    variants={}
    variants['canvas-missing-backend']=copy.deepcopy(base)
    variants['canvas-missing-backend']['runs']=[r for r in base['runs'] if r['backend']=='webgl2']
    variants['canvas-missing-scene']=copy.deepcopy(base)
    variants['canvas-missing-scene']['runs']=[r for r in base['runs'] if r['scene']!='groups1k']
    for name, backends in [('canvas-null-one',['webgpu']),('canvas-null-both',['webgpu','webgl2'])]:
        v=copy.deepcopy(base)
        for r in v['runs']:
            if r['backend'] in backends: r['frames']=r['frames'][-1:]
        variants[name]=v
    for name, v in variants.items():
        d=cli(name,v,'canvas')['decision']
        if name in ('canvas-missing-scene','canvas-null-both'):
            assert d['selected'] is None and d['outcome'].startswith('insufficient evidence')
        else:
            assert d['selected']=='webgl2' and d['provisional']
    for name,path in [('canvas-ci-windows','reviews/evidence/M0-C0.3-r3/canvas-ci-Windows-39129350790.json.gz'),
                      ('canvas-ci-macos','reviews/evidence/M0-C0.3-r3/canvas-ci-macOS-39129350790.json.gz')]:
        v=json.loads(gzip.decompress((ROOT/path).read_bytes()))
        d=cli(name,v,'canvas')['decision']
        assert d['selected']=='webgl2'
    v=copy.deepcopy(base)
    for r in v['runs']:
        if r['backend']=='webgpu': r['error']='webgpu unavailable: no adapter'
    d=cli('canvas-mac-no-adapter',v,'canvas')['decision']
    assert d['selected']=='webgl2' and not d['provisional']
    v['version']=999
    cli('canvas-malformed',v,'canvas',expected=2)
    (OUT/'decision-results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
    print('Recorded',len(results),'independent cases')

if __name__=='__main__': main()
