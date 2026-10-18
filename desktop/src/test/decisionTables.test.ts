// @vitest-environment node
/// <reference types="node" />
// C0.4 (review round 1, C04-R1-01/02): the tablet and canvas decision rule sets are normative in their ADRs and copied
// into the go/no-go report. The copies must stay identical, so a later edit to one cannot silently change what a
// measurement decides.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const repo = fileURLToPath(new URL('../../../', import.meta.url));
const read = (path: string) => readFileSync(`${repo}${path}`, 'utf8').replace(/\r\n/g, '\n');

function block(text: string, name: string): string {
  const begin = `<!-- ${name}:begin -->`;
  const end = `<!-- ${name}:end -->`;
  const start = text.indexOf(begin);
  const stop = text.indexOf(end);
  expect(start, `${name} begin marker`).toBeGreaterThanOrEqual(0);
  expect(stop, `${name} end marker`).toBeGreaterThan(start);
  expect(text.indexOf(begin, start + 1), `${name} appears once`).toBe(-1);
  return text.slice(start, stop + end.length);
}

const REPORT = 'reviews/M0-C0.4-go-no-go.md';
const CASES: [name: string, adr: string, mustContain: string[]][] = [
  [
    'tablet-rules',
    'docs/decisions/ADR-0009-tablet-input.md',
    [
      'Step 0',
      'Step 1',
      'Step 2',
      'Step 3',
      'without any valid measurement',
      'E-R1',
      'E-D1',
      '| 9 |',
      '[120, ∞)',
      'never combined',
    ],
  ],
  [
    'canvas-rules',
    'docs/decisions/ADR-0007-interactive-canvas.md',
    [
      'paths100k',
      'groups1k',
      'brush10k',
      'fps ≥ 57',
      'worst',
      'native-wgpu',
      'not measured',
      'insufficient evidence',
      'valid measurement',
    ],
  ],
];

describe('C0.4 decision rule sets', () => {
  for (const [name, adr, mustContain] of CASES) {
    it(`${name}: the go/no-go report's copy equals ${adr}`, () => {
      const normative = block(read(adr), name);
      expect(block(read(REPORT), name)).toBe(normative);
      for (const s of mustContain) expect(normative, s).toContain(s);
    });
  }
});
