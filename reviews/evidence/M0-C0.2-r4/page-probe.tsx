// Evidence harness: run-page-probe.py temporarily installs this in desktop/src/test.
import { writeFileSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { gunzipSync } from 'node:zlib';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { createHostApi, createMockTransport, HostProvider } from '../ipc';
import { TabletSpike } from '../spikes/tablet/TabletSpike';

const output = resolve(process.cwd(), '../reviews/evidence/M0-C0.2-r4');
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.useRealTimers(); });
function setup() {
  vi.useFakeTimers();
  vi.stubGlobal('PointerEvent', class extends Event {});
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  const transport = createMockTransport();
  const invoke = transport.invoke.bind(transport);
  transport.invoke = (command, args) => {
    if (command === 'spike_environment') return Promise.resolve({os:'macos',arch:'aarch64',osVersion:'test double',webviewVersion:'test double',cpuCount:8,window:null});
    if (command === 'spike_save_log') {
      writeFileSync(resolve(output, 'manual-page-log.json'), JSON.stringify(args?.['log'], null, 2));
      return Promise.resolve({path:'manual-page-log.json',bytes:1});
    }
    return invoke(command, args);
  };
  return transport;
}

it('current Mac-mode page saves metadata, paused strokes and pose-only packets; contextmenu preserves pointerup', async () => {
  const transport = setup();
  let send: ((message: unknown) => void) | undefined;
  transport.invokeWithChannel = (_command, _args, _name, onMessage) => {
    send = onMessage;
    return Promise.resolve({backend:'appkit-nsevent',tablets:[],timestampGranularityMs:null,deviceClock:'test double',window:null});
  };
  render(<HostProvider api={createHostApi(transport)}><TabletSpike mode={{name:'tablet',logPath:'test.json',auto:false,penSimulation:false,nativeBackend:'appkit-nsevent'}} /></HostProvider>);
  await act(async () => { fireEvent.click(screen.getByText('Start native listener')); });
  const fields = ['Computer','macOS / Windows version','Safari / WebKit version','Tablet model','Tablet driver version','Notes'];
  for (const field of fields) fireEvent.change(screen.getByLabelText(field), {target:{value:`R4 ${field} fixture (not hardware)`}});
  const canvas = screen.getByTestId('spike-canvas');
  Object.assign(canvas, {setPointerCapture: () => undefined});
  for (const start of [100, 100100]) {
    for (let i=0;i<6;i++) {
      const kind = i===0 ? 'pointerdown' : i===5 ? 'pointerup' : 'pointermove';
      const event = new Event(kind, {cancelable:true});
      Object.assign(event, {pointerId:7,pointerType:'pen',pressure:i===5?0:(i+1)/10,tiltX:i%2?-30:30,tiltY:i%2?-45:45,twist:i*30,clientX:i,clientY:1});
      Object.defineProperty(event,'timeStamp',{value:start+i*4});
      canvas.dispatchEvent(event);
      if (i===4) {
        const menu = new MouseEvent('contextmenu',{cancelable:true,clientX:4,clientY:1});
        canvas.dispatchEvent(menu);
        expect(menu.defaultPrevented).toBe(true);
      }
    }
    await act(async () => {
      send?.({hostDropped:0,sendFailures:0,packets:[0,.1,.5,1,0].map((pressure,i)=>({kind:'pose',pressure,deviceMs:start+i*4,hostEpochMs:1700000000000+start+i*4,x:i,y:1,tiltX:i%2?-30:30,tiltY:i%2?-45:45,rawTiltX:i%2?-.5:.5,rawTiltY:i%2?-.707:.707,twist:0,tool:'pen',seq:(start===100?0:5)+i,deviceId:'fixture',windowNumber:1}))});
    });
  }
  await act(async () => { fireEvent.click(screen.getByText('Save log')); });
  const saved = JSON.parse(readFileSync(resolve(output, 'manual-page-log.json'), 'utf8'));
  expect(saved.source).toBe('manual');
  expect(saved.environment.mode.auto).toBe(false);
  expect(saved.web.samples.filter((s:any)=>s.kind==='up')).toHaveLength(2);
  expect(saved.native.packets).toHaveLength(10);
  expect(saved.web.contextMenus).toHaveLength(2);
  expect(saved.metadata.tablet).toContain('fixture');
});

