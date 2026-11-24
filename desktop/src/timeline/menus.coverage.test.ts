// Ruling E-47 (C51B-R1S-03): the three Timeline context menus hold every command 04 §11.9 requires (lines 704-706),
// item by item, so a missing item fails here. Each row of the table is one phrase of the spec's list, transcribed in
// order, with the menu items that make it reachable. An item a later checkpoint owns is present and shown disabled with
// that owner (the deferred list, the kernel's "arrives with" for a command not implemented yet, or `owner` when no
// command performs it yet). Toggles present their state (04 line 708): `check`, or on/off wording.
import { describe, expect, it } from 'vitest';

import deferredTable from '../ui-bindings/deferred.json';
import { ADD_TYPES, cellMenu, rowMenu, rulerMenu, type TimelineMenu } from './menus';
import type { TimelineRow } from './model';

function row(type: string, over: Partial<TimelineRow> = {}): TimelineRow {
  return {
    elementId: `el_${type}`,
    name: type,
    type,
    depth: 0,
    parentId: null,
    collapsed: false,
    visibleInScene: true,
    hiddenByParent: false,
    locked: false,
    bar: { start: 1, end: 24, loops: 1 },
    keyframes: [],
    childKeyframeFrames: [],
    ...over,
  };
}

const ids = (m: TimelineMenu): string[] => [
  ...m.groups.flatMap((g) => g.items.map((i) => i.id)),
  ...m.dialogs.map((d) => d.id),
];

// The menus the table is read against: the row context on a peg followed by a parentable row (a peg has every bar
// item), on a drawing (set as annotation) and on an effect (its parameter rows); the cell context on a peg (extend
// children); the ruler.
const peg = row('peg');
const drawing = row('drawing');
const clipping = row('clipping');
const next = row('peg', { elementId: 'el_next' });
const ROW = new Set([
  ...ids(rowMenu(peg, [peg, next], [])),
  ...ids(rowMenu(drawing, [drawing], [])),
  ...ids(rowMenu(clipping, [clipping], [])),
]);
const CELL = new Set(ids(cellMenu(peg, 5)));
const RULER = new Set(ids(rulerMenu(5)));

// 04 line 704, the row (layer) context.
const ROW_TABLE: readonly (readonly [string, readonly string[]])[] = [
  ['All element-creation commands', [...ADD_TYPES.map((t) => `timeline.row.add.${t.type}`)]],
  ['image import', ['timeline.row.importImage']],
  ['import-and-vectorise', ['timeline.row.importVectorize']],
  ['vector/PDF artwork import', ['timeline.row.importArtwork']],
  ['sound import', ['timeline.row.importSound']],
  ['movie-as-image-sequence import (09)', ['timeline.row.importMovie']],
  [
    'cut, copy, paste, delete',
    ['timeline.row.cut', 'timeline.row.copy', 'timeline.row.paste', 'timeline.row.delete'],
  ],
  [
    'arrange (9.1)',
    [
      'timeline.row.arrange.front',
      'timeline.row.arrange.forward',
      'timeline.row.arrange.backward',
      'timeline.row.arrange.back',
    ],
  ],
  ['collapse/expand', ['timeline.row.collapse']],
  [
    'hierarchy selection (9.3)',
    [
      'timeline.row.select.parent',
      'timeline.row.select.children',
      'timeline.row.select.firstChild',
      'timeline.row.select.previousSibling',
      'timeline.row.select.nextSibling',
      'timeline.row.select.skeletonRoot',
    ],
  ],
  ['attach to next object, detach from parent', ['timeline.row.attachNext', 'timeline.row.detach']],
  [
    'visibility (10.1)',
    [
      'timeline.row.showHide',
      'timeline.row.showAll',
      'timeline.row.hideOthers',
      'timeline.row.showOthers',
      'timeline.row.hideAll',
    ],
  ],
  [
    'marker/background/default colour',
    ['timeline.row.markerColour', 'timeline.row.backgroundColour', 'timeline.row.defaultColour'],
  ],
  [
    'effect-parameter display',
    ['timeline.row.effectParameters', 'timeline.row.showAllParameters', 'timeline.row.hideAllParameters'],
  ],
  ['set as annotation / set as drawing element', ['timeline.row.annotation']],
  ['show/hide all annotation elements', ['timeline.row.showAnnotations', 'timeline.row.hideAnnotations']],
  [
    'lock commands (10.4)',
    ['timeline.row.lock', 'timeline.row.lockOthers', 'timeline.row.lockAll', 'timeline.row.unlockAll'],
  ],
  [
    'change start frame, duration and loops (11.10)',
    ['timeline.row.changeStart', 'timeline.row.changeDuration', 'timeline.row.changeLoops'],
  ],
  [
    'clone, duplicate, delete, rename element',
    ['timeline.row.clone', 'timeline.row.duplicate', 'timeline.row.delete', 'timeline.row.rename'],
  ],
  ['set element note', ['timeline.row.note']],
  [
    'peg reset, remove all keyframes, toggle constant Z, add default keyframe (05)',
    [
      'timeline.row.pegReset',
      'timeline.row.removeKeys',
      'timeline.row.constantZ',
      'timeline.row.addDefaultKey',
    ],
  ],
  ['edit sound, sound waveforms (07)', ['timeline.row.editSound', 'timeline.row.waveforms']],
];

