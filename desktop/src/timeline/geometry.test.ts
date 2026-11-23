// The window's Timeline geometry against the kernel's (reviews/M5-design-timeline-view.md section 3): the shared case
// table, and the two JSON files byte-equal to the kernel's originals.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import cases from './timeline-geometry-cases.json';
import {
  frameAt,
  holds,
  moveMode,
  platformOf,
  rowAt,
  zoneAt,
  rowWindow,
  visibleFrames,
  xOf,
  zoomOf,
} from './geometry';

const here = path.dirname(fileURLToPath(import.meta.url));
const crate = (name: string) =>
  readFileSync(path.join(here, '..', '..', '..', 'crates', 'executor', 'src', name), 'utf8');
const local = (name: string) => readFileSync(path.join(here, name), 'utf8');

describe('the Timeline geometry', () => {
  it("keeps byte copies of the kernel's layout and case table", () => {
    for (const name of ['timeline-layout.json', 'timeline-geometry-cases.json']) {
      expect(local(name), name).toBe(crate(name));
    }
  });

  it('maps x to the frame as the kernel does, at every zoom level', () => {
    expect(cases.frameAt.length).toBeGreaterThan(100);
    for (const c of cases.frameAt) {
      expect(frameAt({ zoomLevel: c.zoomLevel, firstFrame: c.firstFrame }, c.x), JSON.stringify(c)).toBe(
        c.frame,
      );
    }
  });

  it("maps a frame to its cell's left edge as the kernel does", () => {
    for (const c of cases.xOf) {
      expect(xOf({ zoomLevel: c.zoomLevel, firstFrame: c.firstFrame }, c.frame), JSON.stringify(c)).toBe(
        c.xLeft,
      );
    }
  });

  it('maps y to the row as the kernel does', () => {
    for (const c of cases.rowAt) {
      expect(rowAt(c.y, c.scrollTop, Number.MAX_SAFE_INTEGER), JSON.stringify(c)).toBe(c.row);
    }
    expect(rowAt(-1, 0, 10)).toBeNull();
    expect(rowAt(22 * 10, 0, 10)).toBeNull();
  });

  it('finds the hit zone under x as the kernel does (ruling E-35)', () => {
    expect(cases.zones.length).toBeGreaterThan(40);
    for (const c of cases.zones) {
      const bar: readonly [number, number] | null = c.bar ? [c.bar[0] ?? 0, c.bar[1] ?? 0] : null;
      const got = zoneAt(
        { zoomLevel: c.zoomLevel, firstFrame: c.firstFrame },
        c.x,
        c.upper,
        bar,
        c.loopable,
        c.keys,
      );
      expect(got, JSON.stringify(c)).toEqual(c.zone);
    }
  });

  it('clamps the zoom index and counts the visible frames', () => {
    expect(zoomOf(-3)).toEqual({ cellWidth: 1, framesPerCell: 10 });
    expect(zoomOf(99)).toEqual({ cellWidth: 48, framesPerCell: 1 });
    expect(visibleFrames(960, 6)).toBe(60);
    expect(visibleFrames(960, 0)).toBe(9600);
    expect(visibleFrames(0, 6)).toBe(1);
  });

  it('keeps only the rows in view plus a margin (the virtualised list)', () => {
    expect(rowWindow(1000, 0, 220)).toEqual({ first: 0, last: 21 });
    expect(rowWindow(1000, 22 * 500, 220)).toEqual({ first: 490, last: 521 });
    expect(rowWindow(1000, 22 * 995, 220)).toEqual({ first: 985, last: 999 });
    expect(rowWindow(0, 0, 220)).toEqual({ first: 0, last: -1 });
  });

  it('reads the modifier roles per platform (SI-211)', () => {
    expect(platformOf('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)')).toBe('macos');
    expect(platformOf('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe('windows');
    expect(moveMode('windows', ['ctrl'])).toBe('copy');
    expect(moveMode('windows', ['shift', 'ctrl'])).toBe('link');
    expect(moveMode('macos', ['alt'])).toBe('copy');
    expect(moveMode('macos', ['alt', 'meta'])).toBe('link');
    expect(moveMode('windows', [])).toBe('move');
    expect(holds([], ['shift'])).toBe(false);
  });
});
