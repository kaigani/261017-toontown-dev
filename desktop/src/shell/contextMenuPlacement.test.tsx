// D-60 item 1 (C44A-R1-01): a context menu is always fully inside the window's client area. The placement
// function at the four corners, a menu taller and one wider than the window; the component placing and
// scrolling a menu taller than the window.
// D-66 item 1 (C44A-R1-01 at 800 by 500): the same at the main window's minimum client area (800 by 500, tauri.conf.json
// `minWidth`/`minHeight`) and a float's (160 by 120, `floats.rs` `min_inner_size`): the outer box, border and padding
// included, inside; an arrow scrolls on a press and while the pointer rests on it; the keyboard's focus scrolls the
// focused item fully into view; a scroll (the wheel's, or an arrow's) survives the menu being measured again.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok, type ScriptedHost } from '../test-support/scriptedHost';
import { ARROW_STEP_PX, HOLD_DELAY_MS, HOLD_REPEAT_MS, HOLD_STEP_PX } from './ContextMenu';
import { MENU_MARGIN, placeMenu, type MenuPlacement, type Size } from './contextMenuModel';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const AREA: Size = { width: 1280, height: 800 };
/** The main window's least client area (tauri.conf.json `minWidth` 800, `minHeight` 500) and a float's
 *  (`floats.rs`, `min_inner_size(160, 120)`). */
const MIN_MAIN: Size = { width: 800, height: 500 };
const MIN_FLOAT: Size = { width: 160, height: 120 };
const MENU: Size = { width: 200, height: 300 };

/** Every edge of the placed menu inside the area, with the margin. */
function inside(p: MenuPlacement, menu: Size, area: Size): boolean {
  const w = p.maxWidth ?? menu.width;
  const h = p.maxHeight ?? menu.height;
  return (
    p.x >= MENU_MARGIN &&
    p.y >= MENU_MARGIN &&
    p.x + w <= area.width - MENU_MARGIN &&
    p.y + h <= area.height - MENU_MARGIN
  );
}

describe('placeMenu', () => {
  it('opens at the pointer when it fits', () => {
    expect(placeMenu({ x: 100, y: 100 }, MENU, AREA)).toEqual({
      x: 100,
      y: 100,
      maxHeight: null,
      maxWidth: null,
    });
  });

  it('flips toward the side with room at each corner', () => {
    // Top left: opens right and down.
    const tl = placeMenu({ x: 10, y: 10 }, MENU, AREA);
    expect([tl.x, tl.y]).toEqual([10, 10]);
    // Top right: flips left, opens down.
    const tr = placeMenu({ x: 1270, y: 10 }, MENU, AREA);
    expect([tr.x, tr.y]).toEqual([1270 - 200, 10]);
    // Bottom left: opens right, flips up.
    const bl = placeMenu({ x: 10, y: 790 }, MENU, AREA);
    expect([bl.x, bl.y]).toEqual([10, 790 - 300]);
    // Bottom right: flips both ways.
    const br = placeMenu({ x: 1270, y: 790 }, MENU, AREA);
    expect([br.x, br.y]).toEqual([1270 - 200, 790 - 300]);
    for (const p of [tl, tr, bl, br]) expect(inside(p, MENU, AREA)).toBe(true);
  });

  it('shifts inside when neither side has room (the reported case: opened at 499,454 at 1280 by 800)', () => {
    const tall: Size = { width: 220, height: 601 };
    const p = placeMenu({ x: 499, y: 454 }, tall, AREA);
    expect(p.maxHeight).toBeNull();
    expect(inside(p, tall, AREA)).toBe(true);
    expect(p.y).toBe(800 - MENU_MARGIN - 601);
  });

  it('scrolls a menu taller than the window, from the top margin', () => {
    const taller: Size = { width: 220, height: 1034 };
    const p = placeMenu({ x: 1115, y: 690 }, taller, AREA);
    expect(p.y).toBe(MENU_MARGIN);
    expect(p.maxHeight).toBe(800 - 2 * MENU_MARGIN);
    expect(inside(p, taller, AREA)).toBe(true);
  });

  it('bounds a menu wider than the window', () => {
    const wide: Size = { width: 2000, height: 100 };
    const p = placeMenu({ x: 600, y: 100 }, wide, AREA);
    expect([p.x, p.maxWidth]).toEqual([MENU_MARGIN, 1280 - 2 * MENU_MARGIN]);
    expect(inside(p, wide, AREA)).toBe(true);
  });

  it('keeps every corner and edge placement inside at the minimum client areas (D-66 item 1)', () => {
    for (const area of [MIN_MAIN, MIN_FLOAT]) {
      for (const menu of [MENU, { width: 220, height: 1034 }, { width: 2000, height: 40 }]) {
        for (const fx of [0, 0.5, 1])
          for (const fy of [0, 0.5, 1]) {
            const p = placeMenu({ x: area.width * fx, y: area.height * fy }, menu, area);
            expect(inside(p, menu, area)).toBe(true);
          }
      }
    }
  });

  it('keeps a menu opened outside the area (a pointer past the edge) inside', () => {
    for (const at of [
      { x: -50, y: -50 },
      { x: 1400, y: 900 },
    ]) {
      expect(inside(placeMenu(at, MENU, AREA), MENU, AREA)).toBe(true);
    }
  });
});

