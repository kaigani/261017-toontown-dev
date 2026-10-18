// @vitest-environment node
/// <reference types="node" />
// Platform floor (ADR-0005, C01-R3-05): Safari 15.0 / macOS 12.0 WKWebView and Chrome 110 / WebView2. The floor is
// declared in three places that must agree, and the lint gate must reject newer APIs in shipped code.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { ESLint } from 'eslint';
import { describe, expect, it } from 'vitest';

const desktop = fileURLToPath(new URL('../../', import.meta.url));
const read = (f: string): unknown => JSON.parse(readFileSync(`${desktop}${f}`, 'utf8'));

describe('declared platform floor', () => {
  it('browserslist, the Vite build target and the Tauri macOS minimum agree', () => {
    const pkg = read('package.json') as { browserslist: string[] };
    expect(pkg.browserslist).toEqual(['safari 15', 'chrome 110']);
    const vite = readFileSync(`${desktop}vite.config.ts`, 'utf8');
    expect(vite).toContain("target: ['es2022', 'chrome110', 'safari15']");
    const tauri = read('src-tauri/tauri.conf.json') as {
      bundle: { macOS: { minimumSystemVersion: string } };
    };
    expect(tauri.bundle.macOS.minimumSystemVersion).toBe('12.0');
  });
});

describe('lint rejects APIs newer than the floor in shipped code', () => {
  const eslint = new ESLint({ cwd: desktop });
  const compatHits = async (code: string, file = 'src/lib/latency.ts') => {
    const [r] = await eslint.lintText(code, { filePath: `${desktop}${file}` });
    return (r?.messages ?? []).filter((m) => m.ruleId === 'compat/compat').map((m) => m.message);
  };

  for (const [api, code] of [
    ['Object.hasOwn (Safari 15.4)', "export const a = Object.hasOwn({}, 'x');\n"],
    ['structuredClone (Safari 15.4)', 'export const b = structuredClone({});\n'],
    ['Array.prototype.at (Safari 15.4)', 'export const c = [1].at(-1);\n'],
  ] as const) {
    it(`flags ${api}`, async () => {
      expect(await compatHits(code)).not.toEqual([]);
    }, 60_000);
  }

  it('accepts the floor-safe replacement', async () => {
    expect(await compatHits("export const d = Object.prototype.hasOwnProperty.call({}, 'x');\n")).toEqual([]);
  }, 60_000);
});
