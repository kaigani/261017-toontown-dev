"""Literal decision-table counterexamples, not new hardware measurements."""
import json
from pathlib import Path

def mac_table(pen, pressure, tilt, hz, coalesced, native_pressure, native_tilt, native_pen):
    rows=[]
    if pen and pressure and tilt and hz>=200 and coalesced:
        rows.append('W')
    if not (pen and pressure and tilt) and native_pen and native_pressure and native_tilt:
        rows.append('A')
    if not pressure and not native_pressure:
        rows.append('no-go')
    return rows

def canvas_table(gl, gpu=None, adr=False):
    rows=[]
    if all(x>=57 for x in gl) and (gpu is None or all(x>=57 for x in gpu)):
        rows.append('go')
    if gl[0]<57 and gl[1]>=57 and gl[2]>=57:
        rows.append('go with culling')
    if gl[1]<57 or (not adr and gl[2]<57):
        rows.append('native-wgpu prototype')
    return rows

cases = [
    {'id':'mac-rate-only-miss','input':'DOM pen/pressure/tilt; 120 Hz; coalescing present; native has all attributes',
     'matched_rows':mac_table(True,True,True,120,True,True,True,True)},
    {'id':'mac-no-coalescing','input':'DOM pen/pressure/tilt; 240 Hz; no coalescing API; native has all attributes',
     'matched_rows':mac_table(True,True,True,240,False,True,True,True)},
    {'id':'mac-partial-native','input':'DOM and native pressure/pen but neither has tilt',
     'matched_rows':mac_table(True,True,False,240,True,True,False,True)},
    {'id':'canvas-brush-only-adr','input':'WebGL2 paths/groups/brush = 60/60/40 fps; no WebGPU adapter',
     'report_rows':canvas_table([60,60,40]),'adr_rows':canvas_table([60,60,40],adr=True)},
    {'id':'canvas-slow-webgpu','input':'WebGL2 60/60/60; WebGPU 60/40/60 with adapter',
     'report_rows':canvas_table([60,60,60],[60,40,60]),'adr_rows':canvas_table([60,60,60],[60,40,60],adr=True)},
]
assert all(not c['matched_rows'] for c in cases[:3])
assert cases[3]['report_rows'] and not cases[3]['adr_rows']
assert not cases[4]['report_rows'] and not cases[4]['adr_rows']
Path(__file__).with_name('decision-counterexamples.json').write_text(json.dumps(cases,indent=2)+'\n',encoding='utf-8')
print('5 decision-table counterexamples reproduced (documentation predicates only)')