const STATE = {
  ...NO_PROJECT_STATE,
  project: {
    id: 'pr_1',
    name: 'Walk',
    path: '/p/Walk.animpkg',
    dirty: false,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

async function app(): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok(STATE),
    'view.list': () => ok([{ viewId: 'cameraView', type: 'camera' }]),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  return h;
}

/** jsdom lays nothing out: the menu's natural size and the client area are stated. */
function sizes(menu: Size, area: Size = AREA): void {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(menu.height);
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(menu.width);
  vi.spyOn(document.documentElement, 'clientWidth', 'get').mockReturnValue(area.width);
  vi.spyOn(document.documentElement, 'clientHeight', 'get').mockReturnValue(area.height);
}

const ITEM_PX = 24;
const LIST_TOP = 20;

/** The scrolling list laid out as a browser does: items `ITEM_PX` tall, `visible` of them showing while the menu is
 *  bounded; unbounded (while it is measured) nothing overflows, so the browser drops the scroll position, as the
 *  live run's WebView2 did. */
function layOut(menu: HTMLElement, visible: number, natural = 1034) {
  const list = menu.querySelector<HTMLElement>('.context-menu-items');
  if (!list) throw new Error('no item list');
  const items = [...menu.querySelectorAll<HTMLElement>('[role^="menuitem"]')];
  const scrollHeight = items.length * ITEM_PX;
  const bounded = () => menu.style.maxHeight !== '';
  const clientHeight = () => (bounded() ? visible * ITEM_PX : scrollHeight);
  let top = 0;
  Object.defineProperty(list, 'scrollTop', {
    configurable: true,
    get: () => {
      if (!bounded()) top = 0;
      return top;
    },
    set: (v: number) => {
      top = Math.min(Math.max(v, 0), Math.max(scrollHeight - clientHeight(), 0));
    },
  });
  Object.defineProperty(list, 'scrollHeight', { configurable: true, get: () => scrollHeight });
  Object.defineProperty(list, 'clientHeight', { configurable: true, get: clientHeight });
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (this: HTMLElement) {
    // Reading the unbounded menu's size lays it out unbounded: the scroll position is lost.
    if (this === menu && !bounded()) top = 0;
    return natural;
  });
  const rect = (t: number, h: number) =>
    ({ x: 0, y: t, left: 0, top: t, right: 200, bottom: t + h, width: 200, height: h, toJSON: () => ({}) }) as DOMRect;
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    if (this === list) return rect(LIST_TOP, clientHeight());
    const i = items.indexOf(this as HTMLElement);
    if (i >= 0) return rect(LIST_TOP + i * ITEM_PX - top, ITEM_PX);
    return rect(0, 0);
  });
  const shows = (i: number) => {
    const r = items[i]?.getBoundingClientRect();
    return !!r && r.top >= LIST_TOP && r.bottom <= LIST_TOP + clientHeight();
  };
  return { list, items, top: () => top, shows };
}

const arrow = (menu: HTMLElement, dir: 'up' | 'down') => {
  const a = menu.querySelector<HTMLElement>(`[data-tt-arrow="${dir}"]`);
  if (!a) throw new Error(`no ${dir} arrow`);
  return a;
};

async function openSceneMenu(x: number, y: number): Promise<HTMLElement> {
  const view = await waitFor(() => {
    const e = document.querySelector('[data-tt-id="cameraView.canvas"]');
    if (!(e instanceof HTMLElement)) throw new Error('no canvas');
    return e;
  });
  act(() => {
    fireEvent.contextMenu(view, { clientX: x, clientY: y });
  });
  return screen.findByRole('menu', { name: 'Scene' });
}

