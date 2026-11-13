// M4 C4.1 round 1 (ruling O-3, SI-118): the same-place `window.panel.move` that records a splitter drag. The kernel
// side (the move leaves the arrangement unchanged except the area's size) is tests/c41/kernel.rs. Builder-owned.
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
  it('re-docks a single-tab group after its predecessor, or a lone group', () => {
    const a = arrangement([
      { tabs: ['colour', 'properties'], current: 'colour', size: 320 },
      { tabs: ['history'], current: 'history' },
    ]);
    expect(samePlaceResize(a, 'right', 401.6)).toEqual({
      panelId: 'history',
      area: 'right',
      relativeTo: 'properties',
      position: 'after',
      size: 402,
    });
    expect(samePlaceResize(arrangement([{ tabs: ['history'], current: 'history' }]), 'right', 300)).toEqual({
      panelId: 'history',
      area: 'right',
      position: 'after',
      size: 300,
    });
    // A first group before a successor: no call (its size would stay on the successor too).
    expect(
      samePlaceResize(
        arrangement([
          { tabs: ['history'], current: 'history' },
          { tabs: ['pen', 'text'], current: 'pen' },
        ]),
        'right',
        250,
      ),
    ).toBeNull();
  });

  it('re-tabs a front tab that is its group’s last tab', () => {
    const a = arrangement([{ tabs: ['pen', 'text', 'cells'], current: 'cells' }]);
    expect(samePlaceResize(a, 'right', 280)).toEqual({
      panelId: 'cells',
      area: 'right',
      relativeTo: 'pen',
      position: 'tab',
      size: 280,
    });
  });

  it('gives no call when every re-insertion would reorder tabs or change a front tab', () => {
    const a = arrangement([{ tabs: ['pen', 'text', 'cells'], current: 'pen' }]);
    expect(samePlaceResize(a, 'right', 280)).toBeNull();
    expect(samePlaceResize(arrangement([]), 'right', 280)).toBeNull();
  });
});
