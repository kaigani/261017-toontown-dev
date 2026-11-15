// @vitest-environment node
// The window's input path (design note 2 §2 and §6; design note 3 §2): G5 (samples as given), F5/F6 (a cancel sends
// nothing), the event rules (dt, pressure, kinds, buttons, modifiers), and the overlay of every tool (02 §4.2 layer 9).
import { describe, expect, it } from 'vitest';

import {
  GestureCollector,
  gestureOverlay,
  gestureRecord,
  kindOf,
  POLYLINE_CLOSE_PX,
  RECORD_EVENTS,
  type DomSample,
} from './input';

const s = (x: number, y: number, t: number, more: Partial<DomSample> = {}): DomSample => ({
  x,
  y,
  pressure: 0.5,
  tiltX: 0,
  tiltY: 0,
  pointerType: 'mouse',
  button: 0,
  buttons: 1,
  shiftKey: false,
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  timeStamp: t,
  ...more,
});

describe('the gesture collector', () => {
  it('G5: a press, every sample as given (coalesced ones in order), a release; dt in whole milliseconds', () => {
    const c = new GestureCollector('drawingView');
    expect(c.down(s(10, 20, 1000.2))).toBe(true);
    c.move([s(11, 21, 1004.9), s(12.5, 22.25, 1008.4)]);
    c.move([s(13, 23, 1016.6)]);
    const req = c.up(s(14, 24, 1020.1));
    expect(req?.view).toBe('drawingView');
    expect(req?.pointerType).toBe('mouse');
    expect(req?.events.map((e) => [e.type, e.x, e.y, e.dt])).toEqual([
      ['down', 10, 20, 0],
      ['move', 11, 21, 5],
      ['move', 12.5, 22.25, 4],
      ['move', 13, 23, 8],
      ['up', 14, 24, 4],
    ]);
    expect(c.active).toBe(false);
  });

  it('a mouse has pressure 1 (02 §13.3); a pen its own, with tilt; the eraser end is penEraser', () => {
    const m = new GestureCollector('v');
    m.down(s(0, 0, 0, { pressure: 0.5 }));
    expect(m.up(s(1, 1, 1))?.events.every((e) => e.pressure === 1 && e.tiltX === undefined)).toBe(true);
    const p = new GestureCollector('v');
    p.down(s(0, 0, 0, { pointerType: 'pen', pressure: 0.25, tiltX: 30, tiltY: -100 }));
    const req = p.up(s(1, 1, 1, { pointerType: 'pen', pressure: 0.75 }));
    expect(req?.pointerType).toBe('pen');
    expect(req?.events.map((e) => [e.pressure, e.tiltX, e.tiltY])).toEqual([
      [0.25, 30, -90],
      [0.75, 0, 0],
    ]);
    expect(kindOf({ pointerType: 'pen', buttons: 32 })).toBe('penEraser');
    expect(kindOf({ pointerType: 'eraser', buttons: 1 })).toBe('penEraser');
    expect(kindOf({ pointerType: 'touch', buttons: 1 })).toBe('mouse');
  });

  it('carries the button at the press and the modifiers on every event', () => {
    const c = new GestureCollector('v');
    c.down(s(0, 0, 0, { button: 2, shiftKey: true }));
    const req = c.up(s(1, 1, 1, { altKey: true, metaKey: true }));
    expect(req?.events[0]?.button).toBe('right');
    expect(req?.events[0]?.modifiers).toEqual(['shift']);
    expect(req?.events[1]?.button).toBeUndefined();
    expect(req?.events[1]?.modifiers).toEqual(['alt', 'meta']);
    expect(new GestureCollector('v').down(s(0, 0, 0, { button: 3 }))).toBe(false);
  });

  it('F5/F6: a cancel sends nothing and leaves the collector idle; one gesture at a time', () => {
    const c = new GestureCollector('v');
    c.down(s(0, 0, 0));
    expect(c.down(s(5, 5, 1))).toBe(false); // a second press during a gesture starts nothing
    c.move([s(1, 1, 2)]);
    expect(c.cancel()).toBe(true);
    expect(c.points).toEqual([]);
    expect(c.up(s(2, 2, 3))).toBeNull();
    expect(c.cancel()).toBe(false);
  });

  it('a wheel step is a one-event gesture, refused during a gesture (G6)', () => {
    const c = new GestureCollector('v');
    expect(c.wheel(s(5, 6, 0, { ctrlKey: true }), -120)?.events).toEqual([
      { type: 'wheel', x: 5, y: 6, modifiers: ['ctrl'], dt: 0, wheelDelta: -120 },
    ]);
    c.down(s(0, 0, 0));
    expect(c.wheel(s(0, 0, 1), 120)).toBeNull();
  });
});

