// C0.3 canvas spike (spike code): WebGL2 back end. Mandatory path (WKWebView before WebGPU, plan §2.3), so it is
// measured on its own as well (`--spike-renderer webgl2`).
import type { Camera } from './camera';
import { FILL_STRIDE, STROKE_STRIDE, type Geometry } from './decode';
import type { FrameTiming, Renderer, UploadTiming } from './renderer';

const COMMON = `#version 300 es
precision highp float;
precision highp int;
uniform vec4 uCam;   // scale, offsetX, offsetY, 0  (world -> device px)
uniform vec2 uView;  // device px
uniform highp sampler2D uGroups; // 2 texels per group: (a, b, c, d), (e, f, 0, 0)
vec4 unpackColour(uint c) {
  vec4 v = vec4(float(c & 255u), float((c >> 8) & 255u), float((c >> 16) & 255u), float(c >> 24)) / 255.0;
  return vec4(v.rgb * v.a, v.a);
}
vec2 toWorld(vec2 p, uint g, out float gscale) {
  ivec2 t = ivec2(int(g) * 2, 0);
  vec4 m0 = texelFetch(uGroups, t, 0);
  vec4 m1 = texelFetch(uGroups, t + ivec2(1, 0), 0);
  gscale = sqrt(abs(m0.x * m0.w - m0.y * m0.z));
  return vec2(m0.x * p.x + m0.z * p.y + m1.x, m0.y * p.x + m0.w * p.y + m1.y);
}
vec4 toClip(vec2 px) { return vec4(px.x / uView.x * 2.0 - 1.0, 1.0 - px.y / uView.y * 2.0, 0.0, 1.0); }
`;

const STROKE_VS = `${COMMON}
layout(location = 0) in vec4 aSeg;
layout(location = 1) in float aHalf;
layout(location = 2) in uint aColour;
layout(location = 3) in uint aGroup;
out vec4 vColour;
out float vAcross;
out float vHalf;
void main() {
  float gs;
  vec2 a = toWorld(aSeg.xy, aGroup, gs) * uCam.x + uCam.yz;
  vec2 b = toWorld(aSeg.zw, aGroup, gs) * uCam.x + uCam.yz;
  vec2 d = b - a;
  float len = length(d);
  vec2 dir = len > 1e-6 ? d / len : vec2(1.0, 0.0);
  vec2 nrm = vec2(-dir.y, dir.x);
  float h = aHalf * gs * uCam.x;
  float hh = max(h, 0.5) + 1.0;
  int v = gl_VertexID;
  float along = (v == 1 || v == 4 || v == 5) ? 1.0 : 0.0;
  float across = (v == 2 || v == 3 || v == 5) ? 1.0 : -1.0;
  vec2 pos = mix(a - dir * hh, b + dir * hh, along) + nrm * across * hh;
  vColour = unpackColour(aColour) * min(h / 0.5, 1.0);
  vAcross = across * hh;
  vHalf = max(h, 0.5);
  gl_Position = toClip(pos);
}`;

const STROKE_FS = `#version 300 es
precision highp float;
in vec4 vColour;
in float vAcross;
in float vHalf;
out vec4 outColour;
void main() {
  float cov = clamp(vHalf + 0.5 - abs(vAcross), 0.0, 1.0);
  outColour = vColour * cov;
}`;

const FILL_VS = `${COMMON}
layout(location = 0) in vec2 aPos;
layout(location = 2) in uint aColour;
layout(location = 3) in uint aGroup;
out vec4 vColour;
void main() {
  float gs;
  vec2 p = toWorld(aPos, aGroup, gs) * uCam.x + uCam.yz;
  vColour = unpackColour(aColour);
  gl_Position = toClip(p);
}`;

const FILL_FS = `#version 300 es
precision highp float;
in vec4 vColour;
out vec4 outColour;
void main() { outColour = vColour; }`;

function compile(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const program = gl.createProgram();
  for (const [type, src] of [
    [gl.VERTEX_SHADER, vs],
    [gl.FRAGMENT_SHADER, fs],
  ] as const) {
    const shader = gl.createShader(type);
    if (!shader) throw new Error('createShader failed');
    gl.shaderSource(shader, src);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      throw new Error(`shader: ${gl.getShaderInfoLog(shader) ?? 'compile failed'}`);
    }
    gl.attachShader(program, shader);
  }
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(`program: ${gl.getProgramInfoLog(program) ?? 'link failed'}`);
  }
  return program;
}

