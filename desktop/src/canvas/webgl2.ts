// The WebGL2 back end (M4 C4.2b; ADR-0007 decisions 3 and 5: the mandatory path, also the software path on WARP or
// SwiftShader; design note 2 §1, §3). Same features as `webgpu.ts`:
// - drawing meshes uploaded as decoded (12-byte interleaved vertices), colours from a per-drawing slot texture so a
//   solid colour change needs no geometry (06 §6.3), gradients evaluated per fragment;
// - a drawing below full opacity, or shaded, is drawn to an offscreen target and composited, so overlaps inside it do
//   not darken (SI-139);
// - layers 1 to 8 render to a cached scene target reused while only the overlay changes (06 §6.9 overlay buffering);
// - MSAA when `interactiveAntialiasing` is on (06 §6.9).

import { drawFrame, type LayerOps } from './layers';
import { VERTEX_LEN, type DecodedDrawing, type Rgba as FormatRgba } from './format';
import type { Affine, ViewTris } from './geometry';
import { BackendUnavailable, type BackendStats, type FrameInput, type Renderer } from './renderer';
import { residentKey, type SceneEntry } from './scene';

const MESH_VS = `#version 300 es
layout(location = 0) in vec2 a_pos;
layout(location = 1) in uint a_slot;
uniform mat3 u_d2c;
uniform sampler2D u_slots;
flat out vec4 v_col;
out vec2 v_dpos;
void main() {
  vec3 c = u_d2c * vec3(a_pos, 1.0);
  gl_Position = vec4(c.xy, 0.0, 1.0);
  v_col = texelFetch(u_slots, ivec2(int(a_slot), 0), 0);
  v_dpos = a_pos;
}`;

const MESH_FS = `#version 300 es
precision highp float;
flat in vec4 v_col;
in vec2 v_dpos;
uniform int u_mode;
uniform mat3 u_d2u;
uniform int u_stopCount;
uniform float u_stopPos[16];
uniform vec4 u_stopCol[16];
uniform vec4 u_tint;
out vec4 o;
vec4 gradient(float t) {
  t = clamp(t, 0.0, 1.0);
  vec4 c = u_stopCol[0];
  if (t <= u_stopPos[0]) return c;
  for (int i = 1; i < 16; i++) {
    if (i >= u_stopCount) break;
    float p0 = u_stopPos[i - 1];
    float p1 = u_stopPos[i];
    if (t <= p1) return mix(u_stopCol[i - 1], u_stopCol[i], clamp((t - p0) / max(p1 - p0, 1e-6), 0.0, 1.0));
    c = u_stopCol[i];
  }
  return c;
}
void main() {
  vec4 c = v_col;
  if (u_mode != 0) {
    vec2 u = (u_d2u * vec3(v_dpos, 1.0)).xy;
    c = gradient(u_mode == 1 ? u.x : length(u - vec2(0.5)) / 0.5);
  }
  if (u_tint.a > 0.0) c.rgb = u_tint.rgb;
  o = vec4(c.rgb * c.a, c.a);
}`;

const VIEW_VS = `#version 300 es
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec4 a_col;
uniform vec2 u_size;
out vec4 v_col;
void main() {
  gl_Position = vec4(a_pos.x / u_size.x * 2.0 - 1.0, 1.0 - a_pos.y / u_size.y * 2.0, 0.0, 1.0);
  v_col = a_col;
}`;

const VIEW_FS = `#version 300 es
precision highp float;
in vec4 v_col;
out vec4 o;
void main() { o = vec4(v_col.rgb * v_col.a, v_col.a); }`;

const COMP_VS = `#version 300 es
out vec2 v_uv;
void main() {
  vec2 p = vec2(gl_VertexID == 1 ? 3.0 : -1.0, gl_VertexID == 2 ? 3.0 : -1.0);
  v_uv = (p + 1.0) * 0.5;
  gl_Position = vec4(p, 0.0, 1.0);
}`;

const COMP_FS = `#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_tex;
uniform float u_alpha;
out vec4 o;
void main() { o = texture(u_tex, v_uv) * u_alpha; }`;

/** Shade's opacity (SI-139). */
export const SHADE_OPACITY = 0.4;

interface GpuMesh {
  vao: WebGLVertexArrayObject;
  buffers: WebGLBuffer[];
  count: number;
  mode: 0 | 1 | 2;
  d2u: number[] | null;
  stopPos: Float32Array;
  stopCol: Float32Array;
  stopCount: number;
}

interface GpuDrawing {
  meshes: GpuMesh[];
  slots: WebGLTexture;
  slotIds: string[];
  slotKinds: string[];
  slotValues: Uint8Array;
  bytes: number;
}

