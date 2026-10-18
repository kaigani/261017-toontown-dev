// @vitest-environment node
/// <reference types="node" />
// C0.4: the M0 diagnostics are compiled only into the diagnostics build. The real app is bundled with the project's
// Vite config in both modes; the default (production) bundle must contain no spike page, no spike chunk and no
// diagnostic command name in any emitted file (source maps included), while the diagnostics bundle contains all of
// them (the control that shows the scan can see them). The command names come from the host's single registry in
// src-tauri/src/lib.rs, so a command added there is checked here too.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { build } from 'vite';
import { describe, expect, it } from 'vitest';

const desktop = fileURLToPath(new URL('../../', import.meta.url));
const T = 180_000;

/** Command names (last path segment) of one `command_registry![...]` block of lib.rs. */
function registry(
  source: string,
  cfg: 'not(feature = "m0-diagnostics")' | 'feature = "m0-diagnostics"',
): string[] {
  const marker = `#[cfg(${cfg})]\ncommand_registry![`;
  const start = source.indexOf(marker);
  if (start < 0) throw new Error(`no registry for cfg(${cfg}) in lib.rs`);
  const body = source.slice(start + marker.length, source.indexOf(']', start + marker.length));
  return body
    .split(',')
    .map((path) => path.trim().split('::').pop() ?? '')
    .filter((name) => name !== '');
}

const lib = readFileSync(`${desktop}src-tauri/src/lib.rs`, 'utf8').replace(/\r\n/g, '\n');
const PRODUCT = registry(lib, 'not(feature = "m0-diagnostics")');
const DIAGNOSTIC = registry(lib, 'feature = "m0-diagnostics"').filter((name) => !PRODUCT.includes(name));

/** Every file the build emits, by name, as text. */
async function emitted(mode: 'production' | 'diagnostics'): Promise<Map<string, string>> {
  const result = await build({
    configFile: `${desktop}vite.config.ts`,
    root: desktop,
    mode,
    logLevel: 'silent',
    build: { write: false },
  });
  const files = new Map<string, string>();
  for (const r of Array.isArray(result) ? result : [result]) {
    if (!('output' in r)) continue;
    for (const o of r.output) {
      const text = o.type === 'chunk' ? o.code : typeof o.source === 'string' ? o.source : '';
      files.set(o.fileName, text);
      if (o.type === 'chunk' && o.map) files.set(`${o.fileName}.map`, JSON.stringify(o.map));
    }
  }
  return files;
}

describe('M0 diagnostics gating (C0.4)', () => {
  it('reads both registries from lib.rs', () => {
    expect(PRODUCT).toEqual(['ping', 'app_info']);
    expect(DIAGNOSTIC).toContain('bench_plan');
    expect(DIAGNOSTIC).toContain('spike_canvas_scene');
    expect(DIAGNOSTIC.length).toBe(12);
  });

  it(
    'the default build contains no spike page and names no diagnostic command',
    async () => {
      const files = await emitted('production');
      expect([...files.keys()].filter((f) => f.includes('Spike'))).toEqual([]);
      for (const [file, text] of files) {
        for (const name of DIAGNOSTIC) expect(text.includes(name), `${name} in ${file}`).toBe(false);
        // A source map carries App.tsx's source text, whose dead branch names the pages; emitted code must not.
        if (!file.endsWith('.map'))
          expect(/TabletSpike|CanvasSpike/.test(text), `spike page in ${file}`).toBe(false);
      }
      // The product commands are still there.
      const all = [...files.values()].join('\n');
      for (const name of PRODUCT)
        expect(all.includes(`'${name}'`) || all.includes(`"${name}"`), name).toBe(true);
    },
    T,
  );

  it(
    'the diagnostics build contains both spike pages and every diagnostic command (control)',
    async () => {
      const files = await emitted('diagnostics');
      const names = [...files.keys()];
      expect(names.some((f) => f.includes('TabletSpike'))).toBe(true);
      expect(names.some((f) => f.includes('CanvasSpike'))).toBe(true);
      const code = [...files]
        .filter(([f]) => !f.endsWith('.map'))
        .map(([, t]) => t)
        .join('\n');
      for (const name of DIAGNOSTIC) expect(code.includes(name), name).toBe(true);
    },
    T,
  );
});
