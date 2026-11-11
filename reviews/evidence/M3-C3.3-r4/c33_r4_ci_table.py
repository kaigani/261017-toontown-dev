from pathlib import Path
import json,collections
E=Path('reviews/evidence/M3-C3.3-r4');r=json.loads((E/'ci-attribution.json').read_text())
lines=['# CI budget attribution','', 'Durations from authenticated run metadata and downloaded logs. Test durations are harness wall time per binary, excluding nested summaries, which are retained separately in JSON; compile durations come from Cargo finish lines.','', '| Windows check step | 42915011320 s | 42938979758 s | Delta s |','|---|---:|---:|---:|']
old=next(j for j in r['old']['jobs'] if j['name']=='check (windows-latest)');new=next(j for j in r['new']['jobs'] if j['name']=='check (windows-latest)')
for a,b in zip(old['steps'],new['steps']):
 assert a['name']==b['name']
 lines.append(f"| {a['name']} | {a['seconds']:.0f} | {b['seconds']:.0f} | {b['seconds']-a['seconds']:+.0f} |")
lines += ['', '| Job | Old Windows | New Windows | Old macOS | New macOS |','|---|---:|---:|---:|---:|']
def fmt(t): return f'{int(t)//60}m{int(t)%60:02d}s'
for name in ['check','c33 render release','review properties release','history oracles release']:
 vals=[]
 for os in ['windows','macos']:
  for tag in ['old','new']:
   vals.append(fmt(next(j['seconds'] for j in r[tag]['jobs'] if j['name']==f'{name} ({os}-latest)')))
 lines.append('| '+name+' | '+' | '.join(vals)+' |')
lines += ['', '## Windows per-binary test harness durations', '', 'Repeated filenames are disambiguated by occurrence in execution order; raw rows retain step labels. A missing occurrence is shown as absent, not zero.', '', '| Binary occurrence | Old seconds | New seconds | Delta |','|---|---:|---:|---:|']
by={}
for tag in ['old','new']:
 counts=collections.Counter(); v={}
 for row in r[tag]['binaries']['check (windows-latest)']:
  binary=row['binary'].replace('\\','/');counts[binary]+=1;key=f'{binary} #{counts[binary]}'
  v[key]=row['seconds']
 by[tag]=v
for key in sorted(set(by['old'])|set(by['new'])):
 a=by['old'].get(key);b=by['new'].get(key)
 lines.append(f'| {key} | {a if a is not None else "absent"} | {b if b is not None else "absent"} | {round(b-a,2) if a is not None and b is not None else "new/absent"} |')
(E/'ci-attribution.md').write_text('\n'.join(lines)+'\n', encoding='utf-8')
print('\n'.join(lines[:50]))