class Target {
  fb: WebGLFramebuffer;
  rb: WebGLRenderbuffer | null = null;
  resolveFb: WebGLFramebuffer;
  tex: WebGLTexture;
  w = 0;
  h = 0;
  constructor(
    private readonly gl: WebGL2RenderingContext,
    private readonly samples: number,
  ) {
    const fb = gl.createFramebuffer();
    const resolveFb = gl.createFramebuffer();
    const tex = gl.createTexture();
    this.fb = fb;
    this.resolveFb = resolveFb;
    this.tex = tex;
  }
  ensure(w: number, h: number) {
    if (w === this.w && h === this.h) return;
    const gl = this.gl;
    this.w = w;
    this.h = h;
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.resolveFb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, this.tex, 0);
    if (this.samples > 0) {
      if (this.rb) gl.deleteRenderbuffer(this.rb);
      this.rb = gl.createRenderbuffer();
      gl.bindRenderbuffer(gl.RENDERBUFFER, this.rb);
      gl.renderbufferStorageMultisample(gl.RENDERBUFFER, this.samples, gl.RGBA8, w, h);
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.fb);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, this.rb);
    }
  }
  /** The framebuffer to draw into. */
  drawFb(): WebGLFramebuffer {
    return this.samples > 0 ? this.fb : this.resolveFb;
  }
  resolve() {
    if (this.samples === 0) return;
    const gl = this.gl;
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.fb);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this.resolveFb);
    gl.blitFramebuffer(0, 0, this.w, this.h, 0, 0, this.w, this.h, gl.COLOR_BUFFER_BIT, gl.NEAREST);
  }
  bytes(): number {
    return this.w * this.h * 4 * (1 + this.samples);
  }
  destroy() {
    const gl = this.gl;
    gl.deleteFramebuffer(this.fb);
    gl.deleteFramebuffer(this.resolveFb);
    gl.deleteTexture(this.tex);
    if (this.rb) gl.deleteRenderbuffer(this.rb);
  }
}

function program(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const shader = (type: number, src: string) => {
    const s = gl.createShader(type);
    if (!s) throw new Error('cannot create a shader');
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS))
      throw new Error(gl.getShaderInfoLog(s) ?? 'shader compile');
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, shader(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, shader(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? 'program link');
  return p;
}

/** Drawing → clip space for a view of `w` × `h` view pixels, as a column-major mat3. */
export function drawingToClip(d2v: Affine, w: number, h: number): Float32Array {
  const sx = 2 / w;
  const sy = -2 / h;
  const [a, b, c, d, e, f] = d2v;
  return new Float32Array([sx * a, sy * b, 0, sx * c, sy * d, 0, sx * e - 1, sy * f + 1, 1]);
}

const mat3 = (m: readonly number[]) =>
  new Float32Array([m[0] ?? 1, m[1] ?? 0, 0, m[2] ?? 0, m[3] ?? 1, 0, m[4] ?? 0, m[5] ?? 0, 1]);

export interface WebGL2Options {
  /** 06 §6.9 `interactiveAntialiasing` (default on): 4× MSAA. */
  antialias?: boolean;
}