export function createWebGL2(canvas: HTMLCanvasElement): Renderer | null {
  const gl = canvas.getContext('webgl2', {
    antialias: true,
    alpha: false,
    premultipliedAlpha: true,
    powerPreference: 'high-performance',
    preserveDrawingBuffer: false,
  });
  if (!gl) return null;
  const strokeProgram = compile(gl, STROKE_VS, STROKE_FS);
  const fillProgram = compile(gl, FILL_VS, FILL_FS);
  const debug = gl.getExtension('WEBGL_debug_renderer_info');
  const info: Record<string, unknown> = {
    vendor: gl.getParameter(gl.VENDOR) as unknown,
    renderer: gl.getParameter(gl.RENDERER) as unknown,
    version: gl.getParameter(gl.VERSION) as unknown,
    unmaskedVendor: debug ? (gl.getParameter(debug.UNMASKED_VENDOR_WEBGL) as unknown) : null,
    unmaskedRenderer: debug ? (gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) as unknown) : null,
    maxTextureSize: gl.getParameter(gl.MAX_TEXTURE_SIZE) as unknown,
    samples: gl.getParameter(gl.SAMPLES) as unknown,
    antialias: gl.getContextAttributes()?.antialias ?? null,
  };

  const strokeVao = gl.createVertexArray();
  const fillVao = gl.createVertexArray();
  const strokeBuf = gl.createBuffer();
  const fillBuf = gl.createBuffer();
  const indexBuf = gl.createBuffer();
  const groupTex = gl.createTexture();
  const pixelTex = gl.createTexture();
  let strokeCount = 0;
  let indexCount = 0;
  let groupCount = 0;

  const uniforms = (program: WebGLProgram) => ({
    cam: gl.getUniformLocation(program, 'uCam'),
    view: gl.getUniformLocation(program, 'uView'),
    groups: gl.getUniformLocation(program, 'uGroups'),
  });
  const strokeU = uniforms(strokeProgram);
  const fillU = uniforms(fillProgram);

  return {
    backend: 'webgl2',
    info,
    upload(geometry: Geometry): Promise<UploadTiming> {
      const t0 = performance.now();
      gl.bindVertexArray(strokeVao);
      gl.bindBuffer(gl.ARRAY_BUFFER, strokeBuf);
      gl.bufferData(gl.ARRAY_BUFFER, geometry.strokes, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 4, gl.FLOAT, false, STROKE_STRIDE, 0);
      gl.vertexAttribDivisor(0, 1);
      gl.enableVertexAttribArray(1);
      gl.vertexAttribPointer(1, 1, gl.FLOAT, false, STROKE_STRIDE, 16);
      gl.vertexAttribDivisor(1, 1);
      gl.enableVertexAttribArray(2);
      gl.vertexAttribIPointer(2, 1, gl.UNSIGNED_INT, STROKE_STRIDE, 20);
      gl.vertexAttribDivisor(2, 1);
      gl.enableVertexAttribArray(3);
      gl.vertexAttribIPointer(3, 1, gl.UNSIGNED_INT, STROKE_STRIDE, 24);
      gl.vertexAttribDivisor(3, 1);
      gl.bindVertexArray(fillVao);
      gl.bindBuffer(gl.ARRAY_BUFFER, fillBuf);
      gl.bufferData(gl.ARRAY_BUFFER, geometry.fillVertices, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, FILL_STRIDE, 0);
      gl.enableVertexAttribArray(2);
      gl.vertexAttribIPointer(2, 1, gl.UNSIGNED_INT, FILL_STRIDE, 8);
      gl.enableVertexAttribArray(3);
      gl.vertexAttribIPointer(3, 1, gl.UNSIGNED_INT, FILL_STRIDE, 12);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuf);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, geometry.fillIndices, gl.STATIC_DRAW);
      gl.bindVertexArray(null);
      strokeCount = geometry.strokeCount;
      indexCount = geometry.fillIndices.length;
      groupCount = geometry.groups.length / 6;
      gl.bindTexture(gl.TEXTURE_2D, groupTex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, 2 * groupCount, 1, 0, gl.RGBA, gl.FLOAT, null);
      const cpuMs = performance.now() - t0;
      gl.finish();
      return Promise.resolve({ cpuMs, completeMs: performance.now() - t0 });
    },
    frame(camera: Camera, groups: Float32Array): FrameTiming {
      const t0 = performance.now();
      gl.viewport(0, 0, camera.width, camera.height);
      gl.clearColor(1, 1, 1, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, groupTex);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, 2 * groupCount, 1, gl.RGBA, gl.FLOAT, groups);
      for (const [program, u, vao] of [
        [fillProgram, fillU, fillVao],
        [strokeProgram, strokeU, strokeVao],
      ] as const) {
        gl.useProgram(program);
        gl.uniform4f(u.cam, camera.scale, camera.offsetX, camera.offsetY, 0);
        gl.uniform2f(u.view, camera.width, camera.height);
        gl.uniform1i(u.groups, 0);
        gl.bindVertexArray(vao);
        if (program === fillProgram) {
          if (indexCount > 0) gl.drawElements(gl.TRIANGLES, indexCount, gl.UNSIGNED_INT, 0);
        } else if (strokeCount > 0) {
          gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, strokeCount);
        }
      }
      gl.bindVertexArray(null);
      gl.flush();
      return { cpuMs: performance.now() - t0, gpuDone: Promise.resolve(null) };
    },
    uploadPixels(rgba: Uint8Array, width: number, height: number): Promise<number> {
      const t0 = performance.now();
      gl.bindTexture(gl.TEXTURE_2D, pixelTex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, rgba);
      gl.finish();
      return Promise.resolve(performance.now() - t0);
    },
    destroy() {
      for (const b of [strokeBuf, fillBuf, indexBuf]) gl.deleteBuffer(b);
      gl.deleteTexture(groupTex);
      gl.deleteTexture(pixelTex);
      gl.deleteVertexArray(strokeVao);
      gl.deleteVertexArray(fillVao);
      gl.deleteProgram(strokeProgram);
      gl.deleteProgram(fillProgram);
      gl.getExtension('WEBGL_lose_context')?.loseContext();
    },
  };
}
