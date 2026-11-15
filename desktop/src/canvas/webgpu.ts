// The WebGPU back end (M4 C4.2b; ADR-0007 decision 3: same features as `webgl2.ts`; design note 2 §1, §3). Meshes as
// decoded (12-byte vertices; the slot is the first half of a uint16x2), per-drawing slot textures (06 §6.3), gradients
// per fragment, group opacity through an offscreen target (SI-139), cached static layers (06 §6.9), 4× MSAA.

import { drawFrame, entryToView, type LayerOps } from './layers';
import { VERTEX_LEN, type DecodedDrawing, type Rgba as FormatRgba } from './format';
import { BackendUnavailable, type BackendStats, type FrameInput, type Renderer } from './renderer';
import { residentKey, type SceneEntry } from './scene';
import { drawingToClip, SHADE_OPACITY } from './webgl2';

/** WebGPU's API, when the page has one. */
export function webgpuApi(): GPU | null {
  return typeof navigator !== 'undefined' && 'gpu' in navigator ? navigator.gpu : null;
}

/** Samples with `display.antiAliasing` on (off: 1). */
const SAMPLES = 4;
/** Bytes of one draw's uniforms, padded to the dynamic-offset alignment. */
const DRAW_STRIDE = 512;

const MESH_WGSL = /* wgsl */ `
struct Draw {
  d2c0: vec4f, d2c1: vec4f, d2c2: vec4f,
  d2u0: vec4f, d2u1: vec4f, d2u2: vec4f,
  tint: vec4f,
  mode: u32, stopCount: u32, alpha: f32, pad: u32,
  stopPos: array<vec4f, 4>,
  stopCol: array<vec4f, 16>,
};
@group(0) @binding(0) var<uniform> u: Draw;
@group(1) @binding(0) var slots: texture_2d<f32>;
struct VOut { @builtin(position) pos: vec4f, @location(0) @interpolate(flat) col: vec4f, @location(1) dpos: vec2f };
@vertex fn vs(@location(0) p: vec2f, @location(1) slot: vec2u) -> VOut {
  var o: VOut;
  let c = mat3x3f(u.d2c0.xyz, u.d2c1.xyz, u.d2c2.xyz) * vec3f(p, 1.0);
  o.pos = vec4f(c.xy, 0.0, 1.0);
  o.col = textureLoad(slots, vec2i(i32(slot.x), 0), 0);
  o.dpos = p;
  return o;
}
fn stopPos(i: u32) -> f32 { return u.stopPos[i / 4u][i % 4u]; }
fn gradient(t0: f32) -> vec4f {
  let t = clamp(t0, 0.0, 1.0);
  var c = u.stopCol[0];
  if (t <= stopPos(0u)) { return c; }
  for (var i = 1u; i < 16u; i++) {
    if (i >= u.stopCount) { break; }
    let p0 = stopPos(i - 1u);
    let p1 = stopPos(i);
    if (t <= p1) { return mix(u.stopCol[i - 1u], u.stopCol[i], clamp((t - p0) / max(p1 - p0, 1e-6), 0.0, 1.0)); }
    c = u.stopCol[i];
  }
  return c;
}
@fragment fn fs(i: VOut) -> @location(0) vec4f {
  var c = i.col;
  if (u.mode != 0u) {
    let q = mat3x3f(u.d2u0.xyz, u.d2u1.xyz, u.d2u2.xyz) * vec3f(i.dpos, 1.0);
    var t = q.x;
    if (u.mode == 2u) { t = length(q.xy - vec2f(0.5)) / 0.5; }
    c = gradient(t);
  }
  if (u.tint.a > 0.0) { c = vec4f(u.tint.rgb, c.a); }
  return vec4f(c.rgb * c.a, c.a);
}`;

const VIEW_WGSL = /* wgsl */ `
struct V { size: vec4f };
@group(0) @binding(0) var<uniform> v: V;
struct VOut { @builtin(position) pos: vec4f, @location(0) col: vec4f };
@vertex fn vs(@location(0) p: vec2f, @location(1) c: vec4f) -> VOut {
  var o: VOut;
  o.pos = vec4f(p.x / v.size.x * 2.0 - 1.0, 1.0 - p.y / v.size.y * 2.0, 0.0, 1.0);
  o.col = c;
  return o;
}
@fragment fn fs(i: VOut) -> @location(0) vec4f { return vec4f(i.col.rgb * i.col.a, i.col.a); }`;

