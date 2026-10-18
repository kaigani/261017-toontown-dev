"""Independent raw-sample audit; never trusts a saved simulation-match summary as its calculation."""
from pathlib import Path
import gzip
import json
import math
import statistics

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent

def pct(values,p):
    values=sorted(values)
    return values[max(0,math.ceil(len(values)*p/100)-1)] if values else None

def audit(path):
    log=json.loads(gzip.decompress(path.read_bytes()))
    strokes=[]
    for p in log['web']['samples']:
        if p['kind']=='down': strokes.append([p])
        elif strokes and p['kind']=='move' and p['pressure']>0: strokes[-1].append(p)
    result=dict(file=str(path.relative_to(ROOT)),version=log['version'],nativePackets=len(log['native']['packets']),
                injectionTimingExported=any('report' in s for s in log['simulations']),strokes=[])
    for stroke in strokes:
        points=[p for p in stroke if p['kind']=='move']
        received={round(p['pressure']*1024) for p in points}
        intervals=[b['t']-a['t'] for a,b in zip(points,points[1:])]
        dispatch={}
        for p in points: dispatch.setdefault(p['dispatch'],[]).append(p)
        distinct=sorted({p['pressure'] for p in points})
        exact=0
        for p in points:
            i=round(p['pressure']*1024)-1
            # Rust f64::round rounds half-integers away from zero, unlike Python's round.
            rust_round=lambda v: math.floor(v+.5) if v>=0 else math.ceil(v-.5)
            if p['tiltX']==rust_round(-60+120*i/1024) and p['tiltY']==rust_round(60-120*i/1024): exact+=1
        # Infer a rectangle using the generator and require it to predict EVERY received x/y, including down.
        # This is a geometry reconstruction, not proof of which OS/browser layer dropped stationary samples.
        fits=[]
        for height in range(240,550):
            p=stroke[0]; i=round(p['pressure']*1024)-1
            mid=p['y']-math.sin(i/1024*math.tau*3)*(height//3)
            for top in range(math.floor(mid-height/2)-1,math.ceil(mid-height/2)+2):
                xy=[(math.floor(16+i/1024*996+.5),math.floor(top+height/2+math.sin(i/1024*math.tau*3)*(height//3)+.5)) for i in range(1024)]
                if all(xy[round(p['pressure']*1024)-1]==(p['x'],p['y']) for p in stroke):
                    fits.append(dict(left=8,width=1012,top=top,height=height,
                                     stationaryFrameIds=[i+1 for i in range(1,1024) if xy[i]==xy[i-1]]))
        summary=dict(moveSamples=len(points),receivedMoveFrameIds=len(received),
                     receivedIncludingDown=len({round(p['pressure']*1024) for p in stroke}),
                     missingMoveFrameIds=sorted(set(range(1,1025))-received),pointerTypes=sorted({p['pointerType'] for p in stroke}),
                     exactTilt=exact,rateHz=(len(points)-1)*1000/(points[-1]['t']-points[0]['t']),
                     intervalMedian=statistics.median(intervals),intervalMax=max(intervals),
                     pressureDistinct=len(distinct),pressureLevelsImplied=round(1/min(b-a for a,b in zip(distinct,distinct[1:]))),
                     tiltX=[min(p['tiltX'] for p in points),max(p['tiltX'] for p in points)],
                     tiltY=[min(p['tiltY'] for p in points),max(p['tiltY'] for p in points)],
                     twistNonzero=sum(p['twist']!=0 for p in points),
                     coalescedP50=pct([v[0]['coalescedCount'] for v in dispatch.values()],50),
                     coalescedMax=max(v[0]['coalescedCount'] for v in dispatch.values()),
                     oldestSampleHandlerP50=pct([v[0]['handlerT']-v[0]['t'] for v in dispatch.values()],50),
                     oldestSampleHandlerP95=pct([v[0]['handlerT']-v[0]['t'] for v in dispatch.values()],95),
                     latestSampleHandlerP50=pct([v[-1]['handlerT']-v[-1]['t'] for v in dispatch.values()],50),
                     perSampleHandlerP50=pct([p['handlerT']-p['t'] for p in points],50),
                     geometryFits=fits)
        result['strokes'].append(summary)
    return result

if __name__=='__main__':
    paths=[ROOT/f'reviews/evidence/M0-C0.2/tablet-spike-windows-runner-run{i}.json.gz' for i in [1,2]]
    paths.append(OUT/'tablet-spike-final-ci.json.gz')
    results=[audit(p) for p in paths]
    (OUT/'independent-log-audit.json').write_text(json.dumps(results,indent=2),encoding='utf8')
    for r in results:
        print(r['file'])
        for s in r['strokes']: print({k:s[k] for k in ['receivedMoveFrameIds','receivedIncludingDown','exactTilt','rateHz','geometryFits']})