// 04 line 705, the cells (track) context.
const CELL_TABLE: readonly (readonly [string, readonly string[]])[] = [
  ['Add/remove exposure', ['timeline.cell.addExposure', 'timeline.cell.removeExposure']],
  [
    'set exposure to 1/2/3/N',
    [
      'timeline.cell.exposure1',
      'timeline.cell.exposure2',
      'timeline.cell.exposure3',
      'timeline.cell.exposureN',
    ],
  ],
  ['extend exposure (fill)', ['timeline.cell.fill']],
  ['extend children exposure (pegs)', ['timeline.cell.extendChildren']],
  ['insert cells', ['timeline.cell.insertCells']],
  ['insert blank cell', ['timeline.cell.insertBlank']],
  ['delete blank cell', ['timeline.cell.deleteBlank']],
  ['create cycle', ['timeline.cell.cycle']],
  ['columnate (advanced cycle)', ['timeline.cell.columnate']],
  ['duplicate drawing', ['timeline.cell.duplicateDrawing']],
  ['add/remove keyframe', ['timeline.cell.addKey', 'timeline.cell.removeKey']],
  ['set constant/non-constant segment', ['timeline.cell.constant', 'timeline.cell.nonConstant']],
  ['uniform keyframe', ['timeline.cell.uniformLeft', 'timeline.cell.uniformRight']],
  ['add default keyframe (05)', ['timeline.cell.addDefaultKey']],
  [
    'cut, copy, paste, delete',
    ['timeline.cell.cut', 'timeline.cell.copy', 'timeline.cell.paste', 'timeline.cell.delete'],
  ],
  [
    'edit/split/fit sound (07)',
    ['timeline.cell.editSound', 'timeline.cell.splitSound', 'timeline.cell.fitSound'],
  ],
];

// 04 line 706, the ruler / empty track area.
const RULER_TABLE: readonly (readonly [string, readonly string[]])[] = [
  ['Set current frame', ['timeline.ruler.currentFrame']],
  ['playback range automatic / free', ['timeline.ruler.rangeAuto', 'timeline.ruler.rangeFree']],
  ['set playback start / end frame', ['timeline.ruler.rangeStart', 'timeline.ruler.rangeEnd']],
  ['sound waveforms', ['timeline.ruler.waveforms']],
  [
    'sound playback and sound scrubbing (09)',
    ['timeline.ruler.soundPlayback', 'timeline.ruler.soundScrubbing'],
  ],
];

