// The Timeline's context menus (reviews/M5-design-timeline-view.md section 5): every item's parameters are valid
// against its command's generated schema, ids are unique, and the items follow the clicked row's type.
import { describe, expect, it } from 'vitest';

import { paramProblems } from '../ipc';
import { cellMenu, rowMenu, rulerMenu, type TimelineMenu } from './menus';
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

const TYPES = ['drawing', 'annotation', 'peg', 'camera', 'clipping', 'skeleton', 'sound'];

function everyMenu(): [string, TimelineMenu][] {
  const rows = TYPES.map((t) => row(t));
  const out: [string, TimelineMenu][] = [];
  for (const r of rows) {
    out.push([`row ${r.type}`, rowMenu(r, rows, [])]);
    out.push([`row ${r.type} selected`, rowMenu(r, rows, [r.elementId, 'el_other'])]);
    out.push([`cell ${r.type}`, cellMenu(r, 12)]);
  }
  out.push(['ruler', rulerMenu(7)]);
  return out;
}

describe('the Timeline context menus', () => {
  it("give every item parameters its command's schema accepts", () => {
    for (const [name, menu] of everyMenu()) {
      for (const g of menu.groups) {
        for (const item of g.items) {
          expect(paramProblems(item.command, item.params ?? {}), `${name}: ${item.id}`).toEqual([]);
        }
      }
    }
  });

  it('give every item a unique id within its menu', () => {
    for (const [name, menu] of everyMenu()) {
      const ids = [...menu.groups.flatMap((g) => g.items.map((i) => i.id)), ...menu.dialogs.map((d) => d.id)];
      expect(new Set(ids).size, name).toBe(ids.length);
    }
  });

  it('follow the row: keyframe items on bars, extend children on pegs, annotation on drawings, loops when loopable', () => {
    const ids = (m: TimelineMenu) => [
      ...m.groups.flatMap((g) => g.items.map((i) => i.id)),
      ...m.dialogs.map((d) => d.id),
    ];
    expect(ids(cellMenu(row('peg'), 3))).toContain('timeline.cell.addKey');
    expect(ids(cellMenu(row('peg'), 3))).toContain('timeline.cell.extendChildren');
    expect(ids(cellMenu(row('drawing'), 3))).not.toContain('timeline.cell.addKey');
    expect(ids(rowMenu(row('drawing'), [], []))).toContain('timeline.row.annotation');
    expect(ids(rowMenu(row('peg'), [], []))).not.toContain('timeline.row.annotation');
    expect(ids(rowMenu(row('peg'), [], []))).toContain('timeline.row.changeLoops');
    expect(ids(rowMenu(row('camera'), [], []))).not.toContain('timeline.row.changeLoops');
    expect(ids(rowMenu(row('clipping'), [], []))).toContain('timeline.row.effectParameters');
  });

  it('act on the selected rows when the clicked row is one of them, else on the clicked row', () => {
    const r = row('peg');
    const del = (m: TimelineMenu) =>
      m.groups.flatMap((g) => g.items).find((i) => i.id === 'timeline.row.delete');
    expect(del(rowMenu(r, [r], ['el_a', 'el_peg']))?.params).toEqual({ elementIds: ['el_a', 'el_peg'] });
    expect(del(rowMenu(r, [r], ['el_a']))?.params).toEqual({ elementIds: ['el_peg'] });
  });

  it('offer Attach to Next Object only when the next row can be a parent', () => {
    const a = row('drawing', { elementId: 'a' });
    const peg = row('peg', { elementId: 'p' });
    const has = (m: TimelineMenu) =>
      m.groups.some((g) => g.items.some((i) => i.id === 'timeline.row.attachNext'));
    expect(has(rowMenu(a, [a, peg], []))).toBe(true);
    expect(has(rowMenu(peg, [peg, a], []))).toBe(false);
  });
});
