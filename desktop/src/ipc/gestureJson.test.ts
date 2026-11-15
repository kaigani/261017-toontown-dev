// @vitest-environment node
// The JSON text of a gesture_commit request (C42B-R1-04): the native writer for the collector's plain events, the
// wire writer for anything else, and the same value either way.
import { describe, expect, it } from 'vitest';

import { gestureJson } from './engine';
import { parseWireJson, stringifyWireJson } from './wireJson';

describe('gestureJson', () => {
  it('writes a plain gesture natively, the same value as the wire writer', () => {
    const request = {
      view: 'drawingView',
      pointerType: 'pen',
      generation: 3,
      events: [
        {
          type: 'down',
          x: 10.25,
          y: -4,
          pressure: 0.5,
          tiltX: 12,
          tiltY: 0,
          button: 'left',
          modifiers: ['shift'],
          dt: 0,
        },
        { type: 'move', x: 11, y: -3.5, pressure: 0.6, modifiers: [], dt: 4 },
        { type: 'up', x: 12, y: -3, pressure: 0, modifiers: [], dt: 5, wheelDelta: undefined },
      ],
    };
    const text = gestureJson(request);
    expect(text).toBe(JSON.stringify(request));
    expect(parseWireJson(text)).toEqual(parseWireJson(stringifyWireJson(request)));
  });

  it('falls back to the wire writer for what the native writer would write differently', () => {
    const big = { view: 'v', generation: 2n ** 60n, events: [] };
    expect(gestureJson(big)).toBe(stringifyWireJson(big));
    const huge = {
      view: 'v',
      generation: 1,
      events: [{ type: 'down', x: 2 ** 60, y: 0, modifiers: [], dt: 0 }],
    };
    expect(gestureJson(huge)).toBe(stringifyWireJson(huge));
    const text = { view: `vue${String.fromCharCode(0xe9)}`, generation: 1, events: [] };
    expect(gestureJson(text)).toBe(stringifyWireJson(text));
    expect(() => gestureJson({ view: 'v', events: [{ x: Number.NaN }] })).toThrow();
  });
});