describe('the Timeline contexts hold 04 §11.9 (ruling E-47)', () => {
  for (const [context, table, menu] of [
    ['row', ROW_TABLE, ROW],
    ['cell', CELL_TABLE, CELL],
    ['ruler', RULER_TABLE, RULER],
  ] as const) {
    for (const [phrase, items] of table) {
      it(`${context}: ${phrase}`, () => {
        for (const id of items) expect(menu.has(id), `${context} context lacks ${id} (${phrase})`).toBe(true);
      });
    }
  }

  it('an item no command performs yet names its owner and sends nothing', () => {
    const items = [rowMenu(peg, [peg], []), cellMenu(peg, 5)].flatMap((m) =>
      m.groups.flatMap((g) => g.items),
    );
    const owned = items.filter((i) => i.owner !== undefined);
    expect(owned.map((i) => i.id).sort()).toEqual(
      ['timeline.cell.addDefaultKey', 'timeline.cell.fitSound', 'timeline.row.addDefaultKey'].sort(),
    );
    for (const i of owned) expect(i.command, i.id).toBe('');
  });

  it('the C5.2 cell commands are shown disabled with their owner (E-2 a)', () => {
    const deferred = (deferredTable as { deferred: Record<string, { owner: string }> }).deferred;
    const items = cellMenu(peg, 5).groups.flatMap((g) => g.items);
    for (const command of [
      'cell.exposure.add',
      'cell.exposure.remove',
      'cell.exposure.set',
      'cell.fill.apply',
      'cell.range.add',
      'cell.blank.add',
      'cell.blank.remove',
      'cell.cycle.create',
      'cell.columnate.apply',
      'cell.drawing.duplicate',
    ]) {
      expect(
        items.some((i) => i.command === command),
        command,
      ).toBe(true);
      expect(deferred[command]?.owner, command).toBe('M5 C5.2');
    }
  });

  it('the animation commands C5.1b binds have left the deferred list (E-2 b)', () => {
    const deferred = (deferredTable as { deferred: Record<string, unknown> }).deferred;
    for (const command of [
      'anim.key.add',
      'anim.key.remove',
      'anim.segment.set',
      'anim.segment.uniform.apply',
      'anim.path.lockDepth.set',
    ]) {
      expect(deferred[command], command).toBeUndefined();
    }
  });

  it('toggles and radio items present their state (04 line 708)', () => {
    const items = [
      rowMenu(peg, [peg], []),
      rowMenu(clipping, [clipping], []),
      cellMenu(peg, 5),
      rulerMenu(5),
    ].flatMap((m) => m.groups.flatMap((g) => g.items));
    const checked = new Set(items.filter((i) => i.check === true).map((i) => i.id));
    for (const id of [
      'timeline.ruler.rangeAuto',
      'timeline.ruler.rangeFree',
      'timeline.row.effectParameters',
      'timeline.row.waveforms',
      'timeline.ruler.waveforms',
      'timeline.row.constantZ',
      'timeline.ruler.soundPlayback',
      'timeline.ruler.soundScrubbing',
    ]) {
      expect(checked.has(id), id).toBe(true);
    }
    // The toggles a check mark does not show say their next state in their wording.
    const label = (m: TimelineMenu, id: string) =>
      m.groups.flatMap((g) => g.items).find((i) => i.id === id)?.label;
    expect(label(rowMenu(peg, [peg], []), 'timeline.row.showHide')).toBe('Hide');
    expect(label(rowMenu(row('peg', { visibleInScene: false }), [], []), 'timeline.row.showHide')).toBe(
      'Show',
    );
    expect(label(rowMenu(peg, [peg], []), 'timeline.row.lock')).toBe('Lock');
    expect(label(rowMenu(row('peg', { locked: true }), [], []), 'timeline.row.lock')).toBe('Unlock');
    expect(label(rowMenu(peg, [peg], []), 'timeline.row.collapse')).toBe('Collapse');
    expect(label(rowMenu(row('peg', { collapsed: true }), [], []), 'timeline.row.expand')).toBe('Expand');
    // A toggle's reference leaves the next state to the kernel: it can be turned off as well as on.
    const effect = rowMenu(clipping, [clipping], [])
      .groups.flatMap((g) => g.items)
      .find((i) => i.id === 'timeline.row.effectParameters');
    expect(effect?.params).toEqual({ elementId: 'el_clipping' });
  });

  it('Toggle Constant Z names the segment at the current frame', () => {
    const keyed = row('peg', {
      keyframes: [
        { frame: 1, channels: ['position'], segmentAfter: 'interpolated' },
        { frame: 9, channels: ['position', 'rotation'], segmentAfter: 'interpolated' },
        { frame: 20, channels: ['rotation'], segmentAfter: 'interpolated' },
      ],
    });
    const z = (frame: number) =>
      rowMenu(keyed, [keyed], [], frame)
        .groups.flatMap((g) => g.items)
        .find((i) => i.id === 'timeline.row.constantZ')?.params;
    expect(z(12)).toEqual({ nodeId: 'el_peg', frame: 9 });
    expect(z(25)).toEqual({ nodeId: 'el_peg', frame: 9 });
    expect(z(3)).toEqual({ nodeId: 'el_peg', frame: 1 });
  });

  it('the segment items name the keys around the clicked frame (05 §10)', () => {
    const keyed = row('peg', {
      keyframes: [
        { frame: 1, channels: ['position'], segmentAfter: 'interpolated' },
        { frame: 6, channels: ['position'], segmentAfter: 'interpolated' },
        { frame: 12, channels: ['position'], segmentAfter: 'interpolated' },
      ],
    });
    const params = (frame: number, item: string) =>
      cellMenu(keyed, frame)
        .groups.flatMap((g) => g.items)
        .find((i) => i.id === item)?.params;
    expect(params(3, 'timeline.cell.constant')).toEqual({ nodeIds: ['el_peg'], frame: 1, constant: true });
    expect(params(8, 'timeline.cell.nonConstant')).toEqual({
      nodeIds: ['el_peg'],
      frame: 6,
      constant: false,
    });
    expect(params(3, 'timeline.cell.uniformRight')).toEqual({ nodeId: 'el_peg', frame: 1, side: 'right' });
    expect(params(3, 'timeline.cell.uniformLeft')).toEqual({ nodeId: 'el_peg', frame: 6, side: 'left' });
    expect(params(6, 'timeline.cell.uniformLeft')).toEqual({ nodeId: 'el_peg', frame: 6, side: 'left' });
    // Add and Remove Keyframe act at the clicked frame itself.
    expect(params(3, 'timeline.cell.addKey')).toEqual({ nodeIds: ['el_peg'], frame: 3 });
  });
});