const COMP_WGSL = /* wgsl */ `
struct C { alpha: vec4f };
@group(0) @binding(0) var<uniform> c: C;
@group(1) @binding(0) var tex: texture_2d<f32>;
struct VOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VOut {
  var o: VOut;
  let p = vec2f(select(-1.0, 3.0, i == 1u), select(-1.0, 3.0, i == 2u));
  o.pos = vec4f(p, 0.0, 1.0);
  o.uv = vec2f((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
  return o;
}
@fragment fn fs(i: VOut) -> @location(0) vec4f {
  let d = vec2i(textureDimensions(tex));
  let xy = clamp(vec2i(i.uv * vec2f(d)), vec2i(0), d - vec2i(1));
  return textureLoad(tex, xy, 0) * c.alpha.x;
}`;

interface GpuMesh {
  vbo: GPUBuffer;
  ibo: GPUBuffer;
  count: number;
  mode: 0 | 1 | 2;
  d2u: readonly number[] | null;
  stopPos: Float32Array;
  stopCol: Float32Array;
  stopCount: number;
}

interface GpuDrawing {
  meshes: GpuMesh[];
  slots: GPUTexture;
  slotGroup: GPUBindGroup;
  slotIds: string[];
  slotKinds: string[];
  slotValues: Uint8Array;
  bytes: number;
}

