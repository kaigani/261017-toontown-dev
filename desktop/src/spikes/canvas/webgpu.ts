// C0.3 canvas spike (spike code): WebGPU back end (used when available unless `--spike-renderer webgl2`).
//
// WebGPU is newer than the Safari 15 floor, so it is reached through feature detection only, and the usage flags
// are written as their numeric values (WebGPU spec, `GPUBufferUsage` / `GPUTextureUsage`).
import type { Camera } from './camera';
import { FILL_STRIDE, STROKE_STRIDE, type Geometry } from './decode';
import type { FrameTiming, Renderer, UploadTiming } from './renderer';

const BUFFER_INDEX = 0x10;
const BUFFER_VERTEX = 0x20;
const BUFFER_UNIFORM = 0x40;
const BUFFER_STORAGE = 0x80;
const BUFFER_COPY_DST = 0x08;
const TEXTURE_COPY_DST = 0x02;
const TEXTURE_BINDING = 0x04;
const TEXTURE_RENDER_ATTACHMENT = 0x10;
const SAMPLES = 4;

const WGSL = /* wgsl */ `
struct U { cam: vec4f, view: vec4f };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> groups: array<vec4f>;

fn colour(c: u32) -> vec4f { let v = unpack4x8unorm(c); return vec4f(v.rgb * v.a, v.a); }
fn toWorld(p: vec2f, g: u32) -> vec3f {
  let m0 = groups[g * 2u];
  let m1 = groups[g * 2u + 1u];
  let gs = sqrt(abs(m0.x * m0.w - m0.y * m0.z));
  return vec3f(m0.x * p.x + m0.z * p.y + m1.x, m0.y * p.x + m0.w * p.y + m1.y, gs);
}
fn toClip(px: vec2f) -> vec4f { return vec4f(px.x / u.view.x * 2.0 - 1.0, 1.0 - px.y / u.view.y * 2.0, 0.0, 1.0); }

struct S { @builtin(position) pos: vec4f, @location(0) col: vec4f, @location(1) across: f32, @location(2) half_w: f32 };

@vertex fn strokeVs(@builtin(vertex_index) v: u32, @location(0) seg: vec4f, @location(1) hw: f32,
                    @location(2) col: u32, @location(3) g: u32) -> S {
  let wa = toWorld(seg.xy, g);
  let wb = toWorld(seg.zw, g);
  let a = wa.xy * u.cam.x + u.cam.yz;
  let b = wb.xy * u.cam.x + u.cam.yz;
  let d = b - a;
  let len = length(d);
  var dir = vec2f(1.0, 0.0);
  if (len > 1e-6) { dir = d / len; }
  let nrm = vec2f(-dir.y, dir.x);
  let h = hw * wa.z * u.cam.x;
  let hh = max(h, 0.5) + 1.0;
  var along = 0.0;
  if (v == 1u || v == 4u || v == 5u) { along = 1.0; }
  var across = -1.0;
  if (v == 2u || v == 3u || v == 5u) { across = 1.0; }
  let p = mix(a - dir * hh, b + dir * hh, along) + nrm * across * hh;
  var o: S;
  o.pos = toClip(p);
  o.col = colour(col) * min(h / 0.5, 1.0);
  o.across = across * hh;
  o.half_w = max(h, 0.5);
  return o;
}
@fragment fn strokeFs(i: S) -> @location(0) vec4f {
  return i.col * clamp(i.half_w + 0.5 - abs(i.across), 0.0, 1.0);
}

struct F { @builtin(position) pos: vec4f, @location(0) col: vec4f };
@vertex fn fillVs(@location(0) p: vec2f, @location(2) col: u32, @location(3) g: u32) -> F {
  let w = toWorld(p, g);
  var o: F;
  o.pos = toClip(w.xy * u.cam.x + u.cam.yz);
  o.col = colour(col);
  return o;
}
@fragment fn fillFs(i: F) -> @location(0) vec4f { return i.col; }
`;

