// M5 ruling E-52 (C51B-R2S-02): the window's side of an agent's Shift+F10 answers what it did. A control that opens a
// menu takes the event in its own handler; the window's guard against the browser's menu (useNoBrowserMenus) prevents
// every contextmenu later, at the window, so the answer reads the event at the document, before that guard.
import { afterEach, describe, expect, it } from 'vitest';

import { openContextMenuFor } from './windowHooks';

const guard = (e: MouseEvent) => {
  e.preventDefault();
};

afterEach(() => {
  window.removeEventListener('contextmenu', guard);
  document.body.innerHTML = '';
});

function control(id: string, opensMenu: boolean): HTMLElement {
  const el = document.createElement('button');
  el.dataset['ttId'] = id;
  if (opensMenu)
    el.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const menu = document.createElement('div');
      menu.dataset['ttRole'] = 'menu';
      menu.dataset['ttId'] = `${id}.menu`;
      document.body.appendChild(menu);
    });
  document.body.appendChild(el);
  return el;
}

describe("an agent's Shift+F10 in the window", () => {
  it('answers opened with the menu when the control opens one', async () => {
    window.addEventListener('contextmenu', guard);
    control('with.menu', true);
    expect(await openContextMenuFor('with.menu')).toEqual({ outcome: 'opened', menuId: 'with.menu.menu' });
  });

  it('answers noMenu when the control takes no context menu, and gives the focus back', async () => {
    window.addEventListener('contextmenu', guard);
    const before = control('before', false);
    control('plain', false);
    before.focus();
    expect(await openContextMenuFor('plain')).toEqual({ outcome: 'noMenu', menuId: null });
    expect(document.activeElement).toBe(before);
  });

  it('answers notFound for no such control, and for no focused control', async () => {
    expect(await openContextMenuFor('nowhere')).toEqual({ outcome: 'notFound', menuId: null });
    (document.activeElement as HTMLElement | null)?.blur();
    expect(await openContextMenuFor(null)).toEqual({ outcome: 'notFound', menuId: null });
  });
});
