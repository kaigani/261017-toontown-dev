// The front-end IPC boundary, bundler half (C01-R2-01, C01-R3-01/02, C01-R4-01). Used by vite.config.ts for dev,
// build and Vitest, so a bypass that slips past the linter still cannot be bundled. Two plugins:
//
// `toontown-ipc-boundary` (enforce: pre) checks every import edge on RESOLVED module ids, not on spelling:
//   1. A module inside an @tauri-apps package may be imported only by src/ipc/tauriTransport.ts (or by the package
//      itself).
//   2. src/ipc/tauriTransport.ts may be imported only by src/ipc/index.ts.
//   3. Every application module (anything outside node_modules) must be desktop/index.html or live under
//      desktop/src/. Under Vitest only, test code may also load <repo>/tests/ and desktop/boundary/.
//   4. desktop/index.html (including its inline module scripts) may import only src/main.tsx. The dev server and
//      Vitest re-resolve already-resolved modules by URL with exactly `index.html` as a placeholder importer; in
//      `serve` only, such a re-resolution is accepted for module ids this plugin has already approved through a
//      real edge (and, under Vitest, for its own entry files: test files and setup files). `build` has no exception
//      at all.
//
// `toontown-ipc-boundary-scan` (normal order, so it sees code after Vite's TypeScript/JSX transform):
//   5. The transformed code of every application module except the transport is scanned (./scan.mjs: decoded
//      literals and escapes, char-code and base64 sequences, constant string folding) and any mention of
//      `__TAURI` or `@tauri-apps` is an error.
//   6. renderChunk: each chunk's mentions must be attributable, through the chunk's module map, to the transport or
//      to @tauri-apps package code.
//   7. generateBundle, on the FINAL (minified) output: the transport and every @tauri-apps module must sit in the
//      dedicated `tauri-transport` chunk (manualChunks in vite.config.ts) and nowhere else; that chunk may contain
//      nothing else, may export no more than src/ipc/index.ts imports from the transport, and may be imported only
//      by the chunk containing src/ipc/index.ts; no other chunk or emitted .js/.html asset may mention Tauri in any
//      form the scanner detects. This catches constant expressions the minifier folds after renderChunk.
//   8. writeBundle: the same final-output scan on the files as written to disk.
// boundary/check-dist.mjs repeats 7/8 on dist/ after `vite build`, independent of this plugin. The rule and file lists
// are shared with the lint half (./eslint.js).
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { parseAst, type Plugin } from 'vite';

import { IPC_INDEX_FILE, SAMPLE_FILES, TRANSPORT_FILE } from './eslint.js';
import { astMentions, isTransportChunkFile, mentions, TRANSPORT_CHUNK } from './scan.mjs';

export interface IpcBoundaryOptions {
  /** Vitest only: allow test code to load the repository's tests/ folder and desktop/boundary/. */
  allowTestSupport: boolean;
  /**
   * Vitest only: entry files Vitest loads itself (test files, setup files), relative to the desktop package. Vitest
   * resolves them with the index.html placeholder importer, before any import edge could approve them.
   */
  testEntries?: (relativePath: string) => boolean;
  /** The desktop package directory (default: the parent of this file). */
  desktopDir?: string;
}

const slash = (p: string) => p.replaceAll('\\', '/');
const TAURI_PACKAGE = /\/node_modules\/(@tauri-apps\/|\.vite\/deps\/@tauri-apps_)/;
/** vite.config.ts `manualChunks`: the transport and every @tauri-apps module go into one dedicated chunk. */
export function transportChunk(desktopDir: string): (id: string) => string | null {
  const transport = `${slash(desktopDir)}/${TRANSPORT_FILE}`;
  return (id) => {
    const p = slash(id.split('?')[0] ?? id);
    return p === transport || TAURI_PACKAGE.test(p) ? TRANSPORT_CHUNK : null;
  };
}

