// Evidence-only browser harness. Imports unmodified renderers; not shipped in the app.
import { certifyRenderList } from '../../../desktop/src/ipc/binaryEnvelope';
import { decodeRenderList } from '../../../desktop/src/spikes/canvas/decode';
import { createWebGL2 } from '../../../desktop/src/spikes/canvas/webgl2';
import { createWebGPU } from '../../../desktop/src/spikes/canvas/webgpu';
import { animateGroups, cameraAt } from '../../../desktop/src/spikes/canvas/camera';

const raf = () => new Promise<number>(requestAnimationFrame);
const result: any = { timeOrigin: performance.timeOrigin, userAgent: navigator.userAgent, runs: [] };
(window as any).runProbe = async () => {
  const buffer = await (await fetch('http://127.0.0.1:1439/paths100k.ttrl')).arrayBuffer();
  const header = certifyRenderList(buffer);
  const base = decodeRenderList(header, buffer);
  for (const backend of ['webgpu', 'webgl2']) for (const multiple of [1, 2]) {
    const canvas = document.createElement('canvas'); canvas.width = 1904; canvas.height = 757;
    canvas.style.width = '952px'; canvas.style.height = '378.5px';
    document.body.replaceChildren(canvas);
    let draws = 0, instances = 0;
    const gpu: any = navigator.gpu;
    const oldAdapter = gpu?.requestAdapter.bind(gpu);
    if (gpu && backend === 'webgpu') gpu.requestAdapter = async (...args: any[]) => {
      const adapter = await oldAdapter(...args);
      if (!adapter) return adapter;
      const requestDevice = adapter.requestDevice.bind(adapter);
      adapter.requestDevice = async (...deviceArgs: any[]) => {
        const device = await requestDevice(...deviceArgs);
        const createEncoder = device.createCommandEncoder.bind(device);
        device.createCommandEncoder = (...encArgs: any[]) => {
          const encoder = createEncoder(...encArgs);
          const beginPass = encoder.beginRenderPass.bind(encoder);
          encoder.beginRenderPass = (...passArgs: any[]) => {
            const pass = beginPass(...passArgs); const draw = pass.draw.bind(pass);
            pass.draw = (...drawArgs: any[]) => { draws++; instances += drawArgs[1]; return draw(...drawArgs); };
            return pass;
          };
          return encoder;
        };
        return device;
      };
      return adapter;
    };
    const glGet = canvas.getContext.bind(canvas);
    (canvas as any).getContext = (kind: string, options: any) => {
      const ctx: any = glGet(kind as any, options);
      if (kind === 'webgl2' && ctx) {
        const draw = ctx.drawArraysInstanced.bind(ctx);
        ctx.drawArraysInstanced = (...args: any[]) => { draws++; instances += args[3]; return draw(...args); };
      }
      return ctx;
    };
    const geometry = { ...base, strokes: base.strokes.slice(0), strokeCount: base.strokeCount * multiple };
    if (multiple === 2) {
      const bytes = new Uint8Array(base.strokes.byteLength * 2);
      bytes.set(new Uint8Array(base.strokes)); bytes.set(new Uint8Array(base.strokes), base.strokes.byteLength);
      geometry.strokes = bytes.buffer;
    }
    let renderer: any;
    const row: any = { backend, multiple, header, decoded: base.stats, drawnSegments: geometry.strokeCount, frames: [] };
    result.runs.push(row);
    try {
      renderer = backend === 'webgpu' ? await createWebGPU(canvas) : createWebGL2(canvas);
      row.renderer = renderer.info; row.upload = await renderer.upload(geometry);
      const groups = new Float32Array(base.groups.length / 6 * 8);
      animateGroups(base.groups, groups, 0, false);
      renderer.frame(cameraAt(0,1904,757),groups);
      await new Promise(r => setTimeout(r, 500));
      const start = await raf(); const pending = [];
      for (;;) {
        const t = await raf(); const timing = renderer.frame(cameraAt(t-start,1904,757),groups);
        const sample: any = { t, cpuMs: timing.cpuMs, gpuMs: null }; row.frames.push(sample);
        pending.push(timing.gpuDone.then((n: any) => { sample.gpuMs = n; }));
        if(t-start>=10000) break;
      }
      await Promise.all(pending);
      row.drawCalls = draws; row.totalInstances = instances;
      if(backend==='webgl2') row.glError = (canvas.getContext('webgl2') as any).getError();
      row.snapshot = canvas.toDataURL('image/jpeg',0.8);
    } catch (e) { row.error = String(e); } finally { renderer?.destroy(); if (gpu && oldAdapter) gpu.requestAdapter=oldAdapter; }
  }
  await fetch('http://127.0.0.1:1439/save', {method:'POST',body:JSON.stringify(result)});
  return result.runs.map((r:any) => ({backend:r.backend,multiple:r.multiple,frames:r.frames.length,error:r.error,draws:r.drawCalls,segments:r.drawnSegments,renderer:r.renderer}));
};
document.body.textContent = 'C0.3 independent renderer headroom probe';
setTimeout(() => { void (window as any).runProbe(); }, 1000);