class Target {
  samples = SAMPLES;
  private sized = 0;
  msaa: GPUTexture | null = null;
  resolve: GPUTexture | null = null;
  group: GPUBindGroup | null = null;
  w = 0;
  h = 0;
  constructor(
    private readonly device: GPUDevice,
    private readonly format: GPUTextureFormat,
    private readonly layout: GPUBindGroupLayout,
  ) {}
  ensure(w: number, h: number) {
    if (w === this.w && h === this.h && this.resolve && this.sized === this.samples) return;
    this.sized = this.samples;
    this.msaa?.destroy();
    this.resolve?.destroy();
    this.w = w;
    this.h = h;
    this.msaa =
      this.samples > 1
        ? this.device.createTexture({
            size: [w, h],
            format: this.format,
            sampleCount: this.samples,
            usage: GPUTextureUsage.RENDER_ATTACHMENT,
          })
        : null;
    this.resolve = this.device.createTexture({
      size: [w, h],
      format: this.format,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.group = this.device.createBindGroup({
      layout: this.layout,
      entries: [{ binding: 0, resource: this.resolve.createView() }],
    });
  }
  attachment(load: 'clear' | 'load', clear: GPUColor = [0, 0, 0, 0]): GPURenderPassColorAttachment {
    if (!this.resolve) throw new Error('target not sized');
    if (!this.msaa)
      return { view: this.resolve.createView(), loadOp: load, storeOp: 'store', clearValue: clear };
    return {
      view: this.msaa.createView(),
      resolveTarget: this.resolve.createView(),
      loadOp: load,
      storeOp: 'store',
      clearValue: clear,
    };
  }
  bytes() {
    return this.w * this.h * 4 * (1 + (this.samples > 1 ? this.samples : 0));
  }
  destroy() {
    this.msaa?.destroy();
    this.resolve?.destroy();
  }
}

export async function createWebGPU(canvas: HTMLCanvasElement): Promise<Renderer> {
  const gpu = webgpuApi();
  if (!gpu) throw new BackendUnavailable('webgpu', 'no WebGPU API');
  const adapter = await gpu.requestAdapter();
  if (!adapter) throw new BackendUnavailable('webgpu', 'no adapter');
  const device = await adapter.requestDevice();
  const context = canvas.getContext('webgpu');
  if (!context) throw new BackendUnavailable('webgpu', 'no WebGPU canvas context');
  const format = gpu.getPreferredCanvasFormat();
  context.configure({
    device,
    format,
    alphaMode: 'opaque',
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  const premul: GPUBlendState = {
    color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
    alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
  };
  const uniformLayout = device.createBindGroupLayout({
    entries: [
      {
        binding: 0,
        visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
        buffer: { type: 'uniform', hasDynamicOffset: true },
      },
    ],
  });
  const textureLayout = device.createBindGroupLayout({
    entries: [
      {
        binding: 0,
        visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
        texture: { sampleType: 'float' },
      },
    ],
  });
  const meshModule = device.createShaderModule({ code: MESH_WGSL });
  const viewModule = device.createShaderModule({ code: VIEW_WGSL });
  const compModule = device.createShaderModule({ code: COMP_WGSL });
  // Pipelines carry their sample count: one set per count (display.antiAliasing on: 4, off: 1), built when first used.
  const pipelineSets = new Map<
    number,
    {
      mesh: GPURenderPipeline;
      view: GPURenderPipeline;
      compBlend: GPURenderPipeline;
      compCopy: GPURenderPipeline;
    }
  >();
  const pipelines = (count: number) => {
    const have = pipelineSets.get(count);
    if (have) return have;
    const meshPipeline = device.createRenderPipeline({
      layout: device.createPipelineLayout({ bindGroupLayouts: [uniformLayout, textureLayout] }),
      vertex: {
        module: meshModule,
        entryPoint: 'vs',
        buffers: [
          {
            arrayStride: VERTEX_LEN,
            attributes: [
              { shaderLocation: 0, offset: 0, format: 'float32x2' },
              { shaderLocation: 1, offset: 8, format: 'uint16x2' },
            ],
          },
        ],
      },
      fragment: { module: meshModule, entryPoint: 'fs', targets: [{ format, blend: premul }] },
      multisample: { count },
    });
    const viewPipeline = device.createRenderPipeline({
      layout: device.createPipelineLayout({ bindGroupLayouts: [uniformLayout] }),
      vertex: {
        module: viewModule,
        entryPoint: 'vs',
        buffers: [
          {
            arrayStride: 24,
            attributes: [
              { shaderLocation: 0, offset: 0, format: 'float32x2' },
              { shaderLocation: 1, offset: 8, format: 'float32x4' },
            ],
          },
        ],
      },
      fragment: { module: viewModule, entryPoint: 'fs', targets: [{ format, blend: premul }] },
      multisample: { count },
    });
    const compPipeline = (blend: boolean) =>
      device.createRenderPipeline({
        layout: device.createPipelineLayout({ bindGroupLayouts: [uniformLayout, textureLayout] }),
        vertex: { module: compModule, entryPoint: 'vs' },
        fragment: {
          module: compModule,
          entryPoint: 'fs',
          targets: blend ? [{ format, blend: premul }] : [{ format }],
        },
        multisample: { count },
      });
    const compBlend = compPipeline(true);
    const compCopy = compPipeline(false);
    const set = { mesh: meshPipeline, view: viewPipeline, compBlend, compCopy };
    pipelineSets.set(count, set);
    return set;
  };
  let samples = SAMPLES;
  /** The canvas pass's samples (display.smoothToolOverlays): the overlay is drawn there. */
  let overlaySamples = SAMPLES;

  let uniforms = device.createBuffer({
    size: DRAW_STRIDE * 64,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });
  let uniformGroup = device.createBindGroup({
    layout: uniformLayout,
    entries: [{ binding: 0, resource: { buffer: uniforms, size: DRAW_STRIDE } }],
  });
  let staging = new Float32Array((DRAW_STRIDE * 64) / 4);
  let viewBuffer = device.createBuffer({
    size: 1 << 16,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });

  const scene = new Target(device, format, textureLayout);
  const group = new Target(device, format, textureLayout);
  let canvasMsaa: GPUTexture | null = null;
  const drawings = new Map<string, GpuDrawing>();
  let colours: Record<string, FormatRgba> = {};
  const counters = { drawn: 0, staticRedraws: 0, staticReuses: 0 };
  let lastError: string | null = null;
  device.addEventListener('uncapturederror', (e) => {
    lastError = e.error.message;
  });

  const writeSlots = (d: GpuDrawing) => {
    device.queue.writeTexture({ texture: d.slots }, d.slotValues, { bytesPerRow: d.slotValues.byteLength }, [
      Math.max(1, d.slotIds.length),
      1,
    ]);
  };
  const applyColours = (d: GpuDrawing) => {
    let changed = false;
    for (const [i, id] of d.slotIds.entries()) {
      const c = d.slotKinds[i] === 'solid' ? colours[id] : undefined;
      if (!c) continue;
      for (let k = 0; k < 4; k++) {
        if (d.slotValues[i * 4 + k] !== c[k]) {
          d.slotValues[i * 4 + k] = c[k] ?? 0;
          changed = true;
        }
      }
    }
    if (changed) writeSlots(d);
  };
  const buffer = (data: Uint8Array | Uint32Array, usage: number) => {
    const size = Math.max(4, Math.ceil(data.byteLength / 4) * 4);
    const b = device.createBuffer({ size, usage: usage | GPUBufferUsage.COPY_DST, mappedAtCreation: true });
    new Uint8Array(b.getMappedRange()).set(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    b.unmap();
    return b;
  };
  const drop = (id: string) => {
    const d = drawings.get(id);
    if (!d) return;
    for (const m of d.meshes) {
      m.vbo.destroy();
      m.ibo.destroy();
    }
    d.slots.destroy();
    drawings.delete(id);
  };
  const upload = (id: string, dd: DecodedDrawing) => {
    drop(id);
    const n = Math.max(1, dd.slots.length);
    const slotValues = new Uint8Array(n * 4);
    dd.slots.forEach((s, i) => {
      slotValues.set(s.rgba, i * 4);
    });
    const slots = device.createTexture({
      size: [n, 1],
      format: 'rgba8unorm',
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    const slotGroup = device.createBindGroup({
      layout: textureLayout,
      entries: [{ binding: 0, resource: slots.createView() }],
    });
    const meshes: GpuMesh[] = [];
    let bytes = slotValues.byteLength;
    const none = {
      mode: 0 as const,
      d2u: null,
      stopPos: new Float32Array(16),
      stopCol: new Float32Array(64),
      stopCount: 0,
    };
    for (const r of dd.records) {
      if (r.kind === 'bitmap') {
        const v = new DataView(new ArrayBuffer(4 * VERTEX_LEN));
        for (let i = 0; i < 4; i++) {
          v.setFloat32(i * VERTEX_LEN, r.quad[i * 2] ?? 0, true);
          v.setFloat32(i * VERTEX_LEN + 4, r.quad[i * 2 + 1] ?? 0, true);
          v.setUint16(i * VERTEX_LEN + 8, r.slot, true);
        }
        meshes.push({
          vbo: buffer(new Uint8Array(v.buffer), GPUBufferUsage.VERTEX),
          ibo: buffer(new Uint32Array([0, 1, 2, 0, 2, 3]), GPUBufferUsage.INDEX),
          count: 6,
          ...none,
        });
        bytes += 4 * VERTEX_LEN + 24;
        continue;
      }
      if (r.indices.length === 0) continue;
      const base = {
        vbo: buffer(r.vertices, GPUBufferUsage.VERTEX),
        ibo: buffer(r.indices, GPUBufferUsage.INDEX),
        count: r.indices.length,
      };
      bytes += r.vertices.byteLength + r.indices.byteLength;
      if (r.kind === 'solid') {
        meshes.push({ ...base, ...none });
      } else {
        const stopPos = new Float32Array(16);
        const stopCol = new Float32Array(64);
        const k = Math.min(16, r.stops.length);
        for (let i = 0; i < k; i++) {
          const s = r.stops[i];
          if (!s) continue;
          stopPos[i] = s[0];
          stopCol.set(
            s[1].map((c) => c / 255),
            i * 4,
          );
        }
        meshes.push({
          ...base,
          mode: r.radial ? 2 : 1,
          d2u: r.drawingToUnit,
          stopPos,
          stopCol,
          stopCount: k,
        });
      }
    }
    const d: GpuDrawing = {
      meshes,
      slots,
      slotGroup,
      slotIds: dd.slots.map((s) => s.id),
      slotKinds: dd.slots.map((s) => s.kind),
      slotValues,
      bytes,
    };
    writeSlots(d);
    applyColours(d);
    drawings.set(id, d);
  };

  return {
    kind: 'webgpu',
    upload,
    drop,
    setColours(next) {
      colours = next;
      for (const d of drawings.values()) applyColours(d);
    },
    setAntialias(on) {
      samples = on ? SAMPLES : 1;
    },
    setSmoothOverlays(on) {
      overlaySamples = on ? SAMPLES : 1;
    },
    finish() {
      return device.queue.onSubmittedWorkDone();
    },
    frame(input: FrameInput) {
      const W = canvas.width;
      const H = canvas.height;
      if (W < 1 || H < 1 || input.width < 1 || input.height < 1) return;
      scene.samples = samples;
      group.samples = samples;
      scene.ensure(W, H);
      group.ensure(W, H);
      const pl = pipelines(samples);
      if (
        samples > 1 &&
        (canvasMsaa?.width !== W || canvasMsaa.height !== H || canvasMsaa.sampleCount !== overlaySamples)
      ) {
        canvasMsaa?.destroy();
        canvasMsaa = device.createTexture({
          size: [W, H],
          format,
          sampleCount: overlaySamples,
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });
      }
      const d2c = drawingToClip(input.drawingToView, input.width, input.height);
      // One uniform slot per draw this frame; written once before submit.
      let slot = 0;
      // The grid's and the overlay's triangles, written to the view buffer before any pass is encoded.
      const pad = (n: number) => Math.ceil(n / 256) * 256;
      const gridData = input.grid && input.grid.vertexCount > 0 ? new Float32Array(input.grid.data) : null;
      const overlayData =
        input.overlay && input.overlay.vertexCount > 0 ? new Float32Array(input.overlay.data) : null;
      const gridAt = 0;
      const overlayAt = pad(gridData?.byteLength ?? 0);
      const need = overlayAt + pad(overlayData?.byteLength ?? 0);
      if (need > viewBuffer.size) {
        viewBuffer.destroy();
        viewBuffer = device.createBuffer({
          size: Math.max(need, viewBuffer.size * 2),
          usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
      }
      if (gridData) device.queue.writeBuffer(viewBuffer, gridAt, gridData);
      if (overlayData) device.queue.writeBuffer(viewBuffer, overlayAt, overlayData);
      const ensureUniforms = (n: number) => {
        if ((n + 1) * DRAW_STRIDE <= uniforms.size) return;
        uniforms.destroy();
        const size = Math.max(uniforms.size * 2, (n + 1) * DRAW_STRIDE);
        uniforms = device.createBuffer({ size, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
        uniformGroup = device.createBindGroup({
          layout: uniformLayout,
          entries: [{ binding: 0, resource: { buffer: uniforms, size: DRAW_STRIDE } }],
        });
        const grown = new Float32Array(size / 4);
        grown.set(staging);
        staging = grown;
      };
      const nextSlot = (fill: (f: Float32Array) => void) => {
        ensureUniforms(slot);
        const f = staging.subarray((slot * DRAW_STRIDE) / 4, ((slot + 1) * DRAW_STRIDE) / 4);
        f.fill(0);
        fill(f);
        return slot++ * DRAW_STRIDE;
      };
      const encoder = device.createCommandEncoder();
      let pass: GPURenderPassEncoder | null = null;
      const begin = (a: GPURenderPassColorAttachment) => {
        pass?.end();
        pass = encoder.beginRenderPass({ colorAttachments: [a] });
      };
      const viewTris = (data: Float32Array | null, offset: number, set = pl) => {
        if (!data || !pass) return;
        const u = nextSlot((f) => {
          f[0] = input.width;
          f[1] = input.height;
        });
        pass.setPipeline(set.view);
        pass.setBindGroup(0, uniformGroup, [u]);
        pass.setVertexBuffer(0, viewBuffer, offset, data.byteLength);
        pass.draw(data.length / 6);
      };
      const composite = (t: Target, alpha: number, blend: boolean, set = pl) => {
        if (!pass || !t.group) return;
        const u = nextSlot((f) => {
          f[0] = alpha;
        });
        pass.setPipeline(blend ? set.compBlend : set.compCopy);
        pass.setBindGroup(0, uniformGroup, [u]);
        pass.setBindGroup(1, t.group);
        pass.draw(3);
      };
      const drawMeshes = (d: GpuDrawing, d2c: Float32Array, tint: SceneEntry['tint']) => {
        if (!pass) return;
        pass.setPipeline(pl.mesh);
        pass.setBindGroup(1, d.slotGroup);
        for (const m of d.meshes) {
          const u = nextSlot((f) => {
            f.set(
              [
                d2c[0] ?? 0,
                d2c[1] ?? 0,
                d2c[2] ?? 0,
                0,
                d2c[3] ?? 0,
                d2c[4] ?? 0,
                d2c[5] ?? 0,
                0,
                d2c[6] ?? 0,
                d2c[7] ?? 0,
                d2c[8] ?? 1,
                0,
              ],
              0,
            );
            const g = m.d2u ?? [1, 0, 0, 1, 0, 0];
            f.set([g[0] ?? 1, g[1] ?? 0, 0, 0, g[2] ?? 0, g[3] ?? 1, 0, 0, g[4] ?? 0, g[5] ?? 0, 1, 0], 12);
            if (tint) f.set([tint[0] / 255, tint[1] / 255, tint[2] / 255, 1], 24);
            new Uint32Array(f.buffer, f.byteOffset + 28 * 4, 2).set([m.mode, m.stopCount]);
            f[30] = 1;
            f.set(m.stopPos, 32);
            f.set(m.stopCol, 48);
          });
          pass.setBindGroup(0, uniformGroup, [u]);
          pass.setVertexBuffer(0, m.vbo);
          pass.setIndexBuffer(m.ibo, 'uint32');
          pass.drawIndexed(m.count);
        }
      };
      const fresh = scene.w !== W || scene.h !== H;
      // Consecutive drawings at the same reduced opacity share one offscreen pass and one composite (SI-139).
      let groupAlpha: number | null = null;
      const flushGroup = () => {
        if (groupAlpha === null) return;
        begin(scene.attachment('load'));
        composite(group, groupAlpha, true);
        groupAlpha = null;
      };
      const ops: LayerOps = {
        background(c) {
          const a = c[3] / 255;
          begin(scene.attachment('clear', [(c[0] / 255) * a, (c[1] / 255) * a, (c[2] / 255) * a, a]));
        },
        grid() {
          flushGroup();
          viewTris(gridData, gridAt);
        },
        drawing(e) {
          const d = drawings.get(residentKey(e));
          if (!d) return;
          counters.drawn++;
          // A Camera-view occurrence through its own matrix (camera-view note §6); a Drawing-view entry through d2c.
          const m = e.matrix
            ? drawingToClip(
                entryToView(e, input.cameraToView, input.drawingToView),
                input.width,
                input.height,
              )
            : d2c;
          const alpha = Math.max(0, Math.min(1, e.opacity)) * (e.shade ? SHADE_OPACITY : 1);
          if (alpha >= 1) {
            flushGroup();
            drawMeshes(d, m, e.tint);
            return;
          }
          if (groupAlpha !== alpha) {
            flushGroup();
            begin(group.attachment('clear'));
            groupAlpha = alpha;
          }
          drawMeshes(d, m, e.tint);
        },
        endStatic() {
          flushGroup();
          pass?.end();
          pass = null;
        },
        overlay() {
          const current = context.getCurrentTexture().createView();
          begin(
            overlaySamples > 1 && canvasMsaa
              ? {
                  view: canvasMsaa.createView(),
                  resolveTarget: current,
                  loadOp: 'clear',
                  storeOp: 'store',
                  clearValue: [0, 0, 0, 1],
                }
              : { view: current, loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] },
          );
          const po = pipelines(overlaySamples);
          composite(scene, 1, false, po);
          viewTris(overlayData, overlayAt, po);
        },
      };
      const redraw = input.staticDirty || fresh;
      if (redraw) {
        counters.staticRedraws++;
        counters.drawn = 0;
      } else counters.staticReuses++;
      drawFrame(input.scene, ops, (id) => drawings.has(id), redraw);
      (pass as GPURenderPassEncoder | null)?.end();
      device.queue.writeBuffer(uniforms, 0, staging, 0, (slot * DRAW_STRIDE) / 4);
      device.queue.submit([encoder.finish()]);
    },
    stats(): BackendStats {
      let residentBytes = scene.bytes() + group.bytes();
      for (const d of drawings.values()) residentBytes += d.bytes;
      return { kind: 'webgpu', residentBytes, residentDrawings: drawings.size, ...counters, lastError };
    },
    destroy() {
      for (const id of [...drawings.keys()]) drop(id);
      scene.destroy();
      group.destroy();
      canvasMsaa?.destroy();
      uniforms.destroy();
      viewBuffer.destroy();
      device.destroy();
    },
  };
}