export function ipcBoundary(options: IpcBoundaryOptions): Plugin[] {
  const desktop = slash(
    options.desktopDir ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'),
  );
  const repo = slash(path.dirname(desktop));
  const src = `${desktop}/src/`;
  const transport = `${desktop}/${TRANSPORT_FILE}`;
  const index = `${desktop}/${IPC_INDEX_FILE}`;
  const indexHtml = `${desktop}/index.html`;
  const main = `${desktop}/src/main.tsx`;
  const samples = new Set(SAMPLE_FILES.map((f) => `${desktop}/${f}`));
  const testSupport = [`${repo}/tests/`, `${desktop}/boundary/`];
  let command: 'build' | 'serve' = 'serve';
  /** Resolved module ids accepted through a real import edge (serve-mode re-resolution allow-list). */
  const approved = new Set<string>();
  /** Edge resolutions in flight (see resolveId). */
  const pendingEdges = new Set<Promise<unknown>>();

  /** The on-disk path of a module id, or null for virtual/internal modules. */
  function filePath(id: string | undefined): string | null {
    if (!id || id.startsWith('\0') || id.startsWith('/@') || id.startsWith('virtual:')) return null;
    const p = slash(id.split('?')[0] ?? id);
    return path.isAbsolute(p) && existsSync(p) ? p : null;
  }

  const inNodeModules = (p: string) => p.includes('/node_modules/');
  const isApplicationModuleAllowed = (p: string) =>
    p === indexHtml ||
    p.startsWith(src) ||
    (options.allowTestSupport && testSupport.some((d) => p.startsWith(d)));
  /** Modules whose code may legitimately mention the Tauri globals/packages. */
  const mayMentionTauri = (p: string) => p === transport || TAURI_PACKAGE.test(p);

  function edgeViolation(resolved: string, importer: string | null, source: string): string | null {
    const from = importer ?? '(entry)';
    if (
      TAURI_PACKAGE.test(resolved) &&
      importer !== transport &&
      !(importer && TAURI_PACKAGE.test(importer))
    ) {
      return `${from} imports ${source}; only ${TRANSPORT_FILE} may load @tauri-apps packages`;
    }
    if (resolved === transport && importer !== index) {
      return `${from} imports the raw transport; only ${IPC_INDEX_FILE} may (use the validated HostApi)`;
    }
    return locationViolation(resolved, importer);
  }

  function locationViolation(resolved: string, importer: string | null): string | null {
    if (!inNodeModules(resolved) && !isApplicationModuleAllowed(resolved)) {
      return `${importer ?? '(entry)'} loads ${resolved}, which is outside desktop/src`;
    }
    if (importer && !inNodeModules(importer) && !isApplicationModuleAllowed(importer)) {
      return `${importer} is outside desktop/src and may not import application code`;
    }
    return null;
  }

  const resolver: Plugin = {
    name: 'toontown-ipc-boundary',
    enforce: 'pre',
    configResolved(config) {
      command = config.command;
    },
    async resolveId(source, importer, resolveOptions) {
      if (source.startsWith('\0')) return null;
      const placeholder = command === 'serve' && importer !== undefined && slash(importer) === indexHtml;
      const resolving = this.resolve(source, importer, { ...resolveOptions, skipSelf: true });
      if (!placeholder) {
        pendingEdges.add(resolving);
        const done = () => pendingEdges.delete(resolving);
        resolving.then(done, done);
      }
      const resolved = await resolving;
      if (!resolved || resolved.external) return resolved;
      const p = filePath(resolved.id);
      if (!p) return resolved;
      const from = filePath(importer);
      if (placeholder && !approved.has(p)) {
        // The edge that approves this module may still be resolving (Vitest's module runner races them): let every
        // edge in flight finish and record its approval before this fetch is judged.
        while (pendingEdges.size > 0) await Promise.allSettled([...pendingEdges]);
        await new Promise((r) => setTimeout(r, 0));
      }
      let problem: string | null;
      if (from === indexHtml) {
        // Vitest only: its own entry files, and the test-support folders (which only Vitest may load at all).
        const testEntry =
          placeholder &&
          options.allowTestSupport &&
          ((p.startsWith(src) && options.testEntries?.(p.slice(desktop.length + 1)) === true) ||
            testSupport.some((d) => p.startsWith(d)));
        // Third-party packages other than Tauri carry no boundary risk; application modules and Tauri packages must
        // have been approved through a real edge first.
        const harmlessPackage = inNodeModules(p) && !TAURI_PACKAGE.test(p);
        if (p === main) problem = null;
        else if (placeholder && (approved.has(p) || testEntry === true || harmlessPackage)) {
          problem = locationViolation(p, null);
        } else problem = `index.html may import only src/main.tsx, not ${source}`;
      } else {
        problem = edgeViolation(p, from, source);
      }
      if (problem) this.error(`IPC boundary: ${problem}`);
      approved.add(p);
      return resolved;
    },
  };

  /** Final-output checks shared by generateBundle (in memory) and writeBundle (on disk). */
  interface EmittedChunk {
    type: 'chunk';
    fileName: string;
    name: string;
    code: string;
    moduleIds: string[];
    exports: string[];
    imports: string[];
    dynamicImports: string[];
  }
  interface EmittedAsset {
    type: 'asset';
    fileName: string;
    source: string | Uint8Array;
  }
  type Emitted = EmittedChunk | EmittedAsset;

  /** How many bindings src/ipc/index.ts imports from the transport (the most the transport chunk may export). */
  function transportBindingsUsedByIndex(): number {
    const program = parseAst(readFileSync(index, 'utf8'), { lang: 'ts' }) as unknown as {
      body: { type: string; source?: { value?: unknown }; specifiers?: unknown[] }[];
    };
    return program.body
      .filter(
        (s) => s.type === 'ImportDeclaration' && /(^|\/)tauriTransport(\.ts)?$/.test(String(s.source?.value)),
      )
      .reduce((n, s) => n + (s.specifiers?.length ?? 0), 0);
  }

  function finalOutputProblems(files: Emitted[], readCode: (f: Emitted) => string | null): string[] {
    const problems: string[] = [];
    const chunks = files.filter((f): f is EmittedChunk => f.type === 'chunk');
    const transportChunks = chunks.filter((c) => c.name === TRANSPORT_CHUNK);
    const tauriModule = (id: string) => {
      const p = slash(id.split('?')[0] ?? id);
      return p === transport || TAURI_PACKAGE.test(p);
    };
    if (transportChunks.length > 1) problems.push(`more than one ${TRANSPORT_CHUNK} chunk`);
    for (const c of chunks) {
      const isTransportChunk = c.name === TRANSPORT_CHUNK;
      for (const id of c.moduleIds) {
        if (!isTransportChunk && tauriModule(id))
          problems.push(`${c.fileName} contains ${id} outside ${TRANSPORT_CHUNK}`);
        if (isTransportChunk && !tauriModule(id) && !id.startsWith('\0')) {
          problems.push(`${TRANSPORT_CHUNK} also contains ${id}`);
        }
      }
    }
    for (const t of transportChunks) {
      const allowed = transportBindingsUsedByIndex();
      if (t.exports.length > allowed) {
        problems.push(
          `${t.fileName} exports ${String(t.exports.length)} bindings; src/ipc/index.ts uses ${String(allowed)}`,
        );
      }
      for (const c of chunks) {
        if (c === t || !(c.imports.includes(t.fileName) || c.dynamicImports.includes(t.fileName))) continue;
        if (!c.moduleIds.some((id) => slash(id.split('?')[0] ?? id) === index)) {
          problems.push(`${c.fileName} imports ${t.fileName} but does not contain ${IPC_INDEX_FILE}`);
        }
      }
    }
    for (const f of files) {
      if (!/\.(m?js|cjs|html)$/.test(f.fileName)) continue;
      if (f.type === 'chunk' && f.name === TRANSPORT_CHUNK) continue;
      if (f.type === 'asset' && isTransportChunkFile(f.fileName)) continue;
      const code = readCode(f);
      if (code === null) continue;
      const hits = mentions(code, { javascript: !f.fileName.endsWith('.html') });
      if (hits.length > 0)
        problems.push(`${f.fileName} mentions Tauri after minification (${hits.slice(0, 3).join(', ')})`);
    }
    return problems;
  }

  const inMemory = (f: Emitted) =>
    f.type === 'chunk'
      ? f.code
      : typeof f.source === 'string'
        ? f.source
        : Buffer.from(f.source).toString('utf8');

  const scanner: Plugin = {
    name: 'toontown-ipc-boundary-scan',
    transform(code, id) {
      const p = filePath(id);
      // node_modules are checked in the emitted chunks; the boundary definitions and the two sample-holding
      // boundary tests contain the patterns they forbid.
      if (
        !p ||
        inNodeModules(p) ||
        p === transport ||
        samples.has(p) ||
        p.startsWith(`${desktop}/boundary/`)
      ) {
        return null;
      }
      // JS-like modules (by extension, or an inline <script> proxy such as index.html?html-proxy&index=0.js) are
      // parsed; anything else (HTML, CSS) is scanned as text only.
      const jsLike = /\.(?:[cm]?[jt]sx?)$/.test(p) || /[?&]html-proxy\b.*\.js$/.test(id);
      const hits = mentions(code, { javascript: jsLike });
      if (hits.length > 0) {
        this.error(
          `IPC boundary: ${p} mentions __TAURI* or @tauri-apps (${hits.slice(0, 3).join(', ')}); only ${TRANSPORT_FILE} may`,
        );
      }
      return null;
    },
    renderChunk(code, chunk) {
      // Attribution counts parsed (AST) mentions, which add up across modules; every module other than the transport
      // and @tauri-apps code must have no mention at all, of any kind.
      const total = astMentions(code)?.length ?? Number.NaN;
      let attributed = 0;
      for (const [id, rendered] of Object.entries(chunk.modules)) {
        if (!rendered.code) continue;
        if (mayMentionTauri(filePath(id) ?? slash(id))) {
          attributed += astMentions(rendered.code)?.length ?? 0;
        } else if (mentions(rendered.code).length > 0) {
          this.error(
            `IPC boundary: emitted code of ${id} in ${chunk.fileName} mentions __TAURI* or @tauri-apps`,
          );
        }
      }
      if (total !== attributed) {
        this.error(
          `IPC boundary: chunk ${chunk.fileName} has ${String(total)} Tauri mentions but ${String(attributed)} ` +
            `come from ${TRANSPORT_FILE} or @tauri-apps package code`,
        );
      }
      return null;
    },
    generateBundle(_options, bundle) {
      // Runs after minification: constant expressions the minifier folded are visible here (C01-R4-01).
      const problems = finalOutputProblems(Object.values(bundle) as unknown as Emitted[], inMemory);
      if (problems.length > 0) this.error(`IPC boundary (final output): ${problems.join('; ')}`);
    },
    writeBundle(options, bundle) {
      const dir = options.dir ?? (options.file ? path.dirname(options.file) : null);
      if (!dir) return;
      const fromDisk = (f: Emitted) => {
        const file = path.join(dir, f.fileName);
        return existsSync(file) ? readFileSync(file, 'utf8') : null;
      };
      const problems = finalOutputProblems(Object.values(bundle) as unknown as Emitted[], fromDisk);
      if (problems.length > 0) this.error(`IPC boundary (written output): ${problems.join('; ')}`);
    },
  };

  return [resolver, scanner];
}
