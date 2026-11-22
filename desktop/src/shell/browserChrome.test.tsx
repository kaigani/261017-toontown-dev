// Ruling D-70 item 2 (product build): the webview's own context menu opens only on text-entry elements, and the person
// cannot reload or navigate the page by any key. Every other secondary click opens the application's menu or nothing
// (the default is cancelled), and F5, Ctrl+R, Ctrl+Shift+R, Ctrl+F5, Alt+Left, Alt+Right, Backspace and the browser's
// own keys are cancelled, in a text field (the token field among them) or not.
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';
import { isBrowserAccelerator } from './windowHooks';

afterEach(() => {
  cleanup();
  document.querySelectorAll('[data-test-field]').forEach((e) => {
    e.remove();
  });
});

const OPEN = {
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

const PENS = {
  pens: [
    {
      penId: 'pen_1',
      name: 'Pen 1',
      type: 'brush',
      tipShape: 'round',
      minSize: 1,
      maxSize: 4,
      smoothness: 3,
      tipAngle: 0,
    },
    {
      penId: 'pen_3',
      name: 'Eraser 1',
      type: 'eraser',
      tipShape: 'round',
      minSize: 4,
      maxSize: 12,
      smoothness: 0,
      tipAngle: 0,
    },
  ],
  currentBrushPenId: 'pen_1',
  currentEraserPenId: 'pen_3',
};

async function mainWindow(): Promise<void> {
  const h = createScriptedHost({
    'app.state.get': () => ok(OPEN),
    'view.list': () => ok([{ viewId: 'cameraView', type: 'camera' }]),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
}

/** Whether the webview's default context menu would open on `el` (the event's default not cancelled). */
const defaultMenu = (el: Element) => fireEvent.contextMenu(el, { clientX: 20, clientY: 20 });

function textField(attrs: Record<string, string> = {}): HTMLInputElement {
  const e = document.createElement('input');
  e.type = 'text';
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  e.setAttribute('data-test-field', '');
  document.body.appendChild(e);
  return e;
}

describe('the webview context menu (D-70 item 2a)', () => {
  it('opens on a text field only; a toolbar button, the canvas and empty panel space cancel it', async () => {
    await mainWindow();
    expect(defaultMenu(textField())).toBe(true);
    const toolbar = screen.getByRole('toolbar', { name: 'Main toolbar' });
    const button = toolbar.querySelector('button');
    if (!button) throw new Error('no toolbar button');
    expect(defaultMenu(button)).toBe(false);
    // A list box and a slider are not text entry: the webview would show the page's menu there (C44A-R3 live, the
    // onion skin's list box on the main toolbar).
    for (const el of [
      document.createElement('select'),
      Object.assign(document.createElement('input'), { type: 'range' }),
    ]) {
      el.setAttribute('data-test-field', '');
      document.body.appendChild(el);
      expect([el.tagName, defaultMenu(el)]).toEqual([el.tagName, false]);
    }
    const select = toolbar.querySelector('select');
    if (select) expect(defaultMenu(select)).toBe(false);
    // Read-only text (the token field) and a text area keep the text-editing menu.
    expect(defaultMenu(textField({ readonly: '' }))).toBe(true);
    const area = document.createElement('textarea');
    area.setAttribute('data-test-field', '');
    document.body.appendChild(area);
    expect(defaultMenu(area)).toBe(true);
    expect(defaultMenu(toolbar)).toBe(false);
    const canvas = await waitFor(() => {
      const c = document.querySelector('[data-tt-id="cameraView.canvas"]');
      if (!c) throw new Error('no canvas');
      return c;
    });
    expect(defaultMenu(canvas)).toBe(false);
    const panel = document.querySelector('[data-tt-role="panel"]');
    if (!panel) throw new Error('no panel');
    expect(defaultMenu(panel)).toBe(false);
    expect(defaultMenu(document.body)).toBe(false);
  });

  it('a Pen panel row cancels it', async () => {
    const h = createScriptedHost({
      'app.state.get': () => ok(OPEN),
      'tool.pen.list': () => ok(PENS),
      'tool.active.get': () => ok({ toolId: 'brush' }),
      'tool.settings.get': () => ok({ placeBehind: false, symmetry: { mode: 'none' } }),
    });
    render(
      <HostProvider api={createHostApi(h)}>
        <App floatPanel="pen" />
      </HostProvider>,
    );
    await screen.findByText('Pen 1');
    const row = document.querySelector('[data-tt-id="pen.row.pen_1"]');
    if (!row) throw new Error('no pen row');
    expect(defaultMenu(row)).toBe(false);
  });
});

interface Chord {
  readonly name: string;
  readonly key: string;
  readonly code: string;
  readonly ctrlKey?: boolean;
  readonly shiftKey?: boolean;
  readonly altKey?: boolean;
}

const RELOADS: readonly Chord[] = [
  { name: 'F5', key: 'F5', code: 'F5' },
  { name: 'Shift+F5', key: 'F5', code: 'F5', shiftKey: true },
  { name: 'Ctrl+F5', key: 'F5', code: 'F5', ctrlKey: true },
  { name: 'Ctrl+R', key: 'r', code: 'KeyR', ctrlKey: true },
  { name: 'Ctrl+Shift+R', key: 'R', code: 'KeyR', ctrlKey: true, shiftKey: true },
  { name: 'Alt+Left', key: 'ArrowLeft', code: 'ArrowLeft', altKey: true },
  { name: 'Alt+Right', key: 'ArrowRight', code: 'ArrowRight', altKey: true },
  { name: 'BrowserBack', key: 'BrowserBack', code: 'BrowserBack' },
  { name: 'BrowserRefresh', key: 'BrowserRefresh', code: 'BrowserRefresh' },
];

describe('reload and navigation keys (D-70 item 2b)', () => {
  it('are cancelled in the token field (read-only), in a text field and outside any field', async () => {
    await mainWindow();
    const token = textField({ readonly: '', 'data-tt-id': 'agentAccess.token' });
    const field = textField();
    const canvas = await waitFor(() => {
      const c = document.querySelector('[data-tt-id="cameraView.canvas"]');
      if (!c) throw new Error('no canvas');
      return c;
    });
    for (const target of [token, field, canvas, document.body]) {
      for (const k of RELOADS) {
        const kept = fireEvent.keyDown(target, {
          key: k.key,
          code: k.code,
          ctrlKey: k.ctrlKey ?? false,
          shiftKey: k.shiftKey ?? false,
          altKey: k.altKey ?? false,
        });
        expect([target.getAttribute('data-tt-id') ?? target.tagName, k.name, kept]).toEqual([
          target.getAttribute('data-tt-id') ?? target.tagName,
          k.name,
          false,
        ]);
      }
    }
    // Backspace outside a field cannot navigate back; in a field it edits the text.
    expect(fireEvent.keyDown(canvas, { key: 'Backspace', code: 'Backspace' })).toBe(false);
    expect(fireEvent.keyDown(field, { key: 'Backspace', code: 'Backspace' })).toBe(true);
  });

  it("the macOS forms are the guard's too (Cmd+R, Cmd+Shift+R, Cmd+[, Cmd+])", () => {
    for (const seq of [
      'Primary+R',
      'Primary+Shift+R',
      'Primary+[',
      'Primary+]',
      'Primary+F5',
      'Alt+Left',
      'Backspace',
    ])
      expect([seq, isBrowserAccelerator(seq)]).toEqual([seq, true]);
  });
});
