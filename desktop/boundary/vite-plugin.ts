// The front-end IPC boundary, bundler half (C01-R2-01, C01-R3-01/02). Used by vite.config.ts for dev, build and
// Vitest, so a bypass that slips past the linter still cannot be bundled. Two plugins:
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
//   5. The transformed code of every application module except the transport is parsed, and any string literal,
//      template element or identifier mentioning `__TAURI` or `@tauri-apps` is an error. Literals are compared
//      decoded, so escapes such as '__TAURI' are caught; a textual match anywhere (comments included) is
//      an error too.
//   6. Every emitted chunk is parsed and each mention is attributed to a module through the chunk's module map: all
//      mentions must come from the transport or from @tauri-apps package code, and the chunk's total must equal
//      the sum attributed to those modules.
// The rule and file lists are shared with the lint half (./eslint.js).
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { parseAst, type Plugin } from 'vite';

import { IPC_INDEX_FILE, SAMPLE_FILES, TRANSPORT_FILE } from './eslint.js';

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
const TAURI_TEXT = /__TAURI|@tauri-apps/;

/** Mentions of the Tauri globals or package names in parsed code: decoded string literals, template elements and
 * identifiers. Returns null when the code does not parse as JavaScript. */
export function tauriMentions(code: string): string[] | null {
  let program: unknown;
  try {
    program = parseAst(code);
  } catch {
    return null;
  }
  const hits: string[] = [];
  const stack: unknown[] = [program];
  while (stack.length > 0) {
    const node = stack.pop();
    if (typeof node !== 'object' || node === null) continue;
    if (Array.isArray(node)) {
      stack.push(...(node as unknown[]));
      continue;
    }
    const n = node as Record<string, unknown>;
    const type = n['type'];
    if (type === 'Literal' && typeof n['value'] === 'string' && TAURI_TEXT.test(n['value']))
      hits.push(n['value']);
    if ((type === 'Identifier' || type === 'PrivateIdentifier') && typeof n['name'] === 'string') {
      if (TAURI_TEXT.test(n['name'])) hits.push(n['name']);
    }
    if (type === 'TemplateElement') {
      const value = n['value'] as { cooked?: string | null; raw?: string } | undefined;
      const text = value?.cooked ?? value?.raw ?? '';
      if (TAURI_TEXT.test(text)) hits.push(text);
    }
    for (const [k, v] of Object.entries(n)) {
      if (k !== 'type' && typeof v === 'object' && v !== null) stack.push(v);
    }
  }
  return hits;
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
      const hits = jsLike ? tauriMentions(code) : [];
      if (TAURI_TEXT.test(code) || hits === null || hits.length > 0) {
        const what =
          hits === null ? 'cannot be parsed for the Tauri scan' : 'mentions __TAURI* or @tauri-apps';
        this.error(`IPC boundary: ${p} ${what}; only ${TRANSPORT_FILE} may reach Tauri`);
      }
      return null;
    },
    renderChunk(code, chunk) {
      const total = tauriMentions(code);
      if (total === null)
        this.error(`IPC boundary: emitted chunk ${chunk.fileName} cannot be parsed for the scan`);
      let attributed = 0;
      for (const [id, rendered] of Object.entries(chunk.modules)) {
        if (!rendered.code) continue;
        const hits = tauriMentions(rendered.code);
        const p = filePath(id) ?? slash(id);
        if (mayMentionTauri(p)) {
          attributed += hits?.length ?? 0;
        } else if (hits === null || hits.length > 0) {
          this.error(
            `IPC boundary: emitted code of ${p} in ${chunk.fileName} mentions __TAURI* or @tauri-apps`,
          );
        }
      }
      if ((total?.length ?? 0) !== attributed) {
        this.error(
          `IPC boundary: chunk ${chunk.fileName} has ${String(total?.length)} Tauri mentions but only ` +
            `${String(attributed)} come from ${TRANSPORT_FILE} or @tauri-apps package code`,
        );
      }
      return null;
    },
  };

  return [resolver, scanner];
}
