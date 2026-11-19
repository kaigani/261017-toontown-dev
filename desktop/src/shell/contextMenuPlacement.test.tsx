// D-60 item 1 (C44A-R1-01): a context menu is always fully inside the window's client area. The placement
// function at the four corners, a menu taller and one wider than the window; the component placing and
// scrolling a menu taller than the window.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok, type ScriptedHost } from '../test-support/scriptedHost';
import { MENU_MARGIN, placeMenu, type MenuPlacement, type Size } from './contextMenuModel';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const AREA: Size = { width: 1280, height: 800 };
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
    expect(placeMenu({ x: 100, y: 100 }, MENU, AREA)).toEqual({ x: 100, y: 100, maxHeight: null, maxWidth: null });
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
function sizes(menu: Size): void {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(menu.height);
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(menu.width);
  vi.spyOn(document.documentElement, 'clientWidth', 'get').mockReturnValue(AREA.width);
  vi.spyOn(document.documentElement, 'clientHeight', 'get').mockReturnValue(AREA.height);
}

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
    expect(menu.style.left).toBe(`${String(AREA.width - MENU_MARGIN - 220)}px`);
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
});
