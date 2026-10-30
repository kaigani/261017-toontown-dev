// @vitest-environment node
/// <reference types="node" />
// The shared hostile-JSON corpus (C11-R1-17): the front end's parser accepts and rejects exactly the texts the
// engine's reader does (`crates/engine/tests/wire_corpus.rs` runs the same file).
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { MAX_DEPTH } from './schema';
import { decodeWireBytes, parseWireJson, stringifyWireJson, WireJsonError } from './wireJson';

interface Nest {
  open: string;
  close: string;
  count: number;
  inner: string;
}
interface Case {
  why: string;
  text?: string;
  nest?: Nest;
  canonical?: string;
  bytes?: number[];
}
interface Corpus {
  maxDepth: number;
  reject: Case[];
  accept: Case[];
  bytes: Case[];
}

const corpus = JSON.parse(
  readFileSync(
    fileURLToPath(new URL('../../../tests/fixtures/wire/hostile-json.json', import.meta.url)),
    'utf8',
  ),
) as Corpus;

function text(c: Case): string {
  if (c.nest !== undefined) {
    const n = c.nest;
    return n.open.repeat(n.count) + n.inner + n.close.repeat(n.count);
  }
  return c.text ?? '';
}

describe('shared hostile-JSON corpus', () => {
  it('uses the engine depth limit', () => {
    expect(corpus.maxDepth).toBe(MAX_DEPTH);
  });

  it('rejects every hostile text', () => {
    expect(corpus.reject.length).toBeGreaterThan(50);
    for (const c of corpus.reject) expect(() => parseWireJson(text(c)), c.why).toThrow(WireJsonError);
  });

  it('accepts every valid text and writes it canonically', () => {
    for (const c of corpus.accept) expect(stringifyWireJson(parseWireJson(text(c))), c.why).toBe(c.canonical);
  });

  it('rejects invalid UTF-8 bodies', () => {
    for (const c of corpus.bytes) {
      expect(decodeWireBytes(new Uint8Array(c.bytes ?? []).buffer), c.why).toBeUndefined();
    }
  });
});