it('auto keeps first refusal and retry, then records geometry refusals without injection', async () => {
  const transport = setup();
  let top = 58;
  vi.spyOn(HTMLCanvasElement.prototype,'getBoundingClientRect').mockImplementation(()=>new DOMRect(8,top,1012,311));
  const invoke = transport.invoke.bind(transport);
  let attempts = 0;
  let saved: any;
  transport.invoke = (command,args) => {
    if (command==='spike_pen_simulate') {
      attempts++;
      if (attempts===2) top=500;
      return Promise.reject({code:'spike.notForeground',message:'fixture covering window'});
    }
    if (command==='spike_save_log') { saved=args?.['log']; return Promise.resolve({path:'test.json',bytes:1}); }
    return invoke(command,args);
  };
  render(<HostProvider api={createHostApi(transport)}><TabletSpike mode={{name:'tablet',logPath:'test.json',auto:true,penSimulation:true,nativeBackend:'octotablet-ink'}} /></HostProvider>);
  await act(async()=>{await vi.advanceTimersByTimeAsync(30000);});
  expect(attempts).toBe(2);
  expect(saved.source).toBe('auto');
  expect(saved.runs).toHaveLength(11);
  expect(saved.runs[0].error).toContain('spike.notForeground');
  expect(saved.runs[1].label).toContain('(retry)');
  expect(saved.runs[1].error).toContain('spike.notForeground');
  expect(saved.runs.slice(2).every((r:any)=>r.error.startsWith('spike.geometryChanged'))).toBe(true);
  writeFileSync(resolve(output, 'auto-refusal-page-log.json'), JSON.stringify(saved,null,2));
});

for (const auto of [false, true]) {
  it(`source stays ${auto ? 'auto' : 'interactive'} across pending Save, completed injection, manual drawing and Clear`, async () => {
    const transport = setup();
    vi.spyOn(HTMLCanvasElement.prototype,'getBoundingClientRect').mockReturnValue(new DOMRect(8,58,1012,311));
    const originalInvoke = transport.invoke.bind(transport);
    const saves: any[] = [];
    let finish: ((value: unknown) => void) | undefined;
    let calls = 0;
    transport.invoke = (command,args) => {
      if (command==='spike_pen_simulate') {
        calls++;
        if (calls===1) return new Promise(resolve => { finish=resolve; });
        return Promise.reject({code:'fixture.refusal',message:'remaining scenarios deliberately refused'});
      }
      if (command==='spike_probe_native_window') return Promise.reject({code:'fixture.refusal',message:'native probe deliberately refused'});
      if (command==='spike_save_log') {
        saves.push(JSON.parse(JSON.stringify(args?.['log'])));
        return Promise.resolve({path:'fixture.json',bytes:1});
      }
      return originalInvoke(command,args);
    };
    render(<HostProvider api={createHostApi(transport)}><TabletSpike mode={{name:'tablet',logPath:'fixture.json',auto,penSimulation:true,nativeBackend:'octotablet-ink'}} /></HostProvider>);
    const saveAs = async (suffix: string) => {
      await act(async()=>{fireEvent.click(screen.getByText('Save log'));});
      const saved=saves[saves.length-1];
      writeFileSync(resolve(output,`${auto?'auto':'interactive'}-${suffix}-page-log.json`),JSON.stringify(saved,null,2));
      return saved;
    };
    const before=await saveAs('before');
    expect(before.source).toBe(auto?'auto':'manual');
    expect(before.runs).toHaveLength(0);
    if (auto) await act(async()=>{await vi.advanceTimersByTimeAsync(1600);});
    else await act(async()=>{fireEvent.click(screen.getByText('Simulate pen 240 Hz (synthetic)'));});
    expect(calls).toBe(1);
    const during=await saveAs('pending');
    expect(during.source).toBe(auto?'auto':'interactive');
    expect(during.runs).toHaveLength(1);
    expect(during.runs[0].report).toBeNull();
    // Replay one real historical report through the mocked host; no DOM delivery is simulated.
    // This remains a page test double, never a new hardware or latency measurement.
    const report=JSON.parse(gunzipSync(readFileSync(resolve(output,'../M0-C0.2/tablet-spike-windows-runner-r2-matrix5.json.gz'))).toString()).runs[0].report;
    await act(async()=>{finish?.(report); await vi.advanceTimersByTimeAsync(auto?20000:600);});
    const complete=await saveAs('complete');
    expect(complete.source).toBe(auto?'auto':'interactive');
    expect(complete.simulations).toHaveLength(1);
    const canvas=screen.getByTestId('spike-canvas');
    Object.assign(canvas,{setPointerCapture:()=>undefined});
    for (const [kind,pressure] of [['pointerdown',0.5],['pointerup',0]] as const) {
      const e=new Event(kind);
      Object.assign(e,{pointerId:9,pointerType:'pen',pressure,tiltX:0,tiltY:0,twist:0,clientX:15,clientY:65});
      canvas.dispatchEvent(e);
    }
    const drawn=await saveAs('after-drawing');
    expect(drawn.source).toBe(auto?'auto':'interactive');
    expect(drawn.web.samples).toHaveLength(2);
    expect(drawn.runs.length).toBeGreaterThan(0);
    await act(async()=>{fireEvent.click(screen.getByText('Clear'));});
    const cleared=await saveAs('cleared');
    expect(cleared.source).toBe(auto?'auto':'manual');
    expect(cleared.runs).toHaveLength(0);
    expect(cleared.simulations).toHaveLength(0);
    expect(cleared.web.samples).toHaveLength(0);
  });
}