describe('ContextMenu placement', () => {
  it('places a menu taller than the window inside it, scrolling between two arrows', async () => {
    await app();
    sizes({ width: 220, height: 1034 });
    const menu = await openSceneMenu(1115, 690);
    // It would cross the right edge: it flips to end at the pointer.
    expect(menu.style.left).toBe(`${String(1115 - 220)}px`);
    expect(menu.style.top).toBe(`${String(MENU_MARGIN)}px`);
    expect(menu.style.maxHeight).toBe(`${String(AREA.height - 2 * MENU_MARGIN)}px`);
    expect(menu.querySelectorAll('.context-menu-arrow')).toHaveLength(2);
    // The arrows are affordances, not items: neither is a menu item nor takes the focus.
    for (const a of menu.querySelectorAll('.context-menu-arrow')) {
      expect(a.getAttribute('role')).toBeNull();
      expect(a.getAttribute('aria-hidden')).toBe('true');
    }
  });

  it('flips a menu that fits at the bottom right corner and shows no arrows', async () => {
    await app();
    sizes({ width: 200, height: 120 });
    const menu = await openSceneMenu(1270, 790);
    expect([menu.style.left, menu.style.top]).toEqual(['1070px', '670px']);
    expect(menu.style.maxHeight).toBe('');
    expect(menu.querySelectorAll('.context-menu-arrow')).toHaveLength(0);
  });
  it('a shown menu keeps its place while it still fits, and moves only when it would cross an edge', async () => {
    await app();
    sizes({ width: 200, height: 120 });
    const menu = await openSceneMenu(100, 100);
    expect([menu.style.left, menu.style.top]).toEqual(['100px', '100px']);
    // The availability answers widen it; it still fits where it is: it does not move (the item under the pointer
    // stays under it).
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(320);
    act(() => {
      window.dispatchEvent(new Event('resize'));
    });
    expect([menu.style.left, menu.style.top]).toEqual(['100px', '100px']);
    // Grown past the bottom edge: placed again, inside.
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(760);
    act(() => {
      window.dispatchEvent(new Event('resize'));
    });
    expect(menu.style.top).toBe(`${String(AREA.height - MENU_MARGIN - 760)}px`);
  });
  for (const area of [MIN_MAIN, MIN_FLOAT]) {
    it(`at ${String(area.width)} by ${String(area.height)} the outer box, arrows included, is inside the client area`, async () => {
      await app();
      sizes({ width: 220, height: 1034 }, area);
      for (const [x, y] of [
        [0, 0],
        [area.width, 0],
        [0, area.height],
        [area.width, area.height],
        [area.width / 2, area.height / 2],
      ] as const) {
        const menu = await openSceneMenu(x, y);
        // The bounds are the outer box's: border and padding inside them; the arrows are inside that box.
        expect(menu.style.boxSizing).toBe('border-box');
        const top = Number.parseFloat(menu.style.top);
        const left = Number.parseFloat(menu.style.left);
        const height = Number.parseFloat(menu.style.maxHeight);
        expect(top).toBe(MENU_MARGIN);
        expect(top + height).toBeLessThanOrEqual(area.height - MENU_MARGIN);
        const width = menu.style.maxWidth ? Number.parseFloat(menu.style.maxWidth) : 220;
        expect(left).toBeGreaterThanOrEqual(MENU_MARGIN);
        expect(left + width).toBeLessThanOrEqual(area.width - MENU_MARGIN);
        // Narrower than the menu's least width (a float): the bound wins.
        if (area.width - 2 * MENU_MARGIN < 220) expect(menu.style.minWidth).toBe('0px');
        expect(menu.querySelectorAll('.context-menu-arrow')).toHaveLength(2);
        for (const a of menu.querySelectorAll('.context-menu-arrow')) expect(menu.contains(a)).toBe(true);
        act(() => {
          fireEvent.keyDown(menu, { key: 'Escape' });
        });
        await waitFor(() => {
          expect(screen.queryByRole('menu')).toBeNull();
        });
      }
    });
  }

  it('an arrow press scrolls, and the scroll survives the menu being measured again (the wheel too)', async () => {
    await app();
    sizes({ width: 220, height: 1034 }, MIN_MAIN);
    const menu = await openSceneMenu(400, 250);
    const l = layOut(menu, 2);
    expect(l.items.length).toBeGreaterThanOrEqual(4);
    expect(l.top()).toBe(0);
    act(() => {
      fireEvent.pointerDown(arrow(menu, 'down'));
      fireEvent.pointerLeave(arrow(menu, 'down'));
    });
    const step = Math.min(ARROW_STEP_PX, l.items.length * ITEM_PX - 2 * ITEM_PX);
    expect(l.top()).toBe(step);
    // A render measures the menu again (a resize, an availability answer): the position stays.
    act(() => {
      window.dispatchEvent(new Event('resize'));
    });
    expect(l.top()).toBe(step);
    expect(arrow(menu, 'up').className).not.toContain('spent');
    // The wheel scrolls the list natively; its scroll event re-renders the arrows: the position stays.
    act(() => {
      l.list.scrollTop = ITEM_PX;
      fireEvent.scroll(l.list);
    });
    act(() => {
      window.dispatchEvent(new Event('resize'));
    });
    expect(l.top()).toBe(ITEM_PX);
    act(() => {
      fireEvent.pointerDown(arrow(menu, 'up'));
      fireEvent.pointerLeave(arrow(menu, 'up'));
    });
    expect(l.top()).toBe(0);
    expect(arrow(menu, 'up').className).toContain('spent');
  });

  it('an arrow scrolls while the pointer rests on it, and stops when it leaves', async () => {
    await app();
    sizes({ width: 220, height: 1034 }, MIN_MAIN);
    const menu = await openSceneMenu(400, 250);
    const l = layOut(menu, 2);
    vi.useFakeTimers();
    try {
      act(() => {
        fireEvent.pointerEnter(arrow(menu, 'down'));
      });
      act(() => {
        vi.advanceTimersByTime(HOLD_DELAY_MS - 1);
      });
      expect(l.top()).toBe(0);
      act(() => {
        vi.advanceTimersByTime(1 + 3 * HOLD_REPEAT_MS);
      });
      expect(l.top()).toBe(3 * HOLD_STEP_PX);
      act(() => {
        fireEvent.pointerLeave(arrow(menu, 'down'));
        vi.advanceTimersByTime(10 * HOLD_REPEAT_MS);
      });
      expect(l.top()).toBe(3 * HOLD_STEP_PX);
      // Resting on it long enough reaches the last item.
      act(() => {
        fireEvent.pointerEnter(arrow(menu, 'down'));
        vi.advanceTimersByTime(HOLD_DELAY_MS + 200 * HOLD_REPEAT_MS);
      });
      expect(l.shows(l.items.length - 1)).toBe(true);
      expect(arrow(menu, 'down').className).toContain('spent');
    } finally {
      vi.useRealTimers();
    }
  });

  it('moving the focus with the keyboard scrolls the focused item fully into view', async () => {
    await app();
    sizes({ width: 220, height: 1034 }, MIN_MAIN);
    const menu = await openSceneMenu(400, 250);
    const l = layOut(menu, 2);
    const last = l.items.length - 1;
    act(() => {
      fireEvent.keyDown(menu, { key: 'End' });
    });
    expect(document.activeElement).toBe(l.items[last]);
    expect(l.shows(last)).toBe(true);
    for (let i = last - 1; i >= 0; i -= 1) {
      act(() => {
        fireEvent.keyDown(menu, { key: 'ArrowUp' });
      });
      expect(document.activeElement).toBe(l.items[i]);
      expect(l.shows(i)).toBe(true);
    }
    for (let i = 1; i <= last; i += 1) {
      act(() => {
        fireEvent.keyDown(menu, { key: 'ArrowDown' });
      });
      expect(document.activeElement).toBe(l.items[i]);
      expect(l.shows(i)).toBe(true);
    }
    act(() => {
      fireEvent.keyDown(menu, { key: 'Home' });
    });
    expect(l.shows(0)).toBe(true);
    expect(l.top()).toBe(0);
  });

  it('is placed in client-area coordinates when an ancestor is the containing block of fixed elements', async () => {
    await app();
    sizes({ width: 200, height: 120 });
    // The dock's tab: `left: 0; top: 0` puts the menu at (955, 540) of the client area.
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
      x: 955,
      y: 540,
      left: 955,
      top: 540,
      right: 1155,
      bottom: 660,
      width: 200,
      height: 120,
      toJSON: () => ({}),
    });
    const menu = await openSceneMenu(100, 100);
    // Drawn at (100, 100) of the client area: the style is relative to that origin.
    expect([menu.style.left, menu.style.top]).toEqual(['-855px', '-440px']);
  });
});
