// The drawing inspector (02 §9; M4 C4.4a, reviews/M4-design-panels.md §6): the current element's drawing at the
// current frame; the name commits on Enter and shows a refusal under the field; Show preview sends its preference; the
// Browse slider shows another drawing's thumbnail and sends no command; a rename made elsewhere shows at once.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import {
  createScriptedHost,
  fail,
  NO_PROJECT_STATE,
  ok,
  type ScriptedHost,
} from '../test-support/scriptedHost';

afterEach(cleanup);

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
  element: { id: 'el_1', name: 'Bob', type: 'drawing' },
};

function list(firstName = '1') {
  return {
    drawings: [
      { id: 'dr_1', name: firstName, exposures: [{ from: 1, to: 12 }] },
      { id: 'dr_2', name: '2', exposures: [{ from: 13, to: 24 }] },
    ],
  };
}

async function inspector(
  extra: Record<string, (p: Record<string, unknown>) => unknown> = {},
): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok(STATE),
    'element.drawing.list': () => ok(list()),
    'draw.drawing.render': (p) => ok({ png: `png-of-${String(p['drawingId'])}`, width: 160, height: 90 }),
    'prefs.get': () => ok({ values: { 'drawing.inspectorPreview': true } }),
    ...extra,
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App floatPanel="properties" />
    </HostProvider>,
  );
  await screen.findByRole('tab', { name: 'Drawing' });
  act(() => {
    fireEvent.click(screen.getByRole('tab', { name: 'Drawing' }));
  });
  await screen.findByText('Show preview');
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const sent = (h: ScriptedHost, command: string) =>
  h
    .actions()
    .filter((a) => a.command === command)
    .map((a) => a.params);

describe('the drawing inspector (02 §9)', () => {
  it('shows the drawing at the current frame with its thumbnail; Show preview sends its preference', async () => {
    const h = await inspector();
    expect((el('inspector.name') as HTMLInputElement).value).toBe('1');
    expect(el('inspector.element').textContent).toContain('frames 1-12');
    await waitFor(() => {
      expect(el('inspector.thumbnail').getAttribute('src')).toBe('data:image/png;base64,png-of-dr_1');
    });
    act(() => {
      fireEvent.click(el('inspector.preview'));
    });
    await waitFor(() => {
      expect(sent(h, 'draw.inspectorPreview.set')).toEqual([{ visible: false }]);
    });
  });

  it('renames on Enter and shows a refusal under the field without an error box', async () => {
    const h = await inspector({
      'draw.drawing.rename': () => fail('drawing.nameExists', 'a drawing named 2 exists'),
    });
    act(() => {
      fireEvent.change(el('inspector.name'), { target: { value: '2' } });
      fireEvent.keyDown(el('inspector.name'), { key: 'Enter' });
    });
    await waitFor(() => {
      expect(el('inspector.nameProblem').textContent).toContain('named 2 exists');
    });
    expect(sent(h, 'draw.drawing.rename')).toEqual([{ drawingId: 'dr_1', name: '2' }]);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('the Browse slider shows another drawing and sends no command', async () => {
    const h = await inspector();
    // The thumbnail's `draw.drawing.render` is a read (the host's list counts it); nothing else is sent.
    const changes = () => h.actions().filter((a) => a.command !== 'draw.drawing.render').length;
    const before = changes();
    act(() => {
      fireEvent.change(el('inspector.browse'), { target: { value: '1' } });
    });
    await waitFor(() => {
      expect(el('inspector.thumbnail').getAttribute('src')).toBe('data:image/png;base64,png-of-dr_2');
    });
    expect(el('inspector.browsing').textContent).toBe('2');
    expect(changes()).toBe(before);
  });

  it('a rename made elsewhere shows from the event alone', async () => {
    let answer = list();
    const h = await inspector({ 'element.drawing.list': () => ok(answer) });
    answer = list('key-1');
    act(() => {
      h.event('draw.drawing.changed', { drawingId: 'dr_1', rev: 2 });
    });
    await waitFor(() => {
      expect((el('inspector.name') as HTMLInputElement).value).toBe('key-1');
    });
  });
});