interface MaybeGpuNavigator {
  gpu?: GPU;
}

/** The WebGPU interface if this webview exposes it. */
export function webgpuApi(): GPU | null {
  return (navigator as unknown as MaybeGpuNavigator).gpu ?? null;
}

/** Creates the WebGPU back end; throws with the step that failed (no API, no adapter, no context). */
export async function createWebGPU(canvas: HTMLCanvasElement): Promise<Renderer> {
  const gpu = webgpuApi();
  if (!gpu) throw new Error('webgpu unavailable: navigator.gpu is absent');
  const adapter = await gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) throw new Error('webgpu unavailable: navigator.gpu.requestAdapter() returned null');
  const device = await adapter.requestDevice({
    requiredLimits: {
      maxBufferSize: adapter.limits.maxBufferSize,
      maxStorageBufferBindingSize: adapter.limits.maxStorageBufferBindingSize,
    },
  });
  const context = canvas.getContext('webgpu');
  if (!context) throw new Error('webgpu unavailable: canvas.getContext("webgpu") returned null');
  const format = gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: 'opaque' });
  const adapterInfo = adapter.info;
  const info: Record<string, unknown> = {
    vendor: adapterInfo.vendor,
    architecture: adapterInfo.architecture,
    device: adapterInfo.device,
    description: adapterInfo.description,
    format,
    samples: SAMPLES,
    maxBufferSize: adapter.limits.maxBufferSize,
  };

  const module = device.createShaderModule({ code: WGSL });
  const blend: GPUBlendState = {
    color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
    alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
  };
  const layout = device.createBindGroupLayout({
    entries: [
      { binding: 0, visibility: 1, buffer: { type: 'uniform' } }, // VERTEX stage
      { binding: 1, visibility: 1, buffer: { type: 'read-only-storage' } },
    ],
  });
  const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] });
  const target: GPUColorTargetState = { format, blend };
  const strokePipeline = device.createRenderPipeline({
    layout: pipelineLayout,
    vertex: {
      module,
      entryPoint: 'strokeVs',
      buffers: [
        {
          arrayStride: STROKE_STRIDE,
          stepMode: 'instance',
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x4' },
            { shaderLocation: 1, offset: 16, format: 'float32' },
            { shaderLocation: 2, offset: 20, format: 'uint32' },
            { shaderLocation: 3, offset: 24, format: 'uint32' },
          ],
        },
      ],
    },
    fragment: { module, entryPoint: 'strokeFs', targets: [target] },
    primitive: { topology: 'triangle-list' },
    multisample: { count: SAMPLES },
  });
  const fillPipeline = device.createRenderPipeline({
    layout: pipelineLayout,
    vertex: {
      module,
      entryPoint: 'fillVs',
      buffers: [
        {
          arrayStride: FILL_STRIDE,
          stepMode: 'vertex',
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x2' },
            { shaderLocation: 2, offset: 8, format: 'uint32' },
            { shaderLocation: 3, offset: 12, format: 'uint32' },
          ],
        },
      ],
    },
    fragment: { module, entryPoint: 'fillFs', targets: [target] },
    primitive: { topology: 'triangle-list' },
    multisample: { count: SAMPLES },
  });

  const uniforms = device.createBuffer({ size: 32, usage: BUFFER_UNIFORM | BUFFER_COPY_DST });
  let strokeBuf: GPUBuffer | null = null;
  let fillBuf: GPUBuffer | null = null;
  let indexBuf: GPUBuffer | null = null;
  let groupBuf: GPUBuffer | null = null;
  let bindGroup: GPUBindGroup | null = null;
  let strokeCount = 0;
  let indexCount = 0;
  let msaa: GPUTexture | null = null;
  let pixelTex: GPUTexture | null = null;

  const makeBuffer = (data: ArrayBuffer | ArrayBufferView, usage: number): GPUBuffer => {
    const size = Math.max(16, Math.ceil(data.byteLength / 4) * 4);
    const buffer = device.createBuffer({ size, usage: usage | BUFFER_COPY_DST });
    if (data.byteLength > 0) device.queue.writeBuffer(buffer, 0, data as ArrayBuffer);
    return buffer;
  };

  return {
    backend: 'webgpu',
    info,
    async upload(geometry: Geometry): Promise<UploadTiming> {
      const t0 = performance.now();
      strokeBuf = makeBuffer(geometry.strokes, BUFFER_VERTEX);
      fillBuf = makeBuffer(geometry.fillVertices, BUFFER_VERTEX);
      indexBuf = makeBuffer(geometry.fillIndices, BUFFER_INDEX);
      groupBuf = device.createBuffer({
        size: Math.max(32, (geometry.groups.length / 6) * 32),
        usage: BUFFER_STORAGE | BUFFER_COPY_DST,
      });
      bindGroup = device.createBindGroup({
        layout,
        entries: [
          { binding: 0, resource: { buffer: uniforms } },
          { binding: 1, resource: { buffer: groupBuf } },
        ],
      });
      strokeCount = geometry.strokeCount;
      indexCount = geometry.fillIndices.length;
      const cpuMs = performance.now() - t0;
      await device.queue.onSubmittedWorkDone();
      return { cpuMs, completeMs: performance.now() - t0 };
    },
    frame(camera: Camera, groups: Float32Array): FrameTiming {
      const t0 = performance.now();
      if (msaa?.width !== camera.width || msaa.height !== camera.height) {
        msaa?.destroy();
        msaa = device.createTexture({
          size: [camera.width, camera.height],
          sampleCount: SAMPLES,
          format,
          usage: TEXTURE_RENDER_ATTACHMENT,
        });
      }
      device.queue.writeBuffer(
        uniforms,
        0,
        new Float32Array([
          camera.scale,
          camera.offsetX,
          camera.offsetY,
          0,
          camera.width,
          camera.height,
          0,
          0,
        ]),
      );
      if (groupBuf) device.queue.writeBuffer(groupBuf, 0, groups);
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: msaa.createView(),
            resolveTarget: context.getCurrentTexture().createView(),
            clearValue: { r: 1, g: 1, b: 1, a: 1 },
            loadOp: 'clear',
            storeOp: 'discard',
          },
        ],
      });
      if (bindGroup) {
        pass.setBindGroup(0, bindGroup);
        if (fillBuf && indexBuf && indexCount > 0) {
          pass.setPipeline(fillPipeline);
          pass.setVertexBuffer(0, fillBuf);
          pass.setIndexBuffer(indexBuf, 'uint32');
          pass.drawIndexed(indexCount);
        }
        if (strokeBuf && strokeCount > 0) {
          pass.setPipeline(strokePipeline);
          pass.setVertexBuffer(0, strokeBuf);
          pass.draw(6, strokeCount);
        }
      }
      pass.end();
      device.queue.submit([encoder.finish()]);
      const submitted = performance.now();
      return {
        cpuMs: submitted - t0,
        gpuDone: device.queue.onSubmittedWorkDone().then(() => performance.now() - submitted),
      };
    },
    async uploadPixels(rgba: Uint8Array, width: number, height: number): Promise<number> {
      const t0 = performance.now();
      if (pixelTex?.width !== width || pixelTex.height !== height) {
        pixelTex?.destroy();
        pixelTex = device.createTexture({
          size: [width, height],
          format: 'rgba8unorm',
          usage: TEXTURE_COPY_DST | TEXTURE_BINDING,
        });
      }
      device.queue.writeTexture(
        { texture: pixelTex },
        rgba as Uint8Array<ArrayBuffer>,
        { bytesPerRow: width * 4 },
        [width, height],
      );
      await device.queue.onSubmittedWorkDone();
      return performance.now() - t0;
    },
    destroy() {
      for (const b of [strokeBuf, fillBuf, indexBuf, groupBuf, uniforms]) b?.destroy();
      msaa?.destroy();
      pixelTex?.destroy();
      device.destroy();
    },
  };
}
