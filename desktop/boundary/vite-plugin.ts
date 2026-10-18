// The front-end IPC boundary, bundler half (C01-R2-01). Used by vite.config.ts for dev, build and Vitest, so a
// bypass that slips past the linter (another file extension, a suppressed rule, a module outside src/) still cannot
// be bundled. Checks are made on RESOLVED module ids, not on how a specifier is spelled:
//   1. A module inside an @tauri-apps package may be imported only by src/ipc/tauriTransport.ts (or by the package
//      itself).
//   2. src/ipc/tauriTransport.ts may be imported only by src/ipc/index.ts.
//   3. Every application module (anything outside node_modules) must be desktop/index.html or live under
//      desktop/src/. Under Vitest only, test code may also load <repo>/tests/ and desktop/boundary/.
//   4. Application modules other than the transport may not mention the Tauri globals (__TAURI*) or the
//      @tauri-apps package names in their source at all.
// The rule and file lists are shared with the lint half (./eslint.js).
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import type { Plugin } from 'vite';

import { IPC_INDEX_FILE, SAMPLE_FILES, TRANSPORT_FILE } from './eslint.js';

export interface IpcBoundaryOptions {
  /** Vitest only: allow test code to load the repository's tests/ folder and desktop/boundary/. */
  allowTestSupport: boolean;
  /** The desktop package directory (default: the parent of this file). */
  desktopDir?: string;
}

const slash = (p: string) => p.replaceAll('\\', '/');
const TAURI_PACKAGE = /\/node_modules\/@tauri-apps\//;
const TAURI_TEXT = /__TAURI|@tauri-apps/;

export function ipcBoundary(options: IpcBoundaryOptions): Plugin {
  const desktop = slash(
    options.desktopDir ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'),
  );
  const repo = slash(path.dirname(desktop));
  const src = `${desktop}/src/`;
  const transport = `${desktop}/${TRANSPORT_FILE}`;
  const index = `${desktop}/${IPC_INDEX_FILE}`;
  const indexHtml = `${desktop}/index.html`;
  const samples = new Set(SAMPLE_FILES.map((f) => `${desktop}/${f}`));
  const testSupport = [`${repo}/tests/`, `${desktop}/boundary/`];

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

  /** `edge` is false for the dev server's URL re-resolution of an already-resolved module (see resolveId). */
  function violation(
    resolved: string,
    importer: string | null,
    source: string,
    edge: boolean,
  ): string | null {
    const from = importer ?? '(entry)';
    if (
      edge &&
      TAURI_PACKAGE.test(resolved) &&
      importer !== transport &&
      !(importer && TAURI_PACKAGE.test(importer))
    ) {
      return `${from} imports ${source}; only ${TRANSPORT_FILE} may load @tauri-apps packages`;
    }
    if (edge && resolved === transport && importer !== index) {
      return `${from} imports the raw transport; only ${IPC_INDEX_FILE} may (use the validated HostApi)`;
    }
    if (!inNodeModules(resolved) && !isApplicationModuleAllowed(resolved)) {
      return `${from} loads ${resolved}, which is outside desktop/src`;
    }
    if (importer && !inNodeModules(importer) && !isApplicationModuleAllowed(importer)) {
      return `${importer} is outside desktop/src and may not import application code`;
    }
    return null;
  }

  return {
    name: 'toontown-ipc-boundary',
    enforce: 'pre',
    async resolveId(source, importer, resolveOptions) {
      if (source.startsWith('\0')) return null;
      const resolved = await this.resolve(source, importer, { ...resolveOptions, skipSelf: true });
      if (!resolved || resolved.external) return resolved;
      const p = filePath(resolved.id);
      if (p) {
        // The dev server and Vitest re-resolve already-resolved modules by absolute URL with index.html as a
        // placeholder importer. That is not an import edge (the real edge was checked when the importing module
        // resolved its own specifier), so only the location rule applies to it. index.html's one real import,
        // /src/main.tsx, needs no importer rule.
        const from = filePath(importer);
        const reResolution =
          from === indexHtml && /^(\/|[A-Za-z]:)/.test(source) && source !== '/src/main.tsx';
        const problem = reResolution ? violation(p, null, source, false) : violation(p, from, source, true);
        if (problem) this.error(`IPC boundary: ${problem}`);
      }
      return resolved;
    },
    transform(code, id) {
      const p = filePath(id);
      // The boundary definitions themselves contain the patterns they forbid.
      if (
        !p ||
        inNodeModules(p) ||
        p === transport ||
        samples.has(p) ||
        p.startsWith(`${desktop}/boundary/`)
      ) {
        return null;
      }
      if (TAURI_TEXT.test(code)) {
        this.error(`IPC boundary: ${p} mentions __TAURI* or @tauri-apps; only ${TRANSPORT_FILE} may`);
      }
      return null;
    },
  };
}