describe('the overlay of every tool (SI-138)', () => {
  const drag: [number, number][] = [
    [10, 10],
    [40, 25],
    [80, 60],
  ];
  // Every tool id of the kernel (tool.rs ToolId): an overlay while dragging, or nothing by design.
  const cells: [string, boolean][] = [
    ['brush', true],
    ['pencil', true],
    ['polyline', true],
    ['eraser', true],
    ['rectangle', true],
    ['ellipse', true],
    ['line', true],
    ['closeGap', true],
    ['select', true],
    ['zoom', true],
    ['cutter', true],
    ['contourEditor', true],
    ['grabber', false],
    ['paint', false],
    ['unpaint', false],
    ['paintUnpainted', false],
    ['dropper', false],
    ['scissor', false],
    ['perspective', false],
    ['repositionAll', false],
    ['stroke', false],
    ['editTexture', false],
  ];
  it.each(cells)('%s: overlay %s', (tool, shown) => {
    const o = gestureOverlay(drag, { tool, ink: [0, 0, 0, 255], width: 4 });
    expect(o !== null).toBe(shown);
  });

  it('marquees are dashed accent outlines one pixel wide; ink follows the style', () => {
    const marquee = gestureOverlay(drag, { tool: 'select', ink: [9, 9, 9, 255], width: 9 });
    const colours = new Set<string>();
    const d = marquee?.data ?? [];
    for (let i = 0; i < d.length; i += 6) colours.add(d.slice(i + 2, i + 6).join(','));
    expect([...colours]).toEqual([[32, 128, 255, 255].map((c) => c / 255).join(',')]);
    const ink = gestureOverlay(drag, { tool: 'brush', ink: [255, 0, 0, 255], width: 6 });
    expect(ink?.data.slice(2, 6)).toEqual([1, 0, 0, 1]);
    expect(gestureOverlay([], { tool: 'brush', ink: [0, 0, 0, 255], width: 1 })).toBeNull();
  });
});

describe('the record of the last committed gesture (C42B-R1-03; SI-141 item 9)', () => {
  it('keeps the events as committed, the mapping, the tool and the commands the commit produced', () => {
    const c = new GestureCollector('drawingView');
    c.down(s(10, 20, 100, { pointerType: 'pen', pressure: 0.3, tiltX: 12, shiftKey: true }));
    c.move([s(12, 22, 104, { pointerType: 'pen', pressure: 0.6 })]);
    const req = c.up(s(14, 25, 109, { pointerType: 'pen', pressure: 0.2 }));
    expect(req).not.toBeNull();
    if (!req) return;
    const env = {
      status: 'ok',
      result: {
        commands: [
          { commandId: 'tool.active.set', params: { toolId: 'brush' }, extra: 1 },
          { commandId: 'draw.stroke.add', params: { kind: 'brush' } },
        ],
      },
    };
    const r = gestureRecord('user', 'brush', [1, 0, 0, -1, 400, 250], req, env);
    expect(r.events).toEqual(req.events); // as committed: pressure, tilt, modifiers, dt
    expect(r.events[0]?.pressure).toBe(0.3);
    expect(r.events[0]?.modifiers).toEqual(['shift']);
    expect([r.origin, r.tool, r.view, r.pointerType, r.eventCount, r.status, r.code]).toEqual([
      'user',
      'brush',
      'drawingView',
      'pen',
      3,
      'ok',
      null,
    ]);
    expect(r.drawingToView).toEqual([1, 0, 0, -1, 400, 250]);
    expect(r.commands).toEqual([
      { commandId: 'tool.active.set', params: { toolId: 'brush' } },
      { commandId: 'draw.stroke.add', params: { kind: 'brush' } },
    ]);
    const refused = gestureRecord('agent', 'paint', [1, 0, 0, 1, 0, 0], req, {
      status: 'error',
      error: { code: 'drawing.locked' },
    });
    expect([refused.origin, refused.status, refused.code, refused.commands]).toEqual([
      'agent',
      'error',
      'drawing.locked',
      [],
    ]);
  });

  it('a gesture longer than the cap is recorded by its count', () => {
    const events = Array.from({ length: RECORD_EVENTS + 1 }, (_, i) => ({
      type: 'move' as const,
      x: i,
      y: 0,
      modifiers: [],
      dt: 1,
    }));
    const env = {
      status: 'ok',
      result: { commands: [{ commandId: 'draw.stroke.add', params: { points: [] } }] },
    };
    const r = gestureRecord('user', 'brush', [1, 0, 0, 1, 0, 0], { view: 'v', events }, env);
    expect([r.events.length, r.eventCount]).toEqual([0, RECORD_EVENTS + 1]);
    expect(r.commands).toEqual([{ commandId: 'draw.stroke.add', params: null }]);
    const whole = gestureRecord(
      'user',
      'brush',
      [1, 0, 0, 1, 0, 0],
      { view: 'v', events: events.slice(1) },
      { status: 'ok' },
    );
    expect(whole.events.length).toBe(RECORD_EVENTS);
  });
});

