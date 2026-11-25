// M5 ruling E-52 (C51B-R2S-02): the window's side of an agent's Shift+F10 answers what it did. A control that opens a
// menu takes the event in its own handler; the window's guard against the browser's menu (useNoBrowserMenus) prevents
// every contextmenu later, at the window, so the answer reads the event at the document, before that guard.
import { cleanup, render, waitFor } from '@testing-library/react';
import { createElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import type { AgentContextMenu, EngineApi } from '../ipc';
import type { ShellServices } from '../state/context';
import { openContextMenuFor, useAgentContextMenu } from './windowHooks';

const guard = (e: MouseEvent) => {
  e.preventDefault();
};

afterEach(() => {
  cleanup();
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

/** A control whose menu replaces any open one, as the window's context menus do (one open at a time). */
function replacing(id: string): HTMLElement {
  const el = document.createElement('button');
  el.dataset['ttId'] = id;
  el.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    for (const m of document.querySelectorAll('[data-tt-role="menu"]')) m.remove();
    const menu = document.createElement('div');
    menu.dataset['ttRole'] = 'menu';
    menu.dataset['ttId'] = `${id}.menu`;
    document.body.appendChild(menu);
  });
  document.body.appendChild(el);
  return el;
}

/** The window's hook with a host that delivers requests and records the answers. */
function served(generation: number) {
  const answers: { agentRequest: number; menuId: string | null; outcome: string }[] = [];
  const deliver: { to: ((r: AgentContextMenu) => void) | null } = { to: null };
  const engine = {
    generation: () => generation,
    onAgentContextMenu: (handler: (r: AgentContextMenu) => void) => {
      deliver.to = handler;
      return Promise.resolve(() => undefined);
    },
    contextMenuAnswer: (a: { agentRequest: number; menuId: string | null; outcome: string }) => {
      answers.push(a);
      return Promise.resolve();
    },
  } as unknown as EngineApi;
  function Probe() {
    useAgentContextMenu({ engine } as unknown as ShellServices);
    return null;
  }
  render(createElement(Probe));
  return { answers, deliver };
}

// M5 ruling E-62 (C51B-R3S-01): the window serves one request at a time, so each answer names the menu that request
// opened; two requests in one frame no longer both read the second's menu.
describe("the window's agent context-menu requests", () => {
  it('two requests in the same frame each answer with their own menu', async () => {
    window.addEventListener('contextmenu', guard);
    replacing('timeline.tracks');
    replacing('timeline.ruler');
    const { answers, deliver } = served(3);
    await waitFor(() => {
      expect(deliver.to).not.toBeNull();
    });
    deliver.to?.({ requestId: 1, target: 'timeline.tracks', generation: 3 });
    deliver.to?.({ requestId: 2, target: 'timeline.ruler', generation: 3 });
    await waitFor(() => {
      expect(answers).toHaveLength(2);
    });
    expect(answers.map((a) => [a.agentRequest, a.outcome, a.menuId])).toEqual([
      [1, 'opened', 'timeline.tracks.menu'],
      [2, 'opened', 'timeline.ruler.menu'],
    ]);
  });

  it('a request of an earlier engine generation opens nothing', async () => {
    window.addEventListener('contextmenu', guard);
    replacing('timeline.tracks');
    const { answers, deliver } = served(4);
    await waitFor(() => {
      expect(deliver.to).not.toBeNull();
    });
    deliver.to?.({ requestId: 1, target: 'timeline.tracks', generation: 3 });
    deliver.to?.({ requestId: 2, target: 'timeline.tracks', generation: 4 });
    await waitFor(() => {
      expect(answers).toHaveLength(1);
    });
    expect(answers[0]?.agentRequest).toBe(2);
  });
});
