"""Scoped closure checks; synthetic data, never physical tablet evidence.

R/D have no capture adapter: ranking cases call the real aggregate/print functions.
Raw-sample/action/key cases use the real analyzer CLI, retaining input and output.
"""
import contextlib
import copy
import gzip
import importlib.util
import io
import json
import pathlib
import subprocess
import unittest

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT/path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

t = load('tablet', 'scripts/analyse-tablet-log.py')
f = load('fixtures', 'scripts/test_decision_rules.py')
results = []

def cli(name, log):
    path = OUT/(name+'.json.gz')
    path.write_bytes(gzip.compress(json.dumps(log).encode(),mtime=0))
    target = OUT/(name+'.analysis.json')
    target.unlink(missing_ok=True)
    p = subprocess.run(['python','-X','utf8','scripts/analyse-tablet-log.py',str(path),'--json',str(target)],
                       cwd=ROOT,capture_output=True)
    (OUT/(name+'.log.gz')).write_bytes(gzip.compress(p.stdout+p.stderr,mtime=0))
    analysis = json.loads(target.read_bytes()) if target.exists() else None
    results.append(dict(case=name, boundary='real CLI', exit=p.returncode,
                        decision=analysis['decision'] if analysis else None))
    if p.returncode == 0:
        assert p.stdout.count(b'### Decision (') == 1
        assert p.stdout.count(b'- outcome: **') == 1
        assert analysis['decision']['outcome']
    return p.returncode, analysis

def ranked(name, platform, rows):
    d = t.aggregate(platform,rows)
    d.update(platform=platform,rows={c:{str(r):{'result':v,'note':'injected row result'}
                                       for r,v in values.items()} for c,values in rows.items()})
    text = io.StringIO()
    with contextlib.redirect_stdout(text):
        t.print_decision(d)
    assert text.getvalue().count('### Decision (') == 1
    assert text.getvalue().count('- outcome: **') == 1
    for c in d.get('disqualified',{}):
        assert d['shares'][c] in text.getvalue()
        assert f"'{c}': {d['disqualified'][c]}" in text.getvalue()
    (OUT/(name+'.txt')).write_text(text.getvalue(),encoding='utf-8')
    results.append(dict(case=name,boundary='real aggregate/print; injected row results',inputs=rows,decision=d))
    return d

def native_copy(log):
    log['native']['packets'] = [dict(kind={'move':'pose'}.get(s['kind'],s['kind']),tool='pen',
         pressure=s['pressure'],hostEpochMs=1e12+s['t'],tiltX=s['tiltX'],tiltY=s['tiltY'],twist=s['twist'])
         for s in log['web']['samples']]
    log['nativeInfo'] = {'backend':'test-native'}