describe('the Polyline: its clicks are one gesture (02 §7.3.5; SI-131 item 10)', () => {
  const click = (c: GestureCollector, x: number, y: number, t: number) => {
    expect(c.down(s(x, y, t))).toBe(true);
    return c.up(s(x, y, t + 5));
  };

  it('a double click commits the clicks with a doubleClick event, as the headless pointer receives them', () => {
    const c = new GestureCollector('drawingView');
    c.multi = true;
    expect(click(c, 10, 10, 0)).toBeNull(); // pending
    expect(c.active && !c.pressed).toBe(true);
    expect(click(c, 60, 40, 100)).toBeNull();
    const req = c.doubleClick(s(60, 40, 110));
    expect(req?.events.map((e) => e.type)).toEqual(['down', 'up', 'down', 'up', 'doubleClick']);
    expect(c.active).toBe(false);
  });

  it('a click back on the first click closes and commits it; a click-drag gives a handle (moves kept)', () => {
    const c = new GestureCollector('drawingView');
    c.multi = true;
    click(c, 10, 10, 0);
    c.down(s(60, 40, 100));
    c.move([s(70, 45, 105)]);
    expect(c.up(s(80, 50, 110))).toBeNull();
    const req = click(c, 10 + POLYLINE_CLOSE_PX - 1, 10, 200);
    expect(req?.events.map((e) => e.type)).toEqual(['down', 'up', 'down', 'move', 'up', 'down', 'up']);
  });

  it('Enter ends it as it stands (02 §7.2.5); a press continuing the double click that ended it opens nothing', () => {
    const c = new GestureCollector('drawingView');
    c.multi = true;
    click(c, 10, 10, 0);
    click(c, 60, 40, 100);
    expect(c.end()?.events.map((e) => e.type)).toEqual(['down', 'up', 'down', 'up']);
    expect(c.active).toBe(false);
    expect(c.end()).toBeNull(); // nothing open
    // A double click ends a Polyline; the press continuing it (a triple click: soon after and on the same spot; a
    // pointer press carries no click count) does not open the next one, nor does a fourth.
    click(c, 10, 10, 200);
    click(c, 60, 40, 300);
    expect(c.doubleClick(s(60, 40, 310))).not.toBeNull();
    expect(c.down(s(61, 41, 380))).toBe(false);
    expect(c.up(s(61, 41, 400))).toBeNull();
    expect(c.down(s(60, 40, 850))).toBe(false); // within 500 ms of the previous continuing press
    expect(c.active).toBe(false);
    // A press elsewhere, or later, opens the next Polyline.
    expect(c.down(s(90, 90, 900))).toBe(true);
    c.cancel();
    click(c, 10, 10, 1000);
    click(c, 60, 40, 1100);
    expect(c.doubleClick(s(60, 40, 1110))).not.toBeNull();
    expect(c.down(s(60, 40, 1700))).toBe(true);
  });

  it('a cancel between clicks discards it; outside multi-click mode a release commits at once', () => {
    const c = new GestureCollector('drawingView');
    c.multi = true;
    click(c, 10, 10, 0);
    expect(c.cancel()).toBe(true);
    expect(c.active).toBe(false);
    const single = new GestureCollector('drawingView');
    expect(click(single, 10, 10, 0)?.events.map((e) => e.type)).toEqual(['down', 'up']);
    expect(single.doubleClick(s(10, 10, 20))).toBeNull();
  });
});