export function createWebGL2(canvas: HTMLCanvasElement, opts: WebGL2Options = {}): Renderer {
  const antialias = opts.antialias ?? true;
  const gl = canvas.getContext('webgl2', {
    antialias,
    premultipliedAlpha: true,
    alpha: false,
    preserveDrawingBuffer: true,
  });
  if (!gl) throw new BackendUnavailable('webgl2', 'no WebGL2 context');
  const samples = antialias ? Math.min(4, Number(gl.getParameter(gl.MAX_SAMPLES)) || 0) : 0;
  const meshProg = program(gl, MESH_VS, MESH_FS);
  const viewProg = program(gl, VIEW_VS, VIEW_FS);
  const compProg = program(gl, COMP_VS, COMP_FS);
  const u = (p: WebGLProgram, n: string) => gl.getUniformLocation(p, n);
  const mu = {
    d2c: u(meshProg, 'u_d2c'),
    slots: u(meshProg, 'u_slots'),
    mode: u(meshProg, 'u_mode'),
    d2u: u(meshProg, 'u_d2u'),
    stopCount: u(meshProg, 'u_stopCount'),
    stopPos: u(meshProg, 'u_stopPos'),
    stopCol: u(meshProg, 'u_stopCol'),
    tint: u(meshProg, 'u_tint'),
  };
  const vu = { size: u(viewProg, 'u_size') };
  const cu = { tex: u(compProg, 'u_tex'), alpha: u(compProg, 'u_alpha') };
  const viewVao = gl.createVertexArray();
  const viewVbo = gl.createBuffer();
  const emptyVao = gl.createVertexArray();
  gl.bindVertexArray(viewVao);
  gl.bindBuffer(gl.ARRAY_BUFFER, viewVbo);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 24, 0);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(1, 4, gl.FLOAT, false, 24, 8);
  gl.bindVertexArray(null);
  const scene = new Target(gl, samples);
  const group = new Target(gl, samples);
  const drawings = new Map<string, GpuDrawing>();
  let colours: Record<string, FormatRgba> = {};
  const counters = { drawn: 0, staticRedraws: 0, staticReuses: 0 };
  let lastError: string | null = null;

  const slotTexture = (d: GpuDrawing) => {
    gl.bindTexture(gl.TEXTURE_2D, d.slots);
    const n = Math.max(1, d.slotIds.length);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, n, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, d.slotValues);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
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
    if (changed) slotTexture(d);
  };

  const meshOf = (
    vertices: Uint8Array,
    indices: Uint32Array,
    count: number,
  ): Omit<GpuMesh, 'mode' | 'd2u' | 'stopPos' | 'stopCol' | 'stopCount'> => {
    const vao = gl.createVertexArray();
    const vbo = gl.createBuffer();
    const ibo = gl.createBuffer();
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, VERTEX_LEN, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribIPointer(1, 1, gl.UNSIGNED_SHORT, VERTEX_LEN, 8);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ibo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    return { vao, buffers: [vbo, ibo], count };
  };

  const drop = (id: string) => {
    const d = drawings.get(id);
    if (!d) return;
    for (const m of d.meshes) {
      gl.deleteVertexArray(m.vao);
      for (const b of m.buffers) gl.deleteBuffer(b);
    }
    gl.deleteTexture(d.slots);
    drawings.delete(id);
  };

  const upload = (id: string, dd: DecodedDrawing) => {
    drop(id);
    const slots = gl.createTexture();
    const slotValues = new Uint8Array(Math.max(1, dd.slots.length) * 4);
    dd.slots.forEach((s, i) => {
      slotValues.set(s.rgba, i * 4);
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
      if (r.kind === 'solid') {
        if (r.indices.length === 0) continue;
        meshes.push({ ...meshOf(r.vertices, r.indices, r.indices.length), ...none });
        bytes += r.vertices.byteLength + r.indices.byteLength;
      } else if (r.kind === 'gradient') {
        if (r.indices.length === 0) continue;
        const stopPos = new Float32Array(16);
        const stopCol = new Float32Array(64);
        const n = Math.min(16, r.stops.length);
        for (let i = 0; i < n; i++) {
          const s = r.stops[i];
          if (!s) continue;
          stopPos[i] = s[0];
          stopCol.set(
            s[1].map((c) => c / 255),
            i * 4,
          );
        }
        meshes.push({
          ...meshOf(r.vertices, r.indices, r.indices.length),
          mode: r.radial ? 2 : 1,
          d2u: r.drawingToUnit,
          stopPos,
          stopCol,
          stopCount: n,
        });
        bytes += r.vertices.byteLength + r.indices.byteLength;
      } else {
        // A bitmap reference: its quad in the slot's mean colour until bitmap transport exists (SI-137).
        const v = new DataView(new ArrayBuffer(4 * VERTEX_LEN));
        for (let i = 0; i < 4; i++) {
          v.setFloat32(i * VERTEX_LEN, r.quad[i * 2] ?? 0, true);
          v.setFloat32(i * VERTEX_LEN + 4, r.quad[i * 2 + 1] ?? 0, true);
          v.setUint16(i * VERTEX_LEN + 8, r.slot, true);
        }
        meshes.push({ ...meshOf(new Uint8Array(v.buffer), new Uint32Array([0, 1, 2, 0, 2, 3]), 6), ...none });
        bytes += 4 * VERTEX_LEN + 24;
      }
    }
    const d: GpuDrawing = {
      meshes,
      slots,
      slotIds: dd.slots.map((s) => s.id),
      slotKinds: dd.slots.map((s) => s.kind),
      slotValues,
      bytes,
    };
    slotTexture(d);
    applyColours(d);
    drawings.set(id, d);
  };

  const viewTris = (t: ViewTris | null, w: number, h: number) => {
    if (!t || t.vertexCount === 0) return;
    gl.useProgram(viewProg);
    gl.uniform2f(vu.size, w, h);
    gl.bindVertexArray(viewVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, viewVbo);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(t.data), gl.STREAM_DRAW);
    gl.drawArrays(gl.TRIANGLES, 0, t.vertexCount);
    gl.bindVertexArray(null);
  };

  const composite = (tex: WebGLTexture, alpha: number) => {
    gl.useProgram(compProg);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.uniform1i(cu.tex, 0);
    gl.uniform1f(cu.alpha, alpha);
    gl.bindVertexArray(emptyVao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.bindVertexArray(null);
  };

  const drawMeshes = (d: GpuDrawing, d2c: Float32Array, tint: SceneEntry['tint']) => {
    gl.useProgram(meshProg);
    gl.uniformMatrix3fv(mu.d2c, false, d2c);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, d.slots);
    gl.uniform1i(mu.slots, 0);
    gl.uniform4f(
      mu.tint,
      tint ? tint[0] / 255 : 0,
      tint ? tint[1] / 255 : 0,
      tint ? tint[2] / 255 : 0,
      tint ? 1 : 0,
    );
    for (const m of d.meshes) {
      gl.uniform1i(mu.mode, m.mode);
      if (m.d2u) gl.uniformMatrix3fv(mu.d2u, false, mat3(m.d2u));
      gl.uniform1i(mu.stopCount, m.stopCount);
      gl.uniform1fv(mu.stopPos, m.stopPos);
      gl.uniform4fv(mu.stopCol, m.stopCol);
      gl.bindVertexArray(m.vao);
      gl.drawElements(gl.TRIANGLES, m.count, gl.UNSIGNED_INT, 0);
    }
    gl.bindVertexArray(null);
  };

  return {
    kind: 'webgl2',
    upload,
    drop,
    setColours(next) {
      colours = next;
      for (const d of drawings.values()) applyColours(d);
    },
    frame(input: FrameInput) {
      const W = gl.drawingBufferWidth;
      const H = gl.drawingBufferHeight;
      if (W < 1 || H < 1 || input.width < 1 || input.height < 1) return;
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      const d2c = drawingToClip(input.drawingToView, input.width, input.height);
      const fresh = scene.w !== W || scene.h !== H;
      scene.ensure(W, H);
      group.ensure(W, H);
      const ops: LayerOps = {
        background(c) {
          gl.bindFramebuffer(gl.FRAMEBUFFER, scene.drawFb());
          gl.viewport(0, 0, W, H);
          gl.clearColor(
            (c[0] / 255) * (c[3] / 255),
            (c[1] / 255) * (c[3] / 255),
            (c[2] / 255) * (c[3] / 255),
            c[3] / 255,
          );
          gl.clear(gl.COLOR_BUFFER_BIT);
        },
        grid() {
          viewTris(input.grid, input.width, input.height);
        },
        drawing(e) {
          const d = drawings.get(residentKey(e));
          if (!d) return;
          counters.drawn++;
          const alpha = Math.max(0, Math.min(1, e.opacity)) * (e.shade ? SHADE_OPACITY : 1);
          if (alpha >= 1) {
            drawMeshes(d, d2c, e.tint);
            return;
          }
          gl.bindFramebuffer(gl.FRAMEBUFFER, group.drawFb());
          gl.clearColor(0, 0, 0, 0);
          gl.clear(gl.COLOR_BUFFER_BIT);
          drawMeshes(d, d2c, e.tint);
          group.resolve();
          gl.bindFramebuffer(gl.FRAMEBUFFER, scene.drawFb());
          composite(group.tex, alpha);
        },
        endStatic() {
          scene.resolve();
        },
        overlay() {
          gl.bindFramebuffer(gl.FRAMEBUFFER, null);
          gl.viewport(0, 0, W, H);
          gl.disable(gl.BLEND);
          composite(scene.tex, 1);
          gl.enable(gl.BLEND);
          viewTris(input.overlay, input.width, input.height);
        },
      };
      const redraw = input.staticDirty || fresh;
      if (redraw) {
        counters.staticRedraws++;
        counters.drawn = 0;
      } else counters.staticReuses++;
      drawFrame(input.scene, ops, (id) => drawings.has(id), redraw);
    },
    stats(): BackendStats {
      let residentBytes = scene.bytes() + group.bytes();
      for (const d of drawings.values()) residentBytes += d.bytes;
      const e = gl.getError();
      if (e !== gl.NO_ERROR) lastError = `GL error 0x${e.toString(16)}`;
      return { kind: 'webgl2', residentBytes, residentDrawings: drawings.size, ...counters, lastError };
    },
    destroy() {
      for (const id of [...drawings.keys()]) drop(id);
      scene.destroy();
      group.destroy();
      gl.deleteProgram(meshProg);
      gl.deleteProgram(viewProg);
      gl.deleteProgram(compProg);
    },
  };
}