class Ranking(unittest.TestCase):
    def test_raw_macos_example_2(self):
        log=f.manual_log(True)
        log['environment']['userAgent']='Mozilla/5.0 (Macintosh; Intel Mac OS X 12_0)'
        for i,s in enumerate(log['web']['samples']):
            if s['pressure']>0: s['pressure']=(i%1023+1)/1024
            s.update(tiltX=-45 if i%2 else 45,tiltY=-45 if i%2 else 45,twist=i%360,coalescedCount=2)
        log['web']['samples']=[s for s in log['web']['samples'] if not 6000<=s['t']<11100 or s['t']<=6000+240*8.33]
        a2=[s for s in log['web']['samples'] if 6000<=s['t']<11100]
        for i,s in enumerate(a2):
            s['t']=6000+i*20
            s['handlerT']=s['t']+1
        log['web']['samples'].append({**a2[-1],'kind':'up','pressure':0,'t':10820})
        log['web']['samples'].sort(key=lambda q:q['t'])
        native_copy(log)
        for p in log['native']['packets']:
            if 1e12+6000<=p['hostEpochMs']<=1e12+10820:
                p['hostEpochMs']=1e12+6000+(p['hostEpochMs']-1e12-6000)*50/90
        status,d=cli('raw-macos-example-2',log)
        self.assertEqual(status,0)
        decision=d['decision']
        self.assertEqual(decision['platform'],'macos')
        self.assertEqual(decision['disqualified'],{'W':[7]})
        self.assertEqual(decision['selected'],'A')
        self.assertEqual(decision['shares'],{'W':'7/9','A':'3/4'})

    def test_examples_and_adversarial_shares(self):
        ok = {r:'pass' for r in range(1,10)}
        na = {r:'n/a' for r in range(1,10)}
        cases = [
          ('example-1','windows',{'W':{**ok,2:'partial',3:'fail',4:'fail'},'R':{**ok,4:'fail',6:'n/a'},
           'H':{**ok,2:'fail',3:'fail',6:'n/a'},'D':{**ok,2:'fail',3:'fail',4:'fail',6:'n/a'}},'R','go-with-conditions: R'),
          ('example-2','macos',{'W':{**ok,7:'fail',8:'fail'},'A':{**ok,6:'n/a',7:'partial',8:'fail'}},'A','go-with-conditions: A'),
          ('example-3','macos',{'W':{**ok,2:'fail'},'A':{**ok,6:'n/a',7:'fail'}},None,'no-go on this platform'),
          ('all-disqualified-windows','windows',{c:{**ok,2:'fail',7:'fail',6:'n/a' if c!='W' else 'pass'} for c in ('W','R','H','D')},None,'no-go on this platform'),
          # 6/9 == 4/6 exactly, despite different pass counts; W wins the tie.
          ('equal-share-different-counts','windows',{'W':{**ok,3:'partial',4:'partial',5:'partial'},
           'R':{**ok,4:'n/a',5:'n/a',6:'n/a',8:'partial',9:'partial'}},'W','go-with-conditions: W'),
          ('single-applicable-row','windows',{'W':{**ok,9:'partial'},'R':{**na,2:'pass'}},'R','go: R'),
          ('native-eight-of-eight','windows',{'W':{**ok,9:'partial'},'R':{**ok,6:'n/a'}},'R','go: R'),
          ('no-applicable-rows','windows',{'W':{**ok,9:'partial'},'R':na},'W','go-with-conditions: W'),
          ('all-na','windows',{'W':na,'R':na},None,'not evidence'),
          ('only-qualified-rate-partial','windows',{'W':{**ok,2:'fail'},'H':{**ok,6:'n/a',7:'partial',8:'fail'}},'H','go-with-conditions: H'),
          ('mac-tie-different-counts','macos',{'W':ok,'A':{**ok,6:'n/a'}},'W','go: W'),
          ('mac-native-share-wins','macos',{'W':{**ok,8:'partial'},'A':{**ok,6:'n/a'}},'A','go: A'),
          ('native-priority-tie','windows',{c:{**ok,6:'n/a'} for c in ('D','H','R')},'R','go: R'),
        ]
        for name,platform,rows,winner,outcome in cases:
            with self.subTest(case=name):
                d = ranked(name,platform,rows)
                self.assertEqual(d['selected'],winner)
                self.assertTrue(d['outcome'].startswith(outcome),d)
                self.assertNotIn(winner,d.get('disqualified',{}))
                for c,fails in d.get('disqualified',{}).items():
                    self.assertIn(c,d['shares'])
                    self.assertEqual(fails,[r for r in (2,7) if rows[c][r]=='fail'])
                if outcome.startswith('no-go'):
                    self.assertEqual(d['bestEvidence'],'W')
                    self.assertIn('escalate (plan §6 decision 7)',d['outcome'])
                if name=='equal-share-different-counts':
                    self.assertEqual(d['shares'],{'W':'2/3','R':'2/3'})
                    self.assertEqual(d['scores'],{'W':6,'R':4})
                if name=='only-qualified-rate-partial':
                    self.assertEqual([(c['row'],c['result']) for c in d['conditions']],[(7,'partial'),(8,'fail')])

