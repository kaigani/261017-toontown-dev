// @vitest-environment node
/// <reference types="node" />
// The scene answer's corpus (docs/canvas-format.md §11.7; M4 C4.3a): every valid answer decodes to its members, and
// one corrupt answer per rule R1-R6 is refused with the field named.
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { parseWireJson } from '../ipc/wireJson';
import { parseSceneAnswer } from './scene';

const here = fileURLToPath(new URL('./', import.meta.url));
const dir = (sub: string) => `${here}testdata/scene/${sub}/`;
const load = (sub: string, name: string): unknown =>
  parseWireJson(readFileSync(`${dir(sub)}${name}`, 'utf8'));

describe('the scene answer, form 1.1 (§11)', () => {
  it('decodes a Drawing view answer of form 1.0 and 1.1 alike', () => {
    const a = parseSceneAnswer(load('valid', 'drawing-1.0.json'));
    const b = parseSceneAnswer(load('valid', 'drawing-1.1.json'));
    expect(a.scene.sceneForm).toBe('1.0');
    expect(b.scene.sceneForm).toBe('1.1');
    for (const s of [a.scene, b.scene]) {
      expect(s.viewType).toBe('drawing');
      expect(s.camera).toBeNull();
      expect(s.overlays).toBeNull();
      expect(s.entries[0]?.matrix).toBeUndefined();
    }
    expect(b.scene.entries).toEqual(a.scene.entries);
  });

  it('decodes a Camera view answer: occurrences with their elements and matrices, the camera, the overlays', () => {
    const s = parseSceneAnswer(load('valid', 'camera.json')).scene;
    expect(s.viewType).toBe('camera');
    expect(s.frame).toBe(12);
    expect(s.entries.map((e) => e.elementId)).toEqual([
      'el_01K742SG12CTMHXEB7TY99G4XV',
      'el_01K742SG11W3SGB8Q1H50W74VB',
      'el_01K742SG11W3SGB8Q1H50W74VC',
    ]);
    expect(s.entries[1]?.matrix).toEqual([1.4, 0.5, 0.5, -1.4, 900, 520]);
    expect(s.camera).toEqual({
      elementId: 'el_01K742SG0ZZYY8HE4MW501XMJV',
      size: [1920, 1080],
      cameraToView: [0.47, 0.17, -0.17, 0.47, 12, 30],
    });
    expect(s.overlays?.cameraFrame).toEqual([0, 0, 1920, 1080]);
    expect(s.overlays?.otherCameras[0]).toHaveLength(4);
    expect(s.overlays?.selectionDisplay.boxHighlighting).toBe(true);
    expect(s.overlays?.currentOnTop).toBe(true);
  });

  it('decodes a Camera view without a camera (SI-147): no entries, the overlays', () => {
    const s = parseSceneAnswer(load('valid', 'camera-without-camera.json')).scene;
    expect(s.camera).toBeNull();
    expect(s.entries).toEqual([]);
    expect(s.overlays?.selectionUnion).toBeNull();
  });

  const corrupt = readdirSync(dir('corrupt')).filter((n) => n.endsWith('.json'));
  it('has a corrupt answer for each rule R1-R6', () => {
    for (const r of ['R1', 'R2', 'R3', 'R4', 'R5', 'R6'])
      expect(corrupt.some((n) => n.startsWith(r))).toBe(true);
  });
  for (const name of corrupt) {
    it(`refuses ${name}`, () => {
      expect(() => parseSceneAnswer(load('corrupt', name))).toThrow(TypeError);
    });
  }
});
