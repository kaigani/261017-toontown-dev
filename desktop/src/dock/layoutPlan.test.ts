// M4 C4.1 round 1 (ruling O-3a, SI-118): the same-place `window.panel.move` that records a splitter drag. The kernel
// side (the arrangement stays byte-equal except the area's size, in every built-in workspace and side area) is
// crates/executor/tests/c41/kernel.rs. Builder-owned.
import { describe, expect, it } from 'vitest';

import type { Arrangement } from '../state/types';
import { samePlaceResize } from './layoutPlan';

function arrangement(right: { tabs: string[]; current: string; size?: number }[]): Arrangement {
  return {
    centralPanel: 'cameraView',
    corners: 'sides',
    docks: { left: [], right, top: [], bottom: [] },
    floating: [],
    toolPalettes: [],
    mainToolbar: { visible: true, items: [] },
  } as unknown as Arrangement;
}

describe('samePlaceResize', () => {
  it('tabs the area’s first front panel with itself, with the new size, in any arrangement', () => {
    for (const groups of [
      [{ tabs: ['pen', 'text', 'cells'], current: 'pen' }],
      [
        { tabs: ['history'], current: 'history' },
        { tabs: ['colour', 'properties'], current: 'properties' },
      ],
    ]) {
      const first = groups[0]?.current;
      expect(samePlaceResize(arrangement(groups), 'right', 401.6)).toEqual({
        panelId: first,
        area: 'right',
        relativeTo: first,
        position: 'tab',
        size: 402,
      });
    }
  });

  it('gives no call for an empty area', () => {
    expect(samePlaceResize(arrangement([]), 'right', 280)).toBeNull();
  });
});