class Actions(unittest.TestCase):
    def test_independent_clock_overlap_rejects(self):
        for clock in ('Perf','EpochMs'):
            with self.subTest(clock=clock):
                log=f.manual_log(False)
                native_copy(log)
                log['actions'][1]['start'+clock]=log['actions'][0]['start'+clock]
                status,_=cli('overlap-'+clock,log)
                self.assertEqual(status,2)

    def test_single_boundary_episode_filters_every_statistic(self):
        log=f.manual_log(False)
        end=log['actions'][1]['endPerf']
        a2=[s for s in log['web']['samples'] if 6000<=s['t']<11100]
        for i,s in enumerate(a2):
            s['t']=end-1 if i==0 else end+i/10
            s['coalescedCount']=1 if i==0 else 8
        status,d=cli('boundary-one-inside',log)
        self.assertEqual(status,0)
        stats=d['actions']['actions'][1]['web']['stats']
        self.assertEqual(stats['samples'],1)
        self.assertFalse(stats['coalescedObserved'])
        self.assertIsNone(stats['rateHz'])
        self.assertEqual(d['actions']['excludedSamples']['web'],600)
        self.assertEqual(d['decision']['rows']['W']['6']['result'],'partial')
        self.assertEqual(d['decision']['rows']['W']['7']['result'],'insufficient')

    def test_outside_tail_cannot_choose_measured_episode(self):
        baseline=f.manual_log(False)
        # A2 has a real 600-sample episode, plus a stray down just inside its ending boundary.
        # Growing only that stray contact's OUTSIDE tail must not alter the measured episode or candidate score.
        s=copy.deepcopy(baseline['web']['samples'][0])
        s.update(t=11099,handlerT=11100,dispatch=90000,coalescedCount=1)
        up={**s,'t':11100,'kind':'up','pressure':0}
        baseline['web']['samples'].extend([s,up])
        baseline['web']['samples'].sort(key=lambda q:q['t'])
        contaminated=copy.deepcopy(baseline)
        contaminated['web']['samples'].remove(up)
        contaminated['web']['samples'].extend([{**s,'t':11100+i,'kind':'move','dispatch':90001,
            'coalescedCount':8,'pressure':.8} for i in range(700)])
        contaminated['web']['samples'].append({**up,'t':11800})
        contaminated['web']['samples'].sort(key=lambda q:q['t'])
        _,a=cli('split-boundary-baseline',baseline)
        _,b=cli('split-boundary-outside-tail',contaminated)
        # Also retain a two-in-span-sample variant: the outside tail now changes go to no-go, not just repeat.
        slow=copy.deepcopy(baseline)
        slow['web']['samples']=[{**q,'t':11074} if q['t']==11099 else q for q in slow['web']['samples']]
        slow['web']['samples'].append({**s,'kind':'move','t':11099})
        slow['web']['samples'].sort(key=lambda q:q['t'])
        slow_tail=copy.deepcopy(slow)
        slow_tail['web']['samples'].remove(up)
        slow_tail['web']['samples'].extend([{**s,'t':11100+i,'kind':'move','dispatch':90001,
            'coalescedCount':8,'pressure':.8} for i in range(700)])
        slow_tail['web']['samples'].append({**up,'t':11800})
        slow_tail['web']['samples'].sort(key=lambda q:q['t'])
        _,c=cli('split-slow-baseline',slow)
        _,d=cli('split-slow-outside-tail',slow_tail)
        self.assertEqual(c['decision']['outcome'],d['decision']['outcome'],
                         'Outside-span tail must not change a qualified candidate into a platform no-go')
        self.assertEqual(a['decision']['rows'],b['decision']['rows'],
                         'Outside-span tail must not select a different A2 episode')
        self.assertEqual(a['decision']['outcome'],b['decision']['outcome'])

    def test_malformed_key_sequences_reject_even_with_matching_transition_count(self):
        base=f.manual_log(False)
        keys=[]
        for a in base['actions']:
            transitions=[('k',a['startPerf'],'skipped')] if a['state']=='skipped' else [
                (' ',a['startPerf'],'started'),(' ',a['endPerf'],'done')]
            for key,when,state in transitions:
                keys.extend([dict(type='keydown',key=key,repeat=False,t=when,outcome='pressed'),
                             dict(type='keyup',key=key,repeat=False,t=when,outcome=f"transition: {a['id']} {state}")])
        base['keyEvents']=keys
        self.assertEqual(cli('keys-valid',base)[0],0)
        for mutation in ('missing-keydown','499ms-transitions','wrong-action-label','transition-on-keydown'):
            with self.subTest(mutation=mutation):
                log=copy.deepcopy(base)
                if mutation=='missing-keydown':
                    log['keyEvents']=[e for e in log['keyEvents'] if e['type']=='keyup']
                elif mutation=='499ms-transitions':
                    for i,e in enumerate(log['keyEvents']): e['t']=(i//2)*499
                elif mutation=='wrong-action-label':
                    log['keyEvents'][1]['outcome']='transition: A6 skipped'
                else:
                    log['keyEvents'][1]['type']='keydown'
                self.assertEqual(cli('keys-'+mutation,log)[0],2)

if __name__=='__main__':
    try:
        unittest.main(verbosity=2)
    finally:
        (OUT/'closure-results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
