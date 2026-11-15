// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parseSceneAnswer } from '../canvas/scene';
import { parseWireJson } from '../ipc/wireJson';

interface Wire {
  scene: { [key: string]: unknown; entries: Record<string, unknown>[] };
}
const fresh = () =>
  parseWireJson(
    readFileSync(new URL('../canvas/testdata/scene/valid/camera.json', import.meta.url), 'utf8'),
  ) as Wire;
const entry = (w: Wire) => {
  const e = w.scene.entries[0];
  if (!e) throw new Error('fixture has no occurrence');
  return e;
};

describe('C43A static adversarial scene validation', () => {
  const cases: [string, (w: Wire) => void][] = [
    [
      'R1 major',
      (w) => {
        w.scene['sceneForm'] = '2.0';
      },
    ],
    [
      'R2 view',
      (w) => {
        w.scene['viewType'] = 'unknown';
      },
    ],
    [
      'R3 frame',
      (w) => {
        w.scene['frame'] = 0;
      },
    ],
    [
      'R3 missing camera',
      (w) => {
        delete w.scene['camera'];
      },
    ],
    [
      'R4 finite',
      (w) => {
        entry(w)['matrix'] = [Infinity, 0, 0, 1, 0, 0];
      },
    ],
    [
      'R4 singular',
      (w) => {
        entry(w)['matrix'] = [1, 1, 1, 1, 0, 0];
      },
    ],
    [
      'R5 overlay',
      (w) => {
        w.scene['overlays'] = {};
      },
    ],
    [
      'R6 drawing',
      (w) => {
        w.scene['viewType'] = 'drawing';
      },
    ],
  ];
  for (const [name, mutate] of cases) {
    it(name, () => {
      const w = fresh();
      mutate(w);
      expect(() => parseSceneAnswer(w)).toThrow(TypeError);
    });
  }
  it('unknown fields, occurrence order metadata and external drawing ids are unconstrained', () => {
    const w = fresh();
    w.scene['extra'] = true;
    for (const e of w.scene.entries) e['order'] = 0;
    entry(w)['drawingId'] = 'not-resident-yet';
    expect(parseSceneAnswer(w).scene.entries[0]?.drawingId).toBe('not-resident-yet');
  });
  it('retains revisions above 2^53 exactly', () => {
    const w = fresh();
    entry(w)['rev'] = 9007199254740993n;
    expect(parseSceneAnswer(w).scene.entries[0]?.rev).toBe(9007199254740993n);
  });
  for (const opacity of [-0.1, 1.1]) {
    it(`C43A-R1-20 refuses opacity ${String(opacity)} outside format range`, () => {
      const w = fresh();
      entry(w)['opacity'] = opacity;
      expect(() => parseSceneAnswer(w)).toThrow(TypeError);
    });
  }
});
